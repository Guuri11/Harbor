.PHONY: help run build install setup test test/full lint check format format/fix audit audit/fix

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
	@grep -E '^[a-zA-Z/]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  ${YELLOW}%-20s${NC} %s\n", $$1, $$2}'
	@echo ""

run: ## Run puerto CLI interactively (puerto new)
	@echo "${GREEN}Running puerto CLI...${NC}"
	$(CARGO) run --bin puerto -- new

build: ## Build puerto binary (release) → target/release/puerto
	@echo "${GREEN}Building puerto...${NC}"
	$(CARGO) build --release --bin puerto
	@echo "Binary: $(shell pwd)/target/release/puerto"

install: ## Install puerto to ~/.cargo/bin
	@echo "${GREEN}Installing puerto...${NC}"
	$(CARGO) install --path crates/cli

setup: ## Install required dev tools (run once after cloning)
	@echo "${CYAN}Setting up development environment...${NC}"
	$(CARGO) install cargo-nextest --locked

test: ## Fast structural tests
	@echo "${YELLOW}Running structural tests...${NC}"
	$(CARGO) nextest run --workspace

test/full: ## Slow test: generates real projects, compiles them, runs their internal tests
	@echo "${YELLOW}Running compile matrix (generates + compiles + tests real projects)...${NC}"
	$(CARGO) nextest run --workspace --run-ignored all --test-threads 3

lint: ## Run clippy with -D warnings
	@echo "${CYAN}Linting code...${NC}"
	$(CARGO) clippy --workspace -- -D warnings

check: ## Fast compile check (no binary)
	@echo "${CYAN}Checking code...${NC}"
	$(CARGO) check --workspace

format: ## Check code formatting
	@echo "${CYAN}Checking code formatting...${NC}"
	$(CARGO) fmt --all -- --check

format/fix: ## Fix code formatting
	@echo "${CYAN}Fixing code formatting...${NC}"
	$(CARGO) fmt --all

audit: ## Run security audit on dependencies
	@echo "${CYAN}Running security audit...${NC}"
	$(CARGO) audit

audit/fix: ## Preview security fixes (dry-run)
	@echo "${CYAN}Previewing security fixes (dry-run)...${NC}"
	$(CARGO) audit fix --dry-run
