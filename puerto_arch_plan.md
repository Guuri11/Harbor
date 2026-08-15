# Plan: Codebase Review Findings — Action Items

> Generated from a full review of the CLI (v0.9.1, `main` @ `edc482b`).
> Findings were produced by reading every generator + **building the release binary and
> generating real projects**, then running `cargo check --workspace --all-targets` on the output.
> Every P0/P1 item below has a reproducible command.
>
> The completed Tera-migration plan lives in `puerto_tera_migration_plan.md` (phases 0–7 ✅).
> Its one unfinished goal is carried here as **T-10**.

---

## Why these bugs exist

The test suite has 200 tests and they all pass, yet several generator paths emit Rust that
does not compile. The reason is structural:

- **453 `contains()` assertions vs. 1 compile test.** `tests.rs` asserts on substrings of the
  generated *text*. Nothing checks that the text is valid Rust.
- **The single compile test (`tests.rs:238`) compiles the bare template only** — `puerto new`,
  no scaffold, no fields, no value objects. The entire generation surface (fields, VOs, CRUD,
  Pg repositories, bootstrap) has **zero compile coverage**.
- **CI never runs it.** `.github/workflows/ci.yml` runs `make test`, `make lint`, `cargo fmt`.
  `make test/full` — which `.claude/rules/release.md` calls "the most important gate" — only
  runs on tag push, i.e. after the bug is already on `main`.

**T-07 is therefore the highest-leverage task in this document**: it would have caught T-01,
T-02 and T-03 before they shipped. Consider doing it first, watching it fail, then fixing the rest.

---

## Priority overview

| ID | Severity | Task | Files |
|----|----------|------|-------|
| ~~T-01~~ | **P0** ✅ | `Option<VO>` / `Vec<VO>` emit non-compiling model tests | `generators/domain.rs` |
| ~~T-02~~ | **P0** ✅ | `puerto new --no-demo` drops `pub mod tags;` | `generators/project.rs` |
| ~~T-03~~ | **P0** ✅ | Entity with no use cases emits invalid `bootstrap.rs` | `templates/bootstrap/bootstrap.tera` |
| ~~T-04~~ | **P0** ✅ | Re-running `scaffold` destroys hand-written code, silently desyncs `puerto.toml` | `generators/scaffold.rs`, `puerto_toml.rs` |
| ~~T-31~~ | **P0** ✅ | `DateTime<Utc>`/`Uuid` field re-imports chrono/uuid in the model (E0252) | `generators/domain.rs` |
| ~~T-32~~ | **P0** ✅ | `get`/`list`/`delete` test modules miss field-type imports (E0433) | `generators/application.rs` |
| ~~T-33~~ | **P0** ✅ | DTOs use `DateTime`/`HashMap` with no import, no chrono dep, no poem-openapi chrono feature | `generators/presentation.rs`, template `Cargo.toml` |
| ~~T-05~~ | **P1** ✅ | `unique = true` never reaches the SQL migration | `generators/infrastructure.rs` |
| ~~T-06~~ | **P1** ✅ | `generate domain` can never receive typed fields (dead branch) | `generators/domain.rs` |
| ~~T-07~~ | **P1** ✅ | CI does not run `make test/full`; no compile matrix | `.github/workflows/ci.yml`, `tests.rs` |
| ~~T-08~~ | **P1** ✅ | `bootstrap.rs` wires entities whose code does not exist | `generators/consistency.rs`, `generators/bootstrap.rs`, `commands/validate.rs` |
| T-09 | **P2** | VO accessor logic triplicated across three layers | `types.rs`, `infrastructure.rs`, `presentation.rs` |
| T-10 | **P2** | Finish the Tera migration — templates receive pre-rendered Rust | all generators + `templates/` |
| T-11 | **P2** | Errors swallowed (`let _ =`, ignored `bool`, `.expect()` panics) | `generators/scaffold.rs` + all |
| ~~T-12~~ | **P2** ✅ | Validation gaps: system-field collisions, Rust keywords | `validation.rs`, `commands/validate.rs`, `puerto_toml.rs` |
| T-13 | **P2** | Naive pluralization for table names (`categorys`) | `generators/infrastructure.rs` |
| ~~T-14~~ | **P2** ✅ | Documentation drift (flags, syntax, file paths) | `AGENTS.md`, `.claude/rules/*` |
| T-15 | **P2** | `make lint` misses test targets | `Makefile`, `tests.rs` |
| T-16 | P3 | No `puerto destroy` | new command |
| T-17 | P3 | No atomicity / `--dry-run` on generation | `generators/scaffold.rs` |
| T-18 | P3 | Add snapshot tests for generated output | `tests.rs` |
| T-19 | P3 | `--db` projects don't compile until Docker + `sqlx prepare` | `templates/infrastructure/*`, `Makefile` |
| T-20 | P3 | Use case naming: `list_product` should be `list_products` | `generators/scaffold.rs` |

---

## Milestone 1 — DONE

T-07, T-01, T-02, T-03, T-04 are implemented; the sections below are kept as the record of what
was wrong and why. Three further bugs (T-31/T-32/T-33) were found **by the new matrix** and fixed
in the same pass — see "What the matrix caught that this plan did not" at the end of the P0 section.

Suite: **221 tests, 0 skipped, all green** (was 200 passing while generated projects did not
compile). `make lint` clean with `--all-targets`.

---

## Milestone 2 — DONE

T-05, T-06, T-08, T-12 and T-14 are implemented; the sections below are kept as the record of what
was wrong and why. The theme is the same in all five: the documented contract and the code had
drifted apart, and nothing checked either against the other.

What landed:

- **T-05** — `unique = true` now emits `CREATE UNIQUE INDEX {table}_{field}_key` after the
  `CREATE TABLE`. A named index rather than an inline column `UNIQUE`, so a later migration can
  drop it by a name it can predict. The `23505` → `EntityError::Duplicate` mapping already in the
  PG repository template is now actually reachable.
- **T-06** — `puerto generate domain <Name> -- fields` takes the same trailing field arguments as
  `scaffold` and persists them to puerto.toml, which is what makes `application` / `repository` /
  `presentation` field-aware. The dead `find` is gone; the `-- fields` plumbing is one
  `parse_cli_fields()` helper shared by both commands.
- **T-08** — new `generators/consistency.rs`: the manifest-vs-filesystem pass, checking exactly the
  files `bootstrap.rs` imports. `puerto validate` errors on a declared-but-unscaffolded entity and
  warns on the inverse (a domain module with no `[[entity]]`); `puerto generate bootstrap` refuses
  unless `--allow-missing`. The fix it suggests follows the layered flow — `generate application`
  mid-workflow, not `scaffold`.
- **T-12** — new `validation.rs`, the single home for `SYSTEM_FIELDS`, `RUST_KEYWORDS`, reserved
  type/module names and the name predicates. Called from `parse_field_arg`, from `run_validate`,
  and before any scaffold write, so the CLI and the manifest checker cannot diverge again.
- **T-14** — docs swept after the behaviour changes: the non-existent `scaffold --db` flag, the old
  `name:Name[vo:String]` syntax, `crates/template/basic/`, `scaffold.rs` as the home of the
  writers, and tests living in `main.rs`. Every `crates/`-relative path in `AGENTS.md` and
  `.claude/rules/*` is now verified to exist.

**Breaking:** a field name with a leading `_` is now rejected (`_private:String`). It was accepted
by a predicate whose own error message said otherwise.

Suite: **253 tests, 0 skipped, all green** — including a new compile-matrix scenario for the
domain-first layered flow with fields, which is what T-06 was actually about. `make lint` clean
with `--all-targets`.

---

# P0 — Generated projects do not compile

## T-01 · `Option<VO>` and `Vec<VO>` emit model tests that do not compile

**Files:** `crates/cli/src/generators/domain.rs:156-177` (`empty_props`), `:184-205` (`ws_props`)

**Repro**
```bash
puerto new --name demo --no-db && cd demo
puerto generate scaffold Persona -- name:String mid:Mid:opt:String
cargo check --workspace --all-targets     # 5 errors

puerto generate scaffold Product -- title:String tags:Tag:vec:String
cargo check --workspace --all-targets     # 4 errors
```

**Output**
```rust
// business/src/domain/persona/model.rs
mid: Mid::new(None).expect("valid Mid"),          // expected String, found Option<_>
// business/src/domain/product/model.rs
tags: Tag::new(vec![]).expect("valid Tag"),       // expected Vec<Tag>, found Tag
```

**Root cause.** `valid_props_lines` (`:104-123`) branches correctly on
`is_option_vo` → `is_vec_vo` → `is_enum_vo` → `is_value_object`. The two test-props builders
below it only branch on `is_enum_vo` → `is_value_object`, so `Option<VO>` and `Vec<VO>` fall
into the plain-VO arm and get `VO::new(default_expr)` with the wrong inner type.

**Fix.** Extract the props-line builder into one function used by all three call sites:

```rust
/// The single source of truth for "a literal of this field's Props type".
/// `override_value` is used by the validation tests to inject "" / "   ".
fn props_literal(f: &Field, override_value: Option<&str>) -> String
```

Note this is the same class of bug as **T-09** (duplicated VO branching) — solving T-09
properly makes T-01 unrepeatable. Consider doing them together.

**Acceptance**
- [ ] `props_literal()` is the only place that renders a Props field literal
- [ ] Compile test covering `opt:` VO, `vec:` VO, `enum:` VO and a shared VO (see T-07)
- [ ] `cargo test --workspace` passes *inside* the generated project, not just `cargo check`

---

## T-02 · `puerto new --no-demo` produces a project that breaks on first scaffold

**File:** `crates/cli/src/generators/project.rs:242`

**Repro**
```bash
puerto new --name bare --no-db --no-demo && cd bare
cargo check                       # ✅ compiles
puerto generate scaffold Persona -- name:String
cargo check                       # ❌ E0432: unresolved import `crate::api::tags`
```

**Root cause.**
```rust
fs::write(base.join("presentation/src/api.rs"), "pub mod error;\n")?;
```
The template's `api.rs` is `pub mod error; pub mod greeting; pub mod tags;`. Stripping the demo
also strips `pub mod tags;`, but `regenerate_bootstrap()` still writes `api/tags.rs` and every
generated `routes.rs` does `use crate::api::tags::ApiTags;`. The empty project compiles only
because nothing imports tags yet.

**Fix.** Write `"pub mod error;\npub mod tags;\n"`, and additionally make
`patchers/api_rs.rs` idempotently ensure `pub mod tags;` exists (defence in depth — `api.rs` is
a file users may edit).

**Acceptance**
- [ ] `--no-demo` + scaffold compiles
- [ ] Test asserts `api.rs` contains `pub mod tags;` after `apply_no_demo`
- [ ] Covered by the T-07 compile matrix

---

## T-03 · An entity with no use cases emits syntactically invalid `bootstrap.rs`

**File:** `crates/cli/src/templates/bootstrap/bootstrap.tera:43`

**Repro** — add to `puerto.toml` by hand (a documented workflow: puerto.toml is the source of truth):
```toml
[[entity]]
name = "Invoice"
use_cases = []
```
```bash
puerto validate                    # ⚠ warning only — passes
puerto generate bootstrap
cargo check                        # ❌ expected identifier, found `,`
```

**Output**
```rust
let invoice_api = InvoiceApi { , logger: Arc::clone(&logger) };
```

**Root cause.** `use_case_fields` is `entity.use_cases.join(", ")` (`generators/bootstrap.rs:141`),
interpolated as `{{ entity.use_case_fields }}, logger: ...` with no guard for the empty list.

**Fix.** Build the whole field list (use cases + `logger`) in the Rust context builder so the
template interpolates one already-correct string; or guard in the template with
`{% if entity.use_cases | length > 0 %}`.

**Decide as part of T-08:** should an entity with zero use cases be an *error* in
`puerto validate` rather than a warning? It cannot produce a working app.

**Acceptance**
- [ ] `generate_bootstrap_content()` with a use-case-less entity produces parseable Rust
- [ ] Unit test on the generated string + a compile test

---

## T-04 · Re-running `scaffold` destroys hand-written code and desyncs `puerto.toml`

**Files:** `crates/cli/src/generators/scaffold.rs:140-200` (`run`), `puerto_toml.rs:add_entity`

**Repro**
```bash
puerto generate scaffold Product -- title:String price:i64 sku:Sku:String!
echo "// my business rule" >> business/src/domain/product/model.rs
puerto generate scaffold Product -- title:String
# → "✓ Done. Zero manual wiring."
grep "my business rule" business/src/domain/product/model.rs   # gone
grep -c "entity.fields" puerto.toml                            # still 3 fields
```

**Two distinct problems**

1. **Silent destruction.** `run()` calls `write_file()` unconditionally on ~15 files across
   4 layers. Any hand-written domain logic, custom use case body, or added test is lost with
   no prompt, no backup, no diff.
2. **Silent desync.** `add_entity()` is a no-op when the entity already exists, so `puerto.toml`
   keeps the *old* field list while the code now reflects the *new* one. Neither
   `puerto validate` nor `puerto generate bootstrap` can detect the divergence — the manifest
   is no longer the source of truth it claims to be.

**Fix**
- Add a conflict policy in `write_file()` / a new `write_generated()` wrapper:
  - file absent → write
  - file present and byte-identical → skip silently (`identical` in Rails terms)
  - file present and different → prompt `overwrite? [y/N/a/d]` (`d` shows a diff),
    non-interactive default = abort with a clear message
  - `--force` skips the prompt, `--skip` keeps existing files
- Make `add_entity()` an upsert (or add `update_entity_fields()`), so re-scaffolding with new
  fields rewrites the `[[entity.fields]]` block.
- Consider refusing re-scaffold outright and pointing at a future `puerto destroy` (T-16).

**Acceptance**
- [ ] Re-scaffold with a modified file prompts (interactive) / aborts (non-interactive)
- [ ] `--force` restores today's behaviour
- [ ] After a successful re-scaffold, `puerto.toml` fields match the generated model
- [ ] Test: scaffold → modify → re-scaffold → assert modification survives without `--force`

---

## What the matrix caught that this plan did not

The three bugs below were invisible to 200 passing structural tests and to this review's manual
inspection. They surfaced the first time a project with a `DateTime<Utc>` / `map` field was
compiled — the registry advertises those types, but nothing had ever built one end to end.

- **T-31** `generate_model` re-emitted `use chrono::{DateTime, Utc};` / `use uuid::Uuid;` that
  `model.tera` already writes for the system fields → E0252. Fixed by filtering against
  `MODEL_TEMPLATE_IMPORTS`.
- **T-32** The `get`/`list`/`delete` application impls carry no file-level imports (their impls
  need none) but their generated **test modules** build full `EntityProps` literals → `HashMap`
  undeclared, E0433. Fixed with `build_test_imports_block`.
- **T-33** DTO fields keep the domain's primitive types, but `dto.tera` imported only `uuid::Uuid`,
  the presentation crate had no `chrono` dependency, and `poem-openapi` lacked its `chrono` feature
  (no `Type` impl for `DateTime<Utc>`) → three separate failures in one field.

All three are the same shape: **a per-layer import list that nobody kept in sync with the type
registry.** That is T-09/T-10 territory — a `FieldCtx` carrying its own imports would make the
class impossible. Consider that evidence for prioritising them.

---

# P1 — Documented behaviour that does not work / missing safety nets

## T-05 · `unique = true` never reaches the SQL migration

**Files:** `crates/cli/src/generators/infrastructure.rs:22` (`sql_ddl_col`), `:329` (`create_table_sql`)

**Repro**
```bash
puerto new --name dbdemo --db && cd dbdemo
puerto generate scaffold Order -- ref_code:String! amount:i64
cat infrastructure/migrations/*_create_order_table.sql
```
```sql
ref_code TEXT NOT NULL,     -- no UNIQUE
```

`sql_ddl_col(name, field_type)` does not even take the `unique` flag. The feature is documented
in `AGENTS.md`, `.claude/rules/puerto-toml.md` and `.claude/rules/value_objects.md`, and is
exposed in CLI syntax via the `!` suffix.

**Fix.** Pass the `Field` instead of `(name, field_type)` and append `UNIQUE`, or emit a
`CREATE UNIQUE INDEX` per unique field (better: it keeps the column definition uniform and
gives the index a predictable name for later migrations).

**Also decide:** should `Duplicate` in the domain error enum be mapped from the Postgres
unique-violation code (`23505`) in the generated repository? Today `EntityError::Duplicate`
exists but nothing ever returns it.

**Acceptance**
- [ ] `sku:String!` produces a unique constraint in the migration
- [ ] `Option<T>` + `unique` still only warns (existing `validate` behaviour)
- [ ] Test asserting the SQL contains the constraint

---

## T-06 · `generate domain` can never receive typed fields

**File:** `crates/cli/src/generators/domain.rs:830-842`

```rust
if config.entity.iter().any(|e| e.name == pascal) {
    return Err(format!("{pascal} is already in puerto.toml..."));   // aborts
}
let fields: Vec<Field> = config.entity.iter().find(|e| e.name == pascal)   // unreachable
    .map(|e| e.fields.clone()).unwrap_or_default();                        // always vec![]
```

The `find` is dead by construction. Worse, `:872` registers the entity with `fields: vec![]`,
so every downstream layered command (`application`, `repository`, `presentation`) — which all
read fields from `puerto.toml` — is field-blind. **The entire domain-first workflow documented
in `.claude/rules/workflow.md` can only ever produce the default `name: String` entity.**

**Repro**
```bash
# declare the entity with fields first, as the docs suggest
puerto generate domain Invoice
# → Error: Invoice is already in puerto.toml
# with no prior declaration:
puerto generate domain Ticket && grep -c "amount" business/src/domain/ticket/model.rs   # 0
```

**Fix.** Give `generate domain` the same trailing `-- fields` argument as `scaffold`
(`#[arg(raw = true)] fields: Vec<String>`), run it through `parse_field_arg` +
`apply_shared_vo_inference` + `validate_fields`, and persist them via `add_entity`.
Then delete the dead `find`. The same `-- fields` plumbing in `main.rs` is duplicated for
scaffold — factor it into one helper while you're there.

**Acceptance**
- [ ] `puerto generate domain Invoice -- amount:i64 status:Status:enum:Draft/Paid` works
- [ ] Fields land in `puerto.toml` so `application` / `repository` / `presentation` pick them up
- [ ] Full layered flow with fields compiles (add to T-07 matrix)

---

## T-07 · CI does not run the compile gate; add a generation matrix

**Files:** `.github/workflows/ci.yml`, `crates/cli/src/tests.rs:238`, `Makefile:39`

Today `make test/full` = exactly one `#[ignore]`d test that runs `cargo test` on a **bare**
`puerto new` project. CI never runs it at all.

**Fix — two parts**

1. **Run it in CI.** Add a `test-full` job (can be a separate job so the fast feedback loop
   stays fast). It needs a warm cargo cache; `Swatinem/rust-cache` is already configured.

2. **Make it a matrix.** One `#[ignore]`d test per scenario, each: generate → scaffold →
   `cargo test --workspace` inside the generated project. Minimum set, derived from the bugs
   found in this review:

   | Scenario | Command |
   |----------|---------|
   | bare template | `puerto new` (existing test) |
   | `--no-demo` + scaffold | catches **T-02** |
   | primitives, all registry types | `title:String price:i64 ok:bool rate:f64 at:DateTime meta:map` |
   | `opt:` / `vec:` primitives | `desc:opt:String tags:vec:String` |
   | plain VO + unique VO | `name:Name:String sku:Sku:String!` |
   | `Option<VO>` / `Vec<VO>` | catches **T-01** |
   | enum VO | `status:Status:enum:Active/Archived` |
   | shared VO | `value-object Email String` then `email:Email` |
   | multi-entity | two scaffolds → bootstrap wiring with `(a_api, b_api)` |
   | layered flow | `domain` → `application` → `repository` → `presentation` (**T-06**) |
   | `--db` | compile with a live Postgres service container + `sqlx prepare` (**T-19**) |

   These are slow (~20 s each). Keep them `#[ignore]`d for local `make test`, run the whole set
   in CI on PRs to `main`. If wall-clock becomes a problem, run the full matrix nightly and a
   3-scenario smoke set on every PR.

**Acceptance**
- [ ] CI fails on a PR that reintroduces T-01, T-02 or T-03
- [ ] `make test/full` runs the whole matrix locally
- [ ] Each scenario runs `cargo test`, not just `cargo check` (T-01 only breaks test targets)

---

## T-08 · `bootstrap.rs` wires entities whose code does not exist

**Files:** `crates/cli/src/generators/bootstrap.rs`, `crates/cli/src/commands/validate.rs`

`regenerate_bootstrap()` emits imports for every `[[entity]]` in `puerto.toml` with no check
that the modules exist. An entity declared but not scaffolded yields:

```rust
use business::domain::invoice::repository::InvoiceRepositoryTrait;   // E0433
use infrastructure::invoice::repository::InMemoryInvoiceRepository;  // E0433
use crate::api::invoice::routes::InvoiceApi;                         // E0433
```

This is the same root cause as T-04's desync: nothing verifies that manifest and filesystem agree.

**Fix.** Add a filesystem-consistency pass, used by both commands:

- `puerto validate` → error per entity whose expected files are missing, listing the paths and
  suggesting `puerto generate scaffold <Name>`
- `puerto generate bootstrap` → run the same check first and refuse (or `--allow-missing` to
  emit anyway)
- Also flag the inverse: generated entity directories with no `[[entity]]` block

**Acceptance**
- [ ] `puerto validate` fails on a declared-but-unscaffolded entity
- [ ] `puerto generate bootstrap` refuses instead of emitting non-compiling code
- [ ] Field-level drift detection (T-04) reuses this pass

---

# P2 — Design debt

## T-09 · The VO accessor decision tree is triplicated

The same branching (`enum → option → vec → vo → needs_clone → plain`) exists in three places:

| Location | Function |
|----------|----------|
| `generators/types.rs:field_value_accessor` | canonical-ish |
| `generators/infrastructure.rs:from_fields_str` | inline copy (`entity.rs` `From` impl) |
| `generators/presentation.rs:build_dto_from_expr` | inline copy (`dto.rs` `from_domain`) |

`field_needs_clone()` is defined **identically three times** (`types.rs`,
`infrastructure.rs:15`, `presentation.rs:13`). `is_valid_vo_name()` is duplicated in
`puerto_toml.rs` and `commands/validate.rs`.

Adding a registry type means editing all copies and remembering all four VO shapes — which is
precisely how T-01 happened.

**Fix**
- One `field_value_accessor(field, prefix)` in `types.rs`; delete the two inline copies
- One `field_needs_clone()`; delete the two duplicates
- One `is_valid_vo_name()` (move to a shared `naming`/`validation` module)
- Add unit tests over the cartesian product: {plain, Option, Vec, enum} × {String, i64, Uuid, DateTime}
  — asserting the emitted expression string, cheap and fast

**Acceptance**
- [ ] `grep -rn "fn field_needs_clone" crates/cli/src` returns one hit
- [ ] Infrastructure and presentation call the shared helper
- [ ] Matrix unit test in `types.rs`

---

## T-10 · Finish the Tera migration

The stated goal of `puerto_tera_migration_plan.md` was "generator `.rs` files become thin
context-builders". Phases 0–7 are done, but the templates still receive **pre-rendered Rust
source as opaque strings**:

`try_from_fields_str`, `from_fields_str`, `seed_fn`, `update_test`, `validation_tests_str`,
`build_assignments_str`, `dto_from_str`, `create_params_str`, `all_bindings`…

`domain.rs` is still 893 lines of `format!` building Rust. The plan's own contract
(lines 116-160: pass `effective_fields` as structured objects, iterate in the template) was
only applied to `bootstrap.tera`, `errors.tera` and the use-case traits.

**Fix (incremental, one template at a time)**
- Build a `FieldCtx` struct (name, rust_type, is_vo, is_option_vo, is_vec_vo, is_enum_vo,
  vo_name, vo_snake, inner_type, default_expr, needs_clone, accessor_expr, constructor_expr,
  props_literal, sql_type, sql_nullable) — serialize it once, reuse in every layer
- Move the `{% if %}` branching into the `.tera` files
- Order: `model.tera` (biggest win, fixes T-01 structurally) → `mother.tera` →
  `entity.tera` → `dto.tera` → application impls

Exception worth keeping in Rust: SQL `$1, $2…` numbering is index-dependent and genuinely
clearer as a Rust computation (the migration plan already notes this at line 263).

**Acceptance**
- [ ] `domain.rs` under ~300 lines
- [ ] No `format!` producing Rust statements outside `types.rs` accessor helpers
- [ ] Adding a registry type touches `types.rs` + templates only

---

## T-11 · Errors are swallowed; renders panic

**File:** `crates/cli/src/generators/scaffold.rs:151-168`

```rust
try_patch_libs(&snake, base, db, crud);              // returns bool — ignored
let _ = patch_business_lib_value_objects(base, &snake);
let _ = write_shared_vo_files(base, shared_vos);
let _ = patch_business_lib_shared(base);
let _ = write_mother(&pascal, &snake, base, fields, shared_vos);
let _ = patch_mothers_lib(base, &snake);
```

If `lib.rs` patching fails, the CLI still prints `✓ Done. Zero manual wiring.` and the user
gets a project that will not compile with no indication why.

Separately, there are ~40 `.expect("... render failed")` across the generators
(`domain.rs` 16, `scaffold.rs` 7, `application.rs` 6, `presentation.rs` 6, `infrastructure.rs` 4).
A template bug becomes a panic + backtrace instead of a readable CLI error — and the project's
own rules forbid `expect()` outside tests.

**Fix**
- Propagate every `Result` with `?`; make `try_patch_libs` return `Result<(), Error>`
- Convert `render(...).expect(...)` to `render(...)?` (`render` already returns `Result`);
  introduce a `PuertoError` enum (`thiserror`) instead of `Box<dyn Error>` so messages carry
  the file/template that failed
- Only print `✓ Done` when everything succeeded

**Acceptance**
- [ ] No `let _ =` on a fallible call in `generators/`
- [ ] `grep -rn "expect(" crates/cli/src/generators` returns nothing outside `#[cfg(test)]`
- [ ] Test: read-only target directory → clear error, no `✓ Done`

---

## T-12 · Validation gaps that produce non-compiling code

**Files:** `crates/cli/src/commands/validate.rs`, `crates/cli/src/puerto_toml.rs`

Currently unchecked:

| Gap | Consequence |
|-----|-------------|
| Field named `id`, `created_at`, `updated_at`, `deleted`, `deleted_at` | duplicate struct fields in the model — E0124 |
| Field named `type`, `match`, `fn`, `impl`, `move`, … | invalid Rust identifier |
| Entity named `Self`, `Box`, `String`, … | collides in generated code |
| `is_valid_field_name` allows a leading `_` | contradicts its own error message and the docs |
| Enum VO with duplicate variants | `enum` variant defined twice — E0428 |
| Two fields whose VOs share a name but differ in type | conflicting VO definitions in one `value_objects.rs` |

Validation must run in **both** places: `parse_field_arg` (fail fast at the CLI) and
`puerto validate` (catch hand-edited manifests). Today they diverge.

**Fix.** One `validation` module holding `RUST_KEYWORDS`, `SYSTEM_FIELDS`, and the name
predicates; called from `parse_field_arg`, `run_validate`, and before any scaffold write.

**Acceptance**
- [ ] `puerto generate scaffold X -- id:String` fails with a clear message
- [ ] `puerto generate scaffold X -- type:String` fails
- [ ] `puerto validate` catches the same cases in a hand-edited `puerto.toml`

---

## T-13 · Naive pluralization for table names

**File:** `crates/cli/src/generators/infrastructure.rs:329`

```rust
format!("CREATE TABLE {snake}s (…")
```
`Category` → `categorys`, `Person` → `persons`, `Box` → `boxs`.

**Fix.** Either add a small inflector (or the `pluralizer` / `inflector` crate — check the
dependency budget; the CLI is currently lean), or make the table name explicit and overridable
in `puerto.toml` (`[[entity]] table = "categories"`), defaulting to naive pluralization for
backwards compatibility. **Note this is a breaking change** for existing projects if the
default changes — needs a `## Migration Guide` entry per `.claude/rules/release.md`.

**Acceptance**
- [ ] `Category` → `categories`, `Person` → `people` (or documented `table =` override)
- [ ] Changelog entry with migration guide if the default changes

---

## T-14 · Documentation drift

| Location | Says | Reality |
|----------|------|---------|
| `AGENTS.md`, `.claude/rules/puerto-toml.md` | `puerto generate scaffold <Name> --db` | flag removed; db is inferred from `puerto.toml` |
| `.claude/rules/value_objects.md` (parsing section) | `name:Name[vo:String]`, `status:Status[enum:A,B]` | new syntax is `name:Name:String`, `status:Status:enum:A/B` |
| `.claude/rules/workflow.md` (Key Paths) | `crates/cli/src/scaffold.rs` holds writers/patchers/bootstrap | it is a 15-line re-export module; real code is in `generators/` |
| `.claude/rules/workflow.md` | tests live in `crates/cli/src/main.rs` | they live in `crates/cli/src/tests.rs` |
| `.claude/rules/testing-conventions.md` | mothers at `business/src/tests/mothers/` | correct, but `.claude/rules/value_objects.md` says `business/tests/mothers/` |
| `.claude/rules/workflow.md:192` | `crates/template/basic/` | actual path is `crates/cli/template/` |
| `AGENTS.md` | `unique = true` generates a unique DB constraint | not implemented (T-05) |

**Fix.** Sweep after T-05/T-06 land, so the docs describe the fixed behaviour rather than being
corrected twice. Add a check to the completion checklist: docs updated only where behaviour changed.

**Acceptance**
- [ ] Every command/flag in `AGENTS.md` verified against `puerto --help` output
- [ ] Every path in the rules files verified to exist

---

## T-15 · `make lint` misses test targets

**Files:** `Makefile:43`, `crates/cli/src/tests.rs:7`

`cargo clippy --workspace -- -D warnings` does not lint test targets. With `--all-targets`:

```
warning: this import is redundant
 --> crates/cli/src/tests.rs:7:1
  | use serde_json;
```

**Fix.** `cargo clippy --workspace --all-targets -- -D warnings`, then fix the fallout.
`tests.rs` is 4,234 lines — consider splitting it into `tests/` submodules by command
(`tests/scaffold.rs`, `tests/validate.rs`, `tests/value_objects.rs`, `tests/compile.rs`)
while you are in there.

**Acceptance**
- [ ] `make lint` clean with `--all-targets`
- [ ] CI enforces it

---

# P3 — Product gaps (post-stabilisation)

## T-16 · `puerto destroy <Entity>`

Rails' generators are reversible; Puerto's are not. Removing an entity today means deleting
~15 files across 4 layers, un-patching three `lib.rs`/`api.rs` blocks, and editing
`puerto.toml`. Should remove files, revert patches, drop the `[[entity]]` block, regenerate
bootstrap, and warn about (but not delete) the SQL migration.

## T-17 · Atomicity and `--dry-run`

A failed scaffold leaves a half-written project with no rollback. Options: stage writes in a
temp dir and move on success, or record a manifest of written files for undo. `--dry-run`
(print the file list without writing) is cheap and useful on its own, and pairs naturally with
the conflict prompt from T-04.

## T-18 · Snapshot tests for generated output

The 453 `contains()` assertions are brittle in both directions: they miss whole-file
regressions and they break on cosmetic changes. `insta` snapshots of full generated files
(one per scenario in the T-07 matrix) would lock down exact output and make diffs reviewable.
Compile tests (T-07) and snapshots (T-18) are complementary: one proves it works, the other
proves it did not change.

## T-19 · `--db` projects don't compile out of the box

`puerto new --db` + `scaffold` produces a project that fails `cargo check` until Docker is up,
migrations have run and `cargo sqlx prepare` has been executed — because the generated
repositories use the compile-time-checked `sqlx::query_as!` macros with `SQLX_OFFLINE = "true"`
and no `.sqlx` cache.

Options:
1. Use the runtime `sqlx::query_as::<_, T>(...)` API instead of the macros — the project
   compiles immediately, at the cost of losing compile-time SQL verification
2. Keep the macros but ship a `make setup/db` that runs docker-compose → migrate → prepare, and
   print it as the "Next steps" after `puerto new --db`
3. Commit a pre-generated `.sqlx` cache for the demo entity

Recommendation: (2) now (cheap, honest), evaluate (1) as an opt-in `--no-compile-time-sql`.

## T-20 · Use case naming

`scaffold` generates `list_product`; the docs and DDD convention say `list_products`.
Changing it is breaking (module names, `puerto.toml`, bootstrap) — schedule with a migration
guide, or accept and fix the docs.

---

# P4 — Capability gaps vs. a production DDD backend

Derived from comparing Puerto's output against `nexe/projects/nexe-document/document-api` — a
**hand-written** backend (60k LOC, 22 crates, 1170 tests) that adapts the same `ant_backend`
conventions Puerto does, by the same author. It is not a Puerto project; it is the closest thing to
a reference implementation of Puerto's target output. Full analysis: **`puerto_vs_document_api.md`**.
Each item below is work currently done by hand on every project that the generator could do.

| ID | Task | Effort | Retrofit cost if skipped |
|----|------|--------|--------------------------|
| T-21 | Bounded contexts (several aggregates per context directory) | L | High |
| T-22 | `--no-crud`: domain + one intention-revealing use case | S | Low |
| T-23 | Domain events + Unit of Work + outbox + worker | XL | **Very high** |
| T-24 | Newtype IDs instead of raw `Uuid` | **S** | High |
| T-25 | Multi-tenancy: `TenantId`, scoped queries, RLS, leak tests | L | **Very high** |
| T-26 | Presentation DI via `Arc<dyn UseCase>` + generated route tests | **S** | Medium |
| T-27 | `PageRequest` / `PaginatedResult` for `list_*` | **M** | Medium |
| T-28 | Shared `RepositoryError` + sqlx error mapping | M | Low |
| T-29 | `puerto generate worker` / second binary | L | Medium |
| T-30 | Generate `scripts/check-architecture.sh` + `make lint/arch` | **S** | Low |

## T-26 · Presentation depends on concrete use case impls *(fix first — it is a rule violation)*

`.claude/rules/architecture.md` states *"Dependencies injected as `Arc<dyn Trait>` — never concrete
types"*. The generated application layer honours it; the generated presentation layer does not:

```rust
pub struct ProductApi {
    pub create_product: Arc<CreateProductUseCaseImpl>,   // ← concrete
}
```

Consequences: presentation is coupled to `business::application::*`, and routes cannot be tested
with mocked use cases — which is why Puerto generates **0 tests in presentation and 0 in
infrastructure** (vs. 6 domain + 14 application per entity).

**Fix.** Emit `Arc<dyn CreateProductUseCaseTrait>` in `ProductApi` + `bootstrap.rs`, add a
`ProductApi::new(...)` constructor, and generate route tests using the use-case mocks
(requires `mockall` mocks for use case traits, not just repositories).

**Acceptance:** routes hold ports; a generated route test asserts 200 + 400 paths with a mocked
use case; `make test/full` compiles it.

## T-24 · Newtype IDs

`macro_rules! newtype_id` (see `business/src/domain/shared/ids.rs` in the reference — 20 lines)
makes `find_by_id(company_id)` where `product_id` was meant a compile error. Fits the existing VO
machinery; the SQL layer already converts at the boundary. Opt-in via `puerto.toml` to stay
backwards compatible.

## T-27 · Pagination

`find_all() -> Vec<Product>` is unbounded. Port `PageRequest` (clamped in a VO with private fields
so no route can bypass the cap) + `PaginatedResult<T>` into the generated
`business/src/domain/common/`, and wire `list_*` + its DTO to them.

## T-30 · Architecture check as a generated script

Ship `scripts/check-architecture.sh` (cargo metadata + jq) and a `make lint/arch` target in every
generated project, so the dependency rule is enforced rather than merely documented. Also worth
adding to Puerto's own CI for its 3-crate output.

## T-23 / T-25 · Events + multi-tenancy — decide the positioning first

These are not incremental features; they change the spine of every generated write path
(bare repository → UoW + outbox + event, `Uuid` → `TenantId`-scoped everything). See
`puerto_vs_document_api.md` §4: the recommendation is a `[project] profile` key in `puerto.toml`
(`minimal` = today's behaviour, declared now at zero cost) with `events` / `multitenant` profiles
added later — **after** the P0 work, because a second spine multiplies a surface that currently has
no compile coverage at all.

---

## Suggested execution order

**Milestone 1 — stop the bleeding (P0 + the gate)**
`T-07` (write the matrix, watch it go red) → `T-01` → `T-02` → `T-03` → `T-04`
Ship as `0.9.2` (bug fixes; T-04's prompt is arguably `0.10.0` since it changes CLI behaviour).

**Milestone 2 — honour the documented contract** ✅ DONE
`T-05` → `T-06` → `T-08` → `T-12` → `T-14`
Ship as `0.10.0`.

**Milestone 3 — pay down the debt**
`T-09` → `T-11` → `T-15` → `T-10` → `T-18`
No user-visible change; makes every future field type / VO shape cheap.

**Milestone 4 — product**
`T-16` → `T-17` → `T-19` → `T-13` → `T-20`

**Milestone 5 — close the gap with a production backend** (see `puerto_vs_document_api.md`)
Quick wins first: `T-26` → `T-30` → `T-24` → `T-27` → `T-28`.
Then decide the positioning (profiles) before touching `T-23` / `T-25`; `T-21`, `T-22`, `T-29`
follow from whichever positioning is chosen.

Per `.claude/rules/workflow.md`, each task gets its spec here first, then failing tests,
then implementation. For T-01/T-02/T-03 the failing test is the compile scenario from T-07 —
write it before touching the generator.
