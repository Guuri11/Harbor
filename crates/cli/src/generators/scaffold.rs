use std::path::Path;

use crate::generators::conflict::{OverwritePolicy, guard_entity_overwrite};
use crate::generators::types::{is_shared_vo, is_value_object};
use crate::generators::{
    application::{generate_create_use_case_impl, write_application_files},
    domain::{
        generate_create_use_case_trait, generate_model, patch_mothers_lib, write_domain_files,
        write_mother,
    },
    infrastructure::{generate_infra_entity, write_repository_files},
    migration::run_migration,
    naming::{pascal_to_snake, to_pascal_case, write_file},
    presentation::write_presentation_files,
    render::render,
};
use crate::patchers::lib_rs::{
    patch_business_lib_shared, patch_business_lib_value_objects, try_patch_libs,
};
use crate::puerto_toml::{Field, ValueObjectDefinition};

// ── Single use case (non-CRUD) ────────────────────────────────────────────────

fn write_files(
    pascal: &str,
    snake: &str,
    base: &Path,
    db: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    write_file(
        &base.join(format!("business/src/domain/{snake}/model.rs")),
        &generate_model(pascal, snake, &[], &[]),
    )?;
    let mut errors_ctx = tera::Context::new();
    errors_ctx.insert("pascal", pascal);
    errors_ctx.insert("snake", snake);
    errors_ctx.insert("vo_fields", &Vec::<String>::new());
    write_file(
        &base.join(format!("business/src/domain/{snake}/errors.rs")),
        &render("domain/errors.tera", &errors_ctx)?,
    )?;
    {
        let mut repo_ctx = tera::Context::new();
        repo_ctx.insert("pascal", pascal);
        repo_ctx.insert("snake", snake);
        write_file(
            &base.join(format!("business/src/domain/{snake}/repository.rs")),
            &render("domain/repository_simple.tera", &repo_ctx)
                .expect("domain/repository_simple.tera render failed"),
        )?;
    }
    write_file(
        &base.join(format!(
            "business/src/domain/{snake}/use_cases/create_{snake}.rs"
        )),
        &generate_create_use_case_trait(pascal, snake, &[]),
    )?;
    write_file(
        &base.join(format!(
            "business/src/application/{snake}/create_{snake}.rs"
        )),
        &generate_create_use_case_impl(pascal, snake, &[], &[]),
    )?;
    let infra_ctx = {
        let mut c = tera::Context::new();
        c.insert("pascal", pascal);
        c.insert("snake", snake);
        c
    };
    if db {
        write_file(
            &base.join(format!("infrastructure/src/{snake}/entity.rs")),
            &generate_infra_entity(pascal, snake, &[], &[]),
        )?;
        write_file(
            &base.join(format!("infrastructure/src/{snake}/repository.rs")),
            &render("infrastructure/repository_pg_simple.tera", &infra_ctx)
                .expect("infrastructure/repository_pg_simple.tera render failed"),
        )?;
    } else {
        write_file(
            &base.join(format!("infrastructure/src/{snake}/repository.rs")),
            &render("infrastructure/repository_inmemory_simple.tera", &infra_ctx)
                .expect("infrastructure/repository_inmemory_simple.tera render failed"),
        )?;
    }
    write_file(
        &base.join(format!("presentation/src/api/{snake}.rs")),
        "pub mod dto;\npub mod error_mapper;\npub mod responses;\npub mod routes;\n",
    )?;
    {
        use crate::generators::presentation::{
            build_dto_ctx, build_error_mapper_ctx, build_simple_ctx,
        };
        write_file(
            &base.join(format!("presentation/src/api/{snake}/dto.rs")),
            &render(
                "presentation/dto.tera",
                &build_dto_ctx(pascal, snake, &[], false),
            )
            .expect("presentation/dto.tera render failed"),
        )?;
        write_file(
            &base.join(format!("presentation/src/api/{snake}/responses.rs")),
            &render(
                "presentation/responses_simple.tera",
                &build_simple_ctx(pascal, snake),
            )
            .expect("presentation/responses_simple.tera render failed"),
        )?;
        write_file(
            &base.join(format!("presentation/src/api/{snake}/error_mapper.rs")),
            &render(
                "presentation/error_mapper.tera",
                &build_error_mapper_ctx(pascal, snake, &[]),
            )
            .expect("presentation/error_mapper.tera render failed"),
        )?;
        write_file(
            &base.join(format!("presentation/src/api/{snake}/routes.rs")),
            &render(
                "presentation/routes_simple.tera",
                &build_simple_ctx(pascal, snake),
            )
            .expect("presentation/routes_simple.tera render failed"),
        )?;
    }
    Ok(())
}

// ── CRUD (delegates to per-layer writers) ────────────────────────────────────

fn write_files_crud(
    pascal: &str,
    snake: &str,
    base: &Path,
    db: bool,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> Result<(), Box<dyn std::error::Error>> {
    write_domain_files(pascal, snake, base, fields, shared_vos)?;
    write_application_files(pascal, snake, base, fields, shared_vos)?;
    write_repository_files(pascal, snake, base, db, fields, shared_vos)?;
    write_presentation_files(pascal, snake, base, fields)?;
    Ok(())
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Low-level scaffold: write files + patch lib.rs files.
/// `crud = true` generates 5 use cases; `crud = false` generates only create.
pub fn run(
    name: &str,
    base: &Path,
    db: bool,
    crud: bool,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> Result<(), Box<dyn std::error::Error>> {
    let pascal = to_pascal_case(name);
    let snake = pascal_to_snake(&pascal);

    if crud {
        write_files_crud(&pascal, &snake, base, db, fields, shared_vos)?;
    } else {
        write_files(&pascal, &snake, base, db)?;
    }
    try_patch_libs(&snake, base, db, crud);

    if !fields.is_empty()
        && fields
            .iter()
            .any(|f| is_value_object(f) && !is_shared_vo(f, shared_vos))
    {
        let _ = patch_business_lib_value_objects(base, &snake);
    }

    if !shared_vos.is_empty() {
        use crate::generators::domain::write_shared_vo_files;
        let _ = write_shared_vo_files(base, shared_vos);
        let _ = patch_business_lib_shared(base);
    }

    let _ = write_mother(&pascal, &snake, base, fields, shared_vos);
    let _ = patch_mothers_lib(base, &snake);

    let use_cases = if crud {
        vec![
            format!("create_{snake}"),
            format!("get_{snake}"),
            format!("list_{snake}"),
            format!("update_{snake}"),
            format!("delete_{snake}"),
        ]
    } else {
        vec![format!("create_{snake}")]
    };

    let bootstrapped =
        crate::puerto_toml::add_entity(base, &pascal, use_cases, db, fields.to_vec())
            .and_then(|_| crate::generators::bootstrap::regenerate_bootstrap(base))
            .is_ok();

    let repo_name = if db {
        format!("Pg{pascal}Repository")
    } else {
        format!("InMemory{pascal}Repository")
    };
    let uc_label = if crud {
        "create, get, list, update, delete"
    } else {
        "create"
    };
    println!("✓ business/        — model, errors, repository trait, use cases ({uc_label})");
    println!("✓ infrastructure/  — {repo_name}");
    println!("✓ presentation/    — routes, dto, responses, error mapper");
    if bootstrapped {
        println!("✓ Done. Zero manual wiring.");
    } else {
        println!("✓ Done. Run `puerto generate bootstrap` to wire DI.");
    }

    Ok(())
}

/// CLI entry point for `puerto generate scaffold <Name>`.
/// Reads `project.db` from `puerto.toml`, always generates full CRUD.
/// `sqlx_bin` overrides the sqlx binary path (pass `None` in production, `Some("/bin/true")` in tests).
/// `cli_fields` are fields passed from the CLI (e.g. `title:String price:i64`).
///
/// Refuses to overwrite an already-generated entity unless `--force`; see `conflict.rs`.
pub fn run_scaffold(
    name: &str,
    base: &Path,
    sqlx_bin: Option<&str>,
    cli_fields: &[Field],
    overwrite: OverwritePolicy,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = crate::puerto_toml::read(base)?;
    let db = config.project.db;
    let shared_vos = config.value_object.clone();
    let pascal = to_pascal_case(name);
    let fields = if !cli_fields.is_empty() {
        cli_fields.to_vec()
    } else {
        config
            .entity
            .iter()
            .find(|e| e.name == pascal)
            .map(|e| e.fields.clone())
            .unwrap_or_default()
    };

    // Checked before the first byte is written — a half-overwritten project is worse than a
    // refused command.
    crate::validation::validate_entity_name(&pascal)?;
    let vo_conflicts = crate::validation::validate_vo_coherence(&fields, &shared_vos);
    if !vo_conflicts.is_empty() {
        return Err(vo_conflicts.join("\n  ").into());
    }
    guard_entity_overwrite(base, &pascal_to_snake(&pascal), db, overwrite)?;

    run(name, base, db, true, &fields, &shared_vos)?;
    if db {
        let snake = pascal_to_snake(&pascal);
        let migration_sql = crate::generators::infrastructure::create_table_sql(&snake, &fields);
        run_migration(
            &format!("create_{snake}_table"),
            base,
            sqlx_bin,
            Some(&migration_sql),
        )?;
    }
    Ok(())
}
