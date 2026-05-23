use crate::commands::list::{require_puerto_project, run_list};
use crate::commands::new::extract_template;
use crate::commands::validate;
use crate::puerto_toml::ValueObjectDefinition;
use crate::{puerto_toml, scaffold, snippets};
use cargo_generate::{GenerateArgs, TemplatePath, Vcs, generate};
use serde_json;
use std::fs;
use std::path::{Path, PathBuf};

fn temp_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("puerto_test_{name}"))
}

fn cleanup(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

fn generate_project(name: &str, destination: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let tmp = extract_template()?;

    let args = GenerateArgs {
        template_path: TemplatePath {
            path: Some(tmp.path().to_string_lossy().into_owned()),
            ..Default::default()
        },
        name: Some(name.to_string()),
        destination: Some(destination.to_path_buf()),
        vcs: Some(Vcs::None),
        no_workspace: true,
        ..Default::default()
    };

    let output_dir = generate(args)?;
    drop(tmp);
    Ok(output_dir)
}

// ── puerto new (non-interactive / flag-driven) ────────────────────────────

#[test]
fn new_name_flag_sets_project_name() {
    let dir = temp_dir("new_name_flag");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = new_project_non_interactive(Some("explicit-name".into()), false, &dir).unwrap();

    let content = fs::read_to_string(output.join("presentation/Cargo.toml")).unwrap();
    assert!(content.contains("name = \"explicit-name\""));

    cleanup(&dir);
}

#[test]
fn new_db_flag_creates_db_files() {
    let dir = temp_dir("new_db_flag");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = new_project_non_interactive(Some("db-project".into()), true, &dir).unwrap();

    assert!(output.join("docker-compose.yml").exists());
    assert!(output.join(".env").exists());
    assert!(output.join(".env.example").exists());
    assert!(output.join(".cargo/config.toml").exists());
    assert!(output.join("infrastructure/migrations").is_dir());
    assert!(output.join("infrastructure/src/db.rs").exists());

    cleanup(&dir);
}

#[test]
fn new_without_db_flag_has_no_db_files() {
    let dir = temp_dir("new_no_db");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = new_project_non_interactive(Some("plain-project".into()), false, &dir).unwrap();

    assert!(!output.join(".env.example").exists());
    assert!(!output.join(".cargo/config.toml").exists());
    assert!(!output.join("infrastructure/src/db.rs").exists());

    cleanup(&dir);
}

// ── puerto new ────────────────────────────────────────────────────────────

#[test]
fn creates_project_structure() {
    let dir = temp_dir("cg_structure");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = generate_project("test-app", &dir).unwrap();

    assert!(output.join("Cargo.toml").exists());
    assert!(output.join("business/src/lib.rs").exists());
    assert!(output.join("infrastructure/src/lib.rs").exists());
    assert!(output.join("presentation/src/main.rs").exists());

    cleanup(&dir);
}

#[test]
fn presentation_cargo_toml_has_project_name() {
    let dir = temp_dir("cg_cargo");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = generate_project("my-puerto-app", &dir).unwrap();

    let content = fs::read_to_string(output.join("presentation/Cargo.toml")).unwrap();
    assert!(content.contains("name = \"my-puerto-app\""));

    cleanup(&dir);
}

#[test]
fn main_rs_is_minimal_entry_point() {
    let dir = temp_dir("cg_main");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = generate_project("cool-project", &dir).unwrap();

    let content = fs::read_to_string(output.join("presentation/src/main.rs")).unwrap();
    assert!(content.contains("generated::bootstrap::build_app"));
    assert!(content.contains("mod generated"));
    assert!(!content.contains("{{project-name}}"));

    cleanup(&dir);
}

#[test]
fn bootstrap_rs_wires_ddd_layers() {
    let dir = temp_dir("cg_bootstrap");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = generate_project("api-test", &dir).unwrap();

    let content =
        fs::read_to_string(output.join("presentation/src/generated/bootstrap.rs")).unwrap();
    assert!(content.contains("GetGreetingUseCaseImpl"));
    assert!(content.contains("InMemoryGreetingRepository"));
    assert!(content.contains("GreetingApi"));
    assert!(content.contains("OpenApiService"));
    assert!(content.contains("8080"));

    cleanup(&dir);
}

#[test]
fn ddd_layers_exist() {
    let dir = temp_dir("cg_ddd");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = generate_project("ddd-app", &dir).unwrap();

    assert!(
        output
            .join("business/src/domain/greeting/model.rs")
            .exists()
    );
    assert!(
        output
            .join("business/src/domain/greeting/errors.rs")
            .exists()
    );
    assert!(
        output
            .join("business/src/domain/greeting/repository.rs")
            .exists()
    );
    assert!(
        output
            .join("business/src/domain/greeting/use_cases/get_greeting.rs")
            .exists()
    );
    assert!(
        output
            .join("business/src/application/greeting/get_greeting.rs")
            .exists()
    );
    assert!(
        output
            .join("infrastructure/src/greeting/repository.rs")
            .exists()
    );
    assert!(
        output
            .join("presentation/src/api/greeting/routes.rs")
            .exists()
    );
    assert!(output.join("presentation/src/api/greeting/dto.rs").exists());
    assert!(
        output
            .join("presentation/src/api/greeting/error_mapper.rs")
            .exists()
    );
    assert!(
        output
            .join("presentation/src/api/greeting/responses.rs")
            .exists()
    );
    assert!(output.join("puerto.toml").exists());
    assert!(output.join("presentation/src/generated.rs").exists());
    assert!(
        output
            .join("presentation/src/generated/bootstrap.rs")
            .exists()
    );
    assert!(output.join("presentation/src/api/tags.rs").exists());

    cleanup(&dir);
}

#[test]
fn puerto_toml_has_project_name_and_greeting_entity() {
    let dir = temp_dir("cg_puerto_toml");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = generate_project("my-app", &dir).unwrap();

    let content = fs::read_to_string(output.join("puerto.toml")).unwrap();
    assert!(content.contains("name = \"my-app\""));
    assert!(content.contains("name = \"Greeting\""));
    assert!(content.contains("get_greeting"));

    cleanup(&dir);
}

#[test]
#[ignore = "slow: compiles and tests a full generated project"]
fn generated_project_compiles_and_tests_pass() {
    let dir = temp_dir("cg_compile");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = generate_project("compile-test", &dir).unwrap();

    let result = std::process::Command::new("cargo")
        .args(["test", "--workspace"])
        .current_dir(&output)
        .output()
        .expect("failed to run cargo test");

    if !result.status.success() {
        eprintln!("stdout:\n{}", String::from_utf8_lossy(&result.stdout));
        eprintln!("stderr:\n{}", String::from_utf8_lossy(&result.stderr));
        panic!("cargo test failed in generated project");
    }

    cleanup(&dir);
}

// ── puerto generate scaffold ──────────────────────────────────────────────

#[test]
fn scaffold_creates_all_files_for_single_word_entity() {
    let dir = temp_dir("scaffold_single_word");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    scaffold::run("Product", &dir, false, false, &[], &[]).unwrap();

    assert!(dir.join("business/src/domain/product/model.rs").exists());
    assert!(dir.join("business/src/domain/product/errors.rs").exists());
    assert!(
        dir.join("business/src/domain/product/repository.rs")
            .exists()
    );
    assert!(
        dir.join("business/src/domain/product/use_cases/create_product.rs")
            .exists()
    );
    assert!(
        dir.join("business/src/application/product/create_product.rs")
            .exists()
    );
    assert!(
        dir.join("infrastructure/src/product/repository.rs")
            .exists()
    );
    assert!(dir.join("presentation/src/api/product.rs").exists());
    assert!(dir.join("presentation/src/api/product/dto.rs").exists());
    assert!(
        dir.join("presentation/src/api/product/responses.rs")
            .exists()
    );
    assert!(
        dir.join("presentation/src/api/product/error_mapper.rs")
            .exists()
    );
    assert!(dir.join("presentation/src/api/product/routes.rs").exists());

    cleanup(&dir);
}

#[test]
fn scaffold_creates_all_files_for_multi_word_entity() {
    let dir = temp_dir("scaffold_multi_word");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    scaffold::run("OrderItem", &dir, false, false, &[], &[]).unwrap();

    assert!(dir.join("business/src/domain/order_item/model.rs").exists());
    assert!(
        dir.join("business/src/domain/order_item/use_cases/create_order_item.rs")
            .exists()
    );
    assert!(
        dir.join("business/src/application/order_item/create_order_item.rs")
            .exists()
    );
    assert!(
        dir.join("infrastructure/src/order_item/repository.rs")
            .exists()
    );
    assert!(
        dir.join("presentation/src/api/order_item/routes.rs")
            .exists()
    );

    cleanup(&dir);
}

#[test]
fn scaffold_normalizes_lowercase_input() {
    let dir = temp_dir("scaffold_lowercase");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    scaffold::run("product", &dir, false, false, &[], &[]).unwrap();

    assert!(dir.join("business/src/domain/product/model.rs").exists());
    assert!(
        dir.join("business/src/application/product/create_product.rs")
            .exists()
    );

    cleanup(&dir);
}

#[test]
fn scaffold_substitutes_pascal_name_in_model() {
    let dir = temp_dir("scaffold_model_content");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    scaffold::run("Product", &dir, false, false, &[], &[]).unwrap();

    let content = fs::read_to_string(dir.join("business/src/domain/product/model.rs")).unwrap();
    assert!(content.contains("pub struct ProductProps"));
    assert!(content.contains("pub struct Product {"));
    assert!(content.contains("ProductError::ValidationError"));
    assert!(content.contains("product.validation_error.name_empty"));

    cleanup(&dir);
}

#[test]
fn scaffold_substitutes_pascal_name_in_errors() {
    let dir = temp_dir("scaffold_errors_content");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    scaffold::run("Product", &dir, false, false, &[], &[]).unwrap();

    let content = fs::read_to_string(dir.join("business/src/domain/product/errors.rs")).unwrap();
    assert!(content.contains("pub enum ProductError"));
    assert!(content.contains("product.not_found"));
    assert!(content.contains("product.repository_error"));

    cleanup(&dir);
}

#[test]
fn scaffold_substitutes_pascal_name_in_use_case_impl() {
    let dir = temp_dir("scaffold_impl_content");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    scaffold::run("Product", &dir, false, false, &[], &[]).unwrap();

    let content =
        fs::read_to_string(dir.join("business/src/application/product/create_product.rs")).unwrap();
    assert!(!content.contains("Create{Pascal}UseCaseImpl"));
    assert!(content.contains("CreateProductUseCaseImpl"));
    assert!(content.contains("ProductRepositoryTrait"));
    assert!(content.contains("product.validation_error.name_empty"));

    cleanup(&dir);
}

#[test]
fn scaffold_crud_impls_import_model_struct_in_tests() {
    let dir = temp_dir("scaffold_model_import");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    scaffold::run("Product", &dir, false, true, &[], &[]).unwrap();

    for uc in &[
        "get_product",
        "list_product",
        "update_product",
        "delete_product",
    ] {
        let path = dir.join(format!("business/src/application/product/{uc}.rs"));
        let content = fs::read_to_string(&path).unwrap();
        assert!(
            content.contains("model::{Product, ProductProps}"),
            "{uc}.rs test block missing `model::{{Product, ProductProps}}` import"
        );
    }

    cleanup(&dir);
}

// ── lib.rs auto-patching ──────────────────────────────────────────────────

fn setup_puerto_stubs(base: &Path) {
    fs::create_dir_all(base.join("business/src")).unwrap();
    fs::write(
        base.join("business/src/lib.rs"),
        "pub mod domain {\n  pub mod greeting {\n    pub mod errors;\n    pub mod model;\n    pub mod repository;\n    pub mod use_cases {\n      pub mod get_greeting;\n    }\n  }\n}\npub mod application {\n  pub mod greeting {\n    pub mod get_greeting;\n  }\n}\n",
    )
    .unwrap();

    fs::create_dir_all(base.join("infrastructure/src")).unwrap();
    fs::write(
        base.join("infrastructure/src/lib.rs"),
        "pub mod greeting {\n    pub mod repository;\n}\n",
    )
    .unwrap();

    fs::create_dir_all(base.join("presentation/src")).unwrap();
    fs::write(
        base.join("presentation/src/api.rs"),
        "pub mod error;\npub mod greeting;\n",
    )
    .unwrap();

    fs::write(
        base.join("puerto.toml"),
        "[project]\nname = \"test-app\"\n\n[[entity]]\nname = \"Greeting\"\nuse_cases = [\"get_greeting\"]\n",
    )
    .unwrap();
}

#[test]
fn scaffold_patches_business_lib_rs() {
    let dir = temp_dir("scaffold_patch_business");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs(&dir);
    scaffold::run("Product", &dir, false, false, &[], &[]).unwrap();

    let content = fs::read_to_string(dir.join("business/src/lib.rs")).unwrap();
    assert!(content.contains("pub mod product {"));
    assert!(content.contains("pub mod errors;"));
    assert!(content.contains("pub mod use_cases {"));
    assert!(!content.contains("pub mod use_cases;")); // never the bare form
    assert!(content.contains("pub mod create_product;"));
    assert!(content.contains("pub mod greeting {"));

    cleanup(&dir);
}

#[test]
fn scaffold_patches_infra_lib_rs() {
    let dir = temp_dir("scaffold_patch_infra");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs(&dir);
    scaffold::run("Product", &dir, false, false, &[], &[]).unwrap();

    let content = fs::read_to_string(dir.join("infrastructure/src/lib.rs")).unwrap();
    assert!(content.contains("pub mod product {"));
    assert!(content.contains("pub mod repository;"));
    assert!(content.contains("pub mod greeting {"));

    cleanup(&dir);
}

#[test]
fn scaffold_patches_api_rs() {
    let dir = temp_dir("scaffold_patch_api");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs(&dir);
    scaffold::run("Product", &dir, false, false, &[], &[]).unwrap();

    let content = fs::read_to_string(dir.join("presentation/src/api.rs")).unwrap();
    assert!(content.contains("pub mod product;"));
    assert!(content.contains("pub mod error;"));
    assert!(content.contains("pub mod greeting;"));

    cleanup(&dir);
}

#[test]
fn scaffold_updates_puerto_toml_and_regenerates_bootstrap() {
    let dir = temp_dir("scaffold_bootstrap");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs(&dir);
    scaffold::run("Product", &dir, false, false, &[], &[]).unwrap();

    let toml = fs::read_to_string(dir.join("puerto.toml")).unwrap();
    assert!(toml.contains("name = \"Product\""));
    assert!(toml.contains("create_product"));
    assert!(toml.contains("name = \"Greeting\"")); // original still present

    let bootstrap =
        fs::read_to_string(dir.join("presentation/src/generated/bootstrap.rs")).unwrap();
    assert!(bootstrap.contains("CreateProductUseCaseImpl"));
    assert!(bootstrap.contains("InMemoryProductRepository"));
    assert!(bootstrap.contains("ProductApi"));
    assert!(bootstrap.contains("GetGreetingUseCaseImpl"));
    assert!(bootstrap.contains("InMemoryGreetingRepository"));
    assert!(bootstrap.contains("GreetingApi"));

    cleanup(&dir);
}

// ── puerto generate use-case ──────────────────────────────────────────────

fn setup_use_case_stubs(base: &Path) {
    fs::write(
        base.join("puerto.toml"),
        "[project]\nname = \"test-app\"\n\n[[entity]]\nname = \"Product\"\nuse_cases = [\"create_product\"]\n",
    )
    .unwrap();

    fs::create_dir_all(base.join("business/src/domain/product")).unwrap();
    fs::write(
        base.join("business/src/lib.rs"),
        "pub mod domain {\n    pub mod product {\n        pub mod errors;\n        pub mod model;\n        pub mod repository;\n        pub mod use_cases {\n            pub mod create_product;\n        }\n    }\n}\npub mod application {\n    pub mod product {\n        pub mod create_product;\n    }\n}\n",
    )
    .unwrap();

    fs::create_dir_all(base.join("business/src/application/product")).unwrap();
}

#[test]
fn use_case_creates_trait_file() {
    let dir = temp_dir("uc_trait");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_use_case_stubs(&dir);
    scaffold::run_use_case("Product", "delete_product", &dir).unwrap();

    assert!(
        dir.join("business/src/domain/product/use_cases/delete_product.rs")
            .exists()
    );

    cleanup(&dir);
}

#[test]
fn use_case_creates_impl_file() {
    let dir = temp_dir("uc_impl");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_use_case_stubs(&dir);
    scaffold::run_use_case("Product", "delete_product", &dir).unwrap();

    assert!(
        dir.join("business/src/application/product/delete_product.rs")
            .exists()
    );

    cleanup(&dir);
}

#[test]
fn use_case_trait_file_has_correct_content() {
    let dir = temp_dir("uc_trait_content");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_use_case_stubs(&dir);
    scaffold::run_use_case("Product", "delete_product", &dir).unwrap();

    let content =
        fs::read_to_string(dir.join("business/src/domain/product/use_cases/delete_product.rs"))
            .unwrap();
    assert!(content.contains("DeleteProductParams"));
    assert!(content.contains("DeleteProductUseCaseTrait"));

    cleanup(&dir);
}

#[test]
fn use_case_impl_file_has_correct_content() {
    let dir = temp_dir("uc_impl_content");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_use_case_stubs(&dir);
    scaffold::run_use_case("Product", "delete_product", &dir).unwrap();

    let content =
        fs::read_to_string(dir.join("business/src/application/product/delete_product.rs")).unwrap();
    assert!(content.contains("DeleteProductUseCaseImpl"));
    assert!(content.contains("ProductRepositoryTrait"));
    assert!(content.contains("DeleteProductUseCaseTrait"));

    cleanup(&dir);
}

#[test]
fn use_case_patches_business_lib_rs() {
    let dir = temp_dir("uc_patch_lib");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_use_case_stubs(&dir);
    scaffold::run_use_case("Product", "delete_product", &dir).unwrap();

    let content = fs::read_to_string(dir.join("business/src/lib.rs")).unwrap();
    assert!(content.contains("pub mod delete_product;"));
    assert!(content.contains("pub mod create_product;")); // existing preserved
    assert_eq!(content.matches("pub mod delete_product;").count(), 2); // domain + application

    cleanup(&dir);
}

#[test]
fn use_case_updates_puerto_toml() {
    let dir = temp_dir("uc_puerto_toml");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_use_case_stubs(&dir);
    scaffold::run_use_case("Product", "delete_product", &dir).unwrap();

    let content = fs::read_to_string(dir.join("puerto.toml")).unwrap();
    assert!(content.contains("delete_product"));
    assert!(content.contains("create_product")); // existing preserved

    cleanup(&dir);
}

#[test]
fn use_case_regenerates_bootstrap() {
    let dir = temp_dir("uc_bootstrap");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_use_case_stubs(&dir);
    scaffold::run_use_case("Product", "delete_product", &dir).unwrap();

    let content = fs::read_to_string(dir.join("presentation/src/generated/bootstrap.rs")).unwrap();
    assert!(content.contains("DeleteProductUseCaseImpl"));
    assert!(content.contains("CreateProductUseCaseImpl"));

    cleanup(&dir);
}

#[test]
fn use_case_normalizes_entity_casing() {
    let dir = temp_dir("uc_normalize");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_use_case_stubs(&dir);
    scaffold::run_use_case("product", "delete_product", &dir).unwrap();

    assert!(
        dir.join("business/src/domain/product/use_cases/delete_product.rs")
            .exists()
    );

    cleanup(&dir);
}

#[test]
fn use_case_errors_when_entity_not_in_puerto_toml() {
    let dir = temp_dir("uc_missing_entity");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_use_case_stubs(&dir);

    let result = scaffold::run_use_case("NonExistent", "do_something", &dir);
    assert!(result.is_err());

    cleanup(&dir);
}

// ── puerto new --db ───────────────────────────────────────────────────────

fn new_project_non_interactive(
    name: Option<String>,
    db: bool,
    destination: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let tmp = extract_template()?;
    let args = GenerateArgs {
        template_path: TemplatePath {
            path: Some(tmp.path().to_string_lossy().into_owned()),
            ..Default::default()
        },
        name: name.clone(),
        destination: Some(destination.to_path_buf()),
        vcs: Some(Vcs::None),
        no_workspace: true,
        ..Default::default()
    };
    let output = generate(args)?;
    drop(tmp);
    if db {
        scaffold::apply_db_to_new_project(&output)?;
    }
    Ok(output)
}

fn generate_db_project(
    name: &str,
    destination: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output = generate_project(name, destination)?;
    scaffold::apply_db_to_new_project(&output)?;
    Ok(output)
}

#[test]
fn db_project_has_env_example() {
    let dir = temp_dir("db_env");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_db_project("db-app", &dir).unwrap();
    let content = fs::read_to_string(output.join(".env.example")).unwrap();
    assert!(content.contains("DATABASE_URL"));
    assert!(content.contains("db_app"));
    cleanup(&dir);
}

#[test]
fn db_project_has_env_file_with_project_db_name() {
    let dir = temp_dir("db_env_file");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_db_project("my-api", &dir).unwrap();
    let content = fs::read_to_string(output.join(".env")).unwrap();
    assert!(content.contains("DATABASE_URL"));
    assert!(content.contains("my_api"));
    cleanup(&dir);
}

#[test]
fn db_project_has_docker_compose_with_project_name() {
    let dir = temp_dir("db_docker_compose");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_db_project("my-api", &dir).unwrap();
    let content = fs::read_to_string(output.join("docker-compose.yml")).unwrap();
    assert!(content.contains("postgres:16"));
    assert!(content.contains("my-api-postgres"));
    assert!(content.contains("my_api"));
    assert!(content.contains("5432"));
    cleanup(&dir);
}

#[test]
fn db_project_has_gitignore_with_env() {
    let dir = temp_dir("db_gitignore");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_db_project("db-app", &dir).unwrap();
    let gitignore_path = output.join(".gitignore");
    if gitignore_path.exists() {
        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains(".env"));
    }
    cleanup(&dir);
}

#[test]
fn db_project_makefile_has_docker_and_sqlx_targets() {
    let dir = temp_dir("db_makefile_targets");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_db_project("db-app", &dir).unwrap();
    let content = fs::read_to_string(output.join("Makefile")).unwrap();
    assert!(content.contains("docker-compose/up"));
    assert!(content.contains("docker-compose/down"));
    assert!(content.contains("sqlx/migrate"));
    assert!(content.contains("sqlx/prepare"));
    assert!(content.contains("sqlx/check"));
    assert!(content.contains("sqlx/online"));
    assert!(content.contains("sqlx/offline"));
    assert!(content.contains("reset-db"));
    assert!(content.contains("test/infrastructure"));
    cleanup(&dir);
}

#[test]
fn db_project_has_cargo_config_with_sqlx_offline() {
    let dir = temp_dir("db_cargo_config");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_db_project("db-app", &dir).unwrap();
    let content = fs::read_to_string(output.join(".cargo/config.toml")).unwrap();
    assert!(content.contains("SQLX_OFFLINE"));
    cleanup(&dir);
}

#[test]
fn db_project_has_migrations_dir() {
    let dir = temp_dir("db_migrations");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_db_project("db-app", &dir).unwrap();
    assert!(output.join("infrastructure/migrations").is_dir());
    cleanup(&dir);
}

#[test]
fn db_project_has_db_rs_with_pool_function() {
    let dir = temp_dir("db_db_rs");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_db_project("db-app", &dir).unwrap();
    let content = fs::read_to_string(output.join("infrastructure/src/db.rs")).unwrap();
    assert!(content.contains("create_postgres_pool"));
    assert!(content.contains("PgPool"));
    cleanup(&dir);
}

#[test]
fn db_project_infrastructure_cargo_has_sqlx() {
    let dir = temp_dir("db_infra_cargo");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_db_project("db-app", &dir).unwrap();
    let content = fs::read_to_string(output.join("infrastructure/Cargo.toml")).unwrap();
    assert!(content.contains("sqlx"));
    assert!(content.contains("postgres"));
    cleanup(&dir);
}

#[test]
fn db_project_puerto_toml_has_project_db_true() {
    let dir = temp_dir("db_proj_toml");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_db_project("db-app", &dir).unwrap();
    let config = puerto_toml::read(&output).unwrap();
    assert!(
        config.project.db,
        "project.db should be true for --db projects"
    );
    cleanup(&dir);
}

#[test]
fn no_db_project_puerto_toml_omits_project_db() {
    let dir = temp_dir("no_db_proj_toml");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_project("plain-app", &dir).unwrap();
    let config = puerto_toml::read(&output).unwrap();
    assert!(
        !config.project.db,
        "project.db should be false for non-db projects"
    );
    let content = fs::read_to_string(output.join("puerto.toml")).unwrap();
    assert!(
        !content.contains("db = true"),
        "puerto.toml should not contain db = true for non-db projects"
    );
    cleanup(&dir);
}

// ── puerto generate scaffold --db ─────────────────────────────────────────

fn setup_db_puerto_stubs(base: &Path) {
    setup_puerto_stubs(base);
    fs::create_dir_all(base.join("infrastructure/migrations")).unwrap();
}

#[test]
fn scaffold_db_creates_entity_rs() {
    let dir = temp_dir("scaffold_db_entity");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_db_puerto_stubs(&dir);
    scaffold::run("Product", &dir, true, false, &[], &[]).unwrap();
    assert!(dir.join("infrastructure/src/product/entity.rs").exists());
    cleanup(&dir);
}

#[test]
fn scaffold_db_repository_uses_pgpool() {
    let dir = temp_dir("scaffold_db_repo");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_db_puerto_stubs(&dir);
    scaffold::run("Product", &dir, true, false, &[], &[]).unwrap();
    let content = fs::read_to_string(dir.join("infrastructure/src/product/repository.rs")).unwrap();
    assert!(content.contains("PgPool"));
    assert!(content.contains("PgProductRepository"));
    assert!(!content.contains("InMemoryProductRepository"));
    cleanup(&dir);
}

#[test]
fn scaffold_db_puerto_toml_has_db_true() {
    let dir = temp_dir("scaffold_db_toml");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_db_puerto_stubs(&dir);
    scaffold::run("Product", &dir, true, false, &[], &[]).unwrap();
    let content = fs::read_to_string(dir.join("puerto.toml")).unwrap();
    assert!(content.contains("db = true"));
    cleanup(&dir);
}

#[test]
fn scaffold_db_bootstrap_uses_pg_repo() {
    let dir = temp_dir("scaffold_db_bootstrap");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_db_puerto_stubs(&dir);
    scaffold::run("Product", &dir, true, false, &[], &[]).unwrap();
    let content = fs::read_to_string(dir.join("presentation/src/generated/bootstrap.rs")).unwrap();
    assert!(content.contains("PgProductRepository"));
    assert!(!content.contains("InMemoryProductRepository"));
    assert!(content.contains("InMemoryGreetingRepository"));
    cleanup(&dir);
}

#[test]
fn scaffold_without_db_still_uses_inmemory() {
    let dir = temp_dir("scaffold_no_db");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs(&dir);
    scaffold::run("Product", &dir, false, false, &[], &[]).unwrap();
    let content = fs::read_to_string(dir.join("infrastructure/src/product/repository.rs")).unwrap();
    assert!(content.contains("InMemoryProductRepository"));
    assert!(!content.contains("PgPool"));
    assert!(!dir.join("infrastructure/src/product/entity.rs").exists());
    cleanup(&dir);
}

// ── puerto generate migration ─────────────────────────────────────────────

#[test]
fn migration_errors_when_sqlx_not_in_path() {
    let dir = temp_dir("migration_no_sqlx");
    cleanup(&dir);
    fs::create_dir_all(dir.join("infrastructure/migrations")).unwrap();

    let result = scaffold::run_migration(
        "add_products_table",
        &dir,
        Some("nonexistent_sqlx_bin"),
        None,
    );
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("sqlx CLI not found"),
        "expected 'sqlx CLI not found' in: {msg}"
    );
    assert!(
        msg.contains("cargo install sqlx-cli"),
        "expected install hint in: {msg}"
    );

    cleanup(&dir);
}

#[test]
fn migration_creates_migrations_dir_when_missing() {
    let dir = temp_dir("migration_no_dir");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let _ = scaffold::run_migration("add_products_table", &dir, Some("/bin/true"), None);

    assert!(
        dir.join("infrastructure/migrations").exists(),
        "expected infrastructure/migrations to be created automatically"
    );

    cleanup(&dir);
}

#[test]
fn use_case_is_idempotent() {
    let dir = temp_dir("uc_idempotent");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_use_case_stubs(&dir);
    scaffold::run_use_case("Product", "delete_product", &dir).unwrap();
    scaffold::run_use_case("Product", "delete_product", &dir).unwrap();

    let toml = fs::read_to_string(dir.join("puerto.toml")).unwrap();
    assert_eq!(toml.matches("delete_product").count(), 1);

    let lib = fs::read_to_string(dir.join("business/src/lib.rs")).unwrap();
    assert_eq!(lib.matches("pub mod delete_product;").count(), 2);

    cleanup(&dir);
}

// ── Phase 5: logger ───────────────────────────────────────────────────────

#[test]
fn new_project_has_domain_logger_file() {
    let dir = temp_dir("logger_domain_file");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = generate_project("logger-test", &dir).unwrap();

    assert!(output.join("business/src/domain/logger.rs").exists());

    cleanup(&dir);
}

#[test]
fn new_project_has_infrastructure_logger_file() {
    let dir = temp_dir("logger_infra_file");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = generate_project("logger-test", &dir).unwrap();

    assert!(output.join("infrastructure/src/logger.rs").exists());

    cleanup(&dir);
}

#[test]
fn new_project_bootstrap_wires_tracing_logger() {
    let dir = temp_dir("logger_bootstrap");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = generate_project("logger-test", &dir).unwrap();

    let content =
        fs::read_to_string(output.join("presentation/src/generated/bootstrap.rs")).unwrap();
    assert!(content.contains("TracingLogger"));
    assert!(content.contains("let logger: Arc<dyn LoggerTrait> = Arc::new(TracingLogger)"));
    assert!(content.contains("Arc::clone(&logger)"));

    cleanup(&dir);
}

#[test]
fn scaffold_use_case_impl_has_logger_field() {
    let dir = temp_dir("logger_scaffold_impl");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    scaffold::run("Product", &dir, false, false, &[], &[]).unwrap();

    let content =
        fs::read_to_string(dir.join("business/src/application/product/create_product.rs")).unwrap();
    assert!(content.contains("pub logger: Arc<dyn LoggerTrait>"));
    assert!(content.contains("use crate::domain::logger::LoggerTrait"));

    cleanup(&dir);
}

#[test]
fn use_case_generator_impl_has_logger_field() {
    let dir = temp_dir("logger_uc_impl");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_use_case_stubs(&dir);
    scaffold::run_use_case("Product", "delete_product", &dir).unwrap();

    let content =
        fs::read_to_string(dir.join("business/src/application/product/delete_product.rs")).unwrap();
    assert!(content.contains("pub logger: Arc<dyn LoggerTrait>"));
    assert!(content.contains("use crate::domain::logger::LoggerTrait"));

    cleanup(&dir);
}

#[test]
fn scaffold_bootstrap_wires_logger_for_all_entities() {
    let dir = temp_dir("logger_bootstrap_scaffold");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs(&dir);
    scaffold::run("Product", &dir, false, false, &[], &[]).unwrap();

    let content = fs::read_to_string(dir.join("presentation/src/generated/bootstrap.rs")).unwrap();
    assert!(content.contains("TracingLogger"));
    assert!(content.contains("let logger: Arc<dyn LoggerTrait> = Arc::new(TracingLogger)"));
    assert_eq!(content.matches("Arc::clone(&logger)").count(), 6);

    cleanup(&dir);
}

// ── Phase 6: IDE snippets ─────────────────────────────────────────────────

#[test]
fn new_project_creates_zed_snippet_file() {
    let dir = temp_dir("snippets_zed_new");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = new_project_non_interactive(Some("snip-test".into()), false, &dir).unwrap();
    snippets::apply(&output, None).unwrap();

    assert!(output.join(".zed/snippets/rust.json").exists());

    cleanup(&dir);
}

#[test]
fn new_project_creates_vscode_snippet_file() {
    let dir = temp_dir("snippets_vscode_new");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = new_project_non_interactive(Some("snip-test".into()), false, &dir).unwrap();
    snippets::apply(&output, None).unwrap();

    assert!(output.join(".vscode/puerto.code-snippets").exists());

    cleanup(&dir);
}

#[test]
fn snippet_files_contain_valid_json() {
    let dir = temp_dir("snippets_valid_json");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    snippets::apply(&dir, None).unwrap();

    let zed = fs::read_to_string(dir.join(".zed/snippets/rust.json")).unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(&zed).is_ok(),
        "zed snippet JSON invalid"
    );

    let vscode = fs::read_to_string(dir.join(".vscode/puerto.code-snippets")).unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(&vscode).is_ok(),
        "vscode snippet JSON invalid"
    );

    let zed_sql = fs::read_to_string(dir.join(".zed/snippets/sql.json")).unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(&zed_sql).is_ok(),
        "zed sql snippet JSON invalid"
    );

    let vscode_sql = fs::read_to_string(dir.join(".vscode/puerto.sql.code-snippets")).unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(&vscode_sql).is_ok(),
        "vscode sql snippet JSON invalid"
    );

    cleanup(&dir);
}

#[test]
fn generate_snippets_ide_zed_creates_only_zed_file() {
    let dir = temp_dir("snippets_ide_zed");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    snippets::run(&dir, Some("zed")).unwrap();

    assert!(dir.join(".zed/snippets/rust.json").exists());
    assert!(!dir.join(".vscode/puerto.code-snippets").exists());

    cleanup(&dir);
}

#[test]
fn generate_snippets_ide_vscode_creates_only_vscode_file() {
    let dir = temp_dir("snippets_ide_vscode");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    snippets::run(&dir, Some("vscode")).unwrap();

    assert!(!dir.join(".zed/snippets/rust.json").exists());
    assert!(dir.join(".vscode/puerto.code-snippets").exists());

    cleanup(&dir);
}

#[test]
fn generate_snippets_is_idempotent() {
    let dir = temp_dir("snippets_idempotent");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    snippets::run(&dir, None).unwrap();
    snippets::run(&dir, None).unwrap();

    assert!(dir.join(".zed/snippets/rust.json").exists());
    assert!(dir.join(".vscode/puerto.code-snippets").exists());

    cleanup(&dir);
}

#[test]
fn generate_snippets_unknown_ide_returns_error() {
    let dir = temp_dir("snippets_unknown_ide");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let result = snippets::run(&dir, Some("neovim"));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("unknown IDE"));

    cleanup(&dir);
}

#[test]
fn snippets_json_contains_vo_prefixes() {
    let json: serde_json::Value =
        serde_json::from_str(snippets::SNIPPETS_JSON).expect("SNIPPETS_JSON must be valid JSON");
    let prefixes: Vec<&str> = json
        .as_object()
        .unwrap()
        .values()
        .filter_map(|v| v.get("prefix")?.as_str())
        .collect();

    for expected in &[
        "vo-string",
        "vo-numeric",
        "vo-enum",
        "vo-option-construct",
        "vo-vec-construct",
    ] {
        assert!(
            prefixes.contains(expected),
            "SNIPPETS_JSON missing prefix '{expected}'"
        );
    }
}

// ── puerto generate scaffold --crud ──────────────────────────────────────

fn setup_puerto_stubs_for_crud(base: &Path) {
    setup_puerto_stubs(base);
}

#[test]
fn scaffold_crud_creates_all_domain_use_case_files() {
    let dir = temp_dir("scaffold_crud_domain_uc");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs_for_crud(&dir);
    scaffold::run("Product", &dir, false, true, &[], &[]).unwrap();

    assert!(
        dir.join("business/src/domain/product/use_cases/create_product.rs")
            .exists()
    );
    assert!(
        dir.join("business/src/domain/product/use_cases/get_product.rs")
            .exists()
    );
    assert!(
        dir.join("business/src/domain/product/use_cases/list_product.rs")
            .exists()
    );
    assert!(
        dir.join("business/src/domain/product/use_cases/update_product.rs")
            .exists()
    );
    assert!(
        dir.join("business/src/domain/product/use_cases/delete_product.rs")
            .exists()
    );

    cleanup(&dir);
}

#[test]
fn scaffold_crud_creates_all_application_use_case_files() {
    let dir = temp_dir("scaffold_crud_app_uc");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs_for_crud(&dir);
    scaffold::run("Product", &dir, false, true, &[], &[]).unwrap();

    assert!(
        dir.join("business/src/application/product/create_product.rs")
            .exists()
    );
    assert!(
        dir.join("business/src/application/product/get_product.rs")
            .exists()
    );
    assert!(
        dir.join("business/src/application/product/list_product.rs")
            .exists()
    );
    assert!(
        dir.join("business/src/application/product/update_product.rs")
            .exists()
    );
    assert!(
        dir.join("business/src/application/product/delete_product.rs")
            .exists()
    );

    cleanup(&dir);
}

#[test]
fn scaffold_crud_repository_has_find_all() {
    let dir = temp_dir("scaffold_crud_repo");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs_for_crud(&dir);
    scaffold::run("Product", &dir, false, true, &[], &[]).unwrap();

    let content =
        fs::read_to_string(dir.join("business/src/domain/product/repository.rs")).unwrap();
    assert!(content.contains("async fn find_all"));
    assert!(content.contains("async fn find_by_id"));
    assert!(content.contains("async fn save"));

    cleanup(&dir);
}

#[test]
fn scaffold_crud_routes_has_all_http_methods() {
    let dir = temp_dir("scaffold_crud_routes");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs_for_crud(&dir);
    scaffold::run("Product", &dir, false, true, &[], &[]).unwrap();

    let content = fs::read_to_string(dir.join("presentation/src/api/product/routes.rs")).unwrap();
    assert!(content.contains("method = \"post\""));
    assert!(content.contains("method = \"get\""));
    assert!(content.contains("method = \"put\""));
    assert!(content.contains("method = \"delete\""));
    assert!(content.contains("pub struct ProductApi"));
    assert!(content.contains("pub create_product"));
    assert!(content.contains("pub get_product"));
    assert!(content.contains("pub list_product"));
    assert!(content.contains("pub update_product"));
    assert!(content.contains("pub delete_product"));

    cleanup(&dir);
}

#[test]
fn scaffold_crud_patches_business_lib_with_all_use_cases() {
    let dir = temp_dir("scaffold_crud_lib");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs_for_crud(&dir);
    scaffold::run("Product", &dir, false, true, &[], &[]).unwrap();

    let content = fs::read_to_string(dir.join("business/src/lib.rs")).unwrap();

    // Verify domain use_cases block exists
    assert!(content.contains("pub mod use_cases {"));
    assert!(content.contains("pub mod create_product;"));
    assert!(content.contains("pub mod get_product;"));
    assert!(content.contains("pub mod list_product;"));
    assert!(content.contains("pub mod update_product;"));
    assert!(content.contains("pub mod delete_product;"));

    // Regression: verify modules appear inside the application block, not only in domain.
    // A location-agnostic contains() check passes even when the idempotency guard fires too
    // early and leaves the application block empty.
    let app_start = content
        .find("pub mod application {")
        .expect("application block missing");
    let app_block = &content[app_start..];
    assert!(
        app_block.contains("pub mod product {"),
        "application block missing pub mod product"
    );
    let product_in_app = app_block.find("pub mod product {").unwrap();
    let product_block = &app_block[product_in_app..];
    assert!(
        product_block.contains("pub mod create_product;"),
        "application.product missing create_product"
    );
    assert!(
        product_block.contains("pub mod get_product;"),
        "application.product missing get_product"
    );
    assert!(
        product_block.contains("pub mod list_product;"),
        "application.product missing list_product"
    );
    assert!(
        product_block.contains("pub mod update_product;"),
        "application.product missing update_product"
    );
    assert!(
        product_block.contains("pub mod delete_product;"),
        "application.product missing delete_product"
    );

    cleanup(&dir);
}

#[test]
fn scaffold_crud_bootstrap_wires_all_use_cases() {
    let dir = temp_dir("scaffold_crud_bootstrap");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs_for_crud(&dir);
    fs::create_dir_all(dir.join("presentation/src/generated")).unwrap();
    fs::write(
        dir.join("presentation/src/generated/bootstrap.rs"),
        "// placeholder\n",
    )
    .unwrap();
    scaffold::run("Product", &dir, false, true, &[], &[]).unwrap();

    let content = fs::read_to_string(dir.join("presentation/src/generated/bootstrap.rs")).unwrap();
    assert!(content.contains("CreateProductUseCaseImpl"));
    assert!(content.contains("GetProductUseCaseImpl"));
    assert!(content.contains("ListProductUseCaseImpl"));
    assert!(content.contains("UpdateProductUseCaseImpl"));
    assert!(content.contains("DeleteProductUseCaseImpl"));

    cleanup(&dir);
}

// ── puerto generate domain / application / repository / presentation ─────

#[test]
fn generate_domain_creates_domain_files_and_mother() {
    let dir = temp_dir("gen_domain");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_project("myapp", &dir).unwrap();

    scaffold::run_generate_domain("Widget", &output).unwrap();

    assert!(output.join("business/src/domain/widget/model.rs").exists());
    assert!(output.join("business/src/domain/widget/errors.rs").exists());
    assert!(
        output
            .join("business/src/domain/widget/repository.rs")
            .exists()
    );
    assert!(
        output
            .join("business/src/domain/widget/use_cases/create_widget.rs")
            .exists()
    );
    assert!(
        output
            .join("business/src/domain/widget/use_cases/delete_widget.rs")
            .exists()
    );
    assert!(
        output
            .join("business/src/tests/mothers/widget_mother.rs")
            .exists()
    );

    let lib = fs::read_to_string(output.join("business/src/lib.rs")).unwrap();
    assert!(lib.contains("pub mod widget"));
    assert!(lib.contains("pub mod widget_mother;"));

    let toml = fs::read_to_string(output.join("puerto.toml")).unwrap();
    assert!(toml.contains("Widget"));

    cleanup(&dir);
}

#[test]
fn generate_application_creates_use_case_impls() {
    let dir = temp_dir("gen_application");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_project("myapp", &dir).unwrap();

    scaffold::run_generate_domain("Widget", &output).unwrap();
    scaffold::run_generate_application("Widget", &output).unwrap();

    assert!(
        output
            .join("business/src/application/widget/create_widget.rs")
            .exists()
    );
    assert!(
        output
            .join("business/src/application/widget/get_widget.rs")
            .exists()
    );
    assert!(
        output
            .join("business/src/application/widget/list_widget.rs")
            .exists()
    );
    assert!(
        output
            .join("business/src/application/widget/update_widget.rs")
            .exists()
    );
    assert!(
        output
            .join("business/src/application/widget/delete_widget.rs")
            .exists()
    );

    let lib = fs::read_to_string(output.join("business/src/lib.rs")).unwrap();
    assert!(lib.contains("pub mod create_widget;"));

    cleanup(&dir);
}

#[test]
fn generate_application_errors_without_prior_domain() {
    let dir = temp_dir("gen_app_no_domain");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_project("myapp", &dir).unwrap();

    let result = scaffold::run_generate_application("Ghost", &output);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("not found in puerto.toml")
    );

    cleanup(&dir);
}

#[test]
fn generate_repository_creates_infra_files() {
    let dir = temp_dir("gen_repository");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_project("myapp", &dir).unwrap();

    scaffold::run_generate_domain("Widget", &output).unwrap();
    scaffold::run_generate_repository("Widget", &output, None).unwrap();

    assert!(
        output
            .join("infrastructure/src/widget/repository.rs")
            .exists()
    );
    let repo = fs::read_to_string(output.join("infrastructure/src/widget/repository.rs")).unwrap();
    assert!(repo.contains("InMemoryWidgetRepository"));

    cleanup(&dir);
}

#[test]
fn generate_presentation_creates_all_files_and_regenerates_bootstrap() {
    let dir = temp_dir("gen_presentation");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_project("myapp", &dir).unwrap();

    scaffold::run_generate_domain("Widget", &output).unwrap();
    scaffold::run_generate_repository("Widget", &output, None).unwrap();
    scaffold::run_generate_presentation("Widget", &output).unwrap();

    assert!(output.join("presentation/src/api/widget.rs").exists());
    assert!(
        output
            .join("presentation/src/api/widget/routes.rs")
            .exists()
    );
    assert!(output.join("presentation/src/api/widget/dto.rs").exists());
    assert!(
        output
            .join("presentation/src/api/widget/responses.rs")
            .exists()
    );
    assert!(
        output
            .join("presentation/src/api/widget/error_mapper.rs")
            .exists()
    );

    let bootstrap =
        fs::read_to_string(output.join("presentation/src/generated/bootstrap.rs")).unwrap();
    assert!(bootstrap.contains("WidgetApi"));

    cleanup(&dir);
}

#[test]
fn generate_presentation_errors_without_prior_domain() {
    let dir = temp_dir("gen_pres_no_domain");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_project("myapp", &dir).unwrap();

    let result = scaffold::run_generate_presentation("Ghost", &output);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("not found in puerto.toml")
    );

    cleanup(&dir);
}

#[test]
fn generate_scaffold_includes_object_mother() {
    let dir = temp_dir("scaffold_with_mother");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    let output = generate_project("myapp", &dir).unwrap();

    scaffold::run("Widget", &output, false, true, &[], &[]).unwrap();

    assert!(
        output
            .join("business/src/tests/mothers/widget_mother.rs")
            .exists()
    );
    let mother =
        fs::read_to_string(output.join("business/src/tests/mothers/widget_mother.rs")).unwrap();
    assert!(mother.contains("WidgetMother"));
    assert!(mother.contains("pub fn random()"));

    cleanup(&dir);
}

// ── puerto list ──────────────────────────────────────────────────────────

#[test]
fn list_fails_outside_puerto_project() {
    let dir = temp_dir("list_no_toml");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let result = run_list(&dir);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("puerto.toml not found"));

    cleanup(&dir);
}

#[test]
fn list_succeeds_inside_puerto_project() {
    let dir = temp_dir("list_with_toml");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = generate_project("list-app", &dir).unwrap();
    run_list(&output).unwrap();

    cleanup(&dir);
}

// ── require_puerto_project ────────────────────────────────────────────────

#[test]
fn generate_scaffold_fails_outside_puerto_project() {
    let dir = temp_dir("gen_no_toml");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let result = require_puerto_project(&dir);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("puerto.toml not found")
    );

    cleanup(&dir);
}

// ── Phase 7.3: puerto new --no-demo ──────────────────────────────────────

#[test]
fn no_demo_has_no_greeting_files() {
    let dir = temp_dir("no_demo_files");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = new_project_non_interactive(Some("no-demo-app".into()), false, &dir).unwrap();
    scaffold::apply_no_demo(&output).unwrap();

    assert!(!output.join("business/src/domain/greeting").exists());
    assert!(!output.join("business/src/application/greeting").exists());
    assert!(!output.join("infrastructure/src/greeting").exists());
    assert!(!output.join("presentation/src/api/greeting").exists());
    assert!(!output.join("presentation/src/api/greeting.rs").exists());

    cleanup(&dir);
}

#[test]
fn no_demo_puerto_toml_has_no_entity_block() {
    let dir = temp_dir("no_demo_toml");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = new_project_non_interactive(Some("no-demo-app".into()), false, &dir).unwrap();
    scaffold::apply_no_demo(&output).unwrap();

    let content = fs::read_to_string(output.join("puerto.toml")).unwrap();
    assert!(
        !content.contains("[[entity]]"),
        "puerto.toml should have no entity blocks"
    );
    assert!(
        !content.contains("Greeting"),
        "puerto.toml should not mention Greeting"
    );
    assert!(
        content.contains("[project]"),
        "puerto.toml should still have [project]"
    );

    cleanup(&dir);
}

#[test]
fn no_demo_lib_files_have_no_greeting_modules() {
    let dir = temp_dir("no_demo_lib");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = new_project_non_interactive(Some("no-demo-app".into()), false, &dir).unwrap();
    scaffold::apply_no_demo(&output).unwrap();

    let biz_lib = fs::read_to_string(output.join("business/src/lib.rs")).unwrap();
    assert!(
        !biz_lib.contains("greeting"),
        "business lib.rs should not reference greeting"
    );
    assert!(
        biz_lib.contains("pub mod logger"),
        "business lib.rs should retain logger"
    );

    let infra_lib = fs::read_to_string(output.join("infrastructure/src/lib.rs")).unwrap();
    assert!(
        !infra_lib.contains("greeting"),
        "infra lib.rs should not reference greeting"
    );
    assert!(
        infra_lib.contains("pub mod logger"),
        "infra lib.rs should retain logger"
    );

    let api_rs = fs::read_to_string(output.join("presentation/src/api.rs")).unwrap();
    assert!(
        !api_rs.contains("greeting"),
        "api.rs should not reference greeting"
    );
    assert!(
        api_rs.contains("pub mod error"),
        "api.rs should retain error module"
    );

    cleanup(&dir);
}

#[test]
fn no_demo_bootstrap_has_no_greeting() {
    let dir = temp_dir("no_demo_bootstrap");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = new_project_non_interactive(Some("no-demo-app".into()), false, &dir).unwrap();
    scaffold::apply_no_demo(&output).unwrap();

    let content =
        fs::read_to_string(output.join("presentation/src/generated/bootstrap.rs")).unwrap();
    assert!(
        !content.contains("Greeting"),
        "bootstrap.rs should have no Greeting reference"
    );
    assert!(
        !content.contains("greeting"),
        "bootstrap.rs should have no greeting reference"
    );

    cleanup(&dir);
}

#[test]
fn default_project_still_has_greeting_files() {
    let dir = temp_dir("with_demo");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = new_project_non_interactive(Some("demo-app".into()), false, &dir).unwrap();

    assert!(
        output
            .join("business/src/domain/greeting/model.rs")
            .exists()
    );
    assert!(
        output
            .join("presentation/src/api/greeting/routes.rs")
            .exists()
    );

    cleanup(&dir);
}

#[test]
fn no_demo_then_scaffold_generates_new_entity() {
    let dir = temp_dir("no_demo_then_scaffold");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let output = new_project_non_interactive(Some("no-demo-app".into()), false, &dir).unwrap();
    scaffold::apply_no_demo(&output).unwrap();
    scaffold::run_scaffold("Player", &output, None, &[]).unwrap();

    assert!(output.join("business/src/domain/player/model.rs").exists());
    assert!(
        output
            .join("presentation/src/api/player/routes.rs")
            .exists()
    );
    assert!(!output.join("business/src/domain/greeting").exists());

    cleanup(&dir);
}

// ── Phase 7.4: scaffold infers db + always CRUD ───────────────────────────

fn setup_puerto_stubs_with_project_db(base: &Path) {
    fs::create_dir_all(base.join("business/src")).unwrap();
    fs::write(
        base.join("business/src/lib.rs"),
        "pub mod domain {\n  pub mod greeting {\n    pub mod errors;\n    pub mod model;\n    pub mod repository;\n    pub mod use_cases {\n      pub mod get_greeting;\n    }\n  }\n}\npub mod application {\n  pub mod greeting {\n    pub mod get_greeting;\n  }\n}\n",
    ).unwrap();
    fs::create_dir_all(base.join("infrastructure/src")).unwrap();
    fs::write(
        base.join("infrastructure/src/lib.rs"),
        "pub mod greeting {\n    pub mod repository;\n}\n",
    )
    .unwrap();
    fs::create_dir_all(base.join("presentation/src")).unwrap();
    fs::write(
        base.join("presentation/src/api.rs"),
        "pub mod error;\npub mod greeting;\n",
    )
    .unwrap();
    fs::write(
        base.join("puerto.toml"),
        "[project]\nname = \"test-app\"\ndb = true\n\n[[entity]]\nname = \"Greeting\"\nuse_cases = [\"get_greeting\"]\n",
    ).unwrap();
}

#[test]
fn scaffold_7_4_infers_db_from_project_toml() {
    let dir = temp_dir("scaffold_74_infer_db");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs_with_project_db(&dir);
    fs::create_dir_all(dir.join("infrastructure/migrations")).unwrap();
    scaffold::run_scaffold("Team", &dir, Some("/bin/true"), &[]).unwrap();

    let content = fs::read_to_string(dir.join("infrastructure/src/team/repository.rs")).unwrap();
    assert!(content.contains("PgTeamRepository"));
    assert!(!content.contains("InMemoryTeamRepository"));

    cleanup(&dir);
}

#[test]
fn scaffold_7_4_non_db_project_uses_inmemory() {
    let dir = temp_dir("scaffold_74_no_db");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs(&dir);
    scaffold::run_scaffold("Team", &dir, None, &[]).unwrap();

    let content = fs::read_to_string(dir.join("infrastructure/src/team/repository.rs")).unwrap();
    assert!(content.contains("InMemoryTeamRepository"));
    assert!(!content.contains("PgTeamRepository"));

    cleanup(&dir);
}

#[test]
fn scaffold_7_4_always_generates_5_use_cases() {
    let dir = temp_dir("scaffold_74_5_uc");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs(&dir);
    scaffold::run_scaffold("Team", &dir, None, &[]).unwrap();

    assert!(
        dir.join("business/src/domain/team/use_cases/create_team.rs")
            .exists()
    );
    assert!(
        dir.join("business/src/domain/team/use_cases/get_team.rs")
            .exists()
    );
    assert!(
        dir.join("business/src/domain/team/use_cases/list_team.rs")
            .exists()
    );
    assert!(
        dir.join("business/src/domain/team/use_cases/update_team.rs")
            .exists()
    );
    assert!(
        dir.join("business/src/domain/team/use_cases/delete_team.rs")
            .exists()
    );

    cleanup(&dir);
}

#[test]
fn scaffold_7_4_db_project_auto_creates_migration() {
    let dir = temp_dir("scaffold_74_migration");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs_with_project_db(&dir);
    scaffold::run_scaffold("Team", &dir, Some("/bin/true"), &[]).unwrap();

    assert!(
        dir.join("infrastructure/migrations").is_dir(),
        "run_migration should create infrastructure/migrations/ when project.db = true"
    );

    cleanup(&dir);
}

#[test]
fn scaffold_7_4_non_db_project_does_not_create_migrations_dir() {
    let dir = temp_dir("scaffold_74_no_migration");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs(&dir);
    scaffold::run_scaffold("Team", &dir, None, &[]).unwrap();

    assert!(
        !dir.join("infrastructure/migrations").exists(),
        "run_migration should NOT be called when project.db = false"
    );

    cleanup(&dir);
}

// ── puerto.toml fields ────────────────────────────────────────────────────

#[test]
fn parse_puerto_toml_with_fields() {
    let dir = temp_dir("toml_fields_parse");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("puerto.toml"),
        r#"[project]
name = "my-app"

[[entity]]
name = "Product"
use_cases = ["create_product"]

[[entity.fields]]
name = "name"
type = "String"

[[entity.fields]]
name = "price"
type = "i64"

[[entity.fields]]
name = "sku"
type = "String"
unique = true

[[entity.fields]]
name = "description"
type = "Option<String>"
"#,
    )
    .unwrap();

    let config = puerto_toml::read(&dir).unwrap();
    assert_eq!(config.entity.len(), 1);
    let product = &config.entity[0];
    assert_eq!(product.name, "Product");
    assert_eq!(product.fields.len(), 4);
    assert_eq!(product.fields[0].name, "name");
    assert_eq!(product.fields[0].field_type, "String");
    assert!(!product.fields[0].unique);
    assert_eq!(product.fields[1].name, "price");
    assert_eq!(product.fields[1].field_type, "i64");
    assert_eq!(product.fields[2].name, "sku");
    assert!(product.fields[2].unique);
    assert_eq!(product.fields[3].name, "description");
    assert_eq!(product.fields[3].field_type, "Option<String>");

    cleanup(&dir);
}

#[test]
fn parse_puerto_toml_without_fields_backwards_compat() {
    let dir = temp_dir("toml_no_fields");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("puerto.toml"),
        r#"[project]
name = "my-app"

[[entity]]
name = "Greeting"
use_cases = ["get_greeting"]
"#,
    )
    .unwrap();

    let config = puerto_toml::read(&dir).unwrap();
    assert_eq!(config.entity.len(), 1);
    let greeting = &config.entity[0];
    assert_eq!(greeting.name, "Greeting");
    assert!(greeting.fields.is_empty());

    cleanup(&dir);
}

#[test]
fn add_entity_with_fields() {
    let dir = temp_dir("toml_add_entity_fields");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("puerto.toml"),
        r#"[project]
name = "my-app"
"#,
    )
    .unwrap();

    let fields = vec![
        puerto_toml::Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        puerto_toml::Field {
            name: "quantity".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    puerto_toml::add_entity(
        &dir,
        "Product",
        vec!["create_product".into()],
        false,
        fields,
    )
    .unwrap();

    let config = puerto_toml::read(&dir).unwrap();
    assert_eq!(config.entity.len(), 1);
    assert_eq!(config.entity[0].fields.len(), 2);
    assert_eq!(config.entity[0].fields[0].name, "title");
    assert_eq!(config.entity[0].fields[0].field_type, "String");

    cleanup(&dir);
}

#[test]
fn type_registry_resolve_all_types() {
    use crate::generators::types::resolve_type;

    let valid_types = [
        "String",
        "i64",
        "bool",
        "f64",
        "Option<String>",
        "Option<i64>",
        "Option<bool>",
        "Option<f64>",
        "Uuid",
        "DateTime<Utc>",
        "Option<DateTime<Utc>>",
        "Vec<String>",
        "Vec<i64>",
        "HashMap<String, String>",
    ];
    for t in &valid_types {
        assert!(resolve_type(t).is_ok(), "expected '{t}' to resolve");
    }
}

#[test]
fn type_registry_rejects_unknown() {
    use crate::generators::types::resolve_type;
    let err = resolve_type("FooBar").unwrap_err();
    assert!(err.contains("unsupported field type 'FooBar'"));
}

// ── Phase 2: Domain model with custom fields ──────────────────────────────────

#[test]
fn generate_model_backward_compat_no_fields() {
    use crate::generators::domain::generate_model;

    let result = generate_model("Product", "product", &[], &[]);
    assert!(result.contains("pub struct ProductProps"));
    assert!(result.contains("pub name: String,"));
    assert!(result.contains("pub struct Product"));
    assert!(result.contains("pub id: Uuid,"));
    assert!(result.contains("pub name: String,"));
    assert!(result.contains("if props.name.trim().is_empty()"));
    assert!(result.contains("ProductError::ValidationError(\"name_empty\""));
    assert!(result.contains("should_create_product_when_name_is_valid"));
    assert!(result.contains("should_reject_product_when_name_is_empty"));
    assert!(result.contains("should_reject_product_when_name_is_only_whitespace"));
}

#[test]
fn generate_model_with_custom_fields() {
    use crate::generators::domain::generate_model;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "description".into(),
            field_type: "Option<String>".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_model("Product", "product", &fields, &[]);

    assert!(result.contains("pub struct ProductProps"));
    assert!(result.contains("pub title: String,"));
    assert!(result.contains("pub price: i64,"));
    assert!(result.contains("pub description: Option<String>,"));
    assert!(result.contains("pub struct Product"));
    assert!(result.contains("pub id: Uuid,"));
    assert!(result.contains("pub title: String,"));
    assert!(result.contains("pub price: i64,"));
    assert!(result.contains("pub description: Option<String>,"));
    assert!(result.contains("if props.title.trim().is_empty()"));
    assert!(result.contains("title_empty"));
    assert!(result.contains("should_create_product_when_fields_are_valid"));
    assert!(result.contains("should_reject_product_when_title_is_empty"));
    assert!(result.contains("should_reject_product_when_title_is_only_whitespace"));
    assert!(
        !result.contains("if props.price.trim().is_empty()"),
        "non-String types should not have empty checks"
    );
    assert!(
        !result.contains("if props.description.trim().is_empty()"),
        "Option<String> types should not have empty checks"
    );
}

#[test]
fn generate_model_with_uuid_field() {
    use crate::generators::domain::generate_model;
    use crate::puerto_toml::Field;

    let fields = vec![Field {
        name: "category_id".into(),
        field_type: "Uuid".into(),
        unique: false,
        ..Default::default()
    }];
    let result = generate_model("Item", "item", &fields, &[]);

    assert!(result.contains("use uuid::Uuid;"));
    assert!(result.contains("pub category_id: Uuid,"));
    assert!(result.contains("should_create_item_when_fields_are_valid"));
}

#[test]
fn generate_model_with_hashmap_field() {
    use crate::generators::domain::generate_model;
    use crate::puerto_toml::Field;

    let fields = vec![Field {
        name: "metadata".into(),
        field_type: "HashMap<String, String>".into(),
        unique: false,
        ..Default::default()
    }];
    let result = generate_model("Config", "config", &fields, &[]);

    assert!(result.contains("use std::collections::HashMap;"));
    assert!(result.contains("pub metadata: HashMap<String, String>,"));
}

#[test]
fn generate_model_multiple_string_fields_has_validations() {
    use crate::generators::domain::generate_model;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "name".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "sku".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_model("Product", "product", &fields, &[]);

    assert!(result.contains("if props.name.trim().is_empty()"));
    assert!(result.contains("name_empty"));
    assert!(result.contains("if props.sku.trim().is_empty()"));
    assert!(result.contains("sku_empty"));
    assert!(result.contains("should_reject_product_when_name_is_empty"));
    assert!(result.contains("should_reject_product_when_sku_is_empty"));
    assert!(
        !result.contains("if props.price.trim().is_empty()"),
        "non-String types should not have empty checks"
    );
}

#[test]
fn generate_mother_backward_compat_no_fields() {
    use crate::generators::domain::generate_mother;

    let result = generate_mother("Product", "product", &[], &[]);
    assert!(result.contains("pub struct ProductMother"));
    assert!(result.contains("name: Option<String>,"));
    assert!(result.contains("pub fn with_name(mut self, name: &str) -> Self"));
    assert!(result.contains("pub fn with_empty_name(mut self) -> Self"));
    assert!(result.contains("Product::new(ProductProps"));
    assert!(result.contains("self.name.unwrap_or_else(|| \"example\".to_string())"));
    assert!(result.contains("pub fn random()"));
    assert!(result.contains("pub fn random_vec"));
}

#[test]
fn generate_mother_with_custom_fields() {
    use crate::generators::domain::generate_mother;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "description".into(),
            field_type: "Option<String>".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_mother("Product", "product", &fields, &[]);

    assert!(result.contains("title: Option<String>,"));
    assert!(result.contains("price: Option<i64>,"));
    assert!(result.contains("description: Option<String>,"));
    assert!(result.contains("pub fn with_title(mut self, title: &str) -> Self"));
    assert!(result.contains("self.title = Some(title.to_string())"));
    assert!(result.contains("pub fn with_price(mut self, price: i64) -> Self"));
    assert!(result.contains("self.price = Some(price)"));
    assert!(result.contains("pub fn with_description(mut self, description: &str) -> Self"));
    assert!(result.contains("self.description = Some(description.to_string())"));
    assert!(result.contains("pub fn with_empty_title(mut self) -> Self"));
    assert!(result.contains("self.title = Some(String::new())"));
    assert!(result.contains("self.price.unwrap_or(42)"));
    assert!(result.contains("self.description"));
    assert!(result.contains("pub fn random()"));
    assert!(result.contains("pub fn random_vec"));
}

#[test]
fn scaffold_with_empty_fields_produces_backward_compat_model() {
    let dir = temp_dir("scaffold_empty_fields");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    scaffold::run("Product", &dir, false, false, &[], &[]).unwrap();

    let content = fs::read_to_string(dir.join("business/src/domain/product/model.rs")).unwrap();
    assert!(content.contains("pub struct ProductProps"));
    assert!(content.contains("pub name: String,"));
    assert!(content.contains("pub struct Product"));
    assert!(content.contains("ProductError::ValidationError(\"name_empty\""));
    assert!(content.contains("should_create_product_when_name_is_valid"));

    let mother =
        fs::read_to_string(dir.join("business/src/tests/mothers/product_mother.rs")).unwrap();
    assert!(mother.contains("pub struct ProductMother"));
    assert!(mother.contains("with_name"));

    cleanup(&dir);
}

#[test]
fn scaffold_crud_with_empty_fields_produces_backward_compat_model() {
    let dir = temp_dir("scaffold_crud_empty_fields");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    setup_puerto_stubs(&dir);
    scaffold::run("Product", &dir, false, true, &[], &[]).unwrap();

    let content = fs::read_to_string(dir.join("business/src/domain/product/model.rs")).unwrap();
    assert!(content.contains("pub struct ProductProps"));
    assert!(content.contains("pub name: String,"));

    let mother =
        fs::read_to_string(dir.join("business/src/tests/mothers/product_mother.rs")).unwrap();
    assert!(mother.contains("ProductMother"));
    assert!(mother.contains("with_name"));

    cleanup(&dir);
}

// ── Phase 3: Use case Params with custom fields ───────────────────────────────

#[test]
fn generate_create_use_case_trait_with_custom_fields() {
    use crate::generators::domain::generate_create_use_case_trait;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "description".into(),
            field_type: "Option<String>".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_create_use_case_trait("Product", "product", &fields);

    assert!(result.contains("pub struct CreateProductParams"));
    assert!(result.contains("pub title: String,"));
    assert!(result.contains("pub price: i64,"));
    assert!(result.contains("pub description: Option<String>,"));
    assert!(result.contains("pub trait CreateProductUseCaseTrait"));
    assert!(
        !result.contains("pub name: String,"),
        "should not contain hardcoded 'name' field"
    );
}

#[test]
fn generate_create_use_case_trait_backward_compat_no_fields() {
    use crate::generators::domain::generate_create_use_case_trait;

    let result = generate_create_use_case_trait("Product", "product", &[]);

    assert!(result.contains("pub struct CreateProductParams"));
    assert!(result.contains("pub name: String,"));
    assert!(result.contains("pub trait CreateProductUseCaseTrait"));
}

#[test]
fn generate_update_use_case_trait_with_custom_fields() {
    use crate::generators::domain::generate_update_use_case_trait;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_update_use_case_trait("Product", "product", &fields);

    assert!(result.contains("pub struct UpdateProductParams"));
    assert!(result.contains("pub id: Uuid,"));
    assert!(result.contains("pub title: String,"));
    assert!(result.contains("pub price: i64,"));
    assert!(result.contains("pub trait UpdateProductUseCaseTrait"));
}

#[test]
fn generate_update_use_case_trait_uuid_fields_excluded() {
    use crate::generators::domain::generate_update_use_case_trait;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "category_id".into(),
            field_type: "Uuid".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "name".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_update_use_case_trait("Item", "item", &fields);

    assert!(result.contains("pub name: String,"));
    assert!(result.contains("pub id: Uuid,"));
    assert!(
        !result.contains("pub category_id: Uuid,"),
        "Uuid custom field should not appear in UpdateParams"
    );
    assert!(result.contains("use uuid::Uuid;"));
}

#[test]
fn generate_create_use_case_impl_with_custom_fields() {
    use crate::generators::application::generate_create_use_case_impl;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_create_use_case_impl("Product", "product", &fields, &[]);

    assert!(result.contains("CreateProductUseCaseImpl"));
    assert!(result.contains("CreateProductParams"));
    assert!(result.contains("title: params.title,"));
    assert!(result.contains("price: params.price,"));
    assert!(result.contains("Product::new(ProductProps"));
    assert!(result.contains("Creating product: params.title"));
}

#[test]
fn generate_update_use_case_impl_with_custom_fields() {
    use crate::generators::application::generate_update_use_case_impl;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_update_use_case_impl("Product", "product", &fields, &[]);

    assert!(result.contains("UpdateProductUseCaseImpl"));
    assert!(result.contains("UpdateProductParams"));
    assert!(result.contains("entity.title = params.title;"));
    assert!(result.contains("entity.price = params.price;"));
    assert!(result.contains("if params.title.trim().is_empty()"));
    assert!(
        !result.contains("if params.price.trim().is_empty()"),
        "non-String types should not have empty validation"
    );
    assert!(result.contains("ProductError::ValidationError(\"title_empty\""));
}

#[test]
fn generate_update_use_case_impl_excludes_uuid_fields() {
    use crate::generators::application::generate_update_use_case_impl;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "category_id".into(),
            field_type: "Uuid".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "name".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_update_use_case_impl("Item", "item", &fields, &[]);

    assert!(result.contains("entity.name = params.name;"));
    assert!(
        !result.contains("entity.category_id = params.category_id;"),
        "Uuid fields should not be assigned in update"
    );
}

#[test]
fn scaffold_crud_writes_dynamic_use_case_params() {
    let dir = temp_dir("scaffold_crud_dynamic_params");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("puerto.toml"),
        r#"[project]
name = "test-app"

[[entity]]
name = "Product"
use_cases = ["create_product", "get_product", "list_product", "update_product", "delete_product"]

[[entity.fields]]
name = "name"
type = "String"

[[entity.fields]]
name = "price"
type = "i64"
"#,
    )
    .unwrap();

    let config = puerto_toml::read(&dir).unwrap();
    let fields = config.entity[0].fields.clone();

    let pascal = "Product";
    let snake = "product";

    let create_trait =
        crate::generators::domain::generate_create_use_case_trait(pascal, snake, &fields);
    assert!(create_trait.contains("pub name: String,"));
    assert!(create_trait.contains("pub price: i64,"));
    assert!(
        !create_trait.contains("pub id: Uuid,"),
        "Create params should not have id"
    );

    let update_trait =
        crate::generators::domain::generate_update_use_case_trait(pascal, snake, &fields);
    assert!(update_trait.contains("pub id: Uuid,"));
    assert!(update_trait.contains("pub name: String,"));
    assert!(update_trait.contains("pub price: i64,"));

    cleanup(&dir);
}

#[test]
fn write_application_files_with_fields_generates_dynamic_impls() {
    let dir = temp_dir("app_files_with_fields");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::create_dir_all(dir.join("business/src/application/product")).unwrap();

    let fields = vec![
        crate::puerto_toml::Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        crate::puerto_toml::Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    crate::generators::application::write_application_files(
        "Product",
        "product",
        &dir,
        &fields,
        &[],
    )
    .unwrap();

    let create_content =
        fs::read_to_string(dir.join("business/src/application/product/create_product.rs")).unwrap();
    assert!(create_content.contains("title: params.title,"));
    assert!(create_content.contains("price: params.price,"));
    assert!(create_content.contains("Product::new(ProductProps"));

    let update_content =
        fs::read_to_string(dir.join("business/src/application/product/update_product.rs")).unwrap();
    assert!(update_content.contains("entity.title = params.title;"));
    assert!(update_content.contains("entity.price = params.price;"));
    assert!(update_content.contains("if params.title.trim().is_empty()"));

    cleanup(&dir);
}

#[test]
fn generate_get_use_case_impl_with_custom_fields() {
    use crate::generators::application::generate_get_use_case_impl;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_get_use_case_impl("Product", "product", &fields, &[]);

    assert!(result.contains("GetProductUseCaseImpl"));
    assert!(result.contains("GetProductParams"));
    assert!(result.contains("should_return_product_when_id_exists"));
    assert!(result.contains("should_return_not_found_when_id_does_not_exist"));
    assert!(result.contains("Product::new(ProductProps"));
    assert!(result.contains("title: \"example\".to_string(),"));
    assert!(result.contains("price: 42,"));
    assert!(result.contains("silent_logger"));
}

#[test]
fn generate_get_use_case_impl_backward_compat_no_fields() {
    use crate::generators::application::generate_get_use_case_impl;

    let result = generate_get_use_case_impl("Product", "product", &[], &[]);

    assert!(result.contains("GetProductUseCaseImpl"));
    assert!(result.contains("Product::new(ProductProps"));
    assert!(result.contains("name: \"example\".to_string(),"));
    assert!(result.contains("should_return_product_when_id_exists"));
}

#[test]
fn generate_list_use_case_impl_with_custom_fields() {
    use crate::generators::application::generate_list_use_case_impl;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_list_use_case_impl("Product", "product", &fields, &[]);

    assert!(result.contains("ListProductUseCaseImpl"));
    assert!(result.contains("Product::new(ProductProps"));
    assert!(result.contains("should_return_all_products"));
    assert!(result.contains("should_return_empty_list_when_no_products_exist"));
    assert!(result.contains("title: \"first\".to_string(),"));
    assert!(result.contains("title: \"second\".to_string(),"));
    assert!(result.contains("price: 42,"));
    assert!(result.contains("silent_logger"));
}

#[test]
fn generate_delete_use_case_impl_with_custom_fields() {
    use crate::generators::application::generate_delete_use_case_impl;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_delete_use_case_impl("Product", "product", &fields, &[]);

    assert!(result.contains("DeleteProductUseCaseImpl"));
    assert!(result.contains("Product::new(ProductProps"));
    assert!(result.contains("should_soft_delete_product_when_id_exists"));
    assert!(result.contains("should_return_not_found_when_product_does_not_exist"));
    assert!(result.contains("title: \"example\".to_string(),"));
    assert!(result.contains("price: 42,"));
    assert!(result.contains("silent_logger"));
}

#[test]
fn generate_create_use_case_impl_with_custom_fields_has_tests() {
    use crate::generators::application::generate_create_use_case_impl;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_create_use_case_impl("Product", "product", &fields, &[]);

    assert!(result.contains("#[cfg(test)]"));
    assert!(result.contains("should_create_product_when_fields_are_valid"));
    assert!(result.contains("should_return_error_when_title_is_empty"));
    assert!(result.contains("silent_logger"));
    assert!(result.contains("MockProductRepository"));
    assert!(result.contains("title: \"example\".to_string(),"));
    assert!(result.contains("price: 42,"));
}

#[test]
fn generate_update_use_case_impl_with_custom_fields_has_tests() {
    use crate::generators::application::generate_update_use_case_impl;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_update_use_case_impl("Product", "product", &fields, &[]);

    assert!(result.contains("#[cfg(test)]"));
    assert!(result.contains("should_update_product_when_params_are_valid"));
    assert!(result.contains("should_return_not_found_when_product_does_not_exist"));
    assert!(result.contains("should_return_error_when_title_is_empty"));
    assert!(result.contains("Product::new(ProductProps"));
    assert!(result.contains("silent_logger"));
    assert!(result.contains("title: \"original\".to_string(),"));
    assert!(result.contains("title: \"updated\".to_string(),"));
    assert!(result.contains("price: 42,"));
}

#[test]
fn write_application_files_with_fields_generates_dynamic_get_list_delete() {
    let dir = temp_dir("app_files_dynamic_all");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::create_dir_all(dir.join("business/src/application/product")).unwrap();

    let fields = vec![
        crate::puerto_toml::Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        crate::puerto_toml::Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    crate::generators::application::write_application_files(
        "Product",
        "product",
        &dir,
        &fields,
        &[],
    )
    .unwrap();

    let get_content =
        fs::read_to_string(dir.join("business/src/application/product/get_product.rs")).unwrap();
    assert!(get_content.contains("GetProductUseCaseImpl"));
    assert!(get_content.contains("should_return_product_when_id_exists"));
    assert!(get_content.contains("Product::new(ProductProps"));
    assert!(get_content.contains("title: \"example\".to_string(),"));

    let list_content =
        fs::read_to_string(dir.join("business/src/application/product/list_product.rs")).unwrap();
    assert!(list_content.contains("ListProductUseCaseImpl"));
    assert!(list_content.contains("should_return_all_products"));
    assert!(list_content.contains("title: \"first\".to_string(),"));

    let delete_content =
        fs::read_to_string(dir.join("business/src/application/product/delete_product.rs")).unwrap();
    assert!(delete_content.contains("DeleteProductUseCaseImpl"));
    assert!(delete_content.contains("should_soft_delete_product_when_id_exists"));
    assert!(delete_content.contains("Product::new(ProductProps"));

    cleanup(&dir);
}

// ── Infrastructure generators ─────────────────────────────────────────────────

#[test]
fn generate_infra_entity_backward_compat() {
    use crate::generators::infrastructure::generate_infra_entity;

    let result = generate_infra_entity("Product", "product", &[], &[]);

    assert!(result.contains("pub struct ProductDb"));
    assert!(result.contains("pub id: Uuid"));
    assert!(result.contains("pub name: String"));
    assert!(result.contains("impl TryFrom<ProductDb> for Product"));
    assert!(result.contains("impl From<&Product> for ProductDb"));
    assert!(result.contains("name: row.name"));
    assert!(result.contains("name: entity.name.clone()"));
}

#[test]
fn generate_infra_entity_with_custom_fields() {
    use crate::generators::infrastructure::generate_infra_entity;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "active".into(),
            field_type: "Option<bool>".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_infra_entity("Product", "product", &fields, &[]);

    assert!(result.contains("pub struct ProductDb"));
    assert!(result.contains("pub title: String"));
    assert!(result.contains("pub price: i64"));
    assert!(result.contains("pub active: Option<bool>"));
    assert!(!result.contains("pub name: String"));
    assert!(result.contains("title: row.title"));
    assert!(result.contains("price: row.price"));
    assert!(result.contains("title: entity.title.clone()"));
    assert!(result.contains("price: entity.price,"));
}

#[test]
fn create_table_sql_backward_compat() {
    use crate::generators::infrastructure::create_table_sql;

    let result = create_table_sql("product", &[]);

    assert!(result.contains("CREATE TABLE products"));
    assert!(result.contains("id UUID PRIMARY KEY"));
    assert!(result.contains("name TEXT NOT NULL"));
    assert!(result.contains("created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()"));
    assert!(result.contains("deleted BOOLEAN NOT NULL DEFAULT FALSE"));
    assert!(result.contains("deleted_at TIMESTAMPTZ"));
}

#[test]
fn create_table_sql_with_custom_fields() {
    use crate::generators::infrastructure::create_table_sql;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "description".into(),
            field_type: "Option<String>".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "tags".into(),
            field_type: "Vec<String>".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = create_table_sql("product", &fields);

    assert!(result.contains("CREATE TABLE products"));
    assert!(result.contains("title TEXT NOT NULL"));
    assert!(result.contains("price BIGINT NOT NULL"));
    assert!(result.contains("description TEXT,"));
    assert!(result.contains("tags TEXT[] NOT NULL DEFAULT '{}'"));
    assert!(!result.contains("name TEXT"));
}

#[test]
fn generate_crud_infra_db_repository_backward_compat() {
    use crate::generators::infrastructure::generate_crud_infra_db_repository;

    let result = generate_crud_infra_db_repository("Product", "product", &[], &[]);

    assert!(result.contains("pub struct PgProductRepository"));
    assert!(result.contains("async fn find_all"));
    assert!(result.contains("async fn find_by_id"));
    assert!(result.contains("async fn save"));
    assert!(result.contains("id, created_at, updated_at, deleted, deleted_at, name"));
    assert!(result.contains("should_persist_and_retrieve_by_id"));
    assert!(result.contains("should_list_all_products_excluding_deleted"));
    assert!(result.contains("ProductProps"));
    assert!(result.contains("name: \"example\".to_string()"));
}

#[test]
fn generate_crud_infra_db_repository_with_custom_fields() {
    use crate::generators::infrastructure::generate_crud_infra_db_repository;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_crud_infra_db_repository("Product", "product", &fields, &[]);

    assert!(result.contains("id, created_at, updated_at, deleted, deleted_at, title, price"));
    assert!(result.contains("$1, $2, $3, $4, $5, $6, $7"));
    assert!(result.contains("title = $6, price = $7"));
    assert!(result.contains("db.title"));
    assert!(result.contains("db.price"));
    assert!(result.contains("title: \"example\".to_string()"));
    assert!(result.contains("price: 42"));
    assert!(result.contains("assert_eq!(found.title, entity.title)"));
    assert!(result.contains("assert_eq!(found.price, entity.price)"));
    assert!(result.contains("entity.title = \"updated\".to_string()"));
}

// ── Presentation generators ────────────────────────────────────────────────────

#[test]
fn generate_crud_dto_backward_compat() {
    use crate::generators::presentation::generate_crud_dto;

    let result = generate_crud_dto("Product", "product", &[]);

    assert!(result.contains("pub struct ProductDto"));
    assert!(result.contains("pub id: Uuid"));
    assert!(result.contains("pub name: String"));
    assert!(result.contains("pub fn from_domain"));
    assert!(result.contains("name: entity.name.clone()"));
    assert!(result.contains("pub struct CreateProductRequest"));
    assert!(result.contains("pub struct UpdateProductRequest"));
}

#[test]
fn generate_crud_dto_with_custom_fields() {
    use crate::generators::presentation::generate_crud_dto;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "description".into(),
            field_type: "Option<String>".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_crud_dto("Product", "product", &fields);

    assert!(result.contains("pub struct ProductDto"));
    assert!(result.contains("pub id: Uuid"));
    assert!(result.contains("pub title: String"));
    assert!(result.contains("pub price: i64"));
    assert!(result.contains("pub description: Option<String>"));
    assert!(!result.contains("pub name: String"));
    assert!(result.contains("title: entity.title.clone()"));
    assert!(result.contains("price: entity.price,"));
    assert!(result.contains("pub struct CreateProductRequest"));
    assert!(result.contains("pub struct UpdateProductRequest"));
}

#[test]
fn generate_crud_routes_with_custom_fields() {
    use crate::generators::presentation::generate_crud_routes;
    use crate::puerto_toml::Field;

    let fields = vec![
        Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    let result = generate_crud_routes("Product", "product", &fields);

    assert!(result.contains("method = \"post\""));
    assert!(result.contains("method = \"get\""));
    assert!(result.contains("method = \"put\""));
    assert!(result.contains("method = \"delete\""));
    assert!(result.contains("pub struct ProductApi"));
    assert!(result.contains("title: body.title.clone()"));
    assert!(result.contains("price: body.price,"));
    assert!(result.contains("id: id.0,"));
}

#[test]
fn write_repository_files_with_fields_generates_dynamic_entity_and_repo() {
    let dir = temp_dir("repo_files_dynamic");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let fields = vec![
        crate::puerto_toml::Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        crate::puerto_toml::Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    crate::generators::infrastructure::write_repository_files(
        "Product",
        "product",
        &dir,
        true,
        &fields,
        &[],
    )
    .unwrap();

    let entity = fs::read_to_string(dir.join("infrastructure/src/product/entity.rs")).unwrap();
    assert!(entity.contains("pub title: String"));
    assert!(entity.contains("pub price: i64"));
    assert!(!entity.contains("pub name: String"));

    let repo = fs::read_to_string(dir.join("infrastructure/src/product/repository.rs")).unwrap();
    assert!(repo.contains("title, price"));
    assert!(repo.contains("PgProductRepository"));

    cleanup(&dir);
}

#[test]
fn write_presentation_files_with_fields_generates_dynamic_dto_and_routes() {
    let dir = temp_dir("pres_files_dynamic");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    let fields = vec![
        crate::puerto_toml::Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        crate::puerto_toml::Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    crate::generators::presentation::write_presentation_files("Product", "product", &dir, &fields)
        .unwrap();

    let dto = fs::read_to_string(dir.join("presentation/src/api/product/dto.rs")).unwrap();
    assert!(dto.contains("pub title: String"));
    assert!(dto.contains("pub price: i64"));
    assert!(!dto.contains("pub name: String"));

    let routes = fs::read_to_string(dir.join("presentation/src/api/product/routes.rs")).unwrap();
    assert!(routes.contains("title: body.title.clone()"));
    assert!(routes.contains("price: body.price,"));

    cleanup(&dir);
}

// ── Phase 7: CLI field arguments ──────────────────────────────────────────────

#[test]
fn parse_field_arg_basic() {
    let field = puerto_toml::parse_field_arg("title:String").unwrap();
    assert_eq!(field.name, "title");
    assert_eq!(field.field_type, "String");
    assert!(!field.unique);
}

#[test]
fn parse_field_arg_with_unique() {
    let field = puerto_toml::parse_field_arg("sku:String!").unwrap();
    assert_eq!(field.name, "sku");
    assert_eq!(field.field_type, "String");
    assert!(field.unique);
}

#[test]
fn parse_field_arg_option_primitive() {
    let field = puerto_toml::parse_field_arg("description:opt:String").unwrap();
    assert_eq!(field.name, "description");
    assert_eq!(field.field_type, "Option<String>");
    assert!(!field.unique);
    assert!(field.value_object.is_none());
}

#[test]
fn parse_field_arg_vec_primitive() {
    let field = puerto_toml::parse_field_arg("tags:vec:String").unwrap();
    assert_eq!(field.name, "tags");
    assert_eq!(field.field_type, "Vec<String>");
    assert!(field.value_object.is_none());
}

#[test]
fn parse_field_arg_map_shorthand() {
    let field = puerto_toml::parse_field_arg("metadata:map").unwrap();
    assert_eq!(field.name, "metadata");
    assert_eq!(field.field_type, "HashMap<String, String>");
}

#[test]
fn parse_field_arg_datetime_shorthand() {
    let field = puerto_toml::parse_field_arg("created_at:DateTime").unwrap();
    assert_eq!(field.name, "created_at");
    assert_eq!(field.field_type, "DateTime<Utc>");
}

#[test]
fn parse_field_arg_string_vo() {
    let field = puerto_toml::parse_field_arg("name:Name:String").unwrap();
    assert_eq!(field.name, "name");
    assert_eq!(field.field_type, "String");
    assert_eq!(field.value_object, Some("Name".to_string()));
    assert!(!field.unique);
}

#[test]
fn parse_field_arg_numeric_vo() {
    let field = puerto_toml::parse_field_arg("age:Age:i64").unwrap();
    assert_eq!(field.name, "age");
    assert_eq!(field.field_type, "i64");
    assert_eq!(field.value_object, Some("Age".to_string()));
}

#[test]
fn parse_field_arg_float_vo() {
    let field = puerto_toml::parse_field_arg("height:Height:f64").unwrap();
    assert_eq!(field.name, "height");
    assert_eq!(field.field_type, "f64");
    assert_eq!(field.value_object, Some("Height".to_string()));
}

#[test]
fn parse_field_arg_unique_vo() {
    let field = puerto_toml::parse_field_arg("sku:Sku:String!").unwrap();
    assert_eq!(field.name, "sku");
    assert_eq!(field.field_type, "String");
    assert_eq!(field.value_object, Some("Sku".to_string()));
    assert!(field.unique);
}

#[test]
fn parse_field_arg_option_vo() {
    let field = puerto_toml::parse_field_arg("middle_name:MiddleName:opt:String").unwrap();
    assert_eq!(field.name, "middle_name");
    assert_eq!(field.field_type, "Option<String>");
    assert_eq!(field.value_object, Some("MiddleName".to_string()));
    assert!(field.value_object_kind.is_none());
}

#[test]
fn parse_field_arg_vec_vo() {
    let field = puerto_toml::parse_field_arg("tags:Tag:vec:String").unwrap();
    assert_eq!(field.name, "tags");
    assert_eq!(field.field_type, "Vec<String>");
    assert_eq!(field.value_object, Some("Tag".to_string()));
}

#[test]
fn parse_field_arg_enum_vo() {
    let field =
        puerto_toml::parse_field_arg("status:Status:enum:Pending/Confirmed/Cancelled").unwrap();
    assert_eq!(field.name, "status");
    assert_eq!(field.field_type, "String");
    assert_eq!(field.value_object, Some("Status".to_string()));
    assert_eq!(field.value_object_kind, Some("enum".to_string()));
    assert_eq!(
        field.enum_variants,
        Some(vec![
            "Pending".to_string(),
            "Confirmed".to_string(),
            "Cancelled".to_string()
        ])
    );
}

#[test]
fn parse_field_arg_vo_with_datetime_shorthand() {
    let field = puerto_toml::parse_field_arg("occurred:OccurredAt:DateTime").unwrap();
    assert_eq!(field.field_type, "DateTime<Utc>");
    assert_eq!(field.value_object, Some("OccurredAt".to_string()));
}

#[test]
fn parse_field_arg_option_vo_with_datetime_shorthand() {
    let field = puerto_toml::parse_field_arg("scheduled:ScheduledAt:opt:DateTime").unwrap();
    assert_eq!(field.field_type, "Option<DateTime<Utc>>");
    assert_eq!(field.value_object, Some("ScheduledAt".to_string()));
}

#[test]
fn parse_field_arg_rejects_unknown_keyword() {
    let err = puerto_toml::parse_field_arg("tags:unknown:String").unwrap_err();
    assert!(err.contains("not a known keyword") || err.contains("PascalCase VO name"));
}

#[test]
fn parse_field_arg_rejects_empty_enum_variants() {
    let err = puerto_toml::parse_field_arg("status:Status:enum:").unwrap_err();
    assert!(err.contains("Enum variants cannot be empty") || err.contains("PascalCase"));
}

#[test]
fn parse_field_arg_rejects_lowercase_vo_name() {
    let err = puerto_toml::parse_field_arg("name:myName:String").unwrap_err();
    assert!(err.contains("PascalCase") || err.contains("not a known keyword"));
}

#[test]
fn parse_field_arg_uuid() {
    let field = puerto_toml::parse_field_arg("category_id:Uuid").unwrap();
    assert_eq!(field.name, "category_id");
    assert_eq!(field.field_type, "Uuid");
}

#[test]
fn parse_field_arg_uuid_unique() {
    let field = puerto_toml::parse_field_arg("id:Uuid!").unwrap();
    assert_eq!(field.name, "id");
    assert_eq!(field.field_type, "Uuid");
    assert!(field.unique);
}

#[test]
fn parse_field_arg_rejects_no_colon() {
    let err = puerto_toml::parse_field_arg("titleString").unwrap_err();
    assert!(err.contains("invalid field argument"));
    assert!(err.contains("Expected format: name:Type"));
}

#[test]
fn parse_field_arg_rejects_empty_type() {
    let err = puerto_toml::parse_field_arg("title:").unwrap_err();
    assert!(err.contains("Type cannot be empty"));
}

#[test]
fn parse_field_arg_rejects_uppercase_name() {
    let err = puerto_toml::parse_field_arg("Title:String").unwrap_err();
    assert!(err.contains("invalid field name"));
    assert!(err.contains("snake_case"));
}

#[test]
fn parse_field_arg_rejects_name_starting_with_digit() {
    let err = puerto_toml::parse_field_arg("1title:String").unwrap_err();
    assert!(err.contains("invalid field name"));
}

#[test]
fn parse_field_arg_accepts_underscore_name() {
    let field = puerto_toml::parse_field_arg("_private:String").unwrap();
    assert_eq!(field.name, "_private");
}

#[test]
fn parse_field_arg_accepts_name_with_digits() {
    let field = puerto_toml::parse_field_arg("address2:String").unwrap();
    assert_eq!(field.name, "address2");
}

#[test]
fn validate_cli_fields_against_type_registry() {
    let fields = vec![
        puerto_toml::Field {
            name: "title".into(),
            field_type: "String".into(),
            unique: false,
            ..Default::default()
        },
        puerto_toml::Field {
            name: "price".into(),
            field_type: "i64".into(),
            unique: false,
            ..Default::default()
        },
    ];
    assert!(crate::generators::types::validate_fields(&fields).is_ok());
}

#[test]
fn validate_cli_fields_rejects_unknown_type() {
    let fields = vec![puerto_toml::Field {
        name: "title".into(),
        field_type: "UnknownType".into(),
        unique: false,
        ..Default::default()
    }];
    assert!(crate::generators::types::validate_fields(&fields).is_err());
}

// ── Phase 8: puerto validate ─────────────────────────────────────────────────

#[test]
fn validate_valid_config() {
    let dir = temp_dir("validate_valid");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("puerto.toml"),
        r#"[project]
name = "my-app"

[[entity]]
name = "Product"
use_cases = ["create_product", "get_product"]

[[entity.fields]]
name = "title"
type = "String"

[[entity.fields]]
name = "price"
type = "i64"
"#,
    )
    .unwrap();

    let result = validate::run_validate(&dir);
    assert!(result.is_ok());

    cleanup(&dir);
}

#[test]
fn validate_rejects_unknown_field_type() {
    let dir = temp_dir("validate_bad_type");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("puerto.toml"),
        r#"[project]
name = "my-app"

[[entity]]
name = "Product"
use_cases = ["create_product"]

[[entity.fields]]
name = "title"
type = "FancyType"
"#,
    )
    .unwrap();

    let result = validate::run_validate(&dir);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("unsupported field type 'FancyType'"),
        "expected type error, got: {err}"
    );

    cleanup(&dir);
}

#[test]
fn validate_rejects_invalid_entity_name() {
    let dir = temp_dir("validate_bad_entity");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("puerto.toml"),
        r#"[project]
name = "my-app"

[[entity]]
name = "lowercase_product"
use_cases = ["create_product"]
"#,
    )
    .unwrap();

    let result = validate::run_validate(&dir);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("PascalCase"),
        "expected PascalCase error, got: {err}"
    );

    cleanup(&dir);
}

#[test]
fn validate_rejects_invalid_use_case_name() {
    let dir = temp_dir("validate_bad_usecase");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("puerto.toml"),
        r#"[project]
name = "my-app"

[[entity]]
name = "Product"
use_cases = ["CreateProduct"]
"#,
    )
    .unwrap();

    let result = validate::run_validate(&dir);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("snake_case"),
        "expected snake_case error, got: {err}"
    );

    cleanup(&dir);
}

#[test]
fn validate_rejects_duplicate_field_name() {
    let dir = temp_dir("validate_dup_field");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("puerto.toml"),
        r#"[project]
name = "my-app"

[[entity]]
name = "Product"
use_cases = ["create_product"]

[[entity.fields]]
name = "title"
type = "String"

[[entity.fields]]
name = "title"
type = "i64"
"#,
    )
    .unwrap();

    let result = validate::run_validate(&dir);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("duplicate field name 'title'"),
        "expected duplicate error, got: {err}"
    );

    cleanup(&dir);
}

#[test]
fn validate_warns_on_option_unique_field() {
    let dir = temp_dir("validate_opt_unique");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("puerto.toml"),
        r#"[project]
name = "my-app"

[[entity]]
name = "Product"
use_cases = ["create_product"]

[[entity.fields]]
name = "email"
type = "Option<String>"
unique = true
"#,
    )
    .unwrap();

    let result = validate::run_validate(&dir);
    assert!(result.is_ok());

    cleanup(&dir);
}

#[test]
fn validate_rejects_duplicate_entity_name() {
    let dir = temp_dir("validate_dup_entity");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("puerto.toml"),
        r#"[project]
name = "my-app"

[[entity]]
name = "Product"
use_cases = ["create_product"]

[[entity]]
name = "Product"
use_cases = ["get_product"]
"#,
    )
    .unwrap();

    let result = validate::run_validate(&dir);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("duplicate entity name 'Product'"),
        "expected duplicate entity error, got: {err}"
    );

    cleanup(&dir);
}

#[test]
fn validate_rejects_invalid_field_name() {
    let dir = temp_dir("validate_bad_field_name");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("puerto.toml"),
        r#"[project]
name = "my-app"

[[entity]]
name = "Product"
use_cases = ["create_product"]

[[entity.fields]]
name = "BadName"
type = "String"
"#,
    )
    .unwrap();

    let result = validate::run_validate(&dir);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("snake_case"),
        "expected snake_case error, got: {err}"
    );

    cleanup(&dir);
}

// ── Phase 3.10: Shared VO tests ───────────────────────────────────────────────

#[test]
fn is_shared_vo_detection() {
    use crate::generators::types::is_shared_vo;
    use crate::puerto_toml::{Field, ValueObjectDefinition};

    let shared_vos = vec![ValueObjectDefinition {
        name: "Email".to_string(),
        inner_type: "String".to_string(),
    }];

    let email_field = Field {
        name: "email".into(),
        field_type: "String".into(),
        value_object: Some("Email".to_string()),
        ..Default::default()
    };
    let local_field = Field {
        name: "name".into(),
        field_type: "String".into(),
        value_object: Some("Name".to_string()),
        ..Default::default()
    };
    let primitive_field = Field {
        name: "age".into(),
        field_type: "i64".into(),
        ..Default::default()
    };

    assert!(is_shared_vo(&email_field, &shared_vos));
    assert!(!is_shared_vo(&local_field, &shared_vos));
    assert!(!is_shared_vo(&primitive_field, &shared_vos));
}

#[test]
fn generate_shared_value_objects_string() {
    use crate::generators::domain::generate_shared_value_objects;
    use crate::puerto_toml::ValueObjectDefinition;

    let vos = vec![ValueObjectDefinition {
        name: "Email".to_string(),
        inner_type: "String".to_string(),
    }];

    let result = generate_shared_value_objects(&vos);
    assert!(result.contains("pub struct Email"));
    assert!(result.contains("fn new(value: String)"));
    assert!(result.contains("fn value(&self) -> &str"));
    assert!(result.contains("EmailError::Invalid"));
    assert!(result.contains("trimmed.is_empty()"));
}

#[test]
fn generate_shared_value_objects_numeric() {
    use crate::generators::domain::generate_shared_value_objects;
    use crate::puerto_toml::ValueObjectDefinition;

    let vos = vec![ValueObjectDefinition {
        name: "Amount".to_string(),
        inner_type: "i64".to_string(),
    }];

    let result = generate_shared_value_objects(&vos);
    assert!(result.contains("pub struct Amount(i64)"));
    assert!(result.contains("fn new(value: i64)"));
    assert!(result.contains("fn value(&self) -> i64"));
}

#[test]
fn generate_shared_errors_combined_output() {
    use crate::generators::domain::generate_shared_errors_combined;
    use crate::puerto_toml::ValueObjectDefinition;

    let vos = vec![
        ValueObjectDefinition {
            name: "Email".to_string(),
            inner_type: "String".to_string(),
        },
        ValueObjectDefinition {
            name: "Money".to_string(),
            inner_type: "i64".to_string(),
        },
    ];

    let result = generate_shared_errors_combined(&vos);
    assert!(result.contains("pub enum EmailError"));
    assert!(result.contains("pub enum MoneyError"));
    assert!(result.contains("shared.value_object.email.invalid"));
    assert!(result.contains("shared.value_object.money.invalid"));
}

#[test]
fn generate_model_with_shared_vo_imports() {
    use crate::generators::domain::generate_model;
    use crate::puerto_toml::{Field, ValueObjectDefinition};

    let shared_vos = vec![ValueObjectDefinition {
        name: "Email".to_string(),
        inner_type: "String".to_string(),
    }];

    let fields = vec![Field {
        name: "email".into(),
        field_type: "String".into(),
        value_object: Some("Email".to_string()),
        ..Default::default()
    }];

    let result = generate_model("User", "user", &fields, &shared_vos);
    assert!(result.contains("use crate::domain::shared::value_objects::Email"));
    assert!(result.contains("pub email: Email"));
}

#[test]
fn generate_model_local_vo_uses_local_import() {
    use crate::generators::domain::generate_model;
    use crate::puerto_toml::{Field, ValueObjectDefinition};

    let shared_vos = vec![ValueObjectDefinition {
        name: "Email".to_string(),
        inner_type: "String".to_string(),
    }];

    let fields = vec![Field {
        name: "name".into(),
        field_type: "String".into(),
        value_object: Some("Name".to_string()),
        ..Default::default()
    }];

    let result = generate_model("User", "user", &fields, &shared_vos);
    assert!(result.contains("use super::value_objects::Name"));
    assert!(!result.contains("shared::value_objects::Name"));
}

#[test]
fn generate_create_use_case_shared_vo() {
    use crate::generators::application::generate_create_use_case_impl;
    use crate::puerto_toml::{Field, ValueObjectDefinition};

    let shared_vos = vec![ValueObjectDefinition {
        name: "Email".to_string(),
        inner_type: "String".to_string(),
    }];

    let fields = vec![Field {
        name: "email".into(),
        field_type: "String".into(),
        value_object: Some("Email".to_string()),
        ..Default::default()
    }];

    let result = generate_create_use_case_impl("User", "user", &fields, &shared_vos);
    assert!(result.contains("use crate::domain::shared::value_objects::Email"));
    assert!(result.contains(".map_err(|_| UserError::InvalidEmail)?"));
}

#[test]
fn generate_infra_entity_shared_vo_import_path() {
    use crate::generators::infrastructure::generate_infra_entity;
    use crate::puerto_toml::{Field, ValueObjectDefinition};

    let shared_vos = vec![ValueObjectDefinition {
        name: "Email".to_string(),
        inner_type: "String".to_string(),
    }];

    let fields = vec![Field {
        name: "email".into(),
        field_type: "String".into(),
        value_object: Some("Email".to_string()),
        ..Default::default()
    }];

    let result = generate_infra_entity("User", "user", &fields, &shared_vos);
    assert!(result.contains("use business::domain::shared::value_objects::Email"));
    assert!(!result.contains("use business::domain::user::value_objects::Email"));
}

#[test]
fn generate_infra_entity_shared_vo_try_from_map_err() {
    use crate::generators::infrastructure::generate_infra_entity;
    use crate::puerto_toml::{Field, ValueObjectDefinition};

    let shared_vos = vec![ValueObjectDefinition {
        name: "Email".to_string(),
        inner_type: "String".to_string(),
    }];

    let fields = vec![Field {
        name: "email".into(),
        field_type: "String".into(),
        value_object: Some("Email".to_string()),
        ..Default::default()
    }];

    let result = generate_infra_entity("User", "user", &fields, &shared_vos);
    assert!(result.contains(".map_err(|_| UserError::InvalidEmail)?"));
}

#[test]
fn generate_infra_entity_local_vo_uses_plain_question_mark() {
    use crate::generators::infrastructure::generate_infra_entity;
    use crate::puerto_toml::{Field, ValueObjectDefinition};

    let shared_vos: Vec<ValueObjectDefinition> = vec![];

    let fields = vec![Field {
        name: "name".into(),
        field_type: "String".into(),
        value_object: Some("Name".to_string()),
        ..Default::default()
    }];

    let result = generate_infra_entity("Product", "product", &fields, &shared_vos);
    assert!(result.contains("Name::new(row.name)?"));
    assert!(!result.contains(".map_err"));
}

#[test]
fn patch_business_lib_shared_adds_mod_declaration() {
    use crate::patchers::lib_rs::patch_business_lib_shared;

    let dir = temp_dir("patch_lib_shared");
    cleanup(&dir);
    fs::create_dir_all(dir.join("business/src")).unwrap();

    let lib_content = r#"pub mod domain {
    pub mod greeting {
        pub mod model;
    }
}
pub mod application {
}
"#;
    fs::write(dir.join("business/src/lib.rs"), lib_content).unwrap();

    patch_business_lib_shared(&dir).unwrap();

    let result = fs::read_to_string(dir.join("business/src/lib.rs")).unwrap();
    assert!(result.contains("pub mod shared;"));

    // Idempotent
    patch_business_lib_shared(&dir).unwrap();
    let count = result.matches("pub mod shared;").count();
    assert_eq!(count, 1);

    cleanup(&dir);
}

#[test]
fn scaffold_with_shared_vo_writes_shared_files() {
    let dir = temp_dir("scaffold_shared_vo");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("puerto.toml"),
        r#"[project]
name = "my-app"

[[value_object]]
name = "Email"
inner_type = "String"
"#,
    )
    .unwrap();

    scaffold::run(
        "User",
        &dir,
        false,
        true,
        &[puerto_toml::Field {
            name: "email".into(),
            field_type: "String".into(),
            value_object: Some("Email".to_string()),
            ..Default::default()
        }],
        &[crate::puerto_toml::ValueObjectDefinition {
            name: "Email".to_string(),
            inner_type: "String".to_string(),
        }],
    )
    .unwrap();

    assert!(
        dir.join("business/src/domain/shared/value_objects.rs")
            .exists()
    );
    assert!(dir.join("business/src/domain/shared/errors.rs").exists());
    assert!(dir.join("business/src/domain/shared/mod.rs").exists());

    cleanup(&dir);
}

#[test]
fn scaffold_with_all_shared_vos_does_not_patch_local_value_objects_mod() {
    let dir = temp_dir("scaffold_shared_vo_no_local");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();

    fs::write(dir.join("puerto.toml"), "[project]\nname = \"my-app\"\n").unwrap();

    scaffold::run(
        "Customer",
        &dir,
        false,
        true,
        &[puerto_toml::Field {
            name: "email".into(),
            field_type: "String".into(),
            value_object: Some("Email".to_string()),
            ..Default::default()
        }],
        &[ValueObjectDefinition {
            name: "Email".to_string(),
            inner_type: "String".to_string(),
        }],
    )
    .unwrap();

    // No local value_objects.rs should exist — all VOs are shared
    assert!(
        !dir.join("business/src/domain/customer/value_objects.rs")
            .exists(),
        "local value_objects.rs must not be created when all VOs are shared"
    );
    // Shared files must still be written
    assert!(
        dir.join("business/src/domain/shared/value_objects.rs")
            .exists()
    );

    cleanup(&dir);
}

// ── puerto generate value-object ─────────────────────────────────────────────

#[test]
fn value_object_adds_entry_to_puerto_toml() {
    let dir = temp_dir("vo_add_entry");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("puerto.toml"), "[project]\nname = \"my-app\"\n").unwrap();

    puerto_toml::add_value_object(&dir, "Email", "String").unwrap();

    let config = puerto_toml::read(&dir).unwrap();
    assert_eq!(config.value_object.len(), 1);
    assert_eq!(config.value_object[0].name, "Email");
    assert_eq!(config.value_object[0].inner_type, "String");

    cleanup(&dir);
}

#[test]
fn value_object_add_is_idempotent() {
    let dir = temp_dir("vo_idempotent");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("puerto.toml"), "[project]\nname = \"my-app\"\n").unwrap();

    puerto_toml::add_value_object(&dir, "Email", "String").unwrap();
    puerto_toml::add_value_object(&dir, "Email", "String").unwrap();

    let config = puerto_toml::read(&dir).unwrap();
    assert_eq!(config.value_object.len(), 1, "duplicate entry added");

    cleanup(&dir);
}

#[test]
fn value_object_multiple_entries_preserved() {
    let dir = temp_dir("vo_multiple");
    cleanup(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("puerto.toml"), "[project]\nname = \"my-app\"\n").unwrap();

    puerto_toml::add_value_object(&dir, "Email", "String").unwrap();
    puerto_toml::add_value_object(&dir, "Money", "i64").unwrap();

    let config = puerto_toml::read(&dir).unwrap();
    assert_eq!(config.value_object.len(), 2);
    assert_eq!(config.value_object[0].name, "Email");
    assert_eq!(config.value_object[1].name, "Money");

    cleanup(&dir);
}

// ── Shared VO type inference ───────────────────────────────────────────────

#[test]
fn infer_shared_vo_string_type_from_name() {
    let shared_vos = vec![ValueObjectDefinition {
        name: "Email".to_string(),
        inner_type: "String".to_string(),
    }];
    let fields = vec![puerto_toml::Field {
        name: "email".to_string(),
        field_type: "Email".to_string(),
        ..Default::default()
    }];
    let result = puerto_toml::apply_shared_vo_inference(fields, &shared_vos);
    assert_eq!(result[0].field_type, "String");
    assert_eq!(result[0].value_object, Some("Email".to_string()));
    assert!(result[0].value_object_kind.is_none());
    assert!(!result[0].unique);
}

#[test]
fn infer_shared_vo_numeric_type_from_name() {
    let shared_vos = vec![ValueObjectDefinition {
        name: "Money".to_string(),
        inner_type: "i64".to_string(),
    }];
    let fields = vec![puerto_toml::Field {
        name: "price".to_string(),
        field_type: "Money".to_string(),
        ..Default::default()
    }];
    let result = puerto_toml::apply_shared_vo_inference(fields, &shared_vos);
    assert_eq!(result[0].field_type, "i64");
    assert_eq!(result[0].value_object, Some("Money".to_string()));
}

#[test]
fn infer_shared_vo_preserves_unique_flag() {
    let shared_vos = vec![ValueObjectDefinition {
        name: "Email".to_string(),
        inner_type: "String".to_string(),
    }];
    let fields = vec![puerto_toml::Field {
        name: "email".to_string(),
        field_type: "Email".to_string(),
        unique: true,
        ..Default::default()
    }];
    let result = puerto_toml::apply_shared_vo_inference(fields, &shared_vos);
    assert_eq!(result[0].field_type, "String");
    assert_eq!(result[0].value_object, Some("Email".to_string()));
    assert!(result[0].unique);
}

#[test]
fn infer_shared_vo_leaves_explicit_vo_field_unchanged() {
    let shared_vos = vec![ValueObjectDefinition {
        name: "Email".to_string(),
        inner_type: "String".to_string(),
    }];
    let fields = vec![puerto_toml::Field {
        name: "email".to_string(),
        field_type: "String".to_string(),
        value_object: Some("Email".to_string()),
        ..Default::default()
    }];
    let result = puerto_toml::apply_shared_vo_inference(fields, &shared_vos);
    assert_eq!(result[0].field_type, "String");
    assert_eq!(result[0].value_object, Some("Email".to_string()));
}

#[test]
fn infer_shared_vo_leaves_primitive_field_unchanged() {
    let shared_vos = vec![ValueObjectDefinition {
        name: "Email".to_string(),
        inner_type: "String".to_string(),
    }];
    let fields = vec![puerto_toml::Field {
        name: "title".to_string(),
        field_type: "String".to_string(),
        ..Default::default()
    }];
    let result = puerto_toml::apply_shared_vo_inference(fields, &shared_vos);
    assert_eq!(result[0].field_type, "String");
    assert!(result[0].value_object.is_none());
}

#[test]
fn infer_shared_vo_with_empty_registry_is_identity() {
    let fields = vec![puerto_toml::Field {
        name: "email".to_string(),
        field_type: "Email".to_string(),
        ..Default::default()
    }];
    let result = puerto_toml::apply_shared_vo_inference(fields, &[]);
    assert_eq!(result[0].field_type, "Email");
    assert!(result[0].value_object.is_none());
}

#[test]
fn infer_shared_vo_only_matches_registered_vo_names() {
    let shared_vos = vec![ValueObjectDefinition {
        name: "Email".to_string(),
        inner_type: "String".to_string(),
    }];
    let fields = vec![puerto_toml::Field {
        name: "phone".to_string(),
        field_type: "Phone".to_string(),
        ..Default::default()
    }];
    let result = puerto_toml::apply_shared_vo_inference(fields, &shared_vos);
    // Phone not in shared VOs — field_type left as-is (will fail type validation)
    assert_eq!(result[0].field_type, "Phone");
    assert!(result[0].value_object.is_none());
}

#[test]
fn infer_shared_vo_handles_multiple_fields() {
    let shared_vos = vec![
        ValueObjectDefinition {
            name: "Email".to_string(),
            inner_type: "String".to_string(),
        },
        ValueObjectDefinition {
            name: "Money".to_string(),
            inner_type: "i64".to_string(),
        },
    ];
    let fields = vec![
        puerto_toml::Field {
            name: "email".to_string(),
            field_type: "Email".to_string(),
            ..Default::default()
        },
        puerto_toml::Field {
            name: "price".to_string(),
            field_type: "Money".to_string(),
            ..Default::default()
        },
        puerto_toml::Field {
            name: "active".to_string(),
            field_type: "bool".to_string(),
            ..Default::default()
        },
    ];
    let result = puerto_toml::apply_shared_vo_inference(fields, &shared_vos);
    assert_eq!(result[0].field_type, "String");
    assert_eq!(result[0].value_object, Some("Email".to_string()));
    assert_eq!(result[1].field_type, "i64");
    assert_eq!(result[1].value_object, Some("Money".to_string()));
    assert_eq!(result[2].field_type, "bool");
    assert!(result[2].value_object.is_none());
}
