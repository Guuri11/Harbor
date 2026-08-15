use std::path::Path;

use serde::{Deserialize, Serialize};

// ── Config types ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct PuertoConfig {
    pub project: Project,
    #[serde(default)]
    pub entity: Vec<Entity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub value_object: Vec<ValueObjectDefinition>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ValueObjectDefinition {
    pub name: String,
    #[serde(rename = "type")]
    pub inner_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Project {
    pub name: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub db: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Entity {
    /// PascalCase entity name, e.g. "Product"
    pub name: String,
    /// snake_case use case action names, e.g. ["create_product"]
    pub use_cases: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub db: bool,
    #[serde(default)]
    pub fields: Vec<Field>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Field {
    /// Snake_case field name, e.g. "name"
    pub name: String,
    /// Rust type string, e.g. "String", "i64", "Option<String>"
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unique: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_object_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enum_variants: Option<Vec<String>>,
}

/// Parse a CLI field argument into a `Field`.
///
/// Shell-safe syntax (no quoting required):
///
/// ```text
/// name:Type                          primitive (String, i64, bool, f64, Uuid, DateTime, map)
/// name:opt:Type                      Option<Type> primitive
/// name:vec:Type                      Vec<Type> primitive
/// name:VoName:Type                   VO wrapping a primitive
/// name:VoName:opt:Type               Option<VO> field
/// name:VoName:vec:Type               Vec<VO> field
/// name:VoName:enum:V1/V2/V3          Enum VO (variants separated by /)
/// ```
///
/// Append `!` to the whole argument to mark the field as `unique = true`.
/// Type shorthands: `DateTime` → `DateTime<Utc>`, `map` → `HashMap<String, String>`.
pub fn parse_field_arg(s: &str) -> Result<Field, String> {
    let (raw, unique) = if let Some(stripped) = s.strip_suffix('!') {
        (stripped, true)
    } else {
        (s, false)
    };

    let parts: Vec<&str> = raw.split(':').collect();

    if parts.len() == 1 || parts[0].is_empty() {
        return Err(format!(
            "invalid field argument '{}'. Expected format: name:Type (e.g. title:String)",
            s
        ));
    }

    let name = parts[0].to_string();
    crate::validation::validate_field_name(&name)?;

    let field = parse_field_shape(s, name, &parts, unique)?;
    // Fail at the command that caused it, not at `cargo build` inside the generated project.
    crate::validation::validate_field(&field)?;
    Ok(field)
}

fn parse_field_shape(s: &str, name: String, parts: &[&str], unique: bool) -> Result<Field, String> {
    match parts.len() {
        1 => unreachable!("handled above"),
        2 => {
            // name:PrimitiveType
            let ft = expand_type(parts[1]);
            if ft.is_empty() {
                return Err(format!(
                    "invalid field argument '{}'. Type cannot be empty",
                    s
                ));
            }
            Ok(Field {
                name,
                field_type: ft,
                unique,
                ..Default::default()
            })
        }
        3 => {
            let second = parts[1];
            let third = parts[2];
            match second {
                "opt" => Ok(Field {
                    name,
                    field_type: format!("Option<{}>", expand_type(third)),
                    unique,
                    ..Default::default()
                }),
                "vec" => Ok(Field {
                    name,
                    field_type: format!("Vec<{}>", expand_type(third)),
                    unique,
                    ..Default::default()
                }),
                vo_name if is_valid_vo_name(vo_name) => Ok(Field {
                    name,
                    field_type: expand_type(third),
                    unique,
                    value_object: Some(vo_name.to_string()),
                    ..Default::default()
                }),
                _ => Err(format!(
                    "invalid field argument '{}'. '{}' is not a known keyword (opt, vec) or a PascalCase VO name",
                    s, second
                )),
            }
        }
        4 => {
            let vo_name = parts[1];
            let modifier = parts[2];
            let rest = parts[3];

            if !is_valid_vo_name(vo_name) {
                return Err(format!(
                    "invalid value object name '{}'. Must be PascalCase (starts with uppercase, alphanumeric)",
                    vo_name
                ));
            }

            match modifier {
                "opt" => Ok(Field {
                    name,
                    field_type: format!("Option<{}>", expand_type(rest)),
                    unique,
                    value_object: Some(vo_name.to_string()),
                    ..Default::default()
                }),
                "vec" => Ok(Field {
                    name,
                    field_type: format!("Vec<{}>", expand_type(rest)),
                    unique,
                    value_object: Some(vo_name.to_string()),
                    ..Default::default()
                }),
                "enum" => {
                    if rest.is_empty() {
                        return Err(format!(
                            "invalid field argument '{}'. Enum variants cannot be empty",
                            s
                        ));
                    }
                    let variants: Vec<String> =
                        rest.split('/').map(|v| v.trim().to_string()).collect();
                    for variant in &variants {
                        if !is_valid_vo_name(variant) {
                            return Err(format!(
                                "invalid enum variant '{}'. Must be PascalCase (starts with uppercase, alphanumeric)",
                                variant
                            ));
                        }
                    }
                    Ok(Field {
                        name,
                        field_type: "String".to_string(),
                        unique,
                        value_object: Some(vo_name.to_string()),
                        value_object_kind: Some("enum".to_string()),
                        enum_variants: Some(variants),
                    })
                }
                _ => Err(format!(
                    "invalid field argument '{}'. Modifier '{}' is not recognized (use: opt, vec, enum)",
                    s, modifier
                )),
            }
        }
        _ => Err(format!(
            "invalid field argument '{}'. Too many colon-separated parts (expected 2–4)",
            s
        )),
    }
}

/// Expands CLI type shorthands to full Rust type strings.
/// `DateTime` → `DateTime<Utc>`, `map` → `HashMap<String, String>`.
fn expand_type(t: &str) -> String {
    match t {
        "DateTime" => "DateTime<Utc>".to_string(),
        "map" => "HashMap<String, String>".to_string(),
        other => other.to_string(),
    }
}

fn is_valid_vo_name(name: &str) -> bool {
    crate::validation::is_pascal_case(name)
}

// ── I/O helpers ───────────────────────────────────────────────────────────────

pub fn read(base: &Path) -> Result<PuertoConfig, Box<dyn std::error::Error>> {
    let path = base.join("puerto.toml");
    let src = std::fs::read_to_string(&path)?;
    Ok(toml::from_str(&src)?)
}

pub fn write(base: &Path, config: &PuertoConfig) -> Result<(), Box<dyn std::error::Error>> {
    let path = base.join("puerto.toml");
    std::fs::write(path, toml::to_string_pretty(config)?)?;
    Ok(())
}

/// Insert the entity, or update it in place if already present.
///
/// Upsert rather than no-op: re-scaffolding an entity with a different field list used to leave
/// puerto.toml describing the *old* fields while the code on disk had the new ones — a divergence
/// neither `puerto validate` nor `puerto generate bootstrap` could see, in the one file documented
/// as the source of truth.
///
/// Use cases are **merged**, not replaced: actions added by `puerto generate use-case` survive a
/// re-scaffold. Fields are replaced — they are what the caller just regenerated the code from.
pub fn add_entity(
    base: &Path,
    name: &str,
    use_cases: Vec<String>,
    db: bool,
    fields: Vec<Field>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = read(base)?;
    match config.entity.iter_mut().find(|e| e.name == name) {
        Some(existing) => {
            for uc in use_cases {
                if !existing.use_cases.contains(&uc) {
                    existing.use_cases.push(uc);
                }
            }
            existing.db = db;
            existing.fields = fields;
        }
        None => config.entity.push(Entity {
            name: name.to_string(),
            use_cases,
            db,
            fields,
        }),
    }
    write(base, &config)
}

/// For fields written as `name:VoName` (no explicit inner type), infer the inner type
/// and VO name from the shared VO registry. Only applies when `value_object` is unset
/// and `field_type` matches a declared shared VO name.
pub fn apply_shared_vo_inference(
    fields: Vec<Field>,
    shared_vos: &[ValueObjectDefinition],
) -> Vec<Field> {
    if shared_vos.is_empty() {
        return fields;
    }
    fields
        .into_iter()
        .map(|f| {
            if f.value_object.is_none() {
                if let Some(svo) = shared_vos.iter().find(|v| v.name == f.field_type) {
                    return Field {
                        field_type: svo.inner_type.clone(),
                        value_object: Some(svo.name.clone()),
                        ..f
                    };
                }
            }
            f
        })
        .collect()
}

/// Append a shared VO declaration to puerto.toml. No-op if already present.
pub fn add_value_object(
    base: &Path,
    name: &str,
    inner_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = read(base)?;
    if config.value_object.iter().any(|vo| vo.name == name) {
        return Ok(());
    }
    config.value_object.push(ValueObjectDefinition {
        name: name.to_string(),
        inner_type: inner_type.to_string(),
    });
    write(base, &config)
}

/// Append a use case action to an existing entity. Errors if entity not found. No-op if action already present.
pub fn add_use_case(
    base: &Path,
    entity_name: &str,
    action: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = read(base)?;
    let entity = config
        .entity
        .iter_mut()
        .find(|e| e.name == entity_name)
        .ok_or_else(|| format!("entity '{entity_name}' not found in puerto.toml"))?;
    if !entity.use_cases.contains(&action.to_string()) {
        entity.use_cases.push(action.to_string());
        write(base, &config)?;
    }
    Ok(())
}
