.PHONY: dev build stop executor-base-image executor-snapshot executor-refresh-pool

DEV_COMPOSE ?= docker compose -f docker-compose.yml --project-name codex-cloud
EXECUTOR_SCRIPTS ?= ./executor/scripts

build:
	$(DEV_COMPOSE) build

dev:
	$(DEV_COMPOSE) up

stop:
	$(DEV_COMPOSE) down

executor-base-image:
	$(EXECUTOR_SCRIPTS)/build-base-image.sh

executor-snapshot:
	$(EXECUTOR_SCRIPTS)/create-snapshot.sh

executor-refresh-pool:
	$(EXECUTOR_SCRIPTS)/refresh-pool.sh
