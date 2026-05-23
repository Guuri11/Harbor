use std::{fs, path::Path};

use serde::Serialize;

use crate::generators::naming::{pascal_to_snake, to_pascal_case};
use crate::generators::render::render;

#[derive(Serialize)]
struct BootstrapUseCase {
    action: String,
    uc_pascal: String,
    repo_expr: String,
}

#[derive(Serialize)]
struct BootstrapEntity {
    pascal: String,
    snake: String,
    uc_import_lines: Vec<String>,
    repo_trait_import: String,
    repo_impl_import: String,
    api_import: String,
    needs_repo_var: bool,
    repo_var_line: String,
    use_cases: Vec<BootstrapUseCase>,
    use_case_fields: String,
    api_var: String,
}

fn build_context(entities: &[crate::puerto_toml::Entity]) -> tera::Context {
    let has_db = entities.iter().any(|e| e.db);

    let api_args = if entities.is_empty() {
        "()".to_string()
    } else if entities.len() == 1 {
        format!("{}_api", pascal_to_snake(&entities[0].name))
    } else {
        let apis = entities
            .iter()
            .map(|e| format!("{}_api", pascal_to_snake(&e.name)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("({apis})")
    };

    let ctx_entities: Vec<BootstrapEntity> = entities
        .iter()
        .map(|entity| {
            let pascal = &entity.name;
            let snake = pascal_to_snake(pascal);
            let uc_count = entity.use_cases.len();

            let repo_init = if entity.db {
                format!("Pg{pascal}Repository::new(pool.clone(), Arc::clone(&logger))")
            } else {
                format!("InMemory{pascal}Repository::new(Arc::clone(&logger))")
            };

            let repo_crate_path = if entity.db {
                format!("infrastructure::{snake}::repository::Pg{pascal}Repository")
            } else {
                format!("infrastructure::{snake}::repository::InMemory{pascal}Repository")
            };

            let uc_import_lines = entity
                .use_cases
                .iter()
                .map(|uc| {
                    let uc_pascal = to_pascal_case(uc);
                    format!(
                        "use business::application::{snake}::{uc}::{uc_pascal}UseCaseImpl;"
                    )
                })
                .collect();

            let (needs_repo_var, repo_var_line) = if uc_count <= 1 {
                (false, String::new())
            } else {
                let line = format!(
                    "    let {snake}_repo: Arc<dyn {pascal}RepositoryTrait> = Arc::new({repo_init});"
                );
                (true, line)
            };

            let use_cases = entity
                .use_cases
                .iter()
                .enumerate()
                .map(|(i, uc)| {
                    let uc_pascal = to_pascal_case(uc);
                    let repo_expr = if uc_count == 1 {
                        format!(
                            "Arc::new({repo_init}) as Arc<dyn {pascal}RepositoryTrait>"
                        )
                    } else if i < uc_count - 1 {
                        format!("Arc::clone(&{snake}_repo)")
                    } else {
                        format!("{snake}_repo")
                    };
                    BootstrapUseCase {
                        action: uc.clone(),
                        uc_pascal,
                        repo_expr,
                    }
                })
                .collect();

            BootstrapEntity {
                repo_trait_import: format!(
                    "use business::domain::{snake}::repository::{pascal}RepositoryTrait;"
                ),
                repo_impl_import: format!("use {repo_crate_path};"),
                api_import: format!("use crate::api::{snake}::routes::{pascal}Api;"),
                use_case_fields: entity.use_cases.join(", "),
                api_var: format!("{snake}_api"),
                pascal: pascal.clone(),
                snake,
                uc_import_lines,
                needs_repo_var,
                repo_var_line,
                use_cases,
            }
        })
        .collect();

    let mut ctx = tera::Context::new();
    ctx.insert("entities", &ctx_entities);
    ctx.insert("has_db", &has_db);
    ctx.insert("api_args", &api_args);
    ctx
}

pub fn generate_bootstrap_content(entities: &[crate::puerto_toml::Entity]) -> String {
    let ctx = build_context(entities);
    render("bootstrap/bootstrap.tera", &ctx).expect("bootstrap/bootstrap.tera render failed")
}

pub fn generate_tags_content(entities: &[crate::puerto_toml::Entity]) -> String {
    let tags: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
    let mut ctx = tera::Context::new();
    ctx.insert("tags_entities", &tags);
    render("bootstrap/tags.tera", &ctx).expect("bootstrap/tags.tera render failed")
}

/// Read puerto.toml and overwrite `presentation/src/generated/bootstrap.rs`
/// and `presentation/src/api/tags.rs`.
pub fn regenerate_bootstrap(base: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let config = crate::puerto_toml::read(base)?;

    let bootstrap_path = base.join("presentation/src/generated/bootstrap.rs");
    if let Some(parent) = bootstrap_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(bootstrap_path, generate_bootstrap_content(&config.entity))?;

    let tags_path = base.join("presentation/src/api/tags.rs");
    if let Some(parent) = tags_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(tags_path, generate_tags_content(&config.entity))?;

    Ok(())
}
