use std::path::Path;

use crate::generators::types::{
    is_enum_vo, is_option_vo, is_value_object, is_vec_vo, resolve_type,
};
use crate::puerto_toml;

const SHARED_VO_ALLOWED_INNER_TYPES: &[&str] =
    &["String", "i64", "bool", "f64", "Uuid", "DateTime<Utc>"];

fn is_valid_use_case_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

const VO_ALLOWED_INNER_TYPES: &[&str] = &[
    "String",
    "i64",
    "bool",
    "f64",
    "Uuid",
    "DateTime<Utc>",
    "Option<String>",
    "Option<i64>",
    "Option<bool>",
    "Option<f64>",
    "Option<Uuid>",
    "Option<DateTime<Utc>>",
    "Vec<String>",
    "Vec<i64>",
];

fn vo_inner_type_is_valid(field_type: &str) -> bool {
    if field_type.starts_with("Option<") {
        let inner = &field_type[7..field_type.len() - 1];
        return !inner.starts_with("Vec<")
            && !inner.starts_with("Option<")
            && !inner.starts_with("HashMap<");
    }
    if field_type.starts_with("Vec<") {
        let inner = &field_type[4..field_type.len() - 1];
        return !inner.starts_with("Option<")
            && !inner.starts_with("Vec<")
            && !inner.starts_with("HashMap<");
    }
    !field_type.starts_with("HashMap<")
}

pub fn run_validate(cwd: &Path) -> Result<(), Box<dyn std::error::Error>> {
    crate::commands::list::require_puerto_project(cwd)?;
    let config = puerto_toml::read(cwd)?;

    let mut errors: Vec<String> = vec![];
    let mut warnings: Vec<String> = vec![];

    if config.project.name.is_empty() {
        errors.push("project.name is empty".into());
    }

    let mut seen_entities: Vec<String> = vec![];
    for entity in &config.entity {
        if let Err(e) = crate::validation::validate_entity_name(&entity.name) {
            errors.push(format!("entity '{}': {}", entity.name, e));
        }

        if seen_entities.contains(&entity.name) {
            errors.push(format!("duplicate entity name '{}'", entity.name));
        }
        seen_entities.push(entity.name.clone());

        if entity.use_cases.is_empty() {
            warnings.push(format!(
                "entity '{}': has no use cases defined",
                entity.name
            ));
        }

        for uc in &entity.use_cases {
            if !is_valid_use_case_name(uc) {
                errors.push(format!(
                    "entity '{}': use case '{}' is not valid snake_case",
                    entity.name, uc
                ));
            }
        }

        let mut seen_fields: Vec<String> = vec![];
        for field in &entity.fields {
            if field.name.is_empty() {
                errors.push(format!("entity '{}': field has empty name", entity.name));
                continue;
            }

            if let Err(e) = crate::validation::validate_field_name(&field.name) {
                errors.push(format!("entity '{}': {}", entity.name, e));
            }

            if seen_fields.contains(&field.name) {
                errors.push(format!(
                    "entity '{}': duplicate field name '{}'",
                    entity.name, field.name
                ));
            }
            seen_fields.push(field.name.clone());

            if let Err(e) = resolve_type(&field.field_type) {
                errors.push(format!(
                    "entity '{}': field '{}' has {}",
                    entity.name, field.name, e
                ));
            }

            if field.field_type.starts_with("Option<") && field.unique {
                warnings.push(format!(
                    "entity '{}': field '{}' is Option<> and marked unique — nullable unique fields may not be intended",
                    entity.name, field.name
                ));
            }

            if is_value_object(field) {
                if let Err(e) =
                    crate::validation::validate_vo_name(field.value_object.as_deref().unwrap())
                {
                    errors.push(format!(
                        "entity '{}': field '{}': {}",
                        entity.name, field.name, e
                    ));
                }
                if is_enum_vo(field) {
                    if field.field_type != "String" {
                        errors.push(format!(
                            "entity '{}': field '{}' has enum value_object but type '{}' is not String",
                            entity.name, field.name, field.field_type
                        ));
                    }
                    if field.enum_variants.is_none() {
                        errors.push(format!(
                            "entity '{}': field '{}' has enum value_object_kind but no enum_variants",
                            entity.name, field.name
                        ));
                    }
                    if let Some(variants) = &field.enum_variants {
                        if let Err(e) = crate::validation::validate_enum_variants(variants) {
                            errors.push(format!(
                                "entity '{}': field '{}': {}",
                                entity.name, field.name, e
                            ));
                        }
                    }
                } else {
                    if !VO_ALLOWED_INNER_TYPES.contains(&field.field_type.as_str()) {
                        errors.push(format!(
                            "entity '{}': field '{}' has value_object but inner type '{}' is not a allowed (allowed: {})",
                            entity.name, field.name, field.field_type, VO_ALLOWED_INNER_TYPES.join(", ")
                        ));
                    }
                    if !vo_inner_type_is_valid(&field.field_type) {
                        errors.push(format!(
                            "entity '{}': field '{}' has value_object with invalid composed inner type '{}'",
                            entity.name, field.name, field.field_type
                        ));
                    }
                    if field.enum_variants.is_some() {
                        errors.push(format!(
                            "entity '{}': field '{}' has enum_variants but value_object_kind is not 'enum'",
                            entity.name, field.name
                        ));
                    }
                }
                if field.unique && (is_option_vo(field) || is_vec_vo(field)) {
                    errors.push(format!(
                        "entity '{}': field '{}' is an Option/Vec value object and cannot be marked unique",
                        entity.name, field.name
                    ));
                }
            }
        }

        // One `value_objects.rs` is written per entity — two fields cannot define the same VO
        // with different inner types.
        for e in crate::validation::validate_vo_coherence(&entity.fields, &config.value_object) {
            errors.push(format!("entity '{}': {}", entity.name, e));
        }
    }

    let mut seen_shared_vo_names: Vec<String> = vec![];
    for vo_def in &config.value_object {
        if let Err(e) = crate::validation::validate_vo_name(&vo_def.name) {
            errors.push(format!("shared value_object '{}': {}", vo_def.name, e));
        }
        if seen_shared_vo_names.contains(&vo_def.name) {
            errors.push(format!(
                "duplicate shared value_object name '{}'",
                vo_def.name
            ));
        }
        seen_shared_vo_names.push(vo_def.name.clone());
        if !SHARED_VO_ALLOWED_INNER_TYPES.contains(&vo_def.inner_type.as_str()) {
            errors.push(format!(
                "shared value_object '{}': inner type '{}' is not allowed (allowed: {})",
                vo_def.name,
                vo_def.inner_type,
                SHARED_VO_ALLOWED_INNER_TYPES.join(", ")
            ));
        }
    }

    // The manifest can be internally consistent and still describe code that is not there —
    // `generate bootstrap` would then emit imports for modules that do not exist.
    for line in crate::generators::consistency::drift_lines(
        &crate::generators::consistency::missing_entity_files(cwd, &config),
    ) {
        errors.push(line);
    }
    for orphan in crate::generators::consistency::orphan_entity_dirs(cwd, &config) {
        warnings.push(format!(
            "'business/src/domain/{orphan}' exists but has no [[entity]] block — Puerto commands \
             cannot see it"
        ));
    }

    if !errors.is_empty() {
        eprintln!("Validation failed:");
        for e in &errors {
            eprintln!("  ✗ {e}");
        }
        if !warnings.is_empty() {
            for w in &warnings {
                eprintln!("  ⚠ {w}");
            }
        }
        let msg = errors.join("\n  ");
        return Err(format!("{}\n  {} error(s) found", msg, errors.len()).into());
    }

    if !warnings.is_empty() {
        eprintln!("Warnings:");
        for w in &warnings {
            eprintln!("  ⚠ {w}");
        }
    }

    println!(
        "✓ puerto.toml is valid — {} entit{} validated",
        config.entity.len(),
        if config.entity.len() == 1 { "y" } else { "ies" }
    );
    Ok(())
}
