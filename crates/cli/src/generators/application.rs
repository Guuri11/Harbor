use std::path::Path;

use serde_json::json;

use crate::generators::effective_fields;
use crate::generators::naming::{pascal_to_snake, to_pascal_case, write_file};
use crate::generators::render::render;
use crate::generators::types::{
    field_vo_constructor, is_enum_vo, is_option_vo, is_value_object, is_vec_vo, resolve_type,
    vo_import_path,
};
use crate::patchers::lib_rs::patch_business_lib_application_crud;
use crate::puerto_toml::{Field, ValueObjectDefinition};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn enum_or_string(f: &Field, fallback: &str) -> String {
    if is_enum_vo(f) {
        let first = f.enum_variants.as_deref().unwrap().first().unwrap();
        format!("\"{first}\".to_string()")
    } else {
        format!("\"{fallback}\".to_string()")
    }
}

fn build_test_props(eff: &[Field], string_override: &str) -> String {
    eff.iter()
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
                let value = if f.field_type == "String" {
                    format!("\"{}\".to_string()", string_override)
                } else {
                    mapping.default_expr.to_string()
                };
                format!(
                    "            {}: {}::new({}).expect(\"valid {}\"),",
                    f.name, vo, value, vo
                )
            } else {
                let mapping = resolve_type(&f.field_type).unwrap();
                let value = if f.field_type == "String" {
                    format!("\"{}\".to_string()", string_override)
                } else {
                    mapping.default_expr.to_string()
                };
                format!("            {}: {},", f.name, value)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_create_params_vo(eff: &[Field]) -> String {
    eff.iter()
        .filter(|f| f.field_type != "Uuid")
        .map(|f| {
            let mapping = resolve_type(&f.field_type).unwrap();
            let value = if f.field_type == "String" {
                enum_or_string(f, "example")
            } else {
                mapping.default_expr.to_string()
            };
            format!("            {}: {},", f.name, value)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_empty_create_params(eff: &[Field], empty_field: &str) -> String {
    eff.iter()
        .filter(|f| f.field_type != "Uuid")
        .map(|f| {
            if f.name == empty_field {
                format!("            {}: \"\".to_string(),", f.name)
            } else {
                let mapping = resolve_type(&f.field_type).unwrap();
                let value = if f.field_type == "String" {
                    enum_or_string(f, "example")
                } else {
                    mapping.default_expr.to_string()
                };
                format!("            {}: {},", f.name, value)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_update_params(eff: &[Field]) -> String {
    let mut lines = vec!["            id: entity_id,".to_string()];
    for f in eff.iter().filter(|f| f.field_type != "Uuid") {
        let value = if f.field_type == "String" {
            enum_or_string(f, "updated")
        } else {
            resolve_type(&f.field_type).unwrap().default_expr.to_string()
        };
        lines.push(format!("            {}: {},", f.name, value));
    }
    lines.join("\n")
}

fn build_empty_update_params(eff: &[Field], empty_field: &str) -> String {
    let mut lines = vec!["            id: entity_id,".to_string()];
    for f in eff.iter().filter(|f| f.field_type != "Uuid") {
        if f.name == empty_field {
            lines.push(format!("            {}: \"\".to_string(),", f.name));
        } else {
            let value = if f.field_type == "String" {
                enum_or_string(f, "updated")
            } else {
                resolve_type(&f.field_type).unwrap().default_expr.to_string()
            };
            lines.push(format!("            {}: {},", f.name, value));
        }
    }
    lines.join("\n")
}

fn build_not_found_params(eff: &[Field]) -> String {
    let mut lines = vec!["            id: Uuid::new_v4(),".to_string()];
    for f in eff.iter().filter(|f| f.field_type != "Uuid") {
        let mapping = resolve_type(&f.field_type).unwrap();
        let value = if f.field_type == "String" {
            enum_or_string(f, "new")
        } else {
            mapping.default_expr.to_string()
        };
        lines.push(format!("            {}: {},", f.name, value));
    }
    lines.join("\n")
}

fn build_test_vo_imports_block(
    eff: &[Field],
    snake: &str,
    shared_vos: &[ValueObjectDefinition],
) -> String {
    let mut imports: Vec<String> = vec![];
    for f in eff.iter().filter(|f| is_value_object(f)) {
        let stmt = format!("    use {};", vo_import_path(f, snake, shared_vos));
        if !imports.contains(&stmt) {
            imports.push(stmt);
        }
    }
    if imports.is_empty() {
        String::new()
    } else {
        imports.join("\n") + "\n"
    }
}

fn build_extra_imports(eff: &[Field], snake: &str, shared_vos: &[ValueObjectDefinition]) -> Vec<String> {
    let mut imports: Vec<String> = vec![];
    for f in eff {
        if let Ok(mapping) = resolve_type(&f.field_type) {
            if let Some(imp) = mapping.needs_import {
                let stmt = format!("use {};", imp);
                if !imports.contains(&stmt) {
                    imports.push(stmt);
                }
            }
        }
    }
    for f in eff.iter().filter(|f| is_value_object(f)) {
        let stmt = format!("use {};", vo_import_path(f, snake, shared_vos));
        if !imports.contains(&stmt) {
            imports.push(stmt);
        }
    }
    imports
}

// ── Context builders ──────────────────────────────────────────────────────────

fn build_create_context(
    pascal: &str,
    snake: &str,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> tera::Context {
    let eff = effective_fields(fields);
    let extra_imports = build_extra_imports(&eff, snake, shared_vos);
    let model_import = format!("{{{pascal}, {pascal}Props}}");

    let props_fields = eff
        .iter()
        .filter(|f| f.field_type != "Uuid")
        .map(|f| {
            if is_value_object(f) {
                format!("            {}: {},", f.name, f.name)
            } else {
                format!("            {}: params.{},", f.name, f.name)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let vo_constr: Vec<String> = eff
        .iter()
        .filter(|f| f.field_type != "Uuid" && is_value_object(f))
        .map(|f| field_vo_constructor(f, "params.", pascal, shared_vos))
        .collect();
    let vo_constructions = if vo_constr.is_empty() {
        String::new()
    } else {
        vo_constr.join("\n") + "\n"
    };

    let log_ident = format!("params.{}", eff[0].name);

    let valid_test_name = if eff.len() == 1
        && eff[0].name == "name"
        && eff[0].field_type == "String"
        && !is_value_object(&eff[0])
    {
        format!("should_create_{snake}_when_name_is_valid")
    } else {
        format!("should_create_{snake}_when_fields_are_valid")
    };

    let valid_params = build_create_params_vo(&eff);

    let valid_assertion =
        if let Some(f) = eff.iter().find(|f| f.field_type == "String" && !is_value_object(f)) {
            format!("\n        assert_eq!(result.unwrap().{}, \"example\");", f.name)
        } else if let Some(f) = eff.iter().find(|f| is_value_object(f) && f.field_type == "String")
        {
            format!(
                "\n        assert_eq!(result.unwrap().{}.value(), \"example\");",
                f.name
            )
        } else {
            String::new()
        };

    let mut validation_test_cases: Vec<serde_json::Value> = vec![];
    for sf in eff.iter().filter(|f| f.field_type == "String" && !is_value_object(f)) {
        let empty_params = build_empty_create_params(&eff, &sf.name);
        validation_test_cases.push(json!({
            "test_name": format!("should_return_error_when_{}_is_empty", sf.name),
            "empty_params": empty_params,
            "error_str": format!("{snake}.validation_error.{}_empty", sf.name),
        }));
    }
    for vf in eff.iter().filter(|f| is_value_object(f) && f.field_type == "String") {
        let vo = vf.value_object.as_deref().unwrap();
        let vo_snake = pascal_to_snake(vo);
        let empty_params = build_empty_create_params(&eff, &vf.name);
        validation_test_cases.push(json!({
            "test_name": format!("should_return_error_when_{}_is_empty", vf.name),
            "empty_params": empty_params,
            "error_str": format!("{snake}.invalid_{vo_snake}"),
        }));
    }

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    ctx.insert("extra_imports", &extra_imports);
    ctx.insert("model_import", &model_import);
    ctx.insert("vo_constructions", &vo_constructions);
    ctx.insert("props_fields", &props_fields);
    ctx.insert("log_ident", &log_ident);
    ctx.insert("valid_test_name", &valid_test_name);
    ctx.insert("valid_params", &valid_params);
    ctx.insert("valid_assertion", &valid_assertion);
    ctx.insert("validation_test_cases", &validation_test_cases);
    ctx
}

fn build_get_context(
    pascal: &str,
    snake: &str,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> tera::Context {
    let eff = effective_fields(fields);
    let props_fields = build_test_props(&eff, "example");
    let test_vo_imports = build_test_vo_imports_block(&eff, snake, shared_vos);

    let found_assertion = if let Some(f) =
        eff.iter().find(|f| f.field_type == "String" && !is_value_object(f))
    {
        format!(
            "\n        assert_eq!(result.unwrap().{}, \"example\");",
            f.name
        )
    } else if let Some(f) = eff
        .iter()
        .find(|f| is_value_object(f) && f.field_type == "String" && !is_enum_vo(f))
    {
        format!(
            "\n        assert_eq!(result.unwrap().{}.value(), \"example\");",
            f.name
        )
    } else {
        String::new()
    };

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    ctx.insert("props_fields", &props_fields);
    ctx.insert("found_assertion", &found_assertion);
    ctx.insert("test_vo_imports", &test_vo_imports);
    ctx
}

fn build_list_context(
    pascal: &str,
    snake: &str,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> tera::Context {
    let eff = effective_fields(fields);
    let first_props = build_test_props(&eff, "first");
    let second_props = build_test_props(&eff, "second");
    let test_vo_imports = build_test_vo_imports_block(&eff, snake, shared_vos);

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    ctx.insert("first_props", &first_props);
    ctx.insert("second_props", &second_props);
    ctx.insert("test_vo_imports", &test_vo_imports);
    ctx
}

fn build_update_context(
    pascal: &str,
    snake: &str,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> tera::Context {
    let eff = effective_fields(fields);

    let extra_imports: Vec<String> = {
        let mut imp = vec![];
        for f in &eff {
            if let Ok(mapping) = resolve_type(&f.field_type) {
                if let Some(i) = mapping.needs_import {
                    let stmt = format!("use {};", i);
                    if !imp.contains(&stmt) {
                        imp.push(stmt);
                    }
                }
            }
        }
        for f in eff.iter().filter(|f| is_value_object(f) && f.field_type != "Uuid") {
            let stmt = format!("use {};", vo_import_path(f, snake, shared_vos));
            if !imp.contains(&stmt) {
                imp.push(stmt);
            }
        }
        imp
    };

    let validations: Vec<String> = eff
        .iter()
        .filter(|f| f.field_type == "String" && !is_value_object(f))
        .map(|f| {
            format!(
                "        if params.{}.trim().is_empty() {{\n            let err = {}Error::ValidationError(\"{}_empty\".into());\n            self.logger.warn(&err.to_string());\n            return Err(err);\n        }}",
                f.name, pascal, f.name
            )
        })
        .collect();
    let validations_str = if validations.is_empty() {
        String::new()
    } else {
        validations.join("\n") + "\n"
    };

    let vo_constr: Vec<String> = eff
        .iter()
        .filter(|f| f.field_type != "Uuid" && is_value_object(f))
        .map(|f| field_vo_constructor(f, "params.", pascal, shared_vos))
        .collect();
    let vo_constructions = if vo_constr.is_empty() {
        String::new()
    } else {
        vo_constr.join("\n") + "\n"
    };

    let assignments: Vec<String> = eff
        .iter()
        .filter(|f| f.field_type != "Uuid")
        .map(|f| {
            if is_value_object(f) {
                format!("        entity.{} = {};", f.name, f.name)
            } else {
                format!("        entity.{} = params.{};", f.name, f.name)
            }
        })
        .collect();
    let assignments_str = assignments.join("\n");

    let original_props = build_test_props(&eff, "original");
    let update_params = build_update_params(&eff);
    let not_found_params = build_not_found_params(&eff);
    let test_vo_imports = build_test_vo_imports_block(&eff, snake, shared_vos);

    let primitive_string_fields: Vec<&Field> = eff
        .iter()
        .filter(|f| f.field_type == "String" && !is_value_object(f))
        .collect();
    let valid_assertion = if let Some(f) = primitive_string_fields.first().copied() {
        format!("assert_eq!(result.unwrap().{}, \"updated\");", f.name)
    } else if let Some(f) = eff
        .iter()
        .find(|f| is_value_object(f) && f.field_type == "String" && !is_enum_vo(f))
    {
        format!("assert_eq!(result.unwrap().{}.value(), \"updated\");", f.name)
    } else {
        "assert!(result.is_ok());".to_string()
    };

    let mut validation_test_cases: Vec<serde_json::Value> = vec![];
    for sf in &primitive_string_fields {
        let empty_params = build_empty_update_params(&eff, &sf.name);
        validation_test_cases.push(json!({
            "test_name": format!("should_return_error_when_{}_is_empty", sf.name),
            "empty_params": empty_params,
            "error_str": format!("{snake}.validation_error.{}_empty", sf.name),
        }));
    }
    for vf in eff
        .iter()
        .filter(|f| f.field_type == "String" && is_value_object(f) && !is_enum_vo(f))
    {
        let vo = vf.value_object.as_deref().unwrap();
        let vo_snake = pascal_to_snake(vo);
        let empty_params = build_empty_update_params(&eff, &vf.name);
        validation_test_cases.push(json!({
            "test_name": format!("should_return_error_when_{}_is_empty", vf.name),
            "empty_params": empty_params,
            "error_str": format!("{snake}.invalid_{vo_snake}"),
        }));
    }

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    ctx.insert("extra_imports", &extra_imports);
    ctx.insert("validations", &validations_str);
    ctx.insert("vo_constructions", &vo_constructions);
    ctx.insert("assignments", &assignments_str);
    ctx.insert("original_props", &original_props);
    ctx.insert("update_params", &update_params);
    ctx.insert("not_found_params", &not_found_params);
    ctx.insert("valid_assertion", &valid_assertion);
    ctx.insert("test_vo_imports", &test_vo_imports);
    ctx.insert("validation_test_cases", &validation_test_cases);
    ctx
}

fn build_delete_context(
    pascal: &str,
    snake: &str,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> tera::Context {
    let eff = effective_fields(fields);
    let props_fields = build_test_props(&eff, "example");
    let test_vo_imports = build_test_vo_imports_block(&eff, snake, shared_vos);

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    ctx.insert("props_fields", &props_fields);
    ctx.insert("test_vo_imports", &test_vo_imports);
    ctx
}

// ── Public generators (context builder + render) ──────────────────────────────

pub fn generate_create_use_case_impl(
    pascal: &str,
    snake: &str,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> String {
    let ctx = build_create_context(pascal, snake, fields, shared_vos);
    render("application/create.tera", &ctx).expect("application/create.tera render failed")
}

pub fn generate_get_use_case_impl(
    pascal: &str,
    snake: &str,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> String {
    let ctx = build_get_context(pascal, snake, fields, shared_vos);
    render("application/get.tera", &ctx).expect("application/get.tera render failed")
}

pub fn generate_list_use_case_impl(
    pascal: &str,
    snake: &str,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> String {
    let ctx = build_list_context(pascal, snake, fields, shared_vos);
    render("application/list.tera", &ctx).expect("application/list.tera render failed")
}

pub fn generate_update_use_case_impl(
    pascal: &str,
    snake: &str,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> String {
    let ctx = build_update_context(pascal, snake, fields, shared_vos);
    render("application/update.tera", &ctx).expect("application/update.tera render failed")
}

pub fn generate_delete_use_case_impl(
    pascal: &str,
    snake: &str,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> String {
    let ctx = build_delete_context(pascal, snake, fields, shared_vos);
    render("application/delete.tera", &ctx).expect("application/delete.tera render failed")
}

// ── File writers ──────────────────────────────────────────────────────────────

pub fn write_application_files(
    pascal: &str,
    snake: &str,
    base: &Path,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> Result<(), Box<dyn std::error::Error>> {
    write_file(
        &base.join(format!(
            "business/src/application/{snake}/create_{snake}.rs"
        )),
        &generate_create_use_case_impl(pascal, snake, fields, shared_vos),
    )?;
    write_file(
        &base.join(format!("business/src/application/{snake}/get_{snake}.rs")),
        &generate_get_use_case_impl(pascal, snake, fields, shared_vos),
    )?;
    write_file(
        &base.join(format!(
            "business/src/application/{snake}/list_{snake}.rs"
        )),
        &generate_list_use_case_impl(pascal, snake, fields, shared_vos),
    )?;
    write_file(
        &base.join(format!(
            "business/src/application/{snake}/update_{snake}.rs"
        )),
        &generate_update_use_case_impl(pascal, snake, fields, shared_vos),
    )?;
    write_file(
        &base.join(format!(
            "business/src/application/{snake}/delete_{snake}.rs"
        )),
        &generate_delete_use_case_impl(pascal, snake, fields, shared_vos),
    )?;
    Ok(())
}

pub fn run_generate_application(
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

    let fields: Vec<Field> = config
        .entity
        .iter()
        .find(|e| e.name == pascal)
        .map(|e| e.fields.clone())
        .unwrap_or_default();
    let shared_vos = config.value_object.clone();

    write_application_files(&pascal, &snake, base, &fields, &shared_vos)?;
    patch_business_lib_application_crud(base, &snake)?;

    println!("✓ business/application/ — 5 use case impls (create, get, list, update, delete)");
    println!();
    println!("  Next: puerto generate repository {pascal}");
    Ok(())
}
