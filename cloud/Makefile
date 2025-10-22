.PHONY: dev build stop

DEV_COMPOSE ?= docker compose -f docker-compose.yml --project-name codex-cloud

build:
$(DEV_COMPOSE) build

dev:
$(DEV_COMPOSE) up

stop:
$(DEV_COMPOSE) down
