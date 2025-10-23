#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "${SCRIPT_DIR}/.." && pwd)
IMAGE_NAME=${CODEX_EXECUTOR_BASE_IMAGE:-codex-executor-base}
IMAGE_TAG=${CODEX_EXECUTOR_BASE_TAG:-latest}
DOCKER_BIN=${DOCKER_BIN:-docker}

if ! command -v "${DOCKER_BIN}" >/dev/null 2>&1; then
  echo "Docker binary '${DOCKER_BIN}' not found on PATH" >&2
  exit 1
fi

IMAGE_REF="${IMAGE_NAME}:${IMAGE_TAG}"
DOCKERFILE_PATH="${REPO_ROOT}/images/base/Dockerfile"

if [[ ! -f "${DOCKERFILE_PATH}" ]]; then
  echo "Unable to locate base Dockerfile at ${DOCKERFILE_PATH}" >&2
  exit 1
fi

BUILD_CONTEXT="${REPO_ROOT}/images/base"
echo "Building executor base image ${IMAGE_REF}"
"${DOCKER_BIN}" build \
  --pull \
  -t "${IMAGE_REF}" \
  -f "${DOCKERFILE_PATH}" \
  "${BUILD_CONTEXT}"
