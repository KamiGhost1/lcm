# LCM — Linux Cert Manager · build orchestration
#
# Most targets run inside the `dev` Docker container, because building Linux
# artifacts (trust-store integration, .deb packages) needs Linux while the host
# may be macOS. Bootstrap once with:
#
#     make image up
#
# then e.g. `make test`, `make deb`. Run `make help` for the full list.

DC      := docker compose
# Non-login shell on purpose: a login shell (`bash -lc`) resets PATH and drops
# /usr/local/cargo/bin, so `cargo` would not be found.
DEV     := $(DC) exec -T dev bash -c
BUNDLE  := /workspace/gui/src-tauri/target/release/bundle
# Every Linux package format Tauri can emit (covers Debian/Ubuntu via .deb,
# Fedora/RHEL/openSUSE via .rpm, and everything else via the portable AppImage).
BUNDLES := deb,rpm,appimage
# Collect whatever was produced into ./dist (skips formats that weren't built).
COPY_PKGS := mkdir -p /workspace/dist; for ext in deb rpm AppImage; do cp $(BUNDLE)/*/*.$$ext /workspace/dist/ 2>/dev/null; done; true

.DEFAULT_GOAL := help

## ---- environment ----

image: ## Build the dev Docker image
	$(DC) build

up: ## Start the (native-arch) dev container
	$(DC) up -d dev

down: ## Stop and remove the dev container
	$(DC) down

shell: ## Open an interactive shell in the dev container
	$(DC) exec dev bash

## ---- rust core + cli ----

build: up ## Build the lcm CLI (release)
	$(DEV) 'cargo build --release -p lcm-cli'
	@echo "→ binary: target/release/lcm (inside the container / target volume)"

test: up ## Run tests + clippy + rustfmt check
	$(DEV) 'cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --all --check'

fmt: up ## Apply rustfmt
	$(DEV) 'cargo fmt --all'

## ---- gui (tauri + react) ----

gui-deps: up ## Install frontend dependencies (in the container)
	$(DEV) 'cd gui && npm install'

gui-web: gui-deps ## Type-check and build the frontend bundle only
	$(DEV) 'cd gui && npm run build'

gui-check: up ## cargo check the Tauri backend
	$(DEV) 'cd gui/src-tauri && cargo check'

icons: gui-deps ## Regenerate the icon set from src-tauri/icon.svg
	$(DEV) 'cd gui && npm run tauri icon src-tauri/icon.svg'

gui-dev: gui-deps ## Run the desktop app (needs a real Linux desktop / display)
	$(DC) exec dev bash -c 'cd gui && npm run tauri dev'

## ---- packaging ----

# LONG: a cold release build of the whole Tauri + WebKit tree (~3-20 min).
# Depends on set-version so the package always matches the VERSION file.
deb: set-version gui-deps ## Build just the .deb for the host arch → ./dist/
	$(DEV) 'cargo build --release -p lcm-cli'
	$(DEV) 'cd gui && npm run tauri build'
	@$(DEV) '$(COPY_PKGS)'
	@$(MAKE) --no-print-directory dist-prune
	@echo "→ ./dist/:" && ls -1 dist/ 2>/dev/null

packages: set-version gui-deps ## Build deb + rpm + AppImage for the host arch → ./dist/
	$(DEV) 'cargo build --release -p lcm-cli'
	$(DEV) 'cd gui && npm run tauri -- build --bundles $(BUNDLES)'
	@$(DEV) '$(COPY_PKGS)'
	@$(MAKE) --no-print-directory dist-prune
	@echo "→ ./dist/:" && ls -1 dist/ 2>/dev/null

image-amd64: ## Build the amd64 dev image (emulated on arm64 hosts)
	$(DC) build dev-amd64

# VERY LONG under emulation on Apple Silicon (~20-40+ min cold). Enable Rosetta
# in Docker Desktop (Settings → General) to speed it up substantially.
packages-amd64: set-version image-amd64 ## Build deb + rpm + AppImage for amd64 via emulation → ./dist/
	$(DC) up -d dev-amd64
	$(DC) exec -T dev-amd64 bash -c 'cargo build --release -p lcm-cli'
	$(DC) exec -T dev-amd64 bash -c 'cd gui && npm install && npm run tauri -- build --bundles $(BUNDLES)'
	@$(DC) exec -T dev-amd64 bash -c '$(COPY_PKGS)'
	@$(MAKE) --no-print-directory dist-prune
	@echo "→ ./dist/:" && ls -1 dist/ 2>/dev/null

packages-all: packages packages-amd64 ## Build every package format for BOTH arm64 and amd64

## ---- versioning ----

set-version: ## Set the version everywhere from VERSION (usage: make set-version VERSION=0.2.0)
	@./scripts/set-version.sh $(VERSION)

dist-prune: ## Remove ./dist artifacts that don't match the current VERSION
	@ver=$$(cat VERSION); \
	if [ -d dist ]; then \
		find dist -maxdepth 1 -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) ! -name "*$$ver*" -print -delete 2>/dev/null || true; \
	fi

## ---- housekeeping ----

clean: up ## Remove Rust + frontend build artifacts
	$(DEV) 'cargo clean; cd gui/src-tauri && cargo clean; rm -rf gui/dist'

preview: gui-deps ## Serve the frontend preview (mock data) on :5173
	$(DC) exec dev bash -c 'cd gui && npm run dev -- --host'

help: ## Show this help
	@grep -E '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-10s\033[0m %s\n", $$1, $$2}'

.PHONY: image up down shell build test fmt gui-deps gui-web gui-check icons gui-dev deb packages image-amd64 packages-amd64 packages-all set-version dist-prune clean preview help
