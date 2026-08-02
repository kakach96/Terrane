# GeoFerris — agent guidelines

## Overview

- **Goal**: GeoFerris — cloud-native spatial data server powered by Rust and Angular (a modern re-implementation of GeoServer)
- **Cloud-native**: target is containerized deployment (Docker / Kubernetes); see the cloud-native roadmap in `IMPLEMENTATION_PLAN.md` §7

## Project structure

```
src/              — Rust backend (Actix-web)
  handlers/       — REST + OGC (WMS/WFS/WCS) handlers
  models/         — Shared data structs (Layer, Feature, DataSource, etc.)
  store/          — SQLite store (sqlite_store.rs) with PostGIS extension
    business/     — Business data store abstraction (local dir / PostgreSQL / metadata reuse)
  routes.rs       — All route registrations in one file
frontend/         — Angular 17 + Material
  src/app/
    models/       — TypeScript interfaces (geoserver.models.ts)
    services/     — geoserver.service.ts (API client)
    components/   — One dir per page (dashboard, layers, layer-detail, ...)
# Planned for cloud-native (not yet created):
#   Dockerfile          — multi-stage image build
#   .dockerignore       — exclude target/, node_modules/, static/
#   docker-compose.yml  — local dev (app + postgres [+ minio])
#   .github/workflows/  — CI pipeline
```

## Key commands

### Build(Windows)

```powershell
# Full
./build.bat

# Frontend Only
cd frontend
npm run build

# Backend Only
cargo build
```

### Build(Unix like)

```bash
# Full
./build.sh

# Frontend Only
cd frontend && npm run build

# Backend Only
cargo build
```

## Important quirks

- **API base path**: `/geoserver` (configurable in `geoferris.toml: api_context`)
- **Frontend build**: `optimization.fonts = false` (offline env), Google Fonts loaded at runtime
- **Storage split**: metadata store (`AppState.store`, default SQLite, `[metadata]`) vs business data store (`AppState.business_store`, `[business]`). Defaults: metadata=sqlite → business=local dir (`<data_dir>/business`, one GeoJSON per layer); metadata=postgres → business=metadata (reuse, built-in default data source option). Legacy `[database]` config is accepted as a `[metadata]` alias. PostGIS data sources use `deadpool_postgres` pools cached in `AppState.pg_pools`
- **Layer <-> DB mapping**: `layer.store` = data source name, `layer.native_name` = DB table name
- **Boundary representation**: GET `/layers/{name}` returns `native_bounds.bounds.{minx,miny,maxx,maxy}`; the list endpoint returns `bounds` at top level
- **No test suite** configured (no test dependencies in Cargo.toml, Angular `ng test` untested)
- **Windows-native** (build.bat, PowerShell). `cargo run` expects `./static/` with built frontend
- **Config**: `geoferris.toml` optional; defaults work without it. Environment variables: `RUST_LOG`, `GEOSERVER__SERVER__HOST` etc. (double-underscore separator). `load_from_file()` already mounts the `GEOSERVER__` env source, so env overrides also work with `--config`.
- **Frontend proxy**: `proxy.conf.json` routes `/api`, `/wms`, `/wfs`, `/wcs` to `http://localhost:8080`
- **AGENTS.md** is the single instruction file (no .cursorrules, no copilot-instructions.md)

## Cloud-native status (see IMPLEMENTATION_PLAN.md §7)

- **Containerized**: multi-stage `Dockerfile` (node → rust → debian-slim, non-root) + `.dockerignore` + `docker-compose.yml`; no CI pipeline yet
  ```bash
  docker build -t geoferris:latest .
  docker compose up -d                    # SQLite standalone mode (default)
  docker compose --profile postgres up -d # app + PostgreSQL
  ```
- **JWT secret has a hardcoded default** in `src/auth.rs` (`geoferris-jwt-secret-2026`); inject via env `GEOSERVER__SECURITY__JWT_SECRET` in multi-replica prod (docker-compose passes `GEOSERVER_JWT_SECRET`)
- **Runtime host**: default `127.0.0.1`; Docker image sets `GEOSERVER__SERVER__HOST=0.0.0.0`
- **State**: metadata in SQLite (`geoserver.sqlite`) + business data (default local dir `<data_dir>/business`, or PostgreSQL) + in-memory caches (`Arc<RwLock<...>>` in `src/state.rs`) + local disk tile cache (`./data/gwc`) + uploads (`./data`) → multi-replica needs shared storage or PostgreSQL / object-storage backends
- **Observability**: stdout logs (human-readable, not JSON); split probes `/health/live` + `/health/ready` (registered on root path, decoupled from `api_context`); Prometheus `/metrics` (root path; requests/errors, tile cache hit rate, PG pool watermarks, system). Legacy `/health` & `/monitor/*` retained
- **Lifecycle**: SIGTERM/SIGINT graceful shutdown + `shutdown_timeout_secs` (default 30s, `[server].shutdown_timeout_secs`)

## Code conventions

- **Comments & descriptions in English**: all comments, docstrings, file headers
  and descriptions in newly added files MUST be written in English (applies to
  Docker files, config templates, scripts, and code comments alike)

## Commit Messages Stype

- **Format**: `type: changes content`, 
  - `type` is feat, fix, refactor, chore etc; 
  - `changes content` should be brief description of what has been changed. 