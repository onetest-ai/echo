# Echo: build, run and check. `make` or `make help` lists the targets.
# Recipes mirror .ai-run/guides/development/dev-workflow.md and .ai-run/guides/quality-gates.md.

# A dev app runs beside an installed Echo only with its own folders (dev-workflow, Run and debug R1):
# the config dir holds app.lock, app-path and plugins, the home holds the calls and app.log.
DEV_DIR ?= $(HOME)/.echo-dev
DEV_ENV  = ECHOCTL_CONFIG_DIR=$(DEV_DIR)/config ECHOCTL_HOME=$(DEV_DIR)/home
ECHOCTL  = target/debug/echoctl
PORT    ?= 8787

.DEFAULT_GOAL := help
.PHONY: help build build-lite release run run-no-tray quit status doctor debug window-serve window-export \
        window-selftest log lint check-windows test mod-test check plugin plugin-dev package-macos \
        clean-dev clean

help: ## list the targets
	@awk -F':.*## ' '/^[a-zA-Z_-]+:.*## /{printf "  %-16s %s\n", $$1, $$2}' $(MAKEFILE_LIST)
	@echo "\nDev data lives in $(DEV_DIR) (override: make run DEV_DIR=/some/dir)."

## Build

build: ## debug build with the default features (whisper.cpp, tray, brain, QuickJS)
	cargo build -p echoctl

build-lite: ## debug build without whisper.cpp and the C-heavy parts: app and window only
	cargo build -p echoctl --no-default-features --features tray

release: ## release build
	cargo build -p echoctl --release

## Run (a debug build, on its own folders, never `app start`, which opens the installed Echo.app)

run: build ## run the app in the foreground, output in this terminal (Ctrl-C or `make quit` stops it)
	@mkdir -p $(DEV_DIR)/config $(DEV_DIR)/home
	$(DEV_ENV) $(ECHOCTL) app run

run-no-tray: build ## same without the tray icon
	@mkdir -p $(DEV_DIR)/config $(DEV_DIR)/home
	$(DEV_ENV) $(ECHOCTL) app run --no-tray

debug: build ## run with ECHOCTL_DEBUG=1 (Claude session commands, permission probes, speaker scores on stderr)
	@mkdir -p $(DEV_DIR)/config $(DEV_DIR)/home
	ECHOCTL_DEBUG=1 $(DEV_ENV) $(ECHOCTL) app run

quit: ## stop the running dev app
	$(DEV_ENV) $(ECHOCTL) app quit

status: ## ask the running dev app what it is doing
	$(DEV_ENV) $(ECHOCTL) status

doctor: ## check permissions (add ARGS="--probe", "--request" or "--json")
	$(DEV_ENV) $(ECHOCTL) doctor $(ARGS)

log: ## follow the dev app's log (background launches write it)
	tail -f $(DEV_DIR)/home/.echo/app.log

## Window review in a browser (needs the tray feature)

window-serve: build ## serve the window on http://127.0.0.1:$(PORT) with the dev data (PORT=… to change)
	$(DEV_ENV) $(ECHOCTL) window --serve $(PORT)

window-export: build ## write the window as static files to target/window-export
	$(DEV_ENV) $(ECHOCTL) window --export target/window-export

window-selftest: build ## open the real webview, check echo:// and IPC, exit
	$(DEV_ENV) $(ECHOCTL) window --selftest

## Quality gates (quality-gates.md, fastest to slowest)

lint: ## clippy, warnings are errors
	cargo clippy -- -D warnings

check-windows: ## Windows cross-check (needs: rustup target add x86_64-pc-windows-msvc)
	cargo check --target x86_64-pc-windows-msvc --no-default-features

test: ## cargo test
	cargo test

mod-test: ## plugin validate, window node tests, tsc (needs claude, node, npx)
	scripts/check-mod.sh

check: lint check-windows test mod-test ## all the gates

## Plugin and packaging

plugin: ## build the plugin's launchers into plugin/bin
	scripts/build-plugin.sh

plugin-dev: plugin ## start Claude Code with the local plugin
	claude --plugin-dir plugin

package-macos: ## ad-hoc signed dist/Echo.app for testing capture permissions (no notarization)
	LOCAL=1 scripts/package-macos.sh

## Clean

clean-dev: ## delete the dev data folder (calls, config) after a prompt
	@read -p "Delete $(DEV_DIR)? [y/N] " a && [ "$$a" = y ] && rm -rf $(DEV_DIR) || echo kept

clean: ## cargo clean
	cargo clean
