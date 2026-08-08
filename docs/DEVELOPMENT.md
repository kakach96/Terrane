# Terrane — Development Guide

> Local setup, environment variables, and contribution conventions.

## 1. Prerequisites

- **Rust** 1.95+ (stable toolchain)
- **Node.js** 18+
- **npm** 9+
- (optional) **Docker** for containerized runs / PostgreSQL

## 2. Repository Layout

```
src/               Rust backend (Actix-web)
  handlers/        REST + OGC handlers
  models/          Shared data structs
  services/        OGC service implementations (wms/wfs/wcs/wmts)
  store/           Store (metadata) + Vector/Raster/Cache store abstractions
  utils/           Rendering, tile cache, format parsers, projection
  routes.rs        Single-file route registration
frontend/          Angular 17 + Material admin UI
docs/              Architecture / Roadmap / Development guides
static/            Built frontend served by the backend
```

## 3. Local Startup

### Option A — One-click (recommended)

```bash
cargo run
# builds the frontend automatically, then serves on http://127.0.0.1:8080
```

### Option B — Development mode (separated, hot reload)

Terminal 1 — backend (skips frontend build):

```powershell
# Windows PowerShell
$env:SKIP_FRONTEND=1
cargo run
```

```bash
# Unix-like
SKIP_FRONTEND=1 cargo run
```

Terminal 2 — frontend dev server:

```bash
cd frontend
npm install
npm start        # http://localhost:4200 (proxies /api, /wms, /wfs, /wcs to :8080)
```

### Option C — Full build

```bash
# Windows
./build.bat
# Unix-like
./build.sh

# Backend only
cargo build
# Frontend only
cd frontend && npm run build
```

### Option D — Docker

```bash
docker compose up -d                        # SQLite standalone
docker compose --profile postgres up -d     # app + PostgreSQL (PostGIS)
```

> Default admin account: `admin` / `geoserver`.

## 4. Environment Variables

`GEOSERVER__<SECTION>__<FIELD>` (double underscore). Precedence:
**CLI flags > env vars > terrane.toml > built-in defaults**.

| Variable                                  | Maps to                                      | Default                    |
|-------------------------------------------|----------------------------------------------|----------------------------|
| `RUST_LOG`                                | log level (overrides `[logging].level`)      | `info`                     |
| `GEOSERVER__DATA_DIR`                     | `data_dir`                                   | `./data`                   |
| `GEOSERVER__SERVER__HOST`                 | `[server] host`                              | `127.0.0.1`                |
| `GEOSERVER__SERVER__PORT`                 | `[server] port`                              | `8080`                     |
| `GEOSERVER__SERVER__API_CONTEXT`          | `[server] api_context`                       | `/geoserver`               |
| `GEOSERVER__SERVER__STATIC_DIR`           | `[server] static_dir`                        | `./static`                 |
| `GEOSERVER__SERVER__CONNECT_TIMEOUT_SECS` | `[server] connect_timeout_secs`              | `10`                       |
| `GEOSERVER__SERVER__SHUTDOWN_TIMEOUT_SECS`| `[server] shutdown_timeout_secs`             | `30`                       |
| `GEOSERVER__METADATA__KIND`               | `[metadata] kind` (`sqlite`\|`postgres`)     | `sqlite`                   |
| `GEOSERVER__METADATA__SQLITE_PATH`        | `[metadata] sqlite_path`                     | `geoserver.sqlite`         |
| `GEOSERVER__METADATA__POSTGRES__HOST`     | `[metadata.postgres] host`                   | `127.0.0.1`                |
| `GEOSERVER__METADATA__POSTGRES__PORT`     | `[metadata.postgres] port`                   | `5432`                     |
| `GEOSERVER__METADATA__POSTGRES__INSTANCE` | `[metadata.postgres] instance`               | `geoserver`                |
| `GEOSERVER__METADATA__POSTGRES__SCHEMA`   | `[metadata.postgres] schema`                 | `public`                   |
| `GEOSERVER__METADATA__POSTGRES__USER`     | `[metadata.postgres] user`                   | `postgres`                 |
| `GEOSERVER__METADATA__POSTGRES__PASSWORD` | `[metadata.postgres] password`               | `` (empty)                 |
| `GEOSERVER__METADATA__POSTGRES__POOL_SIZE`| `[metadata.postgres] pool_size`              | `10`                       |
| `GEOSERVER__VECTOR__KIND`                 | `[vector] kind` (`local`\|`metadata`\|`postgres`) | `local`             |
| `GEOSERVER__VECTOR__DIR`                  | `[vector] dir`                               | `<data_dir>/business`      |
| `GEOSERVER__VECTOR__POSTGRES__*`          | `[vector.postgres] *`                        | (mirrors metadata.postgres)|
| `GEOSERVER__RASTER__KIND`                 | `[raster] kind` (`local`)                    | `local`                    |
| `GEOSERVER__RASTER__DIR`                  | `[raster] dir`                               | `<data_dir>/rasters`       |
| `GEOSERVER__SECURITY__JWT_SECRET`         | `[security] jwt_secret`                      | `terrane-jwt-secret-2026`|
| `GEOSERVER__LOGGING__LEVEL`               | `[logging] level`                            | `info`                     |
| `GEOSERVER__CACHE__KIND`                  | `[cache] kind` (`local`)                     | `local`                    |
| `GEOSERVER__CACHE__CACHE_DIR`             | `[cache] cache_dir`                          | `<data_dir>/gwc`           |
| `GEOSERVER__CACHE__META_DIR`              | `[cache] meta_dir`                           | `<data_dir>/gwc/meta`      |
| `GEOSERVER__CACHE__EXPIRE_AFTER_SECS`     | `[cache] expire_after_secs`                  | `86400`                    |
| `GEOSERVER__CACHE__MAX_TILES`             | `[cache] max_tiles`                          | `100000`                   |
| `GEOSERVER__CACHE__ENABLED`               | `[cache] enabled`                            | `true`                     |
| `GEOSERVER__CACHE__DEFAULT_GRIDSET`       | `[cache] default_gridset`                    | `EPSG:4326`                |
| `GEOSERVER__CACHE__SESSION_TTL_SECS`      | `[cache] session_ttl_secs`                   | `86400`                    |
| `GEOSERVER__CORS__ENABLED`                | `[cors] enabled`                             | `true`                     |
| `GEOSERVER__CORS__ALLOWED_ORIGINS`        | `[cors] allowed_origins`                     | `["*"]`                    |
| `GEOSERVER__CORS__ALLOWED_METHODS`        | `[cors] allowed_methods`                     | GET/POST/PUT/DELETE/OPTIONS/PATCH |
| `GEOSERVER__CORS__ALLOWED_HEADERS`        | `[cors] allowed_headers`                     | `["*"]`                    |
| `GEOSERVER__CORS__ALLOW_CREDENTIALS`      | `[cors] allow_credentials`                   | `true`                     |
| `GEOSERVER__CORS__MAX_AGE`                | `[cors] max_age`                             | `3600`                     |

### CLI flags

| Flag                | Description                          |
|---------------------|--------------------------------------|
| `--config <name>`   | Config file name/path (default `terrane`) |
| `--host <host>`     | Override `[server] host`             |
| `-p, --port <port>` | Override `[server] port`             |

## 5. Docker / Compose

- Multi-stage `Dockerfile`: node build → rust release build → debian-slim runtime (non-root).
- The image sets `GEOSERVER__SERVER__HOST=0.0.0.0`; data persists on the `geoserver-data` volume.
- `GEOSERVER_JWT_SECRET` must be injected (via `.env` or env) in production — all replicas must share it.
- Built-in `HEALTHCHECK` hits `/health/ready`; graceful shutdown drains in-flight requests (`shutdown_timeout_secs`).

```bash
docker build -t terrane:latest .
docker compose up -d
docker compose --profile postgres up -d
```

## 6. Git Conventions

### Commit message format

```
type: changes content
```

- `type` is one of `feat`, `fix`, `refactor`, `chore`, etc.
- `changes content` is a brief description of what changed.

Examples:

```
feat: add Redis session store for cloud-native mode
fix: correct WMS GetCapabilities XML escaping
refactor: extract VectorStore trait from local_dir store
chore: bump actix-web to 4.x
```

### Code conventions

- All comments, docstrings, and file headers in **new** files **must be written in English**.
- `AGENTS.md` is the single instruction file (no `.cursorrules`, no `copilot-instructions.md`).

## 7. Testing

- Run `cargo test` for the full suite: **123 lib unit tests + 99 integration tests**
  (+ **5 `#[ignore]` live tests** that require running services, run with
  `cargo test -- --ignored`: 3× PostGIS via `GEOSERVER_TEST_PG_*` env, 2×
  CascadedWms against the reference GeoServer at :18080).
- Integration tests are split by protocol into separate crates under `tests/`
  (`wms_test.rs`, `wfs_test.rs`, `wcs_test.rs`, `wmts_test.rs`, `tms_test.rs`,
  `rest_test.rs`), sharing `tests/common/mod.rs` helpers (`create_test_config` +
  `build_test_app!`).
  The test config uses in-memory SQLite and disables the tile cache, so tests
  write nothing to `./data`. Full coverage matrix: see [PROTOCOLS.md](PROTOCOLS.md) §10.
- Adding `#[cfg(test)]` unit tests and actix-rt integration tests is planned — see [ROADMAP.md](ROADMAP.md).

## 8. Troubleshooting

- **Backend can't find the UI** → ensure `./static/` contains a built frontend, or run `SKIP_FRONTEND=1 cargo run` together with `npm start` on :4200.
- **Config error falls back silently** → a `[config] WARNING` is printed to stderr; keep required keys complete (`[server] host/port/api_context`, `[[workspaces]]`, `[cache] cache_dir/meta_dir` when present).
- **Port 5432 conflict** → docker-compose maps PostgreSQL to host port `5433`.
- **Frontend proxy not working** → verify `proxy.conf.json` routes and that the backend runs on `:8080`.
