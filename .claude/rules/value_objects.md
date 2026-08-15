# Value Objects — Puerto Generation Rules

## Syntax

CLI field arguments use a colon-based syntax — no quoting required in any shell:

```bash
# Plain primitive (no VO)
puerto generate scaffold order -- amount:f64 active:bool

# Option / Vec primitives (shell-safe shorthand)
puerto generate scaffold order -- desc:opt:String tags:vec:String

# DateTime / HashMap shorthands
puerto generate scaffold event -- occurred_at:DateTime meta:map

# Regular VO (wraps a primitive)
puerto generate scaffold persona -- name:Name:String age:Age:i64

# Unique VO
puerto generate scaffold product -- sku:Sku:String!

# Option<VO> (nullable)
puerto generate scaffold persona -- middle_name:MiddleName:opt:String

# Vec<VO> (array)
puerto generate scaffold post -- tags:Tag:vec:String

# Enum VO (variants separated by /)
puerto generate scaffold order -- status:Status:enum:Pending/Confirmed/Cancelled

# Mixed
puerto generate scaffold persona -- name:Name:String age:Age:i64 active:bool
```

| Format | Result |
|--------|--------|
| `field:PrimitiveType` | Primitive field, no VO |
| `field:opt:Type` | `Option<Type>` primitive |
| `field:vec:Type` | `Vec<Type>` primitive |
| `field:DateTime` | `DateTime<Utc>` primitive (shorthand) |
| `field:map` | `HashMap<String, String>` primitive (shorthand) |
| `field:VoName:Type` | VO wrapping a primitive |
| `field:VoName:opt:Type` | Nullable VO (`Option<VoName>`) |
| `field:VoName:vec:Type` | Array VO (`Vec<VoName>`) |
| `field:VoName:enum:V1/V2/...` | Enum VO (String-backed) |
| Append `!` to whole arg | `unique = true` (not valid on Option/Vec VOs) |

### Shared VOs

Declare reusable VOs before scaffolding:

```bash
puerto generate value-object Email String
puerto generate value-object Money i64
```

This appends `[[value_object]]` entries to `puerto.toml`. When a field's VO name matches a declared shared VO, the generated import path switches from local to `crate::domain::shared::value_objects`.

## puerto.toml Schema

```toml
[project]
name = "my-app"

# Shared VOs — reusable across entities
[[value_object]]
name = "Email"
type = "String"

[[value_object]]
name = "Money"
type = "i64"

[[entity]]
name = "User"
use_cases = ["create_user"]

[[entity.fields]]
name = "email"
type = "String"
value_object = "Email"        # matches a [[value_object]] → shared VO

[[entity.fields]]
name = "name"
type = "String"
value_object = "Name"         # no [[value_object]] match → local VO

[[entity.fields]]
name = "middle_name"
type = "Option<String>"
value_object = "MiddleName"   # nullable VO

[[entity.fields]]
name = "tags"
type = "Vec<String>"
value_object = "Tag"          # Vec VO

[[entity.fields]]
name = "status"
type = "String"
value_object = "Status"
value_object_kind = "enum"
enum_variants = ["Active", "Inactive", "Suspended"]

[[entity.fields]]
name = "active"
type = "bool"
# no value_object → primitive field
```

## Field Struct (`puerto_toml.rs`)

```rust
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unique: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_object_kind: Option<String>,   // "enum" for enum VOs
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enum_variants: Option<Vec<String>>,  // ["Active", "Inactive"]
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ValueObjectDefinition {
    pub name: String,
    #[serde(rename = "type")]
    pub inner_type: String,
}
```

### Parsing examples

```
parse_field_arg("name:Name:String")
→ Field { name: "name", field_type: "String", value_object: Some("Name"), .. }

parse_field_arg("middle_name:MiddleName:opt:String")
→ Field { name: "middle_name", field_type: "Option<String>", value_object: Some("MiddleName"), .. }

parse_field_arg("tags:Tag:vec:String")
→ Field { name: "tags", field_type: "Vec<String>", value_object: Some("Tag"), .. }

parse_field_arg("status:Status:enum:Active/Inactive")
→ Field { name: "status", field_type: "String", value_object: Some("Status"),
          value_object_kind: Some("enum"), enum_variants: Some(["Active", "Inactive"]) }

parse_field_arg("sku:Sku:String!")
→ Field { name: "sku", field_type: "String", unique: true, value_object: Some("Sku"), .. }

parse_field_arg("price:i64")
→ Field { name: "price", field_type: "i64", value_object: None, .. }
```

Validation rules:
- VO name must be PascalCase
- For regular VOs: `field_type` must be a plain primitive, `Option<primitive>`, or `Vec<primitive>`
- Allowed VO inner primitives: `String`, `i64`, `bool`, `f64`, `Uuid`, `DateTime<Utc>`
- `Option<T>` and `Vec<T>` of the above are allowed as VO base types
- Enum VOs: `field_type` must be `"String"`, `enum_variants` must be non-empty, all variants PascalCase
- `unique = true` is not valid on `Option<T>` or `Vec<T>` VO fields

## Type Registry Extensions (`types.rs`)

```rust
is_value_object(field)                          // true if field.value_object is Some (any kind)
is_option_vo(field)                             // true if field_type starts with "Option<"
is_vec_vo(field)                                // true if field_type starts with "Vec<"
is_enum_vo(field)                               // true if value_object_kind == "enum"
is_shared_vo(field, shared_vos)                 // true if VO name matches a [[value_object]] entry
vo_name(field)                                  // Option<&str> of the VO name
vo_inner_type(field)                            // inner primitive: "String" from Option<String>, Vec<String>
field_rust_type(field)                          // Rust type to use in structs: VO name, Option<VO>, Vec<VO>, or primitive
field_value_accessor(field, prefix)             // e.g. "entity.name.value().to_string()" for String VO
field_vo_constructor(field, pascal, shared_vos) // VO construction expression for use case impls
field_needs_clone(field_type)                   // true for String/Option<String>/Vec/HashMap
vo_import_path(field, snake, shared_vos)        // import path: local or shared
```

## Architecture: Value Objects Across Layers

### Domain: `value_objects.rs`

One file per entity at `business/src/domain/{snake}/value_objects.rs`. Shared VOs live at `business/src/domain/shared/value_objects.rs`.

#### String VO (trim + empty validation)

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Name {
    value: String,
}

impl Name {
    pub fn new(value: String) -> Result<Self, PersonaError> {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            return Err(PersonaError::InvalidName);
        }
        Ok(Self { value: trimmed })
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}
```

#### Numeric VO (`i64`, `f64`) — pass-through

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Age(i64);

impl Age {
    pub fn new(value: i64) -> Result<Self, PersonaError> {
        Ok(Self(value))
    }

    pub fn value(&self) -> i64 {
        self.0
    }
}
```

#### Bool, Uuid, DateTime<Utc> VOs — same tuple-struct pass-through pattern

#### Option VO

```rust
// The VO struct itself is identical to the primitive variant.
// The Option wrapping happens at the model/props level — Option<MiddleName>.
// Construction: let middle_name = params.middle_name.map(MiddleName::new).transpose()?;
```

#### Vec VO

```rust
// The VO struct itself is identical to the primitive variant.
// Vec wrapping happens at the model/props level — Vec<Tag>.
// Construction: let tags = params.tags.into_iter().map(Tag::new).collect::<Result<Vec<_>, _>>()?;
```

#### Enum VO

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Pending,
    Confirmed,
    Cancelled,
}

impl Status {
    pub fn from_str(s: &str) -> Result<Self, OrderError> {
        match s {
            "Pending" => Ok(Self::Pending),
            "Confirmed" => Ok(Self::Confirmed),
            "Cancelled" => Ok(Self::Cancelled),
            _ => Err(OrderError::InvalidStatus),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Pending => "Pending",
            Self::Confirmed => "Confirmed",
            Self::Cancelled => "Cancelled",
        }
    }
}
```

**Rules:**
- Private inner value (tuple struct for numeric/bool/Uuid, named field for String, enum variants for enum)
- `new()` always returns `Result<Self, EntityError>` — even pass-through VOs
- `value()` returns `&str` (String), primitive by value (numeric/bool/Uuid), owned (DateTime)
- Enum VOs use `from_str()` / `as_str()` instead of `new()` / `value()`
- No `Serialize`/`Deserialize` on VOs — domain-only; DTOs use primitives
- Shared VOs have their own error type in `domain/shared/errors.rs`

### Domain: `errors.rs`

Dynamically generated when the entity has any VO fields. Adds one `Invalid{Name}` variant per VO:

```rust
#[derive(Debug, Error)]
pub enum PersonaError {
    #[error("persona.validation_error.{0}")]
    ValidationError(String),
    #[error("persona.not_found")]
    NotFound,
    #[error("persona.duplicate")]
    Duplicate,
    #[error("persona.repository_error")]
    RepositoryError,
    #[error("persona.unknown")]
    Unknown,
    // Value Object errors
    #[error("persona.invalid_name")]
    InvalidName,
    #[error("persona.invalid_status")]
    InvalidStatus,
}
```

Uses the static `ERRORS` template when no VOs exist.

### Domain: `model.rs`

VO fields use the VO type; primitive fields stay primitive:

```rust
pub struct PersonaProps {
    pub name: Name,                    // VO
    pub middle_name: Option<MiddleName>, // Option VO
    pub tags: Vec<Tag>,                // Vec VO
    pub status: Status,                // Enum VO
    pub active: bool,                  // primitive
}
```

VO fields are NOT validated in `model::new()` — already validated in `VO::new()`. Primitive `String` fields still get the empty-string check.

### Domain: `lib.rs` patch

`pub mod value_objects;` is added to the entity block when VOs are present. `pub mod shared;` is added to the domain block when shared VOs are present:

```rust
pub mod domain {
    pub mod shared;          // ← added when shared VOs exist
    pub mod persona {
        pub mod errors;
        pub mod model;
        pub mod repository;
        pub mod value_objects;  // ← added when local VOs exist
        pub mod use_cases { ... }
    }
}
```

### Application: Use Case Impls

Use case params always use **primitive types**. The impl constructs VOs before calling `Entity::new()`:

```rust
pub struct CreatePersonaParams {
    pub name: String,          // primitive
    pub middle_name: Option<String>,
    pub tags: Vec<String>,
    pub status: String,
    pub active: bool,
}

async fn execute(&self, params: CreatePersonaParams) -> Result<Persona, PersonaError> {
    let name = Name::new(params.name)?;
    let middle_name = params.middle_name.map(MiddleName::new).transpose()?;
    let tags = params.tags.into_iter().map(Tag::new).collect::<Result<Vec<_>, _>>()?;
    let status = Status::from_str(&params.status)?;

    let entity = Persona::new(PersonaProps { name, middle_name, tags, status, active: params.active })?;
    self.repository.save(&entity).await?;
    Ok(entity)
}
```

**Shared VOs** use `.map_err(|_| EntityError::InvalidVoName)?` because their `new()` returns a shared error type.

### Infrastructure: `entity.rs`

`EntityDb` uses **primitive types** — unchanged struct definition. Conversions handle VO reconstruction:

```rust
// TryFrom: DB → Domain (reconstruct VOs)
impl TryFrom<PersonaDb> for Persona {
    fn try_from(row: PersonaDb) -> Result<Self, PersonaError> {
        Ok(Self::from_repository(Persona {
            name: Name::new(row.name)?,
            middle_name: row.middle_name.map(MiddleName::new).transpose()?,
            tags: row.tags.into_iter().map(Tag::new).collect::<Result<Vec<_>, _>>()?,
            status: Status::from_str(&row.status)?,
            active: row.active,
            // ... audit fields
        }))
    }
}

// From: Domain → DB (extract primitives)
impl From<&Persona> for PersonaDb {
    fn from(entity: &Persona) -> Self {
        Self {
            name: entity.name.value().to_string(),
            middle_name: entity.middle_name.as_ref().map(|v| v.value().to_string()),
            tags: entity.tags.iter().map(|v| v.value().to_string()).collect(),
            status: entity.status.as_str().to_string(),
            active: entity.active,
            // ... audit fields
        }
    }
}
```

### Presentation: `dto.rs`

DTO fields stay primitive. `from_domain()` extracts `.value()` / `.as_str()` for VO fields:

```rust
impl PersonaDto {
    pub fn from_domain(entity: &Persona) -> Self {
        Self {
            name: entity.name.value().to_string(),
            middle_name: entity.middle_name.as_ref().map(|v| v.value().to_string()),
            tags: entity.tags.iter().map(|v| v.value().to_string()).collect(),
            status: entity.status.as_str().to_string(),
            active: entity.active,
        }
    }
}
```

### Domain: Object Mother

VO fields store the VO type (not primitives). Defaults wrap in `Vo::new(default).expect(...)`:

```rust
pub struct PersonaMother {
    name: Option<Name>,
    middle_name: Option<Option<MiddleName>>,
    tags: Option<Vec<Tag>>,
    status: Option<Status>,
    active: Option<bool>,
}

impl PersonaMother {
    pub fn with_name(mut self, name: Name) -> Self { self.name = Some(name); self }
    pub fn with_status(mut self, status: Status) -> Self { self.status = Some(status); self }

    pub fn build(self) -> Persona {
        Persona::new(PersonaProps {
            name: self.name.unwrap_or_else(|| Name::new("example".to_string()).expect("valid Name")),
            middle_name: self.middle_name.unwrap_or(None),
            tags: self.tags.unwrap_or_default(),
            status: self.status.unwrap_or(Status::Active),
            active: self.active.unwrap_or(true),
        }).expect("PersonaMother: failed to build valid Persona")
    }
}
```

### Shared VOs

Declared at the top level of `puerto.toml` as `[[value_object]]` sections:

```toml
[[value_object]]
name = "Email"
type = "String"
```

Generated at `business/src/domain/shared/value_objects.rs`. Each shared VO has its own error type in `business/src/domain/shared/errors.rs`. When a field's `value_object` name matches a `[[value_object]]` entry, the import path switches from local (`super::value_objects::Email`) to shared (`crate::domain::shared::value_objects::Email`).

## Validation (`validate.rs`)

- `value_object` must be PascalCase if present
- If `value_object` is set and `value_object_kind != "enum"`: `field_type` must be a plain primitive, `Option<primitive>`, or `Vec<primitive>`
- Allowed VO inner types: `String`, `i64`, `bool`, `f64`, `Uuid`, `DateTime<Utc>`, `Option<*>` of those, `Vec<*>` of those
- `unique = true` cannot be combined with `Option<T>` or `Vec<T>` VO fields
- Enum VOs: `field_type` must be `"String"`, `enum_variants` non-empty, all variants PascalCase
- `value_object_kind` without `value_object` → error
- Shared VO: `[[value_object]]` names must be PascalCase; `type` must be a plain primitive

## Generated File Checklist

### Entity with local VO fields

| File | Action |
|------|--------|
| `business/src/domain/{snake}/value_objects.rs` | **NEW** — VO structs |
| `business/src/domain/{snake}/model.rs` | **MODIFY** — VO types in Props/Entity |
| `business/src/domain/{snake}/errors.rs` | **MODIFY** — `Invalid{Vo}` variants |
| `business/src/lib.rs` | **PATCH** — `pub mod value_objects;` in entity block |
| `business/src/application/{snake}/create_{snake}.rs` | **MODIFY** — VO construction |
| `business/src/application/{snake}/update_{snake}.rs` | **MODIFY** — VO construction |
| `business/src/tests/mothers/{snake}_mother.rs` | **MODIFY** — VO types in builder |
| `infrastructure/src/{snake}/entity.rs` | **MODIFY** — TryFrom/From with VOs |
| `presentation/src/api/{snake}/dto.rs` | **MODIFY** — `.value()` / `.as_str()` in `from_domain()` |

### With shared VOs

| File | Action |
|------|--------|
| `business/src/domain/shared/value_objects.rs` | **NEW** — shared VO structs |
| `business/src/domain/shared/errors.rs` | **NEW** — shared VO error types |
| `business/src/domain/shared/mod.rs` | **NEW** — `pub mod value_objects; pub mod errors;` |
| `business/src/lib.rs` | **PATCH** — `pub mod shared;` in domain block |

## V1 Scope — COMPLETE

- ✅ String VOs with trim + empty validation, `Invalid{Name}` error variant
- ✅ Numeric/Bool/Uuid/DateTime VOs with pass-through
- ✅ Full layer penetration (domain → application → infrastructure → presentation)
- ✅ Object Mother integration
- ✅ `puerto validate` checks VO field constraints
- ✅ CLI parsing: `name:Name:String`, `sku:Sku:String!`
- ✅ 40+ VO-specific tests

## V2 Scope — COMPLETE

- ✅ `Option<VO>` support (nullable VO fields): `middle_name:MiddleName:opt:String`
- ✅ `Vec<VO>` support (array VO fields): `tags:Tag:vec:String`
- ✅ Enum VOs: `status:Status:enum:Active/Inactive/Suspended`
- ✅ Shared/common VOs (`[[value_object]]` in `puerto.toml`, reusable across entities)
- ✅ 60+ additional tests across all new VO kinds

## V3 Scope — IN PROGRESS

- ✅ **Shared VO type inference**: if a field's VO name matches a declared `[[value_object]]`, infer the inner type automatically — `email:Email` instead of `email:Email:String`

## Future Scope (V3+)

- ✅ Snippet generation for VO patterns (`puerto generate snippets` extended for VOs) — `vo-string`, `vo-numeric`, `vo-enum`, `vo-option-construct`, `vo-vec-construct`
- Custom validation rules in CLI (e.g., a `min`/`max` modifier on `name:Name:String`)
- Domain events
- Unit of Work pattern
