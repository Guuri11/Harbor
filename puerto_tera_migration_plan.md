# Plan: Migrate Puerto Generators to Tera Templates

## Context

Puerto's code generators (`crates/cli/src/generators/`) currently use two patterns:

1. **~26 `const &str` raw string templates** with `{Pascal}` / `{snake}` / `{uc_pascal}` / `{uc}` tokens replaced via `.replace()` calls in `naming.rs::apply()` and `apply_uc()`.
2. **~15 dynamic programmatic generators** that build Rust code strings via `String::new()` + `push_str()` + `format!()` loops, with all field-iteration and VO-branching logic embedded in the Rust generator functions.

This means template *content* and template *logic* live in the same Rust files. `domain.rs` is 1,476 lines, `application.rs` is 1,432 lines — the content strings are inseparable from the generation logic. The core pain points:

- Adding a new field type requires touching `types.rs` + every generator that branches on types
- New VO patterns (like the existing Option<VO>, Vec<VO>, enum VO additions) cascade `if/else` chains across multiple generator files
- The 5 Create/Get/List/Update/Delete use-case variants in both domain and application are near-duplicates in code and templates but maintained separately
- `scaffold.rs` imports consts from all four layer generators, coupling it to all of them

**Goal:** Separate template content from generation logic by adopting Tera as a proper template engine, with external `.tera` files embedded at compile time via `include_dir`. Generator `.rs` files become thin context-builders that call `render()`.

**Not in scope:**
- `crates/cli/template/` (the cargo-generate project template used by `puerto new`) — that's a different system
- The `types.rs` type registry and VO helpers — already well-structured, they feed context builders
- The `naming.rs` naming helpers (`to_pascal_case`, `pascal_to_snake`, `write_file`) — these stay

---

## Reference: Loco's Approach

`/home/guuri11/dev/loco/loco-gen/` does this exact migration target:
- `tera = "1.19.1"` + `include_dir = "0.7.4"` for embedded `.t` template files
- `serde_json::Value` passed as Tera context
- Field types in `mappings.json` (data-driven), not branching Rust code
- Custom Tera functions for complex rendering logic

---

## Dependencies ✅ DONE

**`crates/cli/Cargo.toml`** (already added):
```toml
[dependencies]
tera = "1"
serde_json = "1"   # promoted from [dev-dependencies]
# include_dir = "0.7" already present
```

---

## New Architecture

### New module: `crates/cli/src/generators/render.rs`

Owns the embedded templates and the single `render()` entry point:

```rust
use include_dir::{Dir, include_dir};
static TEMPLATES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/templates");

pub fn render(template_name: &str, ctx: &tera::Context) -> Result<String, tera::Error> {
    let content = TEMPLATES.get_file(template_name)
        .ok_or_else(|| tera::Error::msg(format!("template not found: {template_name}")))?
        .contents_utf8()
        .ok_or_else(|| tera::Error::msg("template not UTF-8"))?;
    let mut tera = Tera::default();
    tera.add_raw_template(template_name, content)?;
    tera.render(template_name, ctx)
}
```

Note: `Tera::default()` + `add_raw_template()` per call is acceptable for a CLI tool. Not a hot path.

### New directory: `crates/cli/src/templates/`

```
src/templates/
  domain/
    errors.tera
    repository.tera
    use_case_create.tera
    use_case_get.tera
    use_case_list.tera
    use_case_update.tera
    use_case_delete.tera
    model.tera
    mother.tera
    value_objects.tera
    shared_value_objects.tera
    shared_errors.tera
  application/
    create.tera
    get.tera
    list.tera
    update.tera
    delete.tera
  infrastructure/
    repository_inmemory_simple.tera
    repository_inmemory_crud.tera
    repository_pg_simple.tera
    repository_pg_crud.tera
    entity.tera
    db.tera
  presentation/
    dto.tera
    responses_simple.tera
    responses_crud.tera
    error_mapper.tera
    routes_simple.tera
    routes_crud.tera
  use_case/
    trait.tera
    impl.tera
  bootstrap/
    bootstrap.tera
    tags.tera
```

### Tera context shape (field-aware templates)

The context builder (`render.rs` or a new `context.rs`) pre-computes **all** derived values in Rust so templates remain clean iteration:

```json
{
  "pascal": "Product",
  "snake": "product",
  "uc_pascal": "CreateProduct",
  "uc": "create_product",
  "db": false,
  "has_vo": true,
  "has_enum_vo": false,
  "has_local_vo": true,
  "extra_imports": ["use uuid::Uuid;", "use chrono::{DateTime, Utc};"],
  "vo_imports": ["use super::value_objects::ProductTitle;"],
  "effective_fields": [
    {
      "name": "title",
      "field_type": "String",
      "rust_type": "ProductTitle",
      "is_vo": true,
      "is_enum_vo": false,
      "is_option_vo": false,
      "is_vec_vo": false,
      "is_shared_vo": false,
      "vo_name": "ProductTitle",
      "vo_snake": "product_title",
      "default_expr": "\"example\".to_string()",
      "needs_clone": false,
      "sql_type": "TEXT",
      "sql_nullable": false
    }
  ],
  "validation_test_cases": [
    {
      "test_name": "should_return_error_when_title_is_empty",
      "empty_params": "            title: \"\".to_string(),",
      "error_str": "product.invalid_product_title"
    }
  ]
}
```

**Key rule:** Complex per-field strings (VO constructors, accessor expressions, validation test params) are pre-computed in Rust and stored as strings in the context. Templates iterate over pre-baked values — no complex Jinja logic.

### Brace escaping in Tera

Tera only acts on `{{ }}` (double braces) and `{% %}` tags. Single `{` and `}` in template content pass through literally. This means:
- Rust struct literals `Foo { field: val }` — no escaping needed
- Rust generics `Vec<Product>` — no escaping needed  
- Format strings `format!("{}", x)` — no escaping needed (`{}` is single-brace pair)
- The old `{Pascal}` token becomes `{{ pascal }}` in Tera — straightforward

The only case to watch: if generated Rust code contains `{{` or `}}` (double-brace in format macros like `format!("{{}}")`). These must be written as `{{"{{"}}` in Tera. Scan existing consts before migrating each template.

### Shared `effective_fields()` helper ✅ DONE

Moved to `crates/cli/src/generators/mod.rs` as a public function. The 4 local copies in `domain.rs`, `application.rs`, `infrastructure.rs`, and `presentation.rs` have been removed and replaced with `use crate::generators::effective_fields;`.

---

## Phases

### Phase 0 — Foundation (no behavior change) ✅ DONE
**Goal:** Wire up the infrastructure. Zero generator behavior changes.

Files modified:
- `crates/cli/Cargo.toml` — added `tera = "1"`, promoted `serde_json = "1"` from dev-dependencies
- `crates/cli/src/generators/mod.rs` — added `pub mod render;`, added shared `effective_fields()`
- `crates/cli/src/generators/render.rs` — **new file**: `TEMPLATES` embed + `render()` function
- `crates/cli/src/generators/domain.rs` — removed local `effective_fields()`, imports shared one
- `crates/cli/src/generators/application.rs` — removed local `effective_fields()`, imports shared one
- `crates/cli/src/generators/infrastructure.rs` — removed local `effective_fields()`, imports shared one
- `crates/cli/src/generators/presentation.rs` — removed local `effective_fields()`, imports shared one
- `crates/cli/src/templates/` — **new empty dir** (populated in later phases)

Verification: 200 tests pass (`cargo test -p puerto -- --test-threads=1`).

---

### Phase 1 — Domain Layer: Const Templates ✅ DONE

Replace the 5 simple const templates that use only `{Pascal}` / `{snake}`:
- `ERRORS` → `domain/errors.tera` (unified: handles both no-VO and VO-with-errors cases via `{% for f in vo_fields %}`)
- `CRUD_REPOSITORY` → `domain/repository.tera`
- `GET_USE_CASE_TRAIT` → `domain/use_case_get.tera`
- `LIST_USE_CASE_TRAIT` → `domain/use_case_list.tera`
- `DELETE_USE_CASE_TRAIT` → `domain/use_case_delete.tera`

Files modified:
- `crates/cli/src/generators/domain.rs` — removed `generate_errors()` + 5 consts; `write_domain_files()` builds `tera::Context` with `pascal`, `snake`, `vo_fields` and calls `render()`
- `crates/cli/src/generators/scaffold.rs` — removed `ERRORS` import; `write_files()` uses `render("domain/errors.tera", ...)` with empty `vo_fields`
- `crates/cli/src/templates/domain/` — **new directory** with 5 `.tera` files

Verification: `scaffold_substitutes_pascal_name_in_errors`, `scaffold_crud_repository_has_find_all` pass. All 200 tests pass.

---

### Phase 2 — Domain Layer: Use Case Traits + Dynamic Generators ✅ DONE

**Step 2a:** Replace `USE_CASE_TRAIT` const + `generate_create_use_case_trait()` + `generate_update_use_case_trait()`:
- `domain/use_case_create.tera` and `domain/use_case_update.tera` — iterate `{% for f in params_fields %}`

**Step 2b:** Replace 5 dynamic generators (`generate_model`, `generate_mother`, `generate_value_objects`, `generate_shared_value_objects`, `generate_shared_errors_combined`):
- `domain/model.tera`, `domain/mother.tera`, `domain/value_objects.tera`, `domain/shared_value_objects.tera`, `domain/shared_errors.tera`

`domain.rs` shrank from 1,335 (post-Phase-1) → 893 lines. Generator functions became context-builder + `render()` wrappers; old `USE_CASE_TRAIT` / `UPDATE_USE_CASE_TRAIT` raw-string consts removed; `if fields.is_empty()` branches eliminated.

Files modified:
- `crates/cli/src/generators/domain.rs`
- `crates/cli/src/generators/scaffold.rs` — replaced `apply(USE_CASE_TRAIT, ...)` with `generate_create_use_case_trait(pascal, snake, &[])`
- `crates/cli/src/templates/domain/` — 7 new `.tera` files (use_case_create, use_case_update, model, mother, value_objects, shared_value_objects, shared_errors)

Verification: All 200 tests pass.

---

### Phase 3 — Application Layer ✅ DONE

**Step 3a:** Replace 5 const templates (`USE_CASE_IMPL`, `GET_USE_CASE_IMPL`, `LIST_USE_CASE_IMPL`, `UPDATE_USE_CASE_IMPL`, `DELETE_USE_CASE_IMPL`) with `.tera` files. The consts are currently a fast-path for empty fields — after migration the templates always use `effective_fields`, eliminating the `fields.is_empty()` conditional.

**Step 3b:** Migrate 5 `generate_*_use_case_impl()` dynamic generators into the same template files. The embedded `#[cfg(test)]` modules use the `validation_test_cases` list from context for test generation.

`application.rs` shrank from 1,432 → ~370 lines. The 5 consts and raw string push_str loops removed; generator functions are now thin context-builder + `render()` wrappers. `scaffold.rs` non-CRUD path updated to call `generate_create_use_case_impl` with empty fields instead of `apply(USE_CASE_IMPL, ...)`.

Files modified:
- `crates/cli/src/generators/application.rs`
- `crates/cli/src/generators/scaffold.rs`
- `crates/cli/src/templates/application/` — **new directory** with 5 `.tera` files (create, get, list, update, delete)

Verification: All 200 tests pass. `scaffold_substitutes_pascal_name_in_use_case_impl`, `scaffold_crud_impls_import_model_struct_in_tests` pass.

---

### Phase 4 — Infrastructure Layer ✅ DONE

**Step 4a:** Replace 4 const templates:
- `INFRA_REPOSITORY` → `infrastructure/repository_inmemory_simple.tera`
- `CRUD_INFRA_REPOSITORY` → `infrastructure/repository_inmemory_crud.tera`
- `INFRA_DB_REPOSITORY` → `infrastructure/repository_pg_simple.tera`
- `DB_RS` → `infrastructure/db.tera`

**Step 4b:** Migrate `generate_infra_entity()` and `generate_crud_infra_db_repository()` to:
- `infrastructure/entity.tera`
- `infrastructure/repository_pg_crud.tera`

Note: These already use `template.replace(...)` internally. SQL column lists (`all_cols`, `all_bindings`, `conflict_set`) are computed in Rust and passed as pre-built strings — Tera just expands `{{ all_bindings }}`. This is valid; the SQL numbering `$1, $2...` is index-dependent and better handled in Rust.

After Phase 4, `infrastructure.rs` is 420 lines (plan estimated ~150 — the Rust context-builder logic is larger than expected but all consts/dynamic generators are gone).

Files modified: `crates/cli/src/generators/infrastructure.rs`, `crates/cli/src/generators/scaffold.rs`

Verification: `scaffold_db_creates_entity_rs`, `scaffold_without_db_still_uses_inmemory` pass. All 200 tests pass.

---

### Phase 5 — Presentation Layer ✅ DONE

**Step 5a:** Replace 5 const templates:
- `DTO` → `presentation/dto.tera` (field-aware: `is_crud` flag controls `Update{Pascal}Request`; `dto_fields_str`/`dto_from_str`/`request_fields_str` pre-computed in Rust)
- `RESPONSES` → `presentation/responses_simple.tera`
- `CRUD_RESPONSES` → `presentation/responses_crud.tera`
- `ERROR_MAPPER` → `presentation/error_mapper.tera` (unified: `vo_arms_str` pre-computed, empty for simple case)
- `ROUTES` → `presentation/routes_simple.tera`

**Step 5b:** Migrate `generate_crud_dto()`, `generate_crud_routes()`, `generate_crud_error_mapper()` to:
- `presentation/dto.tera` (unified with 5a via `is_crud` context flag)
- `presentation/routes_crud.tera` (pre-computed `create_params_str` / `update_params_str`)
- `presentation/error_mapper.tera` (unified with 5a via `vo_arms_str`)

`presentation.rs` shrank from 668 → 237 lines. Public `generate_crud_dto` / `generate_crud_routes` thin wrappers kept for backward compat with existing tests. `scaffold.rs` non-CRUD path now uses render calls with `build_dto_ctx`, `build_simple_ctx`, `build_error_mapper_ctx`.

Files modified: `crates/cli/src/generators/presentation.rs`, `crates/cli/src/generators/scaffold.rs`

Verification: `scaffold_crud_routes_has_all_http_methods`, `write_presentation_files_with_fields_generates_dynamic_dto_and_routes` pass. All 200 tests pass.

---

### Phase 6 — Use Case Generator + Bootstrap ✅ DONE

**Step 6a: `use_case.rs`**
- `UC_TRAIT` → `use_case/trait.tera`
- `UC_IMPL` → `use_case/impl.tera`
- `use_case.rs` is now a thin context-builder + `render()` wrapper (~40 lines)

**Step 6b: `bootstrap.rs`**
- `generate_bootstrap_content()` → `bootstrap/bootstrap.tera` with pre-computed `BootstrapEntity`/`BootstrapUseCase` structs in the Rust context builder
- `generate_tags_content()` → `bootstrap/tags.tera`
- `bootstrap.rs` shrank from 222 → ~130 lines (context-builder logic is larger than estimated)

Also migrated the `REPOSITORY` const from `scaffold.rs` to `domain/repository_simple.tera`.

Files modified: `crates/cli/src/generators/bootstrap.rs`, `crates/cli/src/generators/use_case.rs`, `crates/cli/src/generators/scaffold.rs`

Verification: `scaffold_updates_puerto_toml_and_regenerates_bootstrap`, `new_project_bootstrap_wires_tracing_logger` pass. All 200 tests pass.

---

### Phase 7 — Cleanup ✅ DONE

- `apply()` and `apply_uc()` removed from `crates/cli/src/generators/naming.rs` (zero callers)
- `naming.rs` now only contains `to_pascal_case`, `pascal_to_snake`, and `write_file`

---

## Critical Files

| File | Current Lines | Role in Migration |
|------|--------------|-------------------|
| `crates/cli/src/generators/domain.rs` | ~~1,476~~ 893 (Phase 2 done) | All domain templates migrated — context builders only |
| `crates/cli/src/generators/application.rs` | ~~1,432~~ ~370 (Phase 3 done) | All application templates migrated — context builders only |
| `crates/cli/src/generators/scaffold.rs` | ~252 | Imports consts from all layers — updated each phase |
| `crates/cli/src/generators/infrastructure.rs` | ~~920~~ 420 (Phase 4 done) | All infra templates migrated — context builders only |
| `crates/cli/src/generators/presentation.rs` | ~~682~~ 237 (Phase 5 done) | All presentation templates migrated — context builders only |
| `crates/cli/src/generators/bootstrap.rs` | ~~222~~ ~130 (Phase 6 done) | All templates migrated — context builders only |
| `crates/cli/src/generators/naming.rs` | ~~48~~ 27 (Phase 7 done) | `apply()` / `apply_uc()` removed — only `to_pascal_case`, `pascal_to_snake`, `write_file` remain |
| `crates/cli/src/generators/types.rs` | 508 | Stays as Rust — feeds context builders |
| `crates/cli/Cargo.toml` | — | Add `tera`, promote `serde_json` |

## Key Gotchas

1. **`scaffold.rs` non-CRUD `write_files()`** — imports consts from ALL four layers (`ERRORS`, `USE_CASE_TRAIT`, `USE_CASE_IMPL`, `INFRA_REPOSITORY`, `INFRA_DB_REPOSITORY`, `DTO`, `RESPONSES`, `ERROR_MAPPER`, `ROUTES`). Must be updated in EACH phase as the corresponding const is removed. It becomes a series of `render("...", &ctx)` calls with empty fields.

2. **`generate_errors()` unification** — the current code branches: `if fields_with_vo.is_empty() { apply(ERRORS, ...) } else { build dynamically }`. After migration, `errors.tera` handles both cases via `{% for f in vo_fields %}` (empty list = no extra variants). The branch disappears.

3. **`INFRA_DB_REPOSITORY` vs `generate_crud_infra_db_repository()`** — two different things: the const is the non-CRUD single-action PG repo (used in `scaffold.rs::write_files()`), the function generates the full CRUD PG repo. Both need separate templates (`repository_pg_simple.tera` and `repository_pg_crud.tera`).

4. **`pub(crate)` const visibility** — several consts are `pub(crate)` so `scaffold.rs` can import them. Once replaced by `render()` calls, this visibility is no longer needed and is automatically cleaned up.

5. **Tera has no built-in `snake_case` filter** (unlike Loco's rrgen). All case conversions must happen in the context builder. Pass pre-converted strings (e.g., `vo_snake = pascal_to_snake(vo_name)`) rather than computing in templates.

## Verification Per Phase

```bash
# After each phase:
cargo test -p puerto        # structural tests (fast)
make test                   # same, via Makefile

# After all phases:
make test/full              # generated project compiles + internal tests pass (~20s)
make lint                   # zero clippy warnings
make format                 # no formatting diff
```

Content assertions in `tests.rs` (substring `contains()` checks) are the authoritative regression guards. No snapshot framework needed during migration — they can be added as a follow-up to lock down exact output.
