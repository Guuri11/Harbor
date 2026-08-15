use std::{fs, path::Path};

use serde_json::json;

use crate::generators::naming::{pascal_to_snake, to_pascal_case, write_file};
use crate::generators::render::render;
use crate::generators::types::{
    field_rust_type, is_enum_vo, is_option_vo, is_shared_vo, is_value_object, is_vec_vo,
    resolve_type, vo_import_path, vo_inner_type,
};
use crate::patchers::lib_rs::{
    patch_business_lib_domain_crud, patch_business_lib_shared, patch_business_lib_value_objects,
    patch_lib_block,
};
use crate::puerto_toml::{Field, ValueObjectDefinition};

use crate::generators::effective_fields;

/// Imports `domain/model.tera` writes itself, for the always-present system fields. Field-derived
/// imports matching these must not be emitted again.
const MODEL_TEMPLATE_IMPORTS: &[&str] = &["use chrono::{DateTime, Utc};", "use uuid::Uuid;"];

/// A single `EntityProps` field literal, e.g. `            tags: vec![],`.
///
/// **The one place that renders a Props field literal.** The four VO shapes (plain, `Option<VO>`,
/// `Vec<VO>`, enum) must be handled in the same order everywhere: when the validation-test builders
/// carried their own copy of this branching they omitted `Option`/`Vec`, emitting
/// `Tag::new(vec![])` and `Mid::new(None)` — generated tests that did not compile.
fn props_literal(f: &Field) -> String {
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
}

/// Every field of an `EntityProps` literal, one line each.
///
/// `override_field` replaces a single field's value — how the validation tests inject `""` /
/// `"   "` into the field under test while every other field keeps a valid default.
fn props_literal_lines(eff: &[Field], override_field: Option<(&str, &str)>) -> String {
    eff.iter()
        .map(|f| match override_field {
            Some((name, value)) if f.name == name => format!("            {}: {},", f.name, value),
            _ => props_literal(f),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn generate_model(
    pascal: &str,
    snake: &str,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> String {
    let eff = effective_fields(fields);

    let mut extra_imports: Vec<String> = vec![];
    for f in &eff {
        if let Ok(mapping) = resolve_type(&f.field_type) {
            if let Some(imp) = mapping.needs_import {
                let stmt = format!("use {};", imp);
                // `model.tera` already imports chrono and uuid for the system fields (`id`,
                // `created_at`, …). A `DateTime<Utc>` or `Uuid` field would otherwise re-import
                // them: E0252, the name is defined multiple times.
                if MODEL_TEMPLATE_IMPORTS.contains(&stmt.as_str()) {
                    continue;
                }
                if !extra_imports.contains(&stmt) {
                    extra_imports.push(stmt);
                }
            }
        }
    }
    let vo_fields: Vec<&Field> = eff.iter().filter(|f| is_value_object(f)).collect();
    for f in &vo_fields {
        let stmt = if is_shared_vo(f, shared_vos) {
            format!(
                "use crate::domain::shared::value_objects::{};",
                f.value_object.as_deref().unwrap()
            )
        } else {
            format!(
                "use super::value_objects::{};",
                f.value_object.as_deref().unwrap()
            )
        };
        if !extra_imports.contains(&stmt) {
            extra_imports.push(stmt);
        }
    }
    let extra_imports_str = if extra_imports.is_empty() {
        String::new()
    } else {
        format!("\n{}", extra_imports.join("\n"))
    };

    let props_str = eff
        .iter()
        .map(|f| format!("    pub {}: {},", f.name, field_rust_type(f)))
        .collect::<Vec<_>>()
        .join("\n");

    let mut entity_lines = vec![
        "    pub id: Uuid,".to_string(),
        "    pub created_at: DateTime<Utc>,".to_string(),
        "    pub updated_at: DateTime<Utc>,".to_string(),
        "    pub deleted: bool,".to_string(),
        "    pub deleted_at: Option<DateTime<Utc>>,".to_string(),
    ];
    for f in &eff {
        entity_lines.push(format!("    pub {}: {},", f.name, field_rust_type(f)));
    }
    let entity_str = entity_lines.join("\n");

    let validations: Vec<String> = eff
        .iter()
        .filter(|f| f.field_type == "String" && !is_value_object(f))
        .map(|f| {
            format!(
                "        if props.{}.trim().is_empty() {{\n            return Err({}Error::ValidationError(\"{}_empty\".into()));\n        }}",
                f.name, pascal, f.name
            )
        })
        .collect();
    let validations_str = if validations.is_empty() {
        String::new()
    } else {
        validations.join("\n") + "\n"
    };

    let new_assignments_str = eff
        .iter()
        .map(|f| format!("            {}: props.{},", f.name, f.name))
        .collect::<Vec<_>>()
        .join("\n");

    let valid_props_str = props_literal_lines(&eff, None);

    let required_string_fields: Vec<&Field> = eff
        .iter()
        .filter(|f| f.field_type == "String" && !is_value_object(f))
        .collect();

    let valid_test_name = if eff.len() == 1
        && eff[0].name == "name"
        && eff[0].field_type == "String"
        && !is_value_object(&eff[0])
    {
        format!("should_create_{snake}_when_name_is_valid")
    } else {
        format!("should_create_{snake}_when_fields_are_valid")
    };

    let valid_assertion = if let Some(f) = eff
        .iter()
        .find(|f| f.field_type == "String" && !is_value_object(f))
    {
        format!(
            "\n        assert_eq!(result.unwrap().{}, \"example\");",
            f.name
        )
    } else if !eff.is_empty() {
        "\n        assert!(result.is_ok());".to_string()
    } else {
        String::new()
    };

    let mut validation_tests: Vec<String> = vec![];
    for sf in &required_string_fields {
        let field_name = sf.name.clone();
        validation_tests.push(format!(
            "    #[test]\n    fn should_reject_{snake}_when_{field}_is_empty() {{\n        let result = {pascal}::new({pascal}Props {{\n{props}\n        }});\n        assert!(result.is_err());\n        assert_eq!(\n            result.unwrap_err().to_string(),\n            \"{snake}.validation_error.{field}_empty\"\n        );\n    }}",
            field = field_name,
            props = props_literal_lines(&eff, Some((&field_name, "\"\".into()"))),
        ));

        validation_tests.push(format!(
            "    #[test]\n    fn should_reject_{snake}_when_{field}_is_only_whitespace() {{\n        let result = {pascal}::new({pascal}Props {{\n{props}\n        }});\n        assert!(result.is_err());\n    }}",
            field = field_name,
            props = props_literal_lines(&eff, Some((&field_name, "\"   \".into()"))),
        ));
    }

    for vf in eff
        .iter()
        .filter(|f| is_value_object(f) && f.field_type == "String" && !is_enum_vo(f))
    {
        let vo = vf.value_object.as_deref().unwrap();
        let snake_vo = pascal_to_snake(vo);
        let error_str = if is_shared_vo(vf, shared_vos) {
            format!("shared.value_object.{snake_vo}.invalid")
        } else {
            format!("{snake}.invalid_{snake_vo}")
        };
        validation_tests.push(format!(
            "    #[test]\n    fn should_reject_{snake}_when_{snake_vo}_is_empty() {{\n        let result = {vo}::new(\"\".to_string());\n        assert!(result.is_err());\n        assert_eq!(\n            result.unwrap_err().to_string(),\n            \"{error_str}\"\n        );\n    }}"
        ));
    }

    for vf in eff.iter().filter(|f| is_enum_vo(f)) {
        let vo = vf.value_object.as_deref().unwrap();
        let snake_vo = pascal_to_snake(vo);
        let error_str = if is_shared_vo(vf, shared_vos) {
            format!("shared.value_object.{snake_vo}.invalid")
        } else {
            format!("{snake}.invalid_{snake_vo}")
        };
        validation_tests.push(format!(
            "    #[test]\n    fn should_reject_{snake}_when_{snake_vo}_is_invalid() {{\n        let result = {vo}::from_str(\"InvalidVariant\");\n        assert!(result.is_err());\n        assert_eq!(\n            result.unwrap_err().to_string(),\n            \"{error_str}\"\n        );\n    }}"
        ));
    }

    let validation_tests_str = if validation_tests.is_empty() {
        String::new()
    } else {
        format!("\n\n{}", validation_tests.join("\n\n"))
    };

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    ctx.insert("extra_imports", &extra_imports_str);
    ctx.insert("props_str", &props_str);
    ctx.insert("entity_str", &entity_str);
    ctx.insert("validations_str", &validations_str);
    ctx.insert("new_assignments_str", &new_assignments_str);
    ctx.insert("valid_test_name", &valid_test_name);
    ctx.insert("valid_props_str", &valid_props_str);
    ctx.insert("valid_assertion", &valid_assertion);
    ctx.insert("validation_tests_str", &validation_tests_str);
    render("domain/model.tera", &ctx).expect("model.tera render failed")
}

pub fn generate_mother(
    pascal: &str,
    snake: &str,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> String {
    let eff = effective_fields(fields);

    let mut mother_imports: Vec<String> = vec![];
    for f in &eff {
        if let Ok(mapping) = resolve_type(&f.field_type) {
            if let Some(imp) = mapping.needs_import {
                let stmt = format!("use {};", imp);
                if !mother_imports.contains(&stmt) {
                    mother_imports.push(stmt);
                }
            }
        }
    }
    for vf in eff.iter().filter(|f| is_value_object(f)) {
        let import_path = vo_import_path(vf, snake, shared_vos);
        let stmt = format!("use {};", import_path);
        if !mother_imports.contains(&stmt) {
            mother_imports.push(stmt);
        }
    }
    let imports_str = if mother_imports.is_empty() {
        String::new()
    } else {
        format!("\n{}", mother_imports.join("\n"))
    };

    let required_string_fields: Vec<&Field> = eff
        .iter()
        .filter(|f| f.field_type == "String" && !is_value_object(f))
        .collect();

    let fields_str = eff
        .iter()
        .map(|f| {
            if is_vec_vo(f) {
                let vo = f.value_object.as_deref().unwrap();
                format!("    {}: Option<Vec<{}>>,", f.name, vo)
            } else if is_value_object(f) {
                let vo = f.value_object.as_deref().unwrap();
                format!("    {}: Option<{}>,", f.name, vo)
            } else {
                format!(
                    "    {}: Option<{}>,",
                    f.name,
                    mother_storage_type(&f.field_type)
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let with_methods_str = eff
        .iter()
        .map(|f| {
            if is_vec_vo(f) {
                let vo = f.value_object.as_deref().unwrap();
                format!(
                    "    pub fn with_{field}(mut self, {field}: Vec<{vo}>) -> Self {{\n        self.{field} = Some({field});\n        self\n    }}",
                    field = f.name, vo = vo,
                )
            } else if is_value_object(f) {
                let vo = f.value_object.as_deref().unwrap();
                format!(
                    "    pub fn with_{field}(mut self, {field}: {vo}) -> Self {{\n        self.{field} = Some({field});\n        self\n    }}",
                    field = f.name, vo = vo,
                )
            } else {
                let (param_type, conversion) = mother_with_param(&f.field_type, &f.name);
                format!(
                    "    pub fn with_{field}(mut self, {field}: {param_type}) -> Self {{\n        self.{field} = Some({conversion});\n        self\n    }}",
                    field = f.name, param_type = param_type, conversion = conversion,
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let empty_methods = required_string_fields
        .iter()
        .map(|f| format!(
            "    pub fn with_empty_{field}(mut self) -> Self {{\n        self.{field} = Some(String::new());\n        self\n    }}",
            field = f.name,
        ))
        .collect::<Vec<_>>();
    let empty_methods_str = if empty_methods.is_empty() {
        String::new()
    } else {
        format!("\n\n{}", empty_methods.join("\n\n"))
    };

    let build_assignments_str = eff
        .iter()
        .map(|f| {
            if is_option_vo(f) {
                format!("            {}: self.{},", f.name, f.name)
            } else if is_vec_vo(f) {
                format!("            {}: self.{}.unwrap_or_default(),", f.name, f.name)
            } else if is_enum_vo(f) {
                let vo = f.value_object.as_deref().unwrap();
                let first_variant = f.enum_variants.as_deref().unwrap().first().unwrap();
                format!("            {}: self.{}.unwrap_or_else(|| {}::{}),", f.name, f.name, vo, first_variant)
            } else if is_value_object(f) {
                let vo = f.value_object.as_deref().unwrap();
                let mapping = resolve_type(&f.field_type).unwrap();
                match mapping.rust_type {
                    "String" => format!("            {}: self.{}.unwrap_or_else(|| {}::new(\"example\".to_string()).expect(\"valid {}\")),", f.name, f.name, vo, vo),
                    "i64" => format!("            {}: self.{}.unwrap_or_else(|| {}::new(42).expect(\"valid {}\")),", f.name, f.name, vo, vo),
                    "bool" => format!("            {}: self.{}.unwrap_or_else(|| {}::new(true).expect(\"valid {}\")),", f.name, f.name, vo, vo),
                    "f64" => format!("            {}: self.{}.unwrap_or_else(|| {}::new(1.5).expect(\"valid {}\")),", f.name, f.name, vo, vo),
                    "Uuid" => format!("            {}: self.{}.unwrap_or_else(|| {}::new(Uuid::new_v4()).expect(\"valid {}\")),", f.name, f.name, vo, vo),
                    "DateTime<Utc>" => format!("            {}: self.{}.unwrap_or_else(|| {}::new(Utc::now()).expect(\"valid {}\")),", f.name, f.name, vo, vo),
                    _ => format!("            {}: self.{}.unwrap_or_default(),", f.name, f.name),
                }
            } else if is_option_type(&f.field_type) {
                format!("            {}: self.{},", f.name, f.name)
            } else {
                let mapping = resolve_type(&f.field_type).unwrap();
                match mapping.rust_type {
                    "String" => format!("            {}: self.{}.unwrap_or_else(|| \"example\".to_string()),", f.name, f.name),
                    "i64" => format!("            {}: self.{}.unwrap_or(42),", f.name, f.name),
                    "bool" => format!("            {}: self.{}.unwrap_or(true),", f.name, f.name),
                    "f64" => format!("            {}: self.{}.unwrap_or(1.5),", f.name, f.name),
                    "Uuid" => format!("            {}: self.{}.unwrap_or_else(Uuid::new_v4),", f.name, f.name),
                    "DateTime<Utc>" => format!("            {}: self.{}.unwrap_or_else(Utc::now),", f.name, f.name),
                    _ => format!("            {}: self.{}.unwrap_or_default(),", f.name, f.name),
                }
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let defaults = eff
        .iter()
        .map(|f| format!("{}: None", f.name))
        .collect::<Vec<_>>()
        .join(", ");

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    ctx.insert("imports", &imports_str);
    ctx.insert("fields_str", &fields_str);
    ctx.insert("defaults", &defaults);
    ctx.insert("with_methods_str", &with_methods_str);
    ctx.insert("empty_methods_str", &empty_methods_str);
    ctx.insert("build_assignments_str", &build_assignments_str);
    ctx.insert("props_assignments_str", &build_assignments_str);
    render("domain/mother.tera", &ctx).expect("mother.tera render failed")
}

fn is_option_type(field_type: &str) -> bool {
    field_type.starts_with("Option<")
}

fn mother_storage_type(field_type: &str) -> String {
    if is_option_type(field_type) {
        let inner = field_type
            .strip_prefix("Option<")
            .unwrap()
            .strip_suffix('>')
            .unwrap();
        inner.to_string()
    } else {
        field_type.to_string()
    }
}

fn mother_with_param(field_type: &str, field_name: &str) -> (String, String) {
    if is_option_type(field_type) {
        let inner = field_type
            .strip_prefix("Option<")
            .unwrap()
            .strip_suffix('>')
            .unwrap();
        match inner {
            "String" => ("&str".to_string(), format!("{}.to_string()", field_name)),
            "DateTime<Utc>" => ("DateTime<Utc>".to_string(), field_name.to_string()),
            _ => (inner.to_string(), field_name.to_string()),
        }
    } else {
        match field_type {
            "String" => ("&str".to_string(), format!("{}.to_string()", field_name)),
            "DateTime<Utc>" => ("DateTime<Utc>".to_string(), field_name.to_string()),
            _ => (field_type.to_string(), field_name.to_string()),
        }
    }
}

pub fn generate_value_objects(
    pascal: &str,
    _snake: &str,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> String {
    let eff = effective_fields(fields);
    let vo_fields: Vec<&Field> = eff
        .iter()
        .filter(|f| is_value_object(f) && !is_shared_vo(f, shared_vos))
        .collect();

    if vo_fields.is_empty() {
        return String::new();
    }

    let mut file_imports: Vec<String> = vec![format!("use super::errors::{}Error;", pascal)];
    for f in &vo_fields {
        if is_enum_vo(f) {
            continue;
        }
        let inner = vo_inner_type(f);
        let imp = match inner.as_str() {
            "Uuid" => Some("use uuid::Uuid;".to_string()),
            "DateTime<Utc>" => Some("use chrono::{DateTime, Utc};".to_string()),
            _ => None,
        };
        if let Some(i) = imp {
            if !file_imports.contains(&i) {
                file_imports.push(i);
            }
        }
    }

    let mut vo_structs: Vec<String> = vec![];
    for f in &vo_fields {
        let vo = f.value_object.as_deref().unwrap();
        if is_enum_vo(f) {
            let variants = f.enum_variants.as_deref().unwrap();
            let variant_lines = variants
                .iter()
                .map(|v| format!("    {},", v))
                .collect::<Vec<_>>()
                .join("\n");
            let from_str_arms = variants
                .iter()
                .map(|v| format!("            \"{}\" => Ok(Self::{}),", v, v))
                .collect::<Vec<_>>()
                .join("\n");
            let as_str_arms = variants
                .iter()
                .map(|v| format!("            Self::{} => \"{}\",", v, v))
                .collect::<Vec<_>>()
                .join("\n");
            let unknown_arm = format!("            _ => Err({pascal}Error::Invalid{vo}),");
            vo_structs.push(format!(
                "#[derive(Debug, Clone, PartialEq)]\npub enum {vo} {{\n{variant_lines}\n}}\n\nimpl {vo} {{\n    pub fn from_str(s: &str) -> Result<Self, {pascal}Error> {{\n        match s {{\n{from_str_arms}\n{unknown_arm}\n        }}\n    }}\n\n    pub fn as_str(&self) -> &'static str {{\n        match self {{\n{as_str_arms}\n        }}\n    }}\n}}"
            ));
            continue;
        }
        let inner_type = vo_inner_type(f);
        let struct_code = match inner_type.as_str() {
            "String" => format!(
                "#[derive(Debug, Clone, PartialEq)]\npub struct {vo} {{\n    value: String,\n}}\n\nimpl {vo} {{\n    pub fn new(value: String) -> Result<Self, {pascal}Error> {{\n        let trimmed = value.trim().to_string();\n        if trimmed.is_empty() {{\n            return Err({pascal}Error::Invalid{vo});\n        }}\n        Ok(Self {{ value: trimmed }})\n    }}\n\n    pub fn value(&self) -> &str {{\n        &self.value\n    }}\n}}"
            ),
            "i64" | "f64" | "bool" => format!(
                "#[derive(Debug, Clone, Copy, PartialEq)]\npub struct {vo}({inner_type});\n\nimpl {vo} {{\n    pub fn new(value: {inner_type}) -> Result<Self, {pascal}Error> {{\n        Ok(Self(value))\n    }}\n\n    pub fn value(&self) -> {inner_type} {{\n        self.0\n    }}\n}}"
            ),
            "Uuid" => format!(
                "#[derive(Debug, Clone, Copy, PartialEq)]\npub struct {vo}(Uuid);\n\nimpl {vo} {{\n    pub fn new(value: Uuid) -> Result<Self, {pascal}Error> {{\n        Ok(Self(value))\n    }}\n\n    pub fn value(&self) -> Uuid {{\n        self.0\n    }}\n}}"
            ),
            "DateTime<Utc>" => format!(
                "#[derive(Debug, Clone, PartialEq)]\npub struct {vo}(DateTime<Utc>);\n\nimpl {vo} {{\n    pub fn new(value: DateTime<Utc>) -> Result<Self, {pascal}Error> {{\n        Ok(Self(value))\n    }}\n\n    pub fn value(&self) -> DateTime<Utc> {{\n        self.0\n    }}\n}}"
            ),
            _ => continue,
        };
        vo_structs.push(struct_code);
    }

    let file_imports_str = file_imports.join("\n");
    let vo_structs_str = vo_structs.join("\n\n");
    let mut ctx = tera::Context::new();
    ctx.insert("file_imports", &file_imports_str);
    ctx.insert("vo_structs", &vo_structs_str);
    render("domain/value_objects.tera", &ctx).expect("value_objects.tera render failed")
}

pub fn generate_create_use_case_trait(pascal: &str, snake: &str, fields: &[Field]) -> String {
    let eff = effective_fields(fields);

    let mut extra_imports: Vec<String> = vec![];
    for f in &eff {
        if let Ok(mapping) = resolve_type(&f.field_type) {
            if let Some(imp) = mapping.needs_import {
                let stmt = format!("use {};", imp);
                if !extra_imports.contains(&stmt) {
                    extra_imports.push(stmt);
                }
            }
        }
    }

    let params_fields: Vec<serde_json::Value> = eff
        .iter()
        .filter(|f| f.field_type != "Uuid")
        .map(|f| json!({ "name": f.name, "field_type": f.field_type }))
        .collect();

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    ctx.insert("extra_imports", &extra_imports);
    ctx.insert("params_fields", &params_fields);

    render("domain/use_case_create.tera", &ctx).expect("use_case_create.tera render failed")
}

pub fn generate_update_use_case_trait(pascal: &str, snake: &str, fields: &[Field]) -> String {
    let eff = effective_fields(fields);

    let mut extra_imports: Vec<String> = vec![];
    for f in &eff {
        if let Ok(mapping) = resolve_type(&f.field_type) {
            if let Some(imp) = mapping.needs_import {
                let stmt = format!("use {};", imp);
                if !extra_imports.contains(&stmt) {
                    extra_imports.push(stmt);
                }
            }
        }
    }

    let params_fields: Vec<serde_json::Value> = eff
        .iter()
        .filter(|f| f.field_type != "Uuid")
        .map(|f| json!({ "name": f.name, "field_type": f.field_type }))
        .collect();

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    ctx.insert("extra_imports", &extra_imports);
    ctx.insert("params_fields", &params_fields);

    render("domain/use_case_update.tera", &ctx).expect("use_case_update.tera render failed")
}

pub fn generate_shared_value_objects(vos: &[ValueObjectDefinition]) -> String {
    if vos.is_empty() {
        return String::new();
    }

    let mut vo_structs: Vec<String> = vec![];
    for vo_def in vos {
        let vo = &vo_def.name;
        let inner = &vo_def.inner_type;
        let struct_code = match inner.as_str() {
            "String" => format!(
                "use super::errors::{vo}Error;\n\n#[derive(Debug, Clone, PartialEq)]\npub struct {vo} {{\n    value: String,\n}}\n\nimpl {vo} {{\n    pub fn new(value: String) -> Result<Self, {vo}Error> {{\n        let trimmed = value.trim().to_string();\n        if trimmed.is_empty() {{\n            return Err({vo}Error::Invalid);\n        }}\n        Ok(Self {{ value: trimmed }})\n    }}\n\n    pub fn value(&self) -> &str {{\n        &self.value\n    }}\n}}"
            ),
            "i64" | "f64" | "bool" => format!(
                "use super::errors::{vo}Error;\n\n#[derive(Debug, Clone, Copy, PartialEq)]\npub struct {vo}({inner});\n\nimpl {vo} {{\n    pub fn new(value: {inner}) -> Result<Self, {vo}Error> {{\n        Ok(Self(value))\n    }}\n\n    pub fn value(&self) -> {inner} {{\n        self.0\n    }}\n}}"
            ),
            "Uuid" => format!(
                "use super::errors::{vo}Error;\nuse uuid::Uuid;\n\n#[derive(Debug, Clone, Copy, PartialEq)]\npub struct {vo}(Uuid);\n\nimpl {vo} {{\n    pub fn new(value: Uuid) -> Result<Self, {vo}Error> {{\n        Ok(Self(value))\n    }}\n\n    pub fn value(&self) -> Uuid {{\n        self.0\n    }}\n}}"
            ),
            "DateTime<Utc>" => format!(
                "use super::errors::{vo}Error;\nuse chrono::{{DateTime, Utc}};\n\n#[derive(Debug, Clone, PartialEq)]\npub struct {vo}(DateTime<Utc>);\n\nimpl {vo} {{\n    pub fn new(value: DateTime<Utc>) -> Result<Self, {vo}Error> {{\n        Ok(Self(value))\n    }}\n\n    pub fn value(&self) -> DateTime<Utc> {{\n        self.0\n    }}\n}}"
            ),
            _ => continue,
        };
        vo_structs.push(struct_code);
    }

    let vo_structs_str = vo_structs.join("\n\n");
    let mut ctx = tera::Context::new();
    ctx.insert("vo_structs", &vo_structs_str);
    render("domain/shared_value_objects.tera", &ctx)
        .expect("shared_value_objects.tera render failed")
}

#[allow(dead_code)]
pub fn generate_shared_errors(vos: &[ValueObjectDefinition]) -> String {
    if vos.is_empty() {
        return String::new();
    }

    let mut error_variants: Vec<String> = vec![];
    for vo_def in vos {
        let vo = &vo_def.name;
        let vo_snake = pascal_to_snake(vo);
        error_variants.push(format!(
            "    #[error(\"shared.value_object.{}.invalid\")]\n    Invalid,",
            vo_snake
        ));
    }

    let variants_str = error_variants.join("\n");

    format!(
        r#"use thiserror::Error;

#[derive(Debug, Error)]
pub enum {vo}Error {{
{variants}
}}"#,
        vo = if vos.len() == 1 {
            vos[0].name.clone()
        } else {
            "Shared".to_string()
        },
        variants = variants_str,
    )
}

pub fn generate_shared_errors_combined(vos: &[ValueObjectDefinition]) -> String {
    if vos.is_empty() {
        return String::new();
    }

    let error_defs = vos
        .iter()
        .map(|vo_def| {
            let vo = &vo_def.name;
            let vo_snake = pascal_to_snake(vo);
            format!(
                "#[derive(Debug, Error)]\npub enum {vo}Error {{\n    #[error(\"shared.value_object.{vo_snake}.invalid\")]\n    Invalid,\n}}"
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let mut ctx = tera::Context::new();
    ctx.insert("error_defs", &error_defs);
    render("domain/shared_errors.tera", &ctx).expect("shared_errors.tera render failed")
}

pub fn write_shared_vo_files(
    base: &Path,
    shared_vos: &[ValueObjectDefinition],
) -> Result<(), Box<dyn std::error::Error>> {
    if shared_vos.is_empty() {
        return Ok(());
    }

    let shared_dir = base.join("business/src/domain/shared");
    fs::create_dir_all(&shared_dir)?;

    write_file(
        &shared_dir.join("value_objects.rs"),
        &generate_shared_value_objects(shared_vos),
    )?;

    write_file(
        &shared_dir.join("errors.rs"),
        &generate_shared_errors_combined(shared_vos),
    )?;

    write_file(
        &shared_dir.join("mod.rs"),
        "pub mod errors;\npub mod value_objects;\n",
    )?;

    Ok(())
}

pub fn write_domain_files(
    pascal: &str,
    snake: &str,
    base: &Path,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> Result<(), Box<dyn std::error::Error>> {
    write_file(
        &base.join(format!("business/src/domain/{snake}/model.rs")),
        &generate_model(pascal, snake, fields, shared_vos),
    )?;

    let eff = effective_fields(fields);
    let has_local_vo = eff
        .iter()
        .any(|f| is_value_object(f) && !is_shared_vo(f, shared_vos));

    let vo_fields: Vec<serde_json::Value> = eff
        .iter()
        .filter(|f| is_value_object(f))
        .map(|f| {
            let vo = f.value_object.as_deref().unwrap();
            json!({ "vo_pascal": vo, "vo_snake": pascal_to_snake(vo) })
        })
        .collect();
    let mut errors_ctx = tera::Context::new();
    errors_ctx.insert("pascal", pascal);
    errors_ctx.insert("snake", snake);
    errors_ctx.insert("vo_fields", &vo_fields);
    write_file(
        &base.join(format!("business/src/domain/{snake}/errors.rs")),
        &render("domain/errors.tera", &errors_ctx)?,
    )?;

    if has_local_vo {
        let vo_content = generate_value_objects(pascal, snake, fields, shared_vos);
        write_file(
            &base.join(format!("business/src/domain/{snake}/value_objects.rs")),
            &vo_content,
        )?;
    }

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", pascal);
    ctx.insert("snake", snake);
    write_file(
        &base.join(format!("business/src/domain/{snake}/repository.rs")),
        &render("domain/repository.tera", &ctx)?,
    )?;
    write_file(
        &base.join(format!(
            "business/src/domain/{snake}/use_cases/create_{snake}.rs"
        )),
        &generate_create_use_case_trait(pascal, snake, fields),
    )?;
    write_file(
        &base.join(format!(
            "business/src/domain/{snake}/use_cases/get_{snake}.rs"
        )),
        &render("domain/use_case_get.tera", &ctx)?,
    )?;
    write_file(
        &base.join(format!(
            "business/src/domain/{snake}/use_cases/list_{snake}.rs"
        )),
        &render("domain/use_case_list.tera", &ctx)?,
    )?;
    write_file(
        &base.join(format!(
            "business/src/domain/{snake}/use_cases/update_{snake}.rs"
        )),
        &generate_update_use_case_trait(pascal, snake, fields),
    )?;
    write_file(
        &base.join(format!(
            "business/src/domain/{snake}/use_cases/delete_{snake}.rs"
        )),
        &render("domain/use_case_delete.tera", &ctx)?,
    )?;
    Ok(())
}

pub fn write_mother(
    pascal: &str,
    snake: &str,
    base: &Path,
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> Result<(), Box<dyn std::error::Error>> {
    write_file(
        &base.join(format!("business/src/tests/mothers/{snake}_mother.rs")),
        &generate_mother(pascal, snake, fields, shared_vos),
    )?;
    Ok(())
}

pub fn patch_mothers_lib(base: &Path, snake: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = base.join("business/src/lib.rs");
    let src = fs::read_to_string(&path)?;

    if src.contains(&format!("pub mod {snake}_mother;")) {
        return Ok(());
    }

    let new_mod = format!("\n        pub mod {snake}_mother;\n");

    if let Ok(patched) = patch_lib_block(&src, &["tests", "mothers"], &new_mod) {
        fs::write(&path, patched)?;
        return Ok(());
    }

    let mut content = src;
    if !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&format!(
        "\n#[cfg(test)]\npub mod tests {{\n    pub mod mothers {{\n        pub mod {snake}_mother;\n    }}\n}}\n"
    ));
    fs::write(&path, content)?;
    Ok(())
}

/// `puerto generate domain <Name> [-- fields]` — the first step of the domain-first workflow.
///
/// `cli_fields` are the trailing `-- name:Type` arguments, already parsed and type-checked.
/// They are persisted to puerto.toml here, which is what makes the later layered commands
/// (`application`, `repository`, `presentation`) field-aware: they all read their fields back
/// from the manifest.
pub fn run_generate_domain(
    name: &str,
    base: &Path,
    cli_fields: &[Field],
) -> Result<(), Box<dyn std::error::Error>> {
    let config = crate::puerto_toml::read(base)?;
    let pascal = to_pascal_case(name);
    let snake = pascal_to_snake(&pascal);

    crate::validation::validate_entity_name(&pascal)?;

    if config.entity.iter().any(|e| e.name == pascal) {
        return Err(format!(
            "{pascal} is already in puerto.toml. Use `puerto generate use-case` to add a use case."
        )
        .into());
    }

    let fields: Vec<Field> = cli_fields.to_vec();
    let shared_vos = config.value_object.clone();

    let vo_conflicts = crate::validation::validate_vo_coherence(&fields, &shared_vos);
    if !vo_conflicts.is_empty() {
        return Err(vo_conflicts.join("\n  ").into());
    }

    write_domain_files(&pascal, &snake, base, &fields, &shared_vos)?;
    write_mother(&pascal, &snake, base, &fields, &shared_vos)?;
    patch_business_lib_domain_crud(base, &snake)?;

    let has_vo = fields.iter().any(is_value_object);
    let has_local_vo = fields
        .iter()
        .any(|f| is_value_object(f) && !is_shared_vo(f, &shared_vos));
    if has_local_vo {
        patch_business_lib_value_objects(base, &snake)?;
    }

    if !shared_vos.is_empty() {
        write_shared_vo_files(base, &shared_vos)?;
        patch_business_lib_shared(base)?;
    }

    patch_mothers_lib(base, &snake)?;

    let use_cases = vec![
        format!("create_{snake}"),
        format!("get_{snake}"),
        format!("list_{snake}"),
        format!("update_{snake}"),
        format!("delete_{snake}"),
    ];
    crate::puerto_toml::add_entity(base, &pascal, use_cases, config.project.db, fields.clone())?;

    if has_local_vo {
        println!(
            "✓ business/domain/    — model, errors, value_objects, repository trait, 5 use case traits"
        );
    } else if has_vo {
        println!(
            "✓ business/domain/    — model, errors (shared VOs), repository trait, 5 use case traits"
        );
    } else {
        println!("✓ business/domain/    — model, errors, repository trait, 5 use case traits");
    }
    if !shared_vos.is_empty() {
        println!("✓ business/domain/shared/ — shared value objects + errors");
    }
    println!("✓ business/tests/     — {pascal}Mother (Object Mother)");
    println!("✓ puerto.toml         — {pascal} registered");
    println!();
    println!("  Next: puerto generate application {pascal}");
    Ok(())
}
