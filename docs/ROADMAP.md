# Terrane — Roadmap

> Product roadmap, milestone plan, known technical debt, and future vision.
> Complements [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) (feature-gap analysis),
> [ARCHITECTURE.md](ARCHITECTURE.md) (design rationale), and
> [PROTOCOLS.md](PROTOCOLS.md) (protocol adaptation matrix).

## Vision

Terrane is a **cloud-native, high-performance geospatial data server**,
re-implementing the GeoServer feature set in Rust.

The core architectural vision is a **dual-mode** design — one codebase, two deployment
profiles:

| Dimension  | Standalone (local / dev)                     | Cloud-Native (production)                    |
|------------|----------------------------------------------|----------------------------------------------|
| Metadata   | SQLite                                       | PostgreSQL (PostGIS)                         |
| Vector data| Per-datasource: PostGIS tables / GeoJSON / Shapefile / GeoPackage files (local) | PostgreSQL (PostGIS) / object storage |
| Raster data| Local files (GeoTIFF / WorldImage / ArcGrid) | Object storage (MinIO / S3)                  |
| Session    | In-memory                                    | Redis                                        |
| Cache      | In-memory                                    | Redis                                        |
| Service    | Single process, in-process caches            | **Stateless** protocol adapter, horizontal scale |

The server focuses on **protocol adaptation** (WMS / WFS / WCS / WMTS / REST) while all
state lives in external stores, so replicas stay stateless and interchangeable.

## Milestones

### v1.0 — Current Baseline (released)

- OGC services: WMS 1.1.1/1.3.0, WFS 1.0/1.1/2.0, WCS 1.0/1.1/2.0, WMTS 1.0.0
- REST API: workspaces / namespaces / layers / stores / data sources / styles / layer groups / sql views / permissions
- Data sources: PostGIS, MySQL, MongoDB, Shapefile, GeoTIFF, GeoPackage, GeoJSON, WorldImage, CascadedWms, ArcGrid, ImageMosaic, ImagePyramid, Redis (cache backend)
- Security: JWT auth, users/roles, layer-level permissions
- Angular 17 + Material admin console
- Cloud-native foundation: multi-stage Docker image, `build/docker-compose.yml`, split health probes, Prometheus `/metrics`, graceful shutdown
- WMS rendering engine: multi-geometry + opacity compositing + polygon holes + z-order + label (TextSymbolizer) rendering with collision avoidance; raster layers (GeoTIFF/WorldImage/ArcGrid/ImageMosaic) render in WMS GetMap and tiles
- OGC core services 7/7: WMS / WFS (incl. LockFeature + GetFeatureWithLock + GetPropertyValue + GetGmlObject) / WCS (incl. range band subset + INTERPOLATION) / WMTS / WPS / CSW / OGC API series; WMS GetFeatureInfo GML output
- REST API 15/16: store / layer-group / user CRUD, workspace-dimension endpoints, service settings, about + resources, feature-type PUT, tile seed REST (Importer pending)
- Tile caching 6/6: seeding/truncate, metastore, disk quota (LRU), ETag/Last-Modified 304, custom gridsets
- Security 7/7: CORS, user/group/role (+ users PUT), REST API auth, layer permissions, frontend login, LDAP enterprise identity (login fallback + auto-provision), GeoFence fine-grained ACL (per-request layer rules)
- Cloud-native 7/7: containerization, 12-Factor config, observability (JSON logs + trace_id), lifecycle, CI/CD (Trivy + Dependabot), resilience, statelessness (env-injectable credentials + S3/MinIO uploads)
- Overall progress: ~87% (see [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md))

### v1.1 — State Convergence (target: 2026 Q3–Q4)

- Metadata + vector data on PostgreSQL (PostGIS) for HA / multi-replica
- Session storage: Redis (cloud) / in-memory (standalone) — *deferred: session management stays simple JWT + metadata store*
- Storage config simplified: config keeps only `[metadata]`; vector/raster file data sources registered per data source (`file_path` + `file_storage_type`) with `FileStore` abstraction (`src/store/file_store.rs`); cache stays local (`TileCacheBackend` + `SessionCache`)
- Tile cache backend abstraction: local disk backend done; **Redis data source backend done** (`DataSourceType::Redis` + `Layer.cache_store`, shared across replicas); **seeding/truncate, metastore, per-layer disk quota (LRU), ETag/Last-Modified 304, and custom gridsets done** (see PROTOCOLS §5.3)
- Raster data backend abstraction: local files backend done; MinIO / S3 pending
- Session cache: local in-memory backend done; Redis pending (deferred — simple JWT)
- Catalog refresh mechanism to converge in-memory caches across replicas — **periodic reload done** (`[server] catalog_refresh_secs`); **event-triggered refresh done** (REST layer update/delete now immediately reloads the in-memory catalog via `AppState::refresh_catalog`, eliminating the write-after-read-stale window for WMS/WMTS/tile paths)
- Structured JSON logs + optional OpenTelemetry tracing — **JSON logs + request `trace_id` done** (`[logging] format = "json"`, `X-Trace-Id`); OTel pending
- CI/CD pipeline (GitHub Actions / GitLab CI): fmt + clippy + test + frontend build + image push — **GitHub Actions done, incl. Trivy scan + Dependabot**
- Performance test suite — **done**: micro-benchmarks for hot paths (GML serialization, CQL parsing, coordinate transform, label rendering, map/MVT rendering) via `cargo bench` (`benches/core_paths.rs`), plus an HTTP-level load harness over REST / WMS / WFS / tiles reporting p50/p95/p99 latency & throughput (`tests/perf_test.rs`, `#[ignore]`)
- User guide documentation (`docs/USER_GUIDE.md`): install & quick start, data publishing workflow (workspace → data source → layer → style → preview → tiles), OGC service usage examples, security & deployment pointers — **done**

### v1.2 — Naming Convergence & Data Experience (target: 2026 Q4 – 2027 Q1)

> New milestone covering the four planned work items (layer-preview format parity,
> built-in sample data, database cluster connections, and the `geoserver` → `terrane`
> naming migration) plus smaller gaps found during a full codebase review.

- **Layer preview formats aligned with GeoServer** — the backend WMS GetMap already
  emits PNG / JPEG / GIF / WebP / SVG / KML / GeoJSON / GeoRSS / PDF, but the frontend
  preview only offers OpenLayers / PNG / JPEG. Plan:
  - Frontend preview: add SVG, KML, GeoJSON, PDF, GIF, WebP (backend already supports them)
  - Backend: add TIFF / Atom / UTFGrid / GML output to WMS GetMap for full GeoServer parity
  - Wire MVT (`.pbf`) preview through the existing `/tiles/{layer}/{z}/{x}/{y}.pbf` and
    `/mvt/{layer}/{z}/{x}/{y}` endpoints
  - Relax the `getMapImageUrl` format type constraint (`'image/png' | 'image/jpeg'` → all
    supported formats) and add i18n labels for every new format
- **Built-in sample data** — only `data/geojson/uploaded_layer.geojson` exists today;
  integration tests generate fixtures at runtime (GeoTIFF / ArcGrid) and the built-in
  `world` layer carries no business data in SQLite metadata mode. Plan:
  - Add a curated `data/samples/` set (GeoJSON point/line/polygon, Shapefile, GeoTIFF,
    world-map data) mirroring GeoServer's sample data
  - Seed/init mechanism: auto-register sample data sources + layers on first startup
    (opt-in via config), so the product demos out-of-the-box
  - Reuse the samples in integration tests, replacing runtime-generated fixtures
- **Database cluster connections** — `DataSourceConnection` and the PostGIS / MySQL /
  MongoDB pool builders (`src/state.rs`) only support a single `host`/`port`; the
  frontend data-source dialog exposes a single host/port too. Plan:
  - PostGIS: multi-host (comma-separated / array), read/write replica separation,
    connection-string support
  - MySQL: multi-host support
  - MongoDB: replica-set URI (`mongodb://host1,host2/db?replicaSet=…`)
  - Frontend data-source dialog: cluster fields; connection test adapted for clusters
- **`geoserver` → `terrane` naming migration (breaking change)** — the codebase still
  carries `geoserver` in type names, env vars, API paths, defaults, tests and docs. Plan:
  - Backend types: `GeoServerConfig` → `TerraneConfig`, `GeoServerError` → `TerraneError`,
    `GeoServerBackup` → `TerraneBackup`
  - Env prefix: `GEOSERVER__` → `TERRANE__` (keep `GEOSERVER__` as a deprecated alias for
    backward compatibility during the transition)
  - API context: default `/geoserver` → `/terrane`
  - Defaults: admin password `geoserver` → `terrane`, default DB name, default namespace,
    `geoserver.sqlite` → `terrane.sqlite`
  - Frontend: `geoserver.service.ts` → `terrane.service.ts`, `geoserver.models.ts` →
    `terrane.models.ts`, `GeoserverService` → `TerraneService`, `apiUrl` `/geoserver` →
    `/terrane`, localStorage keys (`geoserver_user` / `geoserver_token`)
  - Tests: all `.uri("/geoserver/…")` → `/terrane/…`; hardcoded `/geoserver/wms` in
    `src/utils/cascaded.rs`
  - Docs & config: `terrane.toml.example`, `DEVELOPMENT.md`, `USER_GUIDE.md`,
    `ARCHITECTURE.md`, `PROTOCOLS.md`, `IMPLEMENTATION_PLAN.md`, `README.md`
  - Docker/CI: `Dockerfile` + `build/docker-compose.yml` env vars,
    `GEOSERVER_JWT_SECRET` → `TERRANE_JWT_SECRET`
- **Other gaps found during review** (folded into this milestone):
  - Frontend test suite (`ng test`) not wired into CI
  - OGC CITE compliance tests (referenced in IMPLEMENTATION_PLAN §5.3, not yet run)
  - WFS-T Transaction (currently 501) — kept deferred to v2.0
  - Oracle / SQL Server data sources — kept deferred (evaluated via tiberius / native driver)
  - Importer (bulk-import workflow) — the last REST API gap
  - Printing module — kept deferred to v2.0
  - OpenTelemetry tracing — kept deferred to v2.0
  - Redis session backend — kept deferred (simple JWT by design)

### v2.0 — Fully Stateless & Extensions (target: 2027 Q1–Q2)

- Fully stateless services (no in-memory divergence; all state external)
- Resilience hardening: rate limiting, request timeouts, circuit breaking, upstream retry/backoff for cascaded WMS
- Enterprise extensions: WPS, CSW, OGC API (Features / Tiles / Maps / Coverages), Printing, Importer, GeoFence
- Plugin architecture for data sources and handlers
- Object storage for uploaded files (PVC / MinIO / S3)

## Quarterly Timeline

| Quarter    | Focus                                                                                        |
|------------|----------------------------------------------------------------------------------------------|
| 2026 Q3    | Redis session & cache integration; PostgreSQL HA hardening; object-storage abstraction for raster; performance test suite; user guide documentation |
| 2026 Q4    | CI/CD pipeline; structured logging / OTel; catalog refresh & stateless convergence; v1.1 release; layer-preview format parity; built-in sample data |
| 2027 Q1    | `geoserver` → `terrane` naming migration; database cluster connections; frontend test suite; v1.2 release |
| 2027 Q2    | Resilience middleware (rate limit / timeout / circuit breaker); WPS / CSW foundation; OGC API series + Printing / Importer; plugin architecture; v2.0 release |

## Known Technical Debt

- **JWT secret hardcoded default** in `src/auth.rs` (`terrane-jwt-secret-2026`) — must be injected via `GEOSERVER__SECURITY__JWT_SECRET` in production.
- **In-memory caches** (`Arc<RwLock<...>>` in `src/state.rs`) diverge across replicas; periodic catalog refresh added (`[server] catalog_refresh_secs`, reload layers/styles/groups from the metadata store) plus event-triggered refresh on REST layer writes (`AppState::refresh_catalog`) — multi-replica divergence is bounded, but refresh remains best-effort (no external event bus).
- **WFS feature locks are in-memory only** (`src/utils/wfs_lock.rs`, per-replica): locks guard concurrent consumers within one process and are lost on restart — acceptable while WFS-T writes are not implemented, but multi-replica lock coordination (e.g. Redis) is not implemented.
- **Tile cache backend**: local disk default (`./data/gwc`, `src/store/cache/tile.rs`); layer-level Redis cache data sources added (`Layer.cache_store` → `DataSourceType::Redis`, shared across replicas).
- **Uploads on local disk** (`./data`) — no shared volume / object storage.
- **Sessions persisted in the metadata DB** (SQLite / PostgreSQL) with a local in-memory `SessionCache` fast-path — Redis session backend intentionally not implemented (simple JWT only).
- **CI pipeline** (`.github/workflows/ci.yml` — fmt + clippy + test + frontend build + GHCR push + Trivy scan; `.github/dependabot.yml` for cargo/npm/actions) — created but not yet exercised against a real repository.
- **Human-readable stdout logs by default** — structured JSON available via `[logging] format = "json"` with request `trace_id`; OpenTelemetry still pending.
- **Resilience**: rate limiting + request timeout (`src/middleware.rs`, HTTP 429/504) and cascaded WMS retry/backoff + circuit breaking (`src/utils/cascaded.rs`) all done; no global circuit breaking for other upstream types yet.
- **Test suite** — unit (`#[cfg(test)]`) and protocol-split integration tests exist (see [DEVELOPMENT.md](DEVELOPMENT.md) §7); still missing a **performance test suite** (micro-benchmarks + HTTP load harness, planned for v1.1) and frontend tests (`ng test` untested).
- **`geoserver` naming residue** — the codebase still uses `geoserver` in type names (`GeoServerConfig` / `GeoServerError` / `GeoServerBackup`), the `GEOSERVER__` env prefix, the default `/geoserver` API context, defaults (admin password, DB name, namespace, `geoserver.sqlite`), frontend files (`geoserver.service.ts` / `geoserver.models.ts`), tests, docs and Docker/CI env vars. Planned as a breaking-change migration in v1.2 (keep `GEOSERVER__` as a deprecated alias).
- **Layer preview format gap** — the frontend preview only offers OpenLayers / PNG / JPEG while the backend WMS already emits SVG / KML / GeoJSON / GeoRSS / PDF / GIF / WebP; GeoServer parity (TIFF / Atom / UTFGrid / GML / MVT) is planned in v1.2.
- **No built-in sample data** — only `data/geojson/uploaded_layer.geojson` exists; integration tests generate fixtures at runtime and the built-in `world` layer is empty in SQLite metadata mode. A curated `data/samples/` set + first-startup seeding is planned in v1.2.
- **Database data sources are single-host only** — PostGIS / MySQL / MongoDB pools (`src/state.rs`) and the frontend dialog support one `host`/`port`; no cluster / replica-set / read-write separation. Planned in v1.2.
- **Security-sensitive defaults**: CORS `["*"]` and hardcoded JWT secret — revisit before production.
- **Broken doc link**: README referenced `BUILD_INTEGRATION.md`, which did not exist (fixed in this docs pass).

## Future Vision

- **Redis-first cloud mode**: session, tile cache, and hot caches in Redis; stateless replicas scale horizontally behind a load balancer.
- **Object-storage raster**: GeoTIFF / WorldImage / ArcGrid served from MinIO / S3 / GCS through a coverage-store abstraction.
- **Protocol-adapter purity**: the binary focuses on OGC / REST protocol adaptation while all state lives in external stores.
- **Enterprise ecosystem**: WPS processing, CSW catalog, OGC API series, printing, importer, GeoFence fine-grained access control.
- **Plugin architecture**: trait-based data source and handler plugins, mirroring GeoServer's extension model.
