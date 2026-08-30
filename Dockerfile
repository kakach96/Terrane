# syntax=docker/dockerfile:1
# =============================================================================
# Terrane - multi-stage container image
#
# Stages:
#   1. frontend : build the Angular frontend with Node 24 -> /app/dist/terrane-ui
#   2. builder  : compile the Rust backend -> /build/target/release/terrane
#   3. runtime  : debian-slim image (binary + static/ only, non-root)
#
# The image ships a built-in readiness probe (HEALTHCHECK -> /health/ready) and
# graceful shutdown (SIGTERM drain).
#
# Build (BuildKit is required for the cache mounts below; Docker Desktop enables
# it by default, otherwise export DOCKER_BUILDKIT=1):
#   docker build -t terrane:latest .
#
# To pull base images from a domestic (China) registry mirror instead of Docker
# Hub, override the base-image ARGs:
#   docker build \
#     --build-arg NODE_IMAGE=docker.1ms.run/library/node:24-alpine \
#     --build-arg RUST_IMAGE=docker.1ms.run/library/rust:1.95-bookworm \
#     --build-arg RUNTIME_IMAGE=docker.1ms.run/library/debian:bookworm-slim \
#     -t terrane:latest .
#
# Dependency registries default to domestic mirrors:
#   - npm:   registry.npmmirror.com (override via --build-arg NPM_REGISTRY)
#   - cargo: rsproxy.cn             (override via --build-arg CARGO_MIRROR)
#   - apt:   mirrors.aliyun.com     (override via --build-arg APT_MIRROR)
# so downloads inside the build do not hit slow/unreachable npmjs.org, crates.io
# or deb.debian.org.
# =============================================================================

# ---------------------------------------------------------------------------
# Base image selection (override via --build-arg, e.g. for domestic mirrors)
# NOTE: ARGs referenced by FROM must be declared before the first FROM
# ---------------------------------------------------------------------------
ARG NODE_IMAGE=node:24-alpine
ARG RUST_IMAGE=rust:1.95-bookworm
ARG RUNTIME_IMAGE=debian:bookworm-slim

# ---------------------------------------------------------------------------
# Stage 1: Frontend (Angular 22)
# ---------------------------------------------------------------------------
FROM ${NODE_IMAGE} AS frontend
WORKDIR /app
ARG NPM_REGISTRY=https://registry.npmmirror.com

# Copy the manifest first to leverage Docker layer caching; the npm download
# cache is kept in a BuildKit cache mount so repeat builds do not re-fetch deps
COPY frontend/package.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm install --no-audit --no-fund --registry=${NPM_REGISTRY}

COPY frontend/ ./
RUN npm run build

# Artifacts: /app/dist/terrane-ui (angular.json -> outputPath)

# ---------------------------------------------------------------------------
# Stage 2: Backend (Rust, release)
# ---------------------------------------------------------------------------
FROM ${RUST_IMAGE} AS builder
WORKDIR /build
ARG CARGO_MIRROR=rsproxy.cn

# Configure the (domestic) cargo registry mirror so crate downloads do not hit
# slow/unreachable crates.io; override via --build-arg CARGO_MIRROR
RUN printf '[source.crates-io]\nreplace-with = "mirror"\n[source.mirror]\nregistry = "sparse+https://%s/index/"\n' "$CARGO_MIRROR" > /usr/local/cargo/config.toml

# Two mechanisms avoid re-downloading/re-compiling dependencies on every build:
#   1. Layer caching: pre-build all deps with an empty src, so later source
#      changes only recompile the crate itself
#   2. BuildKit cache mount on the cargo registry dir, so crate downloads are
#      reused even when the layer above is invalidated
COPY service/Cargo.toml service/Cargo.lock* ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    mkdir -p src \
    && echo 'fn main() {}' > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Copy the real sources and build incrementally
COPY service/src/ src/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release

# ---------------------------------------------------------------------------
# Stage 3: Runtime (debian-slim)
# ---------------------------------------------------------------------------
FROM ${RUNTIME_IMAGE} AS runtime

ENV DEBIAN_FRONTEND=noninteractive
ARG APT_MIRROR=mirrors.aliyun.com

# Point apt at a domestic Debian mirror to avoid slow/unreachable deb.debian.org
# (override via --build-arg APT_MIRROR, e.g. mirrors.tuna.tsinghua.edu.cn)
RUN sed -i "s|deb\.debian\.org|${APT_MIRROR}|g" /etc/apt/sources.list.d/*.sources 2>/dev/null; \
    sed -i "s|deb\.debian\.org|${APT_MIRROR}|g" /etc/apt/sources.list 2>/dev/null; \
    true

# Runtime deps: CA certs (cascaded WMS/HTTPS), OpenSSL runtime lib (native-tls),
# curl (health check)
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
        curl \
    && rm -rf /var/lib/apt/lists/*

# Run as a non-root user (container security baseline)
RUN groupadd --system --gid 10001 terrane \
    && useradd --system --uid 10001 --gid terrane terrane

WORKDIR /app

COPY --from=builder   /build/target/release/terrane /app/terrane
COPY --from=frontend  /app/dist/terrane-ui          /app/static
# Built-in sample data (seeded into /data/samples on first startup)
COPY service/samples/ /app/samples/

# Data dir: metadata sqlite + business data + tile cache + uploads (mountable volume)
RUN mkdir -p /data \
    && chown -R terrane:terrane /app /data

# 12-Factor config: listen on 0.0.0.0 inside the container, persist data under
# /data (all overridable via env). NOTE: api_context is a required field with no
# default - it MUST be provided here or config loading falls back to defaults
# Dev-only JWT secret; OVERRIDE with a strong random value in production
# (e.g. TERRANE__SECURITY__JWT_SECRET=... on docker run / K8s Secret)
ENV TERRANE__SERVER__HOST=0.0.0.0 \
    TERRANE__SERVER__PORT=8080 \
    TERRANE__SERVER__API_CONTEXT=/terrane \
    TERRANE__SERVER__STATIC_DIR=/app/static \
    TERRANE__DATA_DIR=/data \
    TERRANE__METADATA__SQLITE_PATH=/data/terrane.sqlite \
    TERRANE__CACHE__CACHE_DIR=/data/gwc \
    TERRANE__CACHE__META_DIR=/data/gwc/meta \
    TERRANE__SECURITY__JWT_SECRET=terrane-dev-secret \
    RUST_LOG=info

EXPOSE 8080

USER terrane

# Readiness probe: /health/ready is registered on the root path, decoupled from api_context
HEALTHCHECK --interval=30s --timeout=3s --start-period=15s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/health/ready || exit 1

CMD ["/app/terrane"]
