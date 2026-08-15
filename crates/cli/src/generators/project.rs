use std::{fs, path::Path};

use crate::generators::naming::write_file;
use crate::generators::render::render;

const DB_MAKEFILE_TARGETS: &str = r#"
.PHONY: db-up db-down docker-compose/up docker-compose/down reset-db \
        test/infrastructure \
        sqlx/online sqlx/offline sqlx/migrate sqlx/prepare sqlx/check \
        generate/migration

DOCKER         := docker
DOCKER_COMPOSE := docker compose

db-up: docker-compose/up ## Start database containers

db-down: docker-compose/down ## Stop database containers

docker-compose/up: ## Start all containers
	@echo "${CYAN}Starting containers...${NC}"
	$(DOCKER_COMPOSE) up -d

docker-compose/down: ## Stop all containers
	@echo "${CYAN}Stopping containers...${NC}"
	$(DOCKER_COMPOSE) down

reset-db: ## Wipe database and re-run migrations (DESTRUCTIVE)
	@printf "${RED}This destroys ALL database data. Type DESTROY to confirm: ${NC}"; \
	read CONFIRM; \
	if [ "$$CONFIRM" != "DESTROY" ]; then \
		echo "${GREEN}Cancelled${NC}"; \
		exit 1; \
	fi; \
	echo "${CYAN}Stopping containers and removing volumes...${NC}"; \
	$(DOCKER_COMPOSE) down -v; \
	echo "${CYAN}Restarting containers...${NC}"; \
	$(DOCKER_COMPOSE) up -d; \
	echo "${GREEN}Running migrations...${NC}"; \
	sqlx migrate run --source infrastructure/migrations

test/infrastructure: docker-compose/up ## Run infrastructure tests (requires live DB)
	@echo "${YELLOW}Running infrastructure tests...${NC}"
	$(CARGO) nextest run -p infrastructure

sqlx/online: ## Switch SQLx to ONLINE mode (check against live DB)
	@printf '[env]\nSQLX_OFFLINE = "false"\n' > .cargo/config.toml
	@echo "${GREEN}SQLx ONLINE mode${NC}"

sqlx/offline: ## Switch SQLx to OFFLINE mode (use saved cache)
	@printf '[env]\nSQLX_OFFLINE = "true"\n' > .cargo/config.toml
	@echo "${GREEN}SQLx OFFLINE mode${NC}"

sqlx/migrate: docker-compose/up ## Run pending database migrations
	@echo "${GREEN}Running migrations...${NC}"
	sqlx migrate run --source infrastructure/migrations

sqlx/prepare: docker-compose/up ## Regenerate SQLx offline cache (requires live DB)
	@echo "${GREEN}Preparing SQLx cache...${NC}"
	SQLX_OFFLINE=false $(CARGO) sqlx prepare --workspace

sqlx/check: docker-compose/up ## Verify SQLx cache matches current queries
	@echo "${GREEN}Checking SQLx cache...${NC}"
	SQLX_OFFLINE=false $(CARGO) sqlx prepare --workspace --check

generate/migration: ## Create a new SQLx migration — make generate/migration NAME=add_users
	@if [ -z "$$NAME" ]; then \
		echo "${RED}Error: provide name — make generate/migration NAME=add_users_table${NC}"; \
		exit 1; \
	fi
	@echo "${GREEN}Creating migration '$$NAME'...${NC}"
	puerto generate migration $$NAME
"#;

fn docker_compose_content(project_name: &str, db_name: &str) -> String {
    format!(
        "services:\n  postgres:\n    image: postgres:16\n    container_name: {project_name}-postgres\n    ports:\n      - \"${{DB_HOST_PORT:-5432}}:5432\"\n    environment:\n      POSTGRES_USER: devuser\n      POSTGRES_PASSWORD: password\n      POSTGRES_DB: {db_name}\n    volumes:\n      - postgres-data:/var/lib/postgresql/data\n    restart: unless-stopped\n    healthcheck:\n      test: [\"CMD-SHELL\", \"pg_isready -U devuser -d {db_name}\"]\n      interval: 5s\n      timeout: 5s\n      retries: 5\n\nvolumes:\n  postgres-data:\n"
    )
}

fn add_env_to_gitignore(base: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let path = base.join(".gitignore");
    if path.exists() {
        let mut src = fs::read_to_string(&path)?;
        if !src.contains(".env") {
            if !src.ends_with('\n') {
                src.push('\n');
            }
            src.push_str(".env\n");
            fs::write(&path, src)?;
        }
    } else {
        write_file(&path, "/target\n.env\n/.sqlx\n")?;
    }
    Ok(())
}

fn add_db_makefile_targets(base: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let path = base.join("Makefile");
    if !path.exists() {
        return Ok(());
    }
    let src = fs::read_to_string(&path)?;
    if src.contains("docker-compose/up") {
        return Ok(()); // idempotent
    }
    let mut patched = src;
    if !patched.ends_with('\n') {
        patched.push('\n');
    }
    patched.push_str(DB_MAKEFILE_TARGETS);
    fs::write(&path, patched)?;
    Ok(())
}

fn patch_makefile_setup_for_db(base: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let path = base.join("Makefile");
    if !path.exists() {
        return Ok(()); // no Makefile — nothing to patch
    }
    let src = fs::read_to_string(&path)?;
    let sqlx_line =
        "\t$(CARGO) install sqlx-cli --no-default-features --features postgres --locked\n";
    if src.contains("sqlx-cli") {
        return Ok(()); // idempotent
    }
    let patched = src.replace(
        "\t$(CARGO) install cargo-nextest --locked\n",
        &format!("\t$(CARGO) install cargo-nextest --locked\n{sqlx_line}"),
    );
    fs::write(&path, patched)?;
    Ok(())
}

fn patch_infra_cargo_toml(base: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let path = base.join("infrastructure/Cargo.toml");
    let mut src = fs::read_to_string(&path)?;
    if src.contains("sqlx") {
        return Ok(()); // idempotent
    }
    if !src.ends_with('\n') {
        src.push('\n');
    }
    src.push_str(
        "\n[dependencies.sqlx]\nversion = \"0.8\"\nfeatures = [\"runtime-tokio-rustls\", \"postgres\", \"macros\", \"migrate\", \"uuid\", \"chrono\"]\n",
    );
    fs::write(&path, src)?;
    Ok(())
}

/// Write the base `.env` and `.env.example` for every new project (db or not).
pub fn apply_base_env(base: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let content = "SERVICE_PORT=8080\nENV=development\nRUST_LOG=info\nCORS_ALLOWED_ORIGINS=http://localhost:3000,http://localhost:5173\n";
    write_file(&base.join(".env"), content)?;
    write_file(&base.join(".env.example"), content)?;
    add_env_to_gitignore(base)?;
    Ok(())
}

/// Write the extra files that `puerto new --db` adds on top of the base template.
pub fn apply_db_to_new_project(base: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let project_name = base.file_name().and_then(|n| n.to_str()).unwrap_or("myapp");
    let db_name = project_name.replace('-', "_");

    // docker-compose.yml
    write_file(
        &base.join("docker-compose.yml"),
        &docker_compose_content(project_name, &db_name),
    )?;

    // Append DB vars to .env and .env.example (apply_base_env already created them)
    let db_vars = format!(
        "\n# Database\nDB_HOST_PORT=5432\nDATABASE_URL=postgres://devuser:password@localhost:5432/{db_name}\n"
    );
    for file in &[".env", ".env.example"] {
        let path = base.join(file);
        let mut content = if path.exists() {
            fs::read_to_string(&path)?
        } else {
            String::new()
        };
        if !content.contains("DATABASE_URL") {
            if !content.ends_with('\n') && !content.is_empty() {
                content.push('\n');
            }
            content.push_str(&db_vars);
            fs::write(&path, content)?;
        }
    }

    // .cargo/config.toml — SQLX_OFFLINE so CI compiles without a live DB
    write_file(
        &base.join(".cargo/config.toml"),
        "[env]\nSQLX_OFFLINE = \"true\"\n",
    )?;

    // infrastructure/migrations/ directory (empty, sqlx needs it)
    fs::create_dir_all(base.join("infrastructure/migrations"))?;

    // infrastructure/src/db.rs
    write_file(
        &base.join("infrastructure/src/db.rs"),
        &render("infrastructure/db.tera", &tera::Context::new())
            .expect("infrastructure/db.tera render failed"),
    )?;

    // Patch infrastructure/Cargo.toml to add sqlx
    patch_infra_cargo_toml(base)?;

    // Patch Makefile: setup target + docker/sqlx targets
    patch_makefile_setup_for_db(base)?;
    add_db_makefile_targets(base)?;

    // Mark the project as db-enabled in puerto.toml
    let mut config = crate::puerto_toml::read(base)?;
    config.project.db = true;
    crate::puerto_toml::write(base, &config)?;

    Ok(())
}

/// Strip the Greeting demo entity from a freshly generated project (`puerto new --no-demo`).
/// Removes greeting files, rewrites the four affected source files, clears puerto.toml entities,
/// and regenerates bootstrap.rs with an empty API.
pub fn apply_no_demo(base: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Remove greeting directories and the sibling declaration file.
    let _ = fs::remove_dir_all(base.join("business/src/domain/greeting"));
    let _ = fs::remove_dir_all(base.join("business/src/application/greeting"));
    let _ = fs::remove_dir_all(base.join("infrastructure/src/greeting"));
    let _ = fs::remove_dir_all(base.join("presentation/src/api/greeting"));
    let _ = fs::remove_file(base.join("presentation/src/api/greeting.rs"));

    // Minimal business lib.rs: keep domain::logger, empty application block.
    fs::write(
        base.join("business/src/lib.rs"),
        "pub mod domain {\n    pub mod logger;\n}\npub mod application {\n}\n",
    )?;

    // Minimal infrastructure lib.rs: logger only.
    fs::write(base.join("infrastructure/src/lib.rs"), "pub mod logger;\n")?;

    // Minimal presentation api.rs. `tags` must survive the demo strip: `regenerate_bootstrap`
    // always writes `api/tags.rs`, and every generated `routes.rs` does
    // `use crate::api::tags::ApiTags;` — dropping the declaration made the first scaffold into a
    // `--no-demo` project fail to compile (E0432).
    fs::write(
        base.join("presentation/src/api.rs"),
        "pub mod error;\npub mod tags;\n",
    )?;

    // Clear entities in puerto.toml.
    let mut config = crate::puerto_toml::read(base)?;
    config.entity.clear();
    crate::puerto_toml::write(base, &config)?;

    // Regenerate bootstrap.rs with no entities.
    crate::generators::bootstrap::regenerate_bootstrap(base)?;

    Ok(())
}
