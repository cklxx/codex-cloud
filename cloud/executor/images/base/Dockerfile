# syntax=docker/dockerfile:1.6

FROM debian:bookworm-slim AS base

ENV DEBIAN_FRONTEND=noninteractive \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        gnupg \
        build-essential \
        pkg-config \
        libssl-dev \
        python3 \
        python3-pip \
        python3-venv \
        openssh-client \
        sudo \
        unzip \
        xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Install Node.js LTS and pnpm for frontend tooling
RUN curl -fsSL https://deb.nodesource.com/setup_lts.x | bash - \
    && apt-get update \
    && apt-get install -y --no-install-recommends nodejs \
    && npm install -g corepack \
    && corepack enable pnpm \
    && rm -rf /var/lib/apt/lists/*

# Install Rust toolchain using rustup
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable \
    && /root/.cargo/bin/rustup component add clippy rustfmt

ENV PATH="/root/.cargo/bin:${PATH}" \
    CODEX_CACHE_ROOT=/var/cache/codex \
    CODEX_GIT_MIRROR=/var/cache/codex/git \
    CODEX_NPM_CACHE=/var/cache/codex/npm \
    CODEX_PIP_CACHE=/var/cache/codex/pip \
    CODEX_CARGO_CACHE=/var/cache/codex/cargo

RUN mkdir -p \
        /workspace \
        ${CODEX_CACHE_ROOT} \
        ${CODEX_GIT_MIRROR} \
        ${CODEX_NPM_CACHE} \
        ${CODEX_PIP_CACHE} \
        ${CODEX_CARGO_CACHE}

# Configure global Git cache mirrors so shallow clones reuse on-disk packs
RUN git config --system gc.auto 0 \
    && git config --system pack.windowMemory 256m \
    && git config --system pack.packSizeLimit 256m \
    && git config --system pack.threads 1 \
    && git config --system http.sslVerify true

# Drop in an unprivileged user for executor workloads
RUN useradd -m -d /home/codex -s /bin/bash codex \
    && echo "codex ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/codex \
    && chmod 440 /etc/sudoers.d/codex

USER codex
WORKDIR /workspace

# Provide a default entrypoint that keeps the VM idle until orchestration attaches
ENTRYPOINT ["/bin/bash", "-lc", "sleep infinity"]
