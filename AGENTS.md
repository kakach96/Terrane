# Terrane — agent guidelines

## Overview

- **Goal**: Terrane — cloud-native spatial data server powered by Rust and Angular (a modern re-implementation of GeoServer)
- **Cloud-native**: target is containerized deployment (Docker / Kubernetes); see the cloud-native roadmap in `docs/IMPLEMENTATION_PLAN.md` §6

## Documentation

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — design rationale, module dependency graph, data flow, API contracts
- [docs/ROADMAP.md](docs/ROADMAP.md) — milestones, quarterly plan, known technical debt, future vision
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) — local setup, environment variables, git commit conventions

## Project structure

```
src/              — Rust backend (Actix-web)
  handlers/       — REST + OGC (WMS/WFS/WCS) handlers
  models/         — Shared data structs (Layer, Feature, DataSource, etc.)
  store/          — SQLite store (sqlite_store.rs) with PostGIS extension
    file_store.rs — File store abstraction (LocalFileStore + S3FileStore via rust-s3)
    cache/        — Cache abstraction (TileCacheBackend + SessionCache; local disk/in-memory)
  utils/          — Rendering + format readers
    bitmap_font.rs — built-in 5×7 bitmap font for SLD label (TextSymbolizer) rendering (zero deps)
    gml.rs        — shared GML serialization helpers (escape_xml, GeoJSON→GML, GML 3.2 feature) used by WFS GetFeature/GetGmlObject and WMS GetFeatureInfo
    mosaic.rs     — ImageMosaic data source: raster-directory granule scan + bounds + composite
    pyramid.rs    — ImagePyramid data source: numeric level subdirs + resolution-based level selection
    wfs_lock.rs   — WFS feature-lock registry (LockFeature / GetFeatureWithLock): in-memory, TTL expiry, lockAction ALL/SOME
  routes.rs       — All route registrations in one file
frontend/         — Angular 17 + Material
  src/app/
    models/       — TypeScript interfaces (geoserver.models.ts)
    services/     — geoserver.service.ts (API client)
    components/   — One dir per page (dashboard, layers, layer-detail, ...)
# Cloud-native files (created; see docs/IMPLEMENTATION_PLAN.md §6):
#   Dockerfile          — multi-stage image build
#   .dockerignore       — exclude target/, node_modules/, static/
#   build/docker-compose.yml  — local dev deps (postgres + redis + minio [+ app])
# TODO: .github/workflows/  — CI pipeline (not yet created)
```

## Key commands

### Build(Windows)

```powershell
# Full
./build/build.bat

# Frontend Only
cd frontend
npm run build

# Backend Only
cargo build
```

### Build(Unix like)

```bash
# Full
./build/build.sh

# Frontend Only
cd frontend && npm run build

# Backend Only
cargo build
```

## Important quirks

- **API base path**: `/geoserver` (configurable in `terrane.toml: api_context`)
- **Frontend build**: `optimization.fonts = false` (offline env), Google Fonts loaded at runtime
- **Storage**: config keeps only `[metadata]` (SQLite / PostgreSQL; legacy `[database]` accepted as a `[metadata]` alias). Vector / raster file data sources are registered **per data source** (persisted in the metadata store) with `file_path` + `file_storage_type` (`local` / `s3` / `oss`). `local` stores the absolute path on the server; `s3` stores an object key and requires `s3_endpoint`/`s3_region`/`s3_bucket`/`s3_access_key`/`s3_secret_key` (`s3_*` persisted in `data_sources`). S3 reads are wired into GeoJSON / Shapefile / GeoPackage / GeoTIFF (WCS) / WorldImage / ArcGrid / **ImageMosaic / ImagePyramid** via `src/store/file_resolver.rs` (downloads to a temp file when readers need a path). **ImageMosaic** (`DataSourceType::ImageMosaic`, `src/utils/mosaic.rs`): `file_path` = a directory of raster granules (GeoTIFF/WorldImage/ArcGrid/PNG/JPEG) served as one coverage — WCS GetCoverage + WMS GetMap + tile pipelines composite the intersecting granules. **ImagePyramid** (`DataSourceType::ImagePyramid`, `src/utils/pyramid.rs`): `file_path` = a directory of numeric level subdirs `0/1/2/…`; the level whose ground resolution best matches the request is selected and its granules composited (same WCS/WMS/tile surfaces). Browse endpoints: `GET /data-sources/browse` (server directory) + `POST /data-sources/s3/browse` (bucket listing) power the frontend directory picker. PostGIS data sources use `deadpool_postgres` pools cached in `AppState.pg_pools`; **MySQL** data sources (`DataSourceType::Mysql`) use `mysql_async` pools cached in `AppState.mysql_pools`, with MBRIntersects spatial filtering + ST_AsGeoJSON geometry output (same feature/WMS query paths as PostGIS, `is_database()` includes both); **MongoDB** data sources (`DataSourceType::Mongo`) use `mongodb` clients cached in `AppState.mongo_clients`, reading GeoJSON documents from a collection (`geometry`/`geom` field) with `$geoWithin` bbox filtering and a `ping` connection test. Tile + session cache stay local by default (`AppState.tile_cache` + `AppState.session_cache`). **Redis 缓存数据源**: `DataSourceType::Redis` persisted in `data_sources` (host/port/database/username/password → `redis://` URL via `src/store/cache/redis.rs::redis_url_from_connection`); a tile layer opts into it via `Layer.cache_store` (REST create/update + `layers.cache_store` column), and `AppState::tile_cache_for` lazily builds a `RedisTileCacheBackend`-backed `TileCache` per data source (shared across replicas); tile render paths (`render_tile_bytes` / `get_tile`) resolve per-layer. Session management stays simple JWT (no Redis session cache). **Terrane is a data publishing platform**: business data lives in external stores
(PostGIS tables / data files) registered per data source, and Terrane focuses on
publishing & OGC protocol adaptation. WFS-T / WCS-T writes are not implemented yet
(WFS-T planned for a later milestone; WCS-T deferred)
- **Built-in `metadata` data source** is treated as an ordinary data source (no special-casing in feature queries): besides storing catalog metadata it also publishes business data. In `postgres` metadata mode it reuses the same PG with PostGIS semantics (`layer.store = "metadata"`, `layer.native_name` = table name); in `sqlite` mode it carries no business tables (feature queries return empty)
- **Layer <-> DB mapping**: `layer.store` = data source name, `layer.native_name` = DB table name
- **Coordinates / preview**: WMS `BBOX` is always in the request SRS. Backend reprojection is generic: PostGIS `ST_Transform` for raster/feature output (any EPSG), and `src/utils/geometry.rs::transform_coordinates` uses `proj4rs` for file layers / in-process math (fallback: built-in WGS84/Mercator). Frontend converts native bounds to the selected CRS (`frontend/src/app/utils/coords.ts`) so previews stay in range after switching CRS
- **Boundary representation**: GET `/layers/{name}` returns `native_bounds.bounds.{minx,miny,maxx,maxy}`; the list endpoint returns `bounds` at top level
- **No test suite** configured (no test dependencies in Cargo.toml, Angular `ng test` untested)
- **Windows-native** (build/build.bat, PowerShell). `cargo run` expects `./static/` with built frontend
- **Config**: `terrane.toml` optional; defaults work without it. Environment variables: `RUST_LOG`, `GEOSERVER__SERVER__HOST` etc. (double-underscore separator). `load_from_file()` already mounts the `GEOSERVER__` env source, so env overrides also work with `--config`.
- **Frontend proxy**: `proxy.conf.json` routes `/api`, `/wms`, `/wfs`, `/wcs` to `http://localhost:8080`
- **AGENTS.md** is the single instruction file (no .cursorrules). `.github/copilot-instructions.md` is a thin entry point that points back to AGENTS.md, so GitHub Copilot contexts (e.g. GitHub.com / PRs) pick up the same guidance.

## Cloud-native status (see docs/IMPLEMENTATION_PLAN.md §6)

- **Containerized**: multi-stage `Dockerfile` (node → rust → debian-slim, non-root) + `.dockerignore` + `build/docker-compose.yml`; no CI pipeline yet
  ```bash
  docker build -t terrane:latest .
  docker compose -f build/docker-compose.yml up -d                        # dev deps: postgres + redis + minio
  docker compose -f build/docker-compose.yml --profile terrane up -d      # dev deps + terrane app
  ```
- **JWT secret has a hardcoded default** in `src/auth.rs` (`terrane-jwt-secret-2026`); inject via env `GEOSERVER__SECURITY__JWT_SECRET` in multi-replica prod (docker-compose passes `GEOSERVER_JWT_SECRET`)
- **Runtime host**: default `127.0.0.1`; Docker image sets `GEOSERVER__SERVER__HOST=0.0.0.0`
- **State**: metadata in SQLite (`geoserver.sqlite`) + vector data (default local dir `<data_dir>/business`, or PostgreSQL) + raster files (default `<data_dir>/rasters`) + in-memory caches (`Arc<RwLock<...>>` in `src/state.rs`) + session cache (in-memory) + local disk tile cache (`./data/gwc`) + uploads (`./data`) → multi-replica needs shared storage or PostgreSQL / object-storage backends
- **Observability**: stdout logs (human-readable by default, structured JSON via `[logging] format = "json"`; request-level `trace_id` attached via `X-Trace-Id` / `X-Request-Id`); split probes `/health/live` + `/health/ready` (registered on root path, decoupled from `api_context`); Prometheus `/metrics` (root path; requests/errors, tile cache hit rate, PG pool watermarks, system). Legacy `/health` & `/monitor/*` retained
- **Lifecycle**: SIGTERM/SIGINT graceful shutdown + `shutdown_timeout_secs` (default 30s, `[server].shutdown_timeout_secs`)

## Code conventions

- **Comments & descriptions in English**: all comments, docstrings, file headers
  and descriptions in newly added files MUST be written in English (applies to
  Docker files, config templates, scripts, and code comments alike)

## Commit Messages Stype

- **Format**: `type: changes content`, 
  - `type` is feat, fix, refactor, chore etc; 
  - `changes content` should be brief description of what has been changed. 