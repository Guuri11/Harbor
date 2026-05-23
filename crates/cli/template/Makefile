.PHONY: help setup run \
        test test/domain test/application test/infrastructure \
        lint format format/fix check clean \
        audit audit/fix \
        generate/entity generate/use-case generate/bootstrap

CARGO := cargo

GREEN  := \033[1;32m
YELLOW := \033[1;33m
CYAN   := \033[1;36m
RED    := \033[1;31m
NC     := \033[0m

help: ## Show available commands
	@echo ""
	@echo "${CYAN}Available targets:${NC}"
	@echo ""
	@grep -E '^[a-zA-Z/_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  ${YELLOW}%-32s${NC} %s\n", $$1, $$2}'
	@echo ""

setup: ## Install required dev tools (run once after cloning)
	@echo "${CYAN}Setting up development environment...${NC}"
	$(CARGO) install cargo-nextest --locked
	$(CARGO) install cargo-audit --locked

run: ## Run the API server (http://localhost:8080)
	@echo "${GREEN}Starting API server...${NC}"
	$(CARGO) run -p presentation

test: ## Run all tests
	@echo "${YELLOW}Running all tests...${NC}"
	$(CARGO) nextest run --workspace

test/domain: ## Run domain layer tests
	@echo "${YELLOW}Running domain tests...${NC}"
	$(CARGO) nextest run -p business -E 'test(domain)'

test/application: ## Run application layer tests
	@echo "${YELLOW}Running application layer tests...${NC}"
	$(CARGO) nextest run -p business -E 'test(application)'

test/infrastructure: ## Run infrastructure layer tests (requires DB)
	@echo "${YELLOW}Running infrastructure tests...${NC}"
	$(CARGO) nextest run -p infrastructure

lint: ## Clippy with -D warnings
	@echo "${CYAN}Linting code...${NC}"
	$(CARGO) clippy --workspace -- -D warnings

format: ## Check code formatting
	@echo "${CYAN}Checking code formatting...${NC}"
	$(CARGO) fmt --all -- --check

format/fix: ## Fix code formatting
	@echo "${CYAN}Fixing code formatting...${NC}"
	$(CARGO) fmt --all

check: ## Fast compile check (no binary)
	@echo "${CYAN}Checking code...${NC}"
	$(CARGO) check --workspace

clean: ## Remove build artifacts
	@echo "${RED}Cleaning build artifacts...${NC}"
	$(CARGO) clean

audit: ## Run security audit on dependencies
	@echo "${CYAN}Running security audit...${NC}"
	$(CARGO) audit

audit/fix: ## Preview security fixes (dry-run)
	@echo "${CYAN}Previewing security fixes (dry-run)...${NC}"
	$(CARGO) audit fix --dry-run

generate/entity: ## Scaffold a new DDD entity — make generate/entity NAME=Product
	@if [ -z "$$NAME" ]; then \
		echo "${RED}Error: provide entity name — make generate/entity NAME=Product${NC}"; \
		exit 1; \
	fi
	@echo "${GREEN}Scaffolding entity '$$NAME'...${NC}"
	puerto generate scaffold $$NAME

generate/use-case: ## Add a use case to an entity — make generate/use-case ENTITY=Product ACTION=delete_product
	@if [ -z "$$ENTITY" ] || [ -z "$$ACTION" ]; then \
		echo "${RED}Error: provide entity and action — make generate/use-case ENTITY=Product ACTION=delete_product${NC}"; \
		exit 1; \
	fi
	@echo "${GREEN}Adding use case '$$ACTION' to '$$ENTITY'...${NC}"
	puerto generate use-case $$ENTITY $$ACTION

generate/bootstrap: ## Regenerate bootstrap.rs from puerto.toml
	@echo "${GREEN}Regenerating bootstrap.rs...${NC}"
	puerto generate bootstrap
