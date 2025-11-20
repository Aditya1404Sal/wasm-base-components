# note that wash providers break when using a rust workspace for some reason

# Get version from .version file or use default
VERSION := `cat .version 2>/dev/null || echo "0.1.0"`
REGISTRY := env_var_or_default("REGISTRY", "ghcr.io")
REPO_OWNER := env_var_or_default("REPO_OWNER", "bettyblocks")

build:
  wash build --config-path providers/smtp-provider
  wash build --config-path helper/data-api
  wash build --config-path helper/crud
  wash build --config-path helper/http-wrapper
  wash build --config-path helper/data-api/component
  just --working-directory helper/log-to-stdout --justfile helper/log-to-stdout/Justfile build

build-test:
  wash build --config-path integration-test/components/fetcher

deploy env version:
  cd deploy && bun install
  cd deploy && bun run publish {{env}} {{version}}
