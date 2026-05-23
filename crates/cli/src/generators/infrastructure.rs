use std::path::Path;

use crate::generators::effective_fields;
use crate::generators::migration::run_migration;
use crate::generators::naming::{pascal_to_snake, to_pascal_case, write_file};
use crate::generators::render::render;
use crate::generators::types::{
    is_enum_vo, is_option_vo, is_shared_vo, is_value_object, is_vec_vo, resolve_type, vo_inner_type,
};
use crate::patchers::lib_rs::patch_infra_lib;
use crate::puerto_toml::{Field, ValueObjectDefinition};

// ── SQL helpers ───────────────────────────────────────────────────────────────

fn field_needs_clone(field_type: &str) -> bool {
    matches!(
        field_type,
        "String" | "Option<String>" | "Vec<String>" | "Vec<i64>" | "HashMap<String, String>"
    )
}

fn sql_ddl_col(name: &str, field_type: &str) -> String {
    let mapping = resolve_type(field_type).unwrap();
    let sql_base = match mapping.sql_type {
        "DOUBLE" => "DOUBLE PRECISION",
        other => other,
    };
    if mapping.sql_nullable {
        format!("    {name} {sql_base}")
    } else {
        let suffix = match mapping.sql_type {
            "TEXT[]" | "BIGINT[]" => " NOT NULL DEFAULT '{}'",
            "JSONB" => " NOT NULL DEFAULT '{}'",
            _ => " NOT NULL",
        };
        format!("    {name} {sql_base}{suffix}")
    }
}

fn sql_col_list(eff: &[Field]) -> String {
    let custom: String = eff.iter().map(|f| format!(", {}", f.name)).collect();
    format!("id, created_at, updated_at, deleted, deleted_at{custom}")
}

fn sql_params_list(n: usize) -> String {
    let custom: String = (6..=n).map(|i| format!(", ${i}")).collect();
    format!("$1, $2, $3, $4, $5{custom}")
}

fn sql_conflict_set(eff: &[Field]) -> String {
    let custom: String = eff
        .iter()
        .enumerate()
        .map(|(i, f)| format!(", {} = ${}", f.name, 6 + i))
        .collect();
    format!("updated_at = $3, deleted = $4, deleted_at = $5{custom}")
}

fn db_bindings_str(eff: &[Field]) -> String {
    let mut lines = vec![
        "            db.id,".to_string(),
        "            db.created_at,".to_string(),
        "            db.updated_at,".to_string(),
        "            db.deleted,".to_string(),
        "            db.deleted_at,".to_string(),
    ];
    for f in eff {
        // SQLx requires array fields to be passed as slice references (&[T])
        let binding = if f.field_type.starts_with("Vec<") {
            format!("            &db.{},", f.name)
        } else {
            format!("            db.{},", f.name)
        };
        lines.push(binding);
    }
    lines.join("\n")
}

fn seed_fn_str(
    pascal: &str,
    eff: &[Field],
    shared_vos: &[crate::puerto_toml::ValueObjectDefinition],
) -> String {
    let props_lines: String = eff
        .iter()
        .map(|f| {
            if is_option_vo(f) {
                format!("            {}: None,", f.name)
            } else if is_vec_vo(f) {
                format!("            {}: vec![],", f.name)
            } else if is_enum_vo(f) {
                let vo = f.value_object.as_deref().unwrap();
                let first_variant = f.enum_variants.as_deref().unwrap().first().unwrap();
                format!("            {}: {}::{},", f.name, vo, first_variant)
            } else if is_value_object(f) {
                let mapping = resolve_type(&f.field_type).unwrap();
                let vo = f.value_object.as_deref().unwrap();
                format!(
                    "            {}: {}::new({}).expect(\"valid {}\"),",
                    f.name, vo, mapping.default_expr, vo
                )
            } else {
                let mapping = resolve_type(&f.field_type).unwrap();
                format!("            {}: {},", f.name, mapping.default_expr)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let _ = shared_vos;
    format!(
        "    async fn seed(pool: &PgPool) -> {pascal} {{\n        let entity = {pascal}::new({pascal}Props {{\n{props_lines}\n        }}).unwrap();\n        test_repo(pool.clone()).save(&entity).await.unwrap();\n        entity\n    }}\n",
        pascal = pascal,
        props_lines = props_lines,
    )
}

fn field_asserts_str(eff: &[Field]) -> String {
    eff.iter()
        .map(|f| format!("        assert_eq!(found.{}, entity.{});", f.name, f.name))
        .collect::<Vec<_>>()
        .join("\n")
}

fn update_test_str(eff: &[Field]) -> String {
    let sf = eff
        .iter()
        .find(|f| f.field_type == "String" && !is_value_object(f))
        .or_else(|| {
            eff.iter().find(|f| {
                f.field_type == "String"
                    && is_value_object(f)
                    && !is_enum_vo(f)
                    && !is_option_vo(f)
                    && !is_vec_vo(f)
            })
        });
    if let Some(sf) = sf {
        if is_value_object(sf) {
            let vo = sf.value_object.as_deref().unwrap();
            format!(
                "        let mut entity = seed(&pool).await;\n        entity.{field} = {vo}::new(\"updated\".to_string()).unwrap();\n        entity.updated_at = chrono::Utc::now();\n\n        // Act\n        test_repo(pool.clone()).save(&entity).await.unwrap();\n\n        // Assert\n        let found = test_repo(pool).find_by_id(entity.id).await.unwrap().unwrap();\n        assert_eq!(found.{field}.value(), \"updated\");",
                field = sf.name,
                vo = vo,
            )
        } else {
            format!(
                "        let mut entity = seed(&pool).await;\n        entity.{field} = \"updated\".to_string();\n        entity.updated_at = chrono::Utc::now();\n\n        // Act\n        test_repo(pool.clone()).save(&entity).await.unwrap();\n\n        // Assert\n        let found = test_repo(pool).find_by_id(entity.id).await.unwrap().unwrap();\n        assert_eq!(found.{field}, \"updated\");",
                field = sf.name,
            )
        }
    } else {
        "        let entity = seed(&pool).await;\n\n        // Act\n        test_repo(pool.clone()).save(&entity).await.unwrap();\n\n        // Assert\n        let found = test_repo(pool).find_by_id(entity.id).await.unwrap();\n        assert!(found.is_some());".to_string()
    }
}

// ── Dynamic generators ────────────────────────────────────────────────────────

pub fn generate_infra_entity(
    pascal: &str,
    snake: &str,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> String {
    let eff = effective_fields(fields);

    let struct_fields_str: String = eff
        .iter()
        .map(|f| format!("    pub {}: {},", f.name, f.field_type))
        .collect::<Vec<_>>()
        .join("\n");

    let mut vo_imports: Vec<String> = vec![];
    if eff.iter().any(is_value_object) {
        for f in eff.iter().filter(|f| is_value_object(f)) {
            let vo = f.value_object.as_deref().unwrap();
            let stmt = if is_shared_vo(f, shared_vos) {
                format!("use business::domain::shared::value_objects::{};", vo)
            } else {
                format!("use business::domain::{}::value_objects::{};", snake, vo)
            };
            if !vo_imports.contains(&stmt) {
                vo_imports.push(stmt);
            }
        }
    }
    let vo_imports_str = if vo_imports.is_empty() {
        String::new()
    } else {
        format!("\n{}", vo_imports.join("\n"))
    };

    let try_from_fields_str: String = eff
        .iter()
        .map(|f| {
            if is_enum_vo(f) {
                let vo = f.value_object.as_deref().unwrap();
                if is_shared_vo(f, shared_vos) {
                    format!("            {}: {}::from_str(&row.{}).map_err(|_| {}Error::Invalid{})?,", f.name, vo, f.name, pascal, vo)
                } else {
                    format!("            {}: {}::from_str(&row.{})?,", f.name, vo, f.name)
                }
            } else if is_option_vo(f) {
                let vo = f.value_object.as_deref().unwrap();
                if is_shared_vo(f, shared_vos) {
                    format!("            {}: row.{}.map({vo}::new).transpose().map_err(|_| {}Error::Invalid{})?,", f.name, f.name, pascal, vo, vo = vo)
                } else {
                    format!("            {}: row.{}.map({vo}::new).transpose()?,", f.name, f.name, vo = vo)
                }
            } else if is_vec_vo(f) {
                let vo = f.value_object.as_deref().unwrap();
                if is_shared_vo(f, shared_vos) {
                    format!("            {}: row.{}.into_iter().map({vo}::new).collect::<Result<Vec<_>,_>>().map_err(|_| {}Error::Invalid{})?,", f.name, f.name, pascal, vo, vo = vo)
                } else {
                    format!("            {}: row.{}.into_iter().map({vo}::new).collect::<Result<Vec<_>,_>>()?,", f.name, f.name, vo = vo)
                }
            } else if is_value_object(f) {
                let vo = f.value_object.as_deref().unwrap();
                if is_shared_vo(f, shared_vos) {
                    format!("            {}: {}::new(row.{}).map_err(|_| {}Error::Invalid{})?,", f.name, vo, f.name, pascal, vo)
                } else {
                    format!("            {}: {}::new(row.{})?,", f.name, vo, f.name)
                }
            } else {
                format!("            {}: row.{},", f.name, f.name)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let from_fields_str: String = eff
        .iter()
        .map(|f| {
            if is_enum_vo(f) {
                format!("            {}: entity.{}.as_str().to_string(),", f.name, f.name)
            } else if is_option_vo(f) {
                let inner = vo_inner_type(f);
                if inner == "String" {
                    format!("            {}: entity.{}.as_ref().map(|v| v.value().to_string()),", f.name, f.name)
                } else {
                    format!("            {}: entity.{}.map(|v| v.value()),", f.name, f.name)
                }
            } else if is_vec_vo(f) {
                let inner = vo_inner_type(f);
                if inner == "String" {
                    format!("            {}: entity.{}.iter().map(|v| v.value().to_string()).collect(),", f.name, f.name)
                } else {
                    format!("            {}: entity.{}.iter().map(|v| v.value()).collect(),", f.name, f.name)
                }
            } else if is_value_object(f) {
                match f.field_type.as_str() {
                    "String" => format!("            {}: entity.{}.value().to_string(),", f.name, f.name),
                    _ => format!("            {}: entity.{}.value(),", f.name, f.name),
                }
            } else if field_needs_clone(&f.field_type) {
                format!("            {}: entity.{}.clone(),", f.name, f.name)
            } else {
                format!("            {}: entity.{},", f.name, f.name)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    ctx.insert("vo_imports_str", &vo_imports_str);
    ctx.insert("struct_fields_str", &struct_fields_str);
    ctx.insert("try_from_fields_str", &try_from_fields_str);
    ctx.insert("from_fields_str", &from_fields_str);
    render("infrastructure/entity.tera", &ctx).expect("infrastructure/entity.tera render failed")
}

pub fn generate_crud_infra_db_repository(
    pascal: &str,
    snake: &str,
    fields: &[Field],
    shared_vos: &[crate::puerto_toml::ValueObjectDefinition],
) -> String {
    let eff = effective_fields(fields);
    let n = 5 + eff.len();

    let all_cols = sql_col_list(&eff);
    let all_params = sql_params_list(n);
    let all_updates = sql_conflict_set(&eff);
    let all_bindings = db_bindings_str(&eff);
    let seed_fn = seed_fn_str(pascal, &eff, shared_vos);
    let field_asserts = field_asserts_str(&eff);
    let update_test = update_test_str(&eff);

    let mut vo_test_imports_vec: Vec<String> = vec![];
    for f in eff
        .iter()
        .filter(|f| is_value_object(f) && !is_option_vo(f) && !is_vec_vo(f) && !is_enum_vo(f))
    {
        let vo = f.value_object.as_deref().unwrap();
        let stmt = if is_shared_vo(f, shared_vos) {
            format!("    use business::domain::shared::value_objects::{};", vo)
        } else {
            format!(
                "    use business::domain::{}::value_objects::{};",
                snake, vo
            )
        };
        if !vo_test_imports_vec.contains(&stmt) {
            vo_test_imports_vec.push(stmt);
        }
    }
    let vo_test_imports = if vo_test_imports_vec.is_empty() {
        String::new()
    } else {
        format!("\n{}", vo_test_imports_vec.join("\n"))
    };

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    ctx.insert("all_cols", &all_cols);
    ctx.insert("all_params", &all_params);
    ctx.insert("all_updates", &all_updates);
    ctx.insert("all_bindings", &all_bindings);
    ctx.insert("seed_fn", &seed_fn);
    ctx.insert("field_asserts", &field_asserts);
    ctx.insert("update_test", &update_test);
    ctx.insert("vo_test_imports", &vo_test_imports);
    render("infrastructure/repository_pg_crud.tera", &ctx)
        .expect("infrastructure/repository_pg_crud.tera render failed")
}

pub fn create_table_sql(snake: &str, fields: &[Field]) -> String {
    let eff = effective_fields(fields);
    let custom_cols: String = eff
        .iter()
        .map(|f| format!("{},", sql_ddl_col(&f.name, &f.field_type)))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "CREATE TABLE {snake}s (\n    id UUID PRIMARY KEY,\n{custom_cols}\n    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),\n    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),\n    deleted BOOLEAN NOT NULL DEFAULT FALSE,\n    deleted_at TIMESTAMPTZ\n);\n",
        snake = snake,
        custom_cols = custom_cols,
    )
}

pub fn write_repository_files(
    pascal: &str,
    snake: &str,
    base: &Path,
    db: bool,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> Result<(), Box<dyn std::error::Error>> {
    if db {
        write_file(
            &base.join(format!("infrastructure/src/{snake}/entity.rs")),
            &generate_infra_entity(pascal, snake, fields, shared_vos),
        )?;
        write_file(
            &base.join(format!("infrastructure/src/{snake}/repository.rs")),
            &generate_crud_infra_db_repository(pascal, snake, fields, shared_vos),
        )?;
    } else {
        let mut ctx = tera::Context::new();
        ctx.insert("pascal", pascal);
        ctx.insert("snake", snake);
        write_file(
            &base.join(format!("infrastructure/src/{snake}/repository.rs")),
            &render("infrastructure/repository_inmemory_crud.tera", &ctx)
                .expect("infrastructure/repository_inmemory_crud.tera render failed"),
        )?;
    }
    Ok(())
}

pub fn run_generate_repository(
    name: &str,
    base: &Path,
    sqlx_bin: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = crate::puerto_toml::read(base)?;
    let pascal = to_pascal_case(name);
    let snake = pascal_to_snake(&pascal);

    if !config.entity.iter().any(|e| e.name == pascal) {
        return Err(format!(
            "{pascal} not found in puerto.toml. Run `puerto generate domain {pascal}` first."
        )
        .into());
    }

    let db = config.project.db;
    let shared_vos = config.value_object.clone();
    let fields = config
        .entity
        .iter()
        .find(|e| e.name == pascal)
        .map(|e| e.fields.clone())
        .unwrap_or_default();

    write_repository_files(&pascal, &snake, base, db, &fields, &shared_vos)?;
    patch_infra_lib(base, &snake, db)?;

    if db {
        run_migration(
            &format!("create_{snake}_table"),
            base,
            sqlx_bin,
            Some(&create_table_sql(&snake, &fields)),
        )?;
    }

    let repo_label = if db {
        format!("Pg{pascal}Repository")
    } else {
        format!("InMemory{pascal}Repository")
    };
    println!("✓ infrastructure/      — {repo_label}");
    println!();
    println!("  Next: puerto generate presentation {pascal}");
    Ok(())
}
