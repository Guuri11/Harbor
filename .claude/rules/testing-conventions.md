# Testing Conventions & Patterns

> Complements `testing.md` with concrete patterns for test data, helpers, and mocks.

---

## Object Mother Pattern

Object Mothers are factory structs that create domain test objects with sensible random defaults. They eliminate boilerplate and keep tests readable and focused on the scenario, not the data setup.

### Location (in generated projects)

```
business/src/tests/
├── mod.rs
└── mothers/
    ├── mod.rs
    └── greeting_mother.rs
```

### API conventions

```rust
// Most common — random valid entity
let greeting = GreetingMother::random();

// Specific field — builder chain
let greeting = GreetingMother::new()
    .with_name("Alice")
    .build();

// Props only — for testing constructors
let props = GreetingMother::new()
    .with_empty_name()
    .build_props();

// Collection
let greetings = GreetingMother::random_vec(5);
```

### Implementation template

```rust
pub struct GreetingMother {
    name: Option<String>,
}

impl GreetingMother {
    pub fn new() -> Self {
        Self { name: None }
    }

    // Builder methods
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn with_empty_name(mut self) -> Self {
        self.name = Some("".to_string());
        self
    }

    // Build methods
    pub fn build(self) -> Greeting {
        Greeting::new(&self.name.unwrap_or_else(|| "World".to_string()))
            .expect("GreetingMother: failed to create valid Greeting")
    }

    pub fn build_props(self) -> GreetingProps {
        // Return props without constructing the entity — useful for testing new()
        ...
    }

    // Convenience static methods
    pub fn random() -> Greeting {
        Self::new().build()
    }

    pub fn random_vec(n:usize) -> Vec<Greeting> {
        (0..n).map(|_| Self::random()).collect()
    }
}
```

### Typed Fields (puerto.toml entity.fields)

When `entity.fields` is defined in `puerto.toml`, the Object Mother generates builder methods for **each custom field** automatically:

```rust
pub struct ProductMother {
    name: Option<String>,
    price: Option<i64>,
    sku: Option<String>,
    description: Option<Option<String>>,
    tags: Option<Vec<String>>,
}

impl ProductMother {
    pub fn new() -> Self {
        Self { name: None, price: None, sku: None, description: None, tags: None }
    }

    pub fn with_name(mut self, name: &str) -> Self { self.name = Some(name.to_string()); self }
    pub fn with_price(mut self, price: i64) -> Self { self.price = Some(price); self }
    pub fn with_sku(mut self, sku: &str) -> Self { self.sku = Some(sku.to_string()); self }
    pub fn with_empty_name(mut self) -> Self { self.name = Some("".to_string()); self }
    // ...
}
```

- `String` fields get `.with_<field>(value)` and `.with_empty_<field>()`
- `Option<T>` fields get `.with_<field>(value)` only (no empty variant)
- Numeric/bool/Uuid/DateTime fields get `.with_<field>(value)`
- `Vec<T>` fields get `.with_<field>(values)`
- Default values come from the type registry (e.g., `i64` defaults to `42`, `String` to `"example"`)

---

## Naming Conventions

| Prefix/Method | Returns | Purpose |
|---------------|---------|---------|
| `random()` | `Entity` | Valid entity with random data |
| `random_props()` | `EntityProps` | Valid props with random data |
| `random_vec(n)` | `Vec<Entity>` | Multiple valid entities |
| `.with_<field>(value)` | `Self` | Builder: set specific field |
| `.with_<field>_str(s)` | `Self` | Builder: set field from string |
| `.with_empty_<field>()` | `Self` | Builder: set field to empty (error scenarios) |
| `.with_invalid_<field>()` | `Self` | Builder: set field to invalid value |
| `.build()` | `Entity` | Create entity (panics on invalid — intentional in tests) |
| `.build_props()` | `EntityProps` | Create props without entity construction |

---

## Mock Setup

### `business/Cargo.toml` pattern

`mockall` must appear in **both** `[dependencies]` (optional) and `[dev-dependencies]`:

```toml
[dependencies]
mockall = { version = "0.13", optional = true }

[features]
test-utils = ["mockall"]  # for external crates (e.g. infrastructure tests)

[dev-dependencies]
mockall = "0.13"  # for tests inside business itself
```

### Mock gate in `repository.rs`

```rust
#[cfg(any(test, feature = "test-utils"))]
pub mod mocks {
    use mockall::mock;
    use async_trait::async_trait;
    use super::*;

    mock! {
        pub GreetingRepository {}

        #[async_trait]
        impl GreetingRepositoryTrait for GreetingRepository {
            async fn find_by_name(&self, name: &str) -> Result<Option<Greeting>, GreetingError>;
        }
    }
}
```

`#[cfg(any(test, feature = "test-utils"))]` means:
- Available for all tests inside `business` (via `#[cfg(test)]`)
- Available for external crates (infrastructure, presentation) via `features = ["test-utils"]`

---

## Full Test Example

```rust
// business/src/application/greeting/get_greeting.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::greeting::repository::mocks::MockGreetingRepository;
    use std::sync::Arc;

    #[tokio::test]
    async fn should_return_greeting_when_name_is_valid() {
        // Arrange
        let mut mock_repo = MockGreetingRepository::new();
        mock_repo
            .expect_find_by_name()
            .returning(|_| Ok(None));

        let use_case = GetGreetingUseCaseImpl {
            repository: Arc::new(mock_repo),
        };

        // Act
        let result = use_case
            .execute(GetGreetingParams { name: "Alice".into() })
            .await;

        // Assert
        assert!(result.is_ok());
        let greeting = result.unwrap();
        assert_eq!(greeting.name, "Alice");
        assert!(greeting.message.contains("Alice"));
    }

    #[tokio::test]
    async fn should_return_error_when_name_is_empty() {
        // Arrange
        let mock_repo = MockGreetingRepository::new(); // no expectations set
        let use_case = GetGreetingUseCaseImpl {
            repository: Arc::new(mock_repo),
        };

        // Act
        let result = use_case
            .execute(GetGreetingParams { name: "".into() })
            .await;

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "greeting.validation_error.name_empty"
        );
    }
}
```

---

## Test File Structure

```
business/src/
├── domain/
│   └── greeting/
│       ├── model.rs              # Unit tests for model constructors + business rules
│       └── use_cases/
│           └── get_greeting.rs   # (trait only — tests live in application layer)
└── application/
    └── greeting/
        └── get_greeting.rs       # Use case tests (mocks + AAA)
```

Integration tests (full project compile + test) live in `crates/cli/src/tests.rs` as `#[ignore]` tests run via `make test/full`.

---

## Mocks vs Fakes vs Stubs

| Concept | Definition | When to use |
|---------|------------|-------------|
| **Stub** | Returns a hardcoded response; no assertions on calls | Isolating a happy-path test where the collaborator's response is irrelevant |
| **Mock** | Records calls; has programmable expectations (`mockall`) | Verifying a collaborator was called with specific args, or testing error paths returned by a dependency |
| **Fake** | Lightweight working implementation (e.g. `InMemoryRepository`) | Integration tests across layers where you want real behaviour without a DB |

**Rule of thumb:** stubs for happy paths, mocks for error paths and interaction assertions, fakes for integration tests. Never mock what you own (domain models).

---

## `fake` Crate for Random Test Data

Add to `business/Cargo.toml`:

```toml
[dev-dependencies]
fake = "2"
```

Use in Object Mothers to generate realistic random data instead of hardcoded strings:

```rust
use fake::{Fake, faker::name::en::Name as FakeName, faker::lorem::en::Word};

impl ProductMother {
    pub fn random() -> Product {
        Self::new()
            .with_name(&FakeName().fake::<String>())
            .with_sku(&Word().fake::<String>())
            .build()
    }
}
```

Random data catches bugs that hardcoded `"test"` strings hide — trimming edge cases, length constraints, encoding issues.

---

## Custom Fakers for Value Objects

Implement `fake::Dummy` on value objects to integrate them into the fake ecosystem:

```rust
use fake::{Dummy, Faker, faker::name::en::Name as FakeName};
use rand::Rng;

impl Dummy<Faker> for Name {
    fn dummy_with_rng<R: Rng + ?Sized>(_: &Faker, rng: &mut R) -> Self {
        let value: String = FakeName().fake_with_rng(rng);
        Name::new(value).expect("fake Name must be valid")
    }
}

// Usage in Object Mother
let name: Name = Faker.fake();
```

---

## Test Constants

Name important boundary values as constants rather than scattering magic literals:

```rust
#[cfg(test)]
pub mod constants {
    pub const VALID_NAME: &str = "Alice";
    pub const EMPTY: &str = "";
    pub const VALID_UUID_STR: &str = "550e8400-e29b-41d4-a716-446655440000";
}
```

```rust
use crate::tests::constants::{VALID_NAME, EMPTY};

let result = use_case.execute(Params { name: EMPTY.into() }).await;
assert!(result.is_err());
```
