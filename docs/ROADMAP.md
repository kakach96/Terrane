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
- Data sources: PostGIS, Shapefile, GeoTIFF, GeoPackage, GeoJSON, WorldImage, CascadedWms, ArcGrid
- Security: JWT auth, users/roles, layer-level permissions
- Angular 17 + Material admin console
- Cloud-native foundation: multi-stage Docker image, `build/docker-compose.yml`, split health probes, Prometheus `/metrics`, graceful shutdown
- Overall progress: ~58% (see [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md))

### v1.1 — State Convergence (target: 2026 Q3–Q4)

- Metadata + vector data on PostgreSQL (PostGIS) for HA / multi-replica
- Session storage: Redis (cloud) / in-memory (standalone)
- Storage config simplified: config keeps only `[metadata]`; vector/raster file data sources registered per data source (`file_path` + `file_storage_type`) with `FileStore` abstraction (`src/store/file_store.rs`); cache stays local (`TileCacheBackend` + `SessionCache`)
- Tile cache backend abstraction: local disk backend done; Redis / object storage pending
- Raster data backend abstraction: local files backend done; MinIO / S3 pending
- Session cache: local in-memory backend done; Redis pending
- Catalog refresh mechanism to converge in-memory caches across replicas
- Structured JSON logs + optional OpenTelemetry tracing
- CI/CD pipeline (GitHub Actions / GitLab CI): fmt + clippy + test + frontend build + image push

### v2.0 — Fully Stateless & Extensions (target: 2027 Q1–Q2)

- Fully stateless services (no in-memory divergence; all state external)
- Resilience hardening: rate limiting, request timeouts, circuit breaking, upstream retry/backoff for cascaded WMS
- Enterprise extensions: WPS, CSW, OGC API (Features / Tiles / Maps / Coverages), Printing, Importer, GeoFence
- Plugin architecture for data sources and handlers
- Object storage for uploaded files (PVC / MinIO / S3)

## Quarterly Timeline

| Quarter    | Focus                                                                                        |
|------------|----------------------------------------------------------------------------------------------|
| 2026 Q3    | Redis session & cache integration; PostgreSQL HA hardening; object-storage abstraction for raster |
| 2026 Q4    | CI/CD pipeline; structured logging / OTel; catalog refresh & stateless convergence; v1.1 release |
| 2027 Q1    | Resilience middleware (rate limit / timeout / circuit breaker); WPS / CSW foundation          |
| 2027 Q2    | OGC API series + Printing / Importer; plugin architecture; v2.0 release                      |

## Known Technical Debt

- **JWT secret hardcoded default** in `src/auth.rs` (`terrane-jwt-secret-2026`) — must be injected via `GEOSERVER__SECURITY__JWT_SECRET` in production.
- **In-memory caches** (`Arc<RwLock<...>>` in `src/state.rs`) diverge across replicas; no refresh mechanism yet.
- **Tile cache backend is disk-only** (`./data/gwc`, `src/store/cache/tile.rs`) — the `TileCacheBackend` trait exists but no Redis / S3 backend yet.
- **Uploads on local disk** (`./data`) — no shared volume / object storage.
- **Sessions persisted in the metadata DB** (SQLite / PostgreSQL) with a local in-memory `SessionCache` fast-path — Redis backend pending.
- **No CI pipeline / image registry push** (`.github/workflows/ci.yml` created — fmt + clippy + test + frontend build + GHCR push; not yet exercised against a real repository).
- **Human-readable stdout logs only** — no structured JSON, no OpenTelemetry.
- **Resilience gaps**: rate limiting + request timeout middleware added (`src/middleware.rs`, `[server]` config, HTTP 429/504); no circuit breaking and no retry/backoff for cascaded WMS upstreams yet.
- **No test suite** (no test deps in `Cargo.toml`; Angular `ng test` untested).
- **Security-sensitive defaults**: CORS `["*"]` and hardcoded JWT secret — revisit before production.
- **Broken doc link**: README referenced `BUILD_INTEGRATION.md`, which did not exist (fixed in this docs pass).

## Future Vision

- **Redis-first cloud mode**: session, tile cache, and hot caches in Redis; stateless replicas scale horizontally behind a load balancer.
- **Object-storage raster**: GeoTIFF / WorldImage / ArcGrid served from MinIO / S3 / GCS through a coverage-store abstraction.
- **Protocol-adapter purity**: the binary focuses on OGC / REST protocol adaptation while all state lives in external stores.
- **Enterprise ecosystem**: WPS processing, CSW catalog, OGC API series, printing, importer, GeoFence fine-grained access control.
- **Plugin architecture**: trait-based data source and handler plugins, mirroring GeoServer's extension model.
