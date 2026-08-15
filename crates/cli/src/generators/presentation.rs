use std::path::Path;

use crate::generators::bootstrap::regenerate_bootstrap;
use crate::generators::effective_fields;
use crate::generators::naming::{pascal_to_snake, to_pascal_case, write_file};
use crate::generators::render::render;
use crate::generators::types::{
    is_enum_vo, is_option_vo, is_value_object, is_vec_vo, resolve_type, vo_inner_type,
};
use crate::patchers::api_rs::patch_api_rs;
use crate::puerto_toml::Field;

/// Imports `presentation/dto.tera` writes itself.
const DTO_TEMPLATE_IMPORTS: &[&str] = &["use uuid::Uuid;"];

fn field_needs_clone(field_type: &str) -> bool {
    matches!(
        field_type,
        "String" | "Option<String>" | "Vec<String>" | "Vec<i64>" | "HashMap<String, String>"
    )
}

fn build_dto_from_expr(f: &Field) -> String {
    if is_enum_vo(f) {
        format!(
            "            {}: entity.{}.as_str().to_string(),",
            f.name, f.name
        )
    } else if is_option_vo(f) {
        let inner = vo_inner_type(f);
        if inner == "String" {
            format!(
                "            {}: entity.{}.as_ref().map(|v| v.value().to_string()),",
                f.name, f.name
            )
        } else {
            format!(
                "            {}: entity.{}.map(|v| v.value()),",
                f.name, f.name
            )
        }
    } else if is_vec_vo(f) {
        let inner = vo_inner_type(f);
        if inner == "String" {
            format!(
                "            {}: entity.{}.iter().map(|v| v.value().to_string()).collect::<Vec<_>>(),",
                f.name, f.name
            )
        } else {
            format!(
                "            {}: entity.{}.iter().map(|v| v.value()).collect::<Vec<_>>(),",
                f.name, f.name
            )
        }
    } else if is_value_object(f) {
        match f.field_type.as_str() {
            "String" => format!(
                "            {}: entity.{}.value().to_string(),",
                f.name, f.name
            ),
            _ => format!("            {}: entity.{}.value(),", f.name, f.name),
        }
    } else if field_needs_clone(&f.field_type) {
        format!("            {}: entity.{}.clone(),", f.name, f.name)
    } else {
        format!("            {}: entity.{},", f.name, f.name)
    }
}

pub(crate) fn build_dto_ctx(
    pascal: &str,
    snake: &str,
    fields: &[Field],
    is_crud: bool,
) -> tera::Context {
    let eff = effective_fields(fields);

    let dto_fields_str = eff
        .iter()
        .map(|f| format!("    pub {}: {},", f.name, f.field_type))
        .collect::<Vec<_>>()
        .join("\n");

    let dto_from_str = eff
        .iter()
        .map(build_dto_from_expr)
        .collect::<Vec<_>>()
        .join("\n");

    let request_fields_str = eff
        .iter()
        .filter(|f| f.field_type != "Uuid")
        .map(|f| format!("    pub {}: {},", f.name, f.field_type))
        .collect::<Vec<_>>()
        .join("\n");

    // DTO fields keep the primitive type of the field, so the DTO file needs the same imports the
    // domain model does. `dto.tera` only imports `uuid::Uuid` itself (for the always-present `id`),
    // so a `DateTime<Utc>` or `HashMap<String, String>` field was written against a type that was
    // never in scope (E0425).
    let mut extra_imports: Vec<String> = vec![];
    for f in &eff {
        if let Ok(mapping) = resolve_type(&f.field_type) {
            if let Some(imp) = mapping.needs_import {
                let stmt = format!("use {};", imp);
                if DTO_TEMPLATE_IMPORTS.contains(&stmt.as_str()) {
                    continue;
                }
                if !extra_imports.contains(&stmt) {
                    extra_imports.push(stmt);
                }
            }
        }
    }
    let extra_imports_str = if extra_imports.is_empty() {
        String::new()
    } else {
        format!("\n{}", extra_imports.join("\n"))
    };

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    ctx.insert("is_crud", &is_crud);
    ctx.insert("dto_fields_str", &dto_fields_str);
    ctx.insert("dto_from_str", &dto_from_str);
    ctx.insert("request_fields_str", &request_fields_str);
    ctx.insert("extra_imports", &extra_imports_str);
    ctx
}

pub(crate) fn build_error_mapper_ctx(pascal: &str, snake: &str, fields: &[Field]) -> tera::Context {
    let eff = effective_fields(fields);
    let vo_arms: Vec<String> = eff
        .iter()
        .filter(|f| is_value_object(f))
        .map(|f| {
            let vo = f.value_object.as_deref().unwrap();
            format!(
                "\n            {pascal}Error::Invalid{vo} => (StatusCode::BAD_REQUEST, self.to_string()),",
            )
        })
        .collect();
    let vo_arms_str = vo_arms.join("");

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    ctx.insert("vo_arms_str", &vo_arms_str);
    ctx
}

pub(crate) fn build_simple_ctx(pascal: &str, snake: &str) -> tera::Context {
    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    ctx
}

fn build_routes_crud_ctx(pascal: &str, snake: &str, fields: &[Field]) -> tera::Context {
    let eff = effective_fields(fields);

    let create_params_str = eff
        .iter()
        .filter(|f| f.field_type != "Uuid")
        .map(|f| {
            if field_needs_clone(&f.field_type) {
                format!("                {}: body.{}.clone(),", f.name, f.name)
            } else {
                format!("                {}: body.{},", f.name, f.name)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut update_lines = vec!["                id: id.0,".to_string()];
    for f in eff.iter().filter(|f| f.field_type != "Uuid") {
        if field_needs_clone(&f.field_type) {
            update_lines.push(format!(
                "                {}: body.{}.clone(),",
                f.name, f.name
            ));
        } else {
            update_lines.push(format!("                {}: body.{},", f.name, f.name));
        }
    }
    let update_params_str = update_lines.join("\n");

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    ctx.insert("create_params_str", &create_params_str);
    ctx.insert("update_params_str", &update_params_str);
    ctx
}

#[cfg(test)]
pub fn generate_crud_dto(pascal: &str, snake: &str, fields: &[Field]) -> String {
    render(
        "presentation/dto.tera",
        &build_dto_ctx(pascal, snake, fields, true),
    )
    .expect("presentation/dto.tera render failed")
}

#[cfg(test)]
pub fn generate_crud_routes(pascal: &str, snake: &str, fields: &[Field]) -> String {
    render(
        "presentation/routes_crud.tera",
        &build_routes_crud_ctx(pascal, snake, fields),
    )
    .expect("presentation/routes_crud.tera render failed")
}

pub fn write_presentation_files(
    pascal: &str,
    snake: &str,
    base: &Path,
    fields: &[Field],
) -> Result<(), Box<dyn std::error::Error>> {
    write_file(
        &base.join(format!("presentation/src/api/{snake}.rs")),
        "pub mod dto;\npub mod error_mapper;\npub mod responses;\npub mod routes;\n",
    )?;
    write_file(
        &base.join(format!("presentation/src/api/{snake}/dto.rs")),
        &render(
            "presentation/dto.tera",
            &build_dto_ctx(pascal, snake, fields, true),
        )
        .expect("presentation/dto.tera render failed"),
    )?;
    write_file(
        &base.join(format!("presentation/src/api/{snake}/responses.rs")),
        &render(
            "presentation/responses_crud.tera",
            &build_simple_ctx(pascal, snake),
        )
        .expect("presentation/responses_crud.tera render failed"),
    )?;
    write_file(
        &base.join(format!("presentation/src/api/{snake}/error_mapper.rs")),
        &render(
            "presentation/error_mapper.tera",
            &build_error_mapper_ctx(pascal, snake, fields),
        )
        .expect("presentation/error_mapper.tera render failed"),
    )?;
    write_file(
        &base.join(format!("presentation/src/api/{snake}/routes.rs")),
        &render(
            "presentation/routes_crud.tera",
            &build_routes_crud_ctx(pascal, snake, fields),
        )
        .expect("presentation/routes_crud.tera render failed"),
    )?;
    Ok(())
}

pub fn run_generate_presentation(
    name: &str,
    base: &Path,
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

    let fields = config
        .entity
        .iter()
        .find(|e| e.name == pascal)
        .map(|e| e.fields.clone())
        .unwrap_or_default();

    write_presentation_files(&pascal, &snake, base, &fields)?;
    patch_api_rs(base, &snake)?;
    regenerate_bootstrap(base)?;

    println!("✓ presentation/        — routes, dto, responses, error_mapper");
    println!("✓ bootstrap.rs         — regenerated");
    println!();
    println!("  All layers wired. Run `make run` to start.");
    Ok(())
}
