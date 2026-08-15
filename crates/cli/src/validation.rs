//! Name and shape validation shared by the CLI parser and `puerto validate`.
//!
//! These two used to diverge: `parse_field_arg` checked snake_case and nothing else, while
//! `run_validate` re-implemented its own predicates. Anything they both missed — a field called
//! `id`, a field called `type`, an entity called `Error` — reached the generators and produced
//! Rust that does not compile, with the error surfacing in the *generated* project rather than at
//! the command that caused it.
//!
//! Everything here is a pure name predicate. Type-registry checks live in `generators::types`.

use crate::puerto_toml::{Field, ValueObjectDefinition};

/// Fields every generated entity already has. A `[[entity.fields]]` entry with one of these names
/// emits the struct field twice (E0124).
pub const SYSTEM_FIELDS: &[&str] = &["id", "created_at", "updated_at", "deleted", "deleted_at"];

/// Rust keywords, strict and reserved. Field names become struct fields and entity names become
/// module names, so either can land on one.
pub const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
    "use", "where", "while", "async", "await", "abstract", "become", "box", "do", "final", "gen",
    "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
];

/// Type names in scope in generated code. An entity named after one of these produces a struct
/// that shadows it in the same file.
const RESERVED_TYPE_NAMES: &[&str] = &[
    "Self", "String", "Option", "Vec", "Box", "Result", "HashMap", "Uuid", "DateTime", "Utc",
    "Arc", "Error", "Logger",
];

/// Module names the generated project already occupies. Entity names become module names
/// (`business/src/domain/{snake}/`, `presentation/src/api/{snake}/`).
const RESERVED_MODULE_NAMES: &[&str] = &[
    "shared",    // business/src/domain/shared — shared value objects
    "logger",    // business/src/domain/logger.rs
    "tags",      // presentation/src/api/tags.rs
    "error",     // presentation/src/api/error.rs
    "generated", // presentation/src/generated
    "api",       // presentation/src/api.rs
    "mocks",     // repository.rs mock module
    "tests",     // business/src/tests
];

/// snake_case: lowercase letters, digits and underscores, starting with a letter.
///
/// A leading `_` is rejected: the docs and the error message both say "not starting with a digit,
/// snake_case", and a leading underscore means "deliberately unused" to every Rust reader.
pub fn is_valid_field_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// PascalCase: an uppercase letter followed by alphanumerics.
pub fn is_pascal_case(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric())
}

/// Full check for a `[[entity.fields]]` name.
pub fn validate_field_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("field name is empty".to_string());
    }
    if !is_valid_field_name(name) {
        return Err(format!(
            "invalid field name '{name}'. Must be snake_case (lowercase letters, digits and \
             underscores, starting with a lowercase letter)"
        ));
    }
    if SYSTEM_FIELDS.contains(&name) {
        return Err(format!(
            "field name '{name}' is reserved — every entity already has {}. Remove it from the \
             field list",
            SYSTEM_FIELDS.join(", ")
        ));
    }
    if RUST_KEYWORDS.contains(&name) {
        return Err(format!(
            "field name '{name}' is a Rust keyword and cannot be a struct field. Pick another name"
        ));
    }
    Ok(())
}

/// Full check for an entity name, in the PascalCase form the user typed.
pub fn validate_entity_name(pascal: &str) -> Result<(), String> {
    if pascal.is_empty() {
        return Err("entity name is empty".to_string());
    }
    if !is_pascal_case(pascal) {
        return Err(format!(
            "invalid entity name '{pascal}'. Must be PascalCase (an uppercase letter followed by \
             alphanumeric characters)"
        ));
    }
    if RESERVED_TYPE_NAMES.contains(&pascal) {
        return Err(format!(
            "entity name '{pascal}' collides with a type used in generated code. Pick another name"
        ));
    }
    let snake = crate::generators::naming::pascal_to_snake(pascal);
    if RUST_KEYWORDS.contains(&snake.as_str()) {
        return Err(format!(
            "entity name '{pascal}' becomes the module name '{snake}', which is a Rust keyword. \
             Pick another name"
        ));
    }
    if RESERVED_MODULE_NAMES.contains(&snake.as_str()) {
        return Err(format!(
            "entity name '{pascal}' becomes the module name '{snake}', which the generated project \
             already uses. Pick another name"
        ));
    }
    Ok(())
}

/// Full check for a value object name — local or shared.
pub fn validate_vo_name(vo_name: &str) -> Result<(), String> {
    if vo_name.is_empty() {
        return Err("value object name is empty".to_string());
    }
    if !is_pascal_case(vo_name) {
        return Err(format!(
            "invalid value object name '{vo_name}'. Must be PascalCase (an uppercase letter \
             followed by alphanumeric characters)"
        ));
    }
    if RESERVED_TYPE_NAMES.contains(&vo_name) {
        return Err(format!(
            "value object name '{vo_name}' collides with a type used in generated code. Pick \
             another name"
        ));
    }
    Ok(())
}

/// Enum VO variants: PascalCase and distinct. Two identical variants are `enum` arms defined
/// twice (E0428).
pub fn validate_enum_variants(variants: &[String]) -> Result<(), String> {
    if variants.is_empty() {
        return Err("enum value object has no variants".to_string());
    }
    let mut seen: Vec<&str> = vec![];
    for v in variants {
        if !is_pascal_case(v) {
            return Err(format!(
                "invalid enum variant '{v}'. Must be PascalCase (an uppercase letter followed by \
                 alphanumeric characters)"
            ));
        }
        if seen.contains(&v.as_str()) {
            return Err(format!(
                "duplicate enum variant '{v}' — every variant must be distinct"
            ));
        }
        seen.push(v);
    }
    Ok(())
}

/// Cross-field checks that need the whole list: one `value_objects.rs` is written per entity, so
/// two fields naming the same VO must agree on its inner type, and a local VO must not contradict
/// a shared one.
pub fn validate_vo_coherence(
    fields: &[Field],
    shared_vos: &[ValueObjectDefinition],
) -> Vec<String> {
    let mut errors = vec![];
    let mut declared: Vec<(&str, &str)> = vec![];

    for field in fields {
        let Some(vo_name) = field.value_object.as_deref() else {
            continue;
        };
        // Enum VOs carry their shape in `enum_variants`, not in `field_type` (always "String").
        let inner = if field.value_object_kind.as_deref() == Some("enum") {
            "enum"
        } else {
            field.field_type.as_str()
        };

        if let Some((_, previous)) = declared.iter().find(|(n, _)| *n == vo_name) {
            if *previous != inner {
                errors.push(format!(
                    "value object '{vo_name}' is declared twice with different inner types \
                     ('{previous}' and '{inner}') — one value_objects.rs cannot define both"
                ));
            }
        } else {
            declared.push((vo_name, inner));
        }

        if let Some(shared) = shared_vos.iter().find(|v| v.name == vo_name) {
            if inner != "enum" && shared.inner_type != inner {
                errors.push(format!(
                    "value object '{vo_name}' is declared as [[value_object]] with type \
                     '{}' but field '{}' uses it as '{inner}'",
                    shared.inner_type, field.name
                ));
            }
        }
    }

    errors
}

/// Every per-field check that does not need the type registry, for one parsed field.
pub fn validate_field(field: &Field) -> Result<(), String> {
    validate_field_name(&field.name)?;
    if let Some(vo_name) = field.value_object.as_deref() {
        validate_vo_name(vo_name)?;
    }
    if let Some(variants) = &field.enum_variants {
        validate_enum_variants(variants)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_system_field_names() {
        for name in SYSTEM_FIELDS {
            assert!(
                validate_field_name(name).is_err(),
                "'{name}' must be rejected"
            );
        }
    }

    #[test]
    fn rejects_rust_keyword_field_names() {
        assert!(validate_field_name("type").is_err());
        assert!(validate_field_name("match").is_err());
        assert!(validate_field_name("move").is_err());
    }

    #[test]
    fn rejects_leading_underscore_field_name() {
        assert!(validate_field_name("_hidden").is_err());
    }

    #[test]
    fn accepts_ordinary_field_names() {
        assert!(validate_field_name("title").is_ok());
        assert!(validate_field_name("unit_price_2").is_ok());
    }

    #[test]
    fn rejects_entity_names_that_collide_with_generated_code() {
        assert!(validate_entity_name("Error").is_err());
        assert!(validate_entity_name("String").is_err());
        assert!(validate_entity_name("Logger").is_err());
        assert!(validate_entity_name("Type").is_err()); // module would be `type`
        assert!(validate_entity_name("Product").is_ok());
    }

    #[test]
    fn rejects_duplicate_enum_variants() {
        let variants = vec!["Active".to_string(), "Active".to_string()];
        assert!(validate_enum_variants(&variants).is_err());
    }

    #[test]
    fn rejects_same_vo_name_with_two_inner_types() {
        let fields = vec![
            Field {
                name: "a".into(),
                field_type: "String".into(),
                value_object: Some("Code".into()),
                ..Default::default()
            },
            Field {
                name: "b".into(),
                field_type: "i64".into(),
                value_object: Some("Code".into()),
                ..Default::default()
            },
        ];
        assert_eq!(validate_vo_coherence(&fields, &[]).len(), 1);
    }

    #[test]
    fn accepts_same_vo_name_with_matching_inner_type() {
        let fields = vec![
            Field {
                name: "a".into(),
                field_type: "String".into(),
                value_object: Some("Code".into()),
                ..Default::default()
            },
            Field {
                name: "b".into(),
                field_type: "String".into(),
                value_object: Some("Code".into()),
                ..Default::default()
            },
        ];
        assert!(validate_vo_coherence(&fields, &[]).is_empty());
    }
}
