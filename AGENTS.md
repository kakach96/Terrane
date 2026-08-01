# Rust GeoServer — agent guidelines

## Overview

- **Goal**: rewrite geoserver by rust and angular
- **Cloud-native**: target is containerized deployment (Docker / Kubernetes); see the cloud-native roadmap in `IMPLEMENTATION_PLAN.md` §7

## Project structure

```
src/              — Rust backend (Actix-web)
  handlers/       — REST + OGC (WMS/WFS/WCS) handlers
  models/         — Shared data structs (Layer, Feature, DataSource, etc.)
  store/          — SQLite store (sqlite_store.rs) with PostGIS extension
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

- **API base path**: `/geoserver` (configurable in `geoserver.toml: api_context`)
- **Frontend build**: `optimization.fonts = false` (offline env), Google Fonts loaded at runtime
- **Two layer stores**: in-memory (default) OR SQLite (`state.store`). When SQLite is active, PostGIS data sources use `deadpool_postgres` pools cached in `AppState.pg_pools`
- **Layer <-> DB mapping**: `layer.store` = data source name, `layer.native_name` = DB table name
- **Boundary representation**: GET `/layers/{name}` returns `native_bounds.bounds.{minx,miny,maxx,maxy}`; the list endpoint returns `bounds` at top level
- **No test suite** configured (no test dependencies in Cargo.toml, Angular `ng test` untested)
- **Windows-native** (build.bat, PowerShell). `cargo run` expects `./static/` with built frontend
- **Config**: `geoserver.toml` optional; defaults work without it. Environment variables: `RUST_LOG`, `GEOSERVER__SERVER__HOST` etc. (double-underscore separator)
  - ⚠️ Cloud-native gap: the `GEOSERVER__` env source exists in `config::GeoServerConfig::load()` but `main.rs` uses `load_from_file()` (no env source). Unify before containerizing.
- **Frontend proxy**: `proxy.conf.json` routes `/api`, `/wms`, `/wfs`, `/wcs` to `http://localhost:8080`
- **AGENTS.md** is the single instruction file (no .cursorrules, no copilot-instructions.md)

## Cloud-native status (see IMPLEMENTATION_PLAN.md §7)

- **Not yet containerized**: no Dockerfile / docker-compose / CI pipeline (planned, not created)
- **JWT secret is hardcoded** in `src/auth.rs` (`rust-geoserver-jwt-secret-2026`); must become env-injected (`GEOSERVER__SECURITY__JWT_SECRET`) before multi-replica prod
- **Default host `127.0.0.1`** — containers must bind `0.0.0.0`
- **State**: metadata in SQLite (`geoserver.sqlite`) + in-memory caches (`Arc<RwLock<...>>` in `src/state.rs`) + local disk tile cache (`./data/gwc`) + uploads (`./data`) → multi-replica needs shared storage or PostgreSQL / object-storage backends
- **Observability**: stdout logs (human-readable, not JSON), single `/health` probe (no liveness/readiness split), no Prometheus `/metrics` (monitoring is in-memory JSON via `/server/status`)
- **Lifecycle**: no SIGTERM graceful shutdown / `shutdown_timeout()`

## Commit Messages Stype

- **Format**: `type: changes content`, 
  - `type` is feat, fix, refactor, chore etc; 
  - `changes content` should be brief description of what has been changed. 