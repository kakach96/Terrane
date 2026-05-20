# Rust GeoServer — agent guidelines

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
```

## Key commands

```powershell
# Dev mode (fast: skip frontend build)
$env:SKIP_FRONTEND=1; cargo run
# Frontend dev server (hot reload, proxies /api /wms /wfs /wcs -> :8080)
cd frontend; ng serve

# Full production build
cargo run                        # auto-builds frontend first
cd frontend; npm run build       # frontend only (--configuration production)
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
- **Frontend proxy**: `proxy.conf.json` routes `/api`, `/wms`, `/wfs`, `/wcs` to `http://localhost:8080`
- **AGENTS.md** is the single instruction file (no .cursorrules, no copilot-instructions.md)
