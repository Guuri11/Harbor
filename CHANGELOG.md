## [0.9.2] - 2026-08-15

Two rounds of work: making generated projects actually compile, and making the CLI honour what the
docs already promised. A compile matrix now gates both — it generates real projects, scaffolds into
them and runs `cargo test --workspace` inside, which is how the bugs below were found.

### 🐛 Fixed

- **`Option<VO>` and `Vec<VO>` emitted model tests that did not compile** — the test-props builders
  branched only on enum/plain VOs, so `Option<VO>` got `VO::new(None)` and `Vec<VO>` got
  `VO::new(vec![])`, both with the wrong inner type
- **`puerto new --no-demo` dropped `pub mod tags;`** from `presentation/src/api.rs`, breaking the
  first scaffold into an empty project
- **An entity with no use cases emitted syntactically invalid `bootstrap.rs`** (`EntityApi { , logger: … }`)
- **`DateTime<Utc>` / `Uuid` fields re-imported chrono and uuid in the model** (E0252)
- **`get` / `list` / `delete` test modules missed field-type imports** (E0433)
- **DTOs used `DateTime` / `HashMap` with no import**, no chrono dependency and no poem-openapi
  chrono feature
- **`unique = true` never reached the SQL migration.** It is documented in `AGENTS.md`, exposed as
  the `!` CLI suffix, and did nothing. Now emits `CREATE UNIQUE INDEX {table}_{field}_key` — a named
  index rather than an inline column `UNIQUE`, so a later migration can drop it by a predictable
  name. This also makes the `23505` → `EntityError::Duplicate` mapping in the Pg repository
  reachable for the first time
- **`puerto generate domain` could never receive typed fields.** A dead `find` guaranteed an empty
  field list, and the entity was registered with `fields: []` — so `application`, `repository` and
  `presentation`, which all read fields back from `puerto.toml`, were field-blind. The entire
  documented domain-first workflow could only ever produce the default `name: String` entity

### 🚀 Features

- **`puerto generate scaffold` no longer overwrites silently.** Re-scaffolding an entity that has
  generated files is refused: interactively it lists the files and asks, non-interactively it aborts
  without writing a byte. `--force` overwrites and updates the `[[entity]]` block in place, so the
  manifest keeps describing the code on disk — use cases added via `generate use-case` survive
- **`puerto generate domain <Name> -- fields`** takes the same field syntax as `scaffold` and
  persists the fields to `puerto.toml`, which is what makes the later layered commands field-aware
- **Manifest / filesystem drift detection.** `puerto validate` now errors when a declared
  `[[entity]]` has no code on disk (naming the missing paths and the command that fixes it) and
  warns on the inverse — a domain module with no `[[entity]]` block. `puerto generate bootstrap`
  refuses to write a bootstrap that will not compile; `--allow-missing` writes it anyway
- **`puerto --version`.** The flag was documented in the install instructions and did not exist —
  clap never generated it because `version` was absent from `#[command(...)]`
- **Stricter name validation**, applied identically by the CLI parser and `puerto validate`:
  system field names (`id`, `created_at`, `updated_at`, `deleted`, `deleted_at`), Rust keywords,
  entity names that collide with generated code (`Error`, `String`, `Logger`, `Type`, …), duplicate
  enum variants, and two fields declaring the same value object with different inner types

### 🚜 Refactor

- New `generators/conflict.rs` — the overwrite guard, checked before the first byte is written
- New `generators/consistency.rs` — the manifest/filesystem pass, covering exactly the files
  `bootstrap.rs` imports
- New `validation.rs` — one home for `SYSTEM_FIELDS`, `RUST_KEYWORDS`, reserved type and module
  names, and the name predicates. Called from `parse_field_arg`, from `run_validate` and before any
  scaffold write, so the two can no longer diverge
- The `-- fields` plumbing is a single `parse_cli_fields()` shared by `scaffold` and `domain`

### ⚙️ CI

- `make test/full` runs on every push and pull request, not only on tag push. It used to be
  tag-only, which is how three separate bugs that produced non-compiling projects reached `main`
- The compile matrix covers primitives, `Option`/`Vec` primitives, plain and unique value objects,
  `Option`/`Vec`/enum/shared value objects, multiple entities, `--no-demo`, and the domain-first
  layered flow with fields

### 📚 Documentation

- Swept the drift between the docs and the CLI: the `scaffold --db` flag that no longer exists, the
  old `name:Name[vo:String]` value object syntax, `crates/template/basic/`, `scaffold.rs` described
  as the home of the writers, and tests described as living in `main.rs`. Every `crates/`-relative
  path in `AGENTS.md` and `.claude/rules/*` is now verified to exist
- The landing docs now document that re-running `scaffold` is refused, and what `--force` does

### 💥 Breaking

- **Field names with a leading underscore are rejected.** `_private:String` used to parse. The
  predicate allowed it while its own error message said otherwise, and a leading underscore reads as
  "deliberately unused" to every Rust reader.
- **Re-running `puerto generate scaffold <Name>` on an existing entity is refused** instead of
  overwriting. Non-interactive callers (CI, scripts) now get a non-zero exit.
- **`puerto generate bootstrap` refuses** when `puerto.toml` declares an entity whose code is not on
  disk, instead of writing a file that does not compile.
- **`puerto validate` fails on inputs it used to accept**: system field names, Rust keywords,
  reserved entity names, duplicate enum variants, incoherent value objects, and declared-but-
  unscaffolded entities.

### Migration Guide

Existing projects need no file changes. The commands are stricter, so adjust any automation:

1. **Scripts that re-run `scaffold` to regenerate an entity** must pass `--force`:

   ```bash
   puerto generate scaffold Product --force -- name:String price:i64
   ```

   Without it the command exits non-zero and writes nothing.

2. **Run `puerto validate` once** and fix what it reports. The new checks catch manifests that were
   already producing code that does not compile:

   - a field named `id` / `created_at` / `updated_at` / `deleted` / `deleted_at` → rename it; every
     entity already has those
   - a field named after a Rust keyword (`type`, `match`, `move`, …) → rename it
   - a field name starting with `_` → rename it
   - an entity named `Error`, `String`, `Logger`, `Type`, … → rename it
   - an entity declared in `puerto.toml` but never scaffolded → run the command `validate` suggests,
     or delete the `[[entity]]` block

3. **If `puerto generate bootstrap` now refuses**, the manifest and the filesystem disagree. Fix the
   drift it names. `--allow-missing` restores the old behaviour if you genuinely want the file
   written anyway.

4. **SQL projects with `unique = true` fields**: the constraint was never generated, so existing
   tables do not have it. New scaffolds emit it. To add it to an existing table:

   ```sql
   CREATE UNIQUE INDEX products_sku_key ON products (sku);
   ```

## [0.8.0] - 2026-05-04

### 🚀 Features

- **Entity fields in `puerto.toml`** — define typed fields on entities via `[[entity.fields]]` blocks. Generated model structs, Props, Params, DTOs, repository rows, and SQL migrations all derive from the field list automatically
- **Type registry** — 13 Rust types supported out of the box: `String`, `i64`, `bool`, `f64`, `Uuid`, `DateTime<Utc>`, `Option<T>`, `Vec<T>`, `HashMap<String, String>`, and their nullable/array variants. Each type maps to SQL column types, OpenAPI formats, and default test values
- **Scaffold with fields** — `puerto generate scaffold Product name:String price:i64! sku:String` passes typed fields via CLI. Fields are persisted to `puerto.toml` and used across all layers
- **`puerto validate`** — new command that validates `puerto.toml`: entity names (PascalCase), field names (snake_case), field types (against the type registry), duplicate entities/fields, and warns about `Option` + `unique` combinations
- **Dynamic generators** — all layer generators now produce typed code from the field list. When `fields` is empty, the previous `name: String` default is preserved for backward compatibility
- **Field-aware Object Mother** — `ProductMother` generates builder methods for each custom field (`with_price()`, `with_empty_name()`, etc.) instead of just `with_name()`

### 🚜 Refactor

- Replaced string-constant generators with dynamic functions across all layers (`generate_model()`, `generate_mother()`, `generate_crud_dto()`, `generate_crud_routes()`, `generate_infra_entity()`, `generate_crud_infra_db_repository()`)
- Added `generators/types.rs` module with `TypeMapping` registry, `resolve_type()`, `validate_fields()`, `collect_imports()`
- Added `commands/validate.rs` module with full puerto.toml validation

## [0.7.2] - 2026-05-04

### Changed

- Refactor: split `main.rs` and `scaffold.rs` into dedicated modules (`commands`, `generators`, `patchers`, `tests`)

## [0.7.0] - 2026-05-03

### 💥 Rebranding — `puerto-framework` → `puerto`

This release marks the rename of the crate from `puerto-framework` to `puerto`.

The name Puerto was taken by many existing tools, making SEO and discoverability difficult. **Puerto** (Spanish for _port_) keeps the original nautical metaphor, is easy to pronounce in English, and has a unique presence on crates.io.

**What changed:**

- Crate name on crates.io: `puerto-framework` → `puerto`
- GitHub repo: `Guuri11/puerto` → `Guuri11/Puerto`
- Everything else is identical — CLI commands, generated project structure, `puerto.toml` schema

**Migrating:**

```toml
# Cargo.toml
[dependencies]
# before
puerto-framework = "0.6"
# after
puerto = "0.7"
```

The `puerto-framework` crate on crates.io is now deprecated. No further versions will be published under that name.

---

## [0.6.0] - 2026-05-03

### 🚀 Features

- Add `puerto generate domain <Name>` — scaffolds domain layer only (model, errors, repository trait, use cases, Object Mother) and adds entity to `puerto.toml`
- Add `puerto generate application <Name>` — scaffolds application layer only (use case impls) for an entity already in `puerto.toml`
- Add `puerto generate repository <Name>` — scaffolds infrastructure layer only (InMemory or Pg repository, inferred from `puerto.toml`)
- Add `puerto generate presentation <Name>` — scaffolds presentation layer only (routes, dto, responses, error_mapper) and regenerates `bootstrap.rs`
- Object Mother test factory (`{Entity}Mother`) is now generated by `puerto generate domain` and `puerto generate scaffold` — provides `random()`, `random_vec(n)`, builder chain, and `build_props()` helpers
- Generated project Makefiles overhauled: ANSI colour output, 20+ targets, split test targets (`test/domain`, `test/application`), `check`, `clean`, `audit`, `audit/fix`, `format/fix`, `generate/entity`, `generate/use-case`, `generate/bootstrap`
- DB project Makefiles now include a full SQLx suite: `sqlx/online`, `sqlx/offline`, `sqlx/migrate`, `sqlx/prepare`, `sqlx/check`, `generate/migration`; Docker helpers `docker-compose/up`, `docker-compose/down`, `db-up`, `db-down`, and a `reset-db` target with destroy confirmation
- Puerto root `Makefile` updated with ANSI colours, `format/fix`, `audit`, `audit/fix` targets
- Generated project `AGENTS.md` files updated with Object Mother docs and layer-by-layer command guidance

### 📚 Documentation

- `/docs` page: added "Layer Generators" section documenting all four new per-layer commands
- `AGENTS.md` (puerto root): CLI commands table updated with four new `puerto generate` commands
- Template `AGENTS.md` files updated to reference per-layer generators for AI-assisted development

## [0.5.0] - 2026-04-27

### 🚀 Features

- `puerto generate scaffold` now infers `--db` from `puerto.toml` (`project.db = true`) — no need to pass the flag when the project already uses SQLx
- Interactive `puerto new` demo prompt: shows a real project preview (`my-app`) before asking for the actual project name
- Bump to v0.5.0

## [0.4.0] - 2026-04-27

### 🚀 Features

- Generated projects now print a Puerto ASCII banner on startup (set `HARBOR_BANNER=false` to suppress)
- HTTP request logging via poem's `Tracing` middleware — every request/response is traced automatically
- `InMemoryRepository` receives the project logger; repository-level operations are now logged at `debug`
- `GreetingApi` (and all generated `*Api` structs) receive the logger; errors are logged at `warn`
- `uuid` and `chrono` template dependencies now include `serde` features for JSON serialisation out of the box
- `poem-openapi` template dependency now includes `uuid` feature for typed UUID path/query params

### 💥 Breaking

- `InMemoryGreetingRepository` is no longer a unit struct — constructor is now `InMemoryGreetingRepository::new(logger: Arc<dyn LoggerTrait>)`. Update `bootstrap.rs` in existing projects (or run `puerto generate bootstrap`).
- `GreetingApi` (and all scaffolded `*Api` structs) now require a `logger: Arc<dyn LoggerTrait>` field. Update `bootstrap.rs` in existing projects.
- `build_app()` in `generated/bootstrap.rs` is now `async` and returns `impl poem::Endpoint` (was `Route`). Update `main.rs` to `generated::bootstrap::build_app().await`.

### 🐛 Bug Fixes

- `puerto new --db` now correctly writes `project.db = true` to `puerto.toml` — was silently omitted, causing `puerto generate scaffold --db` to not detect db mode correctly

### ⚙️ Miscellaneous Tasks

- Test coverage: add `db_project_puerto_toml_has_project_db_true` and `no_db_project_puerto_toml_omits_project_db` structural tests

## [0.3.0] - 2026-04-25

### 🚀 Features

- Add `puerto generate snippets [--ide zed|vscode]` command — writes TextMate-format snippet files for Zed (`.zed/snippets/rust.json`) and VS Code (`.vscode/puerto.code-snippets`), compatible with nvim+LuaSnip
- `puerto new` now automatically writes snippet files to both IDEs on project creation
- 23 Puerto-adapted snippets covering all DDD layers: domain model, errors, repository trait, use case trait/impl, persistence entity/repo, tokio test, sqlx integration test, Object Mother, and more

### 🐛 Bug Fixes

- Fix `cargo fmt` violation in `scaffold.rs` (`apply_db_to_new_project` method chain formatting)

### 📚 Documentation

- Add IDE Snippets section to `/docs` page with snippet inventory, IDE setup instructions, and regeneration commands

## [0.2.0] - 2026-04-25

### 🚀 Features

- Add `LoggerTrait` domain port with `noop()` helper and mockall mock
- Add `TracingLogger` infrastructure adapter backed by the `tracing` crate
- Wire logger into all generated `UseCaseImpl` structs and `bootstrap.rs`
- Initialize `tracing_subscriber` with env-filter in generated `main.rs`
- Add `--no-db` flag to `puerto new` (explicit opt-out without prompting)
- Add `--destination` flag to `puerto new` (controls output directory)
- Generate `docker-compose.yml` and `.env` for `--db` projects
- Add db Makefile targets: `docker/up`, `sqlx/migrate`, `sqlx/prepare`, etc.
- Run migrations automatically on bootstrap startup (`run_migrations`)
- Add `uuid`, `chrono`, `serde`, `tracing`, `dotenvy` to template dependencies

### 💥 Breaking

- All generated `UseCaseImpl` structs now require a `logger: Arc<dyn LoggerTrait>` field. Existing projects must add this field manually and update `bootstrap.rs` (or run `puerto generate bootstrap`).

### 🐛 Bug Fixes

- Correct repo URL, install command, and Rust version in landing

### 🚜 Refactor

- Remove redundant crates/template, clean CLI output

### 📚 Documentation

- Update release.md crate name and AGENTS.md make commands
- Fix install command in README (puerto-framework)

### ⚙️ Miscellaneous Tasks

- Add CI and release GitHub Actions workflows

## [0.1.0] - 2026-04-12

### 🚀 Features

- Auto-wire DI via puerto.toml + generated bootstrap

### 🐛 Bug Fixes

- Rename template Cargo.toml to .liquid to fix crates.io packaging

### ⚙️ Miscellaneous Tasks

- Rename crate to puerto-framework for crates.io
