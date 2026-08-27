# Terrane — Architecture

> Design rationale, module dependency graph, data flow, and API contract design.
> This document explains **why** the code is structured the way it is.

## 1. Design Goals

1. **Cloud-native** — containerized, 12-Factor configuration, observable, horizontally scalable.
2. **High performance** — Rust + Actix-web async runtime, low-latency OGC responses, tile caching.
3. **Stateless service** — the server is a pure protocol adapter; state lives in external stores.
4. **Dual-mode** — standalone (SQLite + local files + in-memory) and cloud-native (PostgreSQL + Redis + object storage).

## 2. Tech Stack & Selection Rationale

| Layer           | Choice                    | Why                                                                                               |
|-----------------|---------------------------|---------------------------------------------------------------------------------------------------|
| Language        | Rust                      | Memory safety without GC, near-C performance, single static binary, strong typing for geometry/parsers |
| Web framework   | Actix-web                 | High-throughput async HTTP, mature middleware (CORS, multipart), clean route/scope model          |
| Async runtime   | Tokio                     | Industry-standard async runtime (`full` features: signals, timers, filesystem, IO)                |
| Metadata store  | SQLite / PostgreSQL       | SQLite: zero-config standalone; PostgreSQL: HA + PostGIS spatial types for cluster deployments    |
| Data sources    | Per-datasource file backends (local / s3) | File data sources record `file_path` + `file_storage_type` + `s3_*`; PostGIS via deadpool pools; S3 via `S3FileStore` (rust-s3) |
| Cache store     | Local disk (tile) + in-memory (session); layer-level Redis via cache data source | Tile cache on disk with TTL; session fast-path in memory; `Layer.cache_store` → Redis data source backend (shared across replicas) |
| Geometry        | geo / geo-types           | Pure-Rust geometry model and spatial predicates                                                   |
| Raster rendering| image                     | Pure-Rust image encode/decode (PNG/JPEG map output, GeoTIFF read)                                 |
| DB pools        | deadpool-postgres         | Async connection pooling; pools cached per data source in `AppState.pg_pools`                     |
| Frontend        | Angular 22 + Material     | Componentized SPA, RxJS reactive data flow, Material design consistency                            |
| Serialization   | serde / serde_json / quick-xml | Fast JSON + XML (OGC capabilities documents)                                                  |
| Auth            | jsonwebtoken + sha2       | Stateless JWT auth; SHA-256 + salt password hashing                                               |

### Storage model

Terrane is a **data publishing platform**: business data lives in external
stores (PostGIS tables / data files) registered **per data source** in the metadata
store (workspaces, data sources, layer definitions, styles, permissions). The
configuration file only keeps `[metadata]` (SQLite / PostgreSQL); file data sources
record `file_path` + `file_storage_type` (`local` / `s3` / `oss`). `local` stores the
absolute path on the server; `s3` stores an object key and carries `s3_endpoint` /
`s3_region` / `s3_bucket` / `s3_access_key` / `s3_secret_key` (persisted in
`data_sources`). Reads are routed through `service/src/store/file_resolver.rs`: GeoJSON is
streamed from bytes, multi-file formats (Shapefile / WorldImage) and raster readers
(GeoPackage / GeoTIFF / ArcGrid / WCS) materialize the objects to a temp file when they
need a filesystem path. Browse endpoints (`GET /data-sources/browse` + `POST
/data-sources/s3/browse`) power the frontend directory picker. Tile + session cache
stay local by default. This keeps the
"structured data → database, raster → file storage, session/cache → Redis" scaling
vision without a complex multi-section config.

> **Built-in `metadata` data source**: treated as an ordinary data source — besides
> storing catalog metadata it also publishes business data. In `postgres` metadata mode
> it reuses the same PG with PostGIS semantics (`layer.store = "metadata"`, `native_name`
> = table name); in `sqlite` mode it carries no business tables (feature queries return empty).

### Coordinate reference systems & preview

- **Backend reprojection is generic**: WMS raster output + feature queries use PostGIS
  `ST_Transform` (storage CRS → request CRS), and the request `BBOX` is reprojected from
  the request CRS to the storage CRS before spatial filtering. Any EPSG registered in the
  metadata PostGIS (`spatial_ref_sys`) is supported (e.g. EPSG:4326, EPSG:3857, EPSG:4490,
  UTM zones). For file-based data sources and in-process math, `service/src/utils/geometry.rs`
  `transform_coordinates` is backed by `proj4rs` (generic EPSG via `crs-definitions`) with
  a built-in WGS84/Web-Mercator fallback.
- **WMS `BBOX` is always in the request SRS** (standard). The Angular frontend converts the
  layer native bounds to the selected CRS via `frontend/src/app/utils/coords.ts`
  (`transformBounds`) before building a WMS request, so previews stay in range after
  switching CRS. The OpenLayers preview uses the request CRS as the view projection; for
  projections not natively shipped by OpenLayers, the backend reprojects the BBOX to
  EPSG:4326 so the map still renders in the correct location.

## 3. Module Dependency Graph

```mermaid
graph TD
    subgraph Binary
        main[main.rs]
        config[config.rs]
        state[state.rs]
        routes[routes.rs]
        auth[auth.rs]
        backup[backup.rs]
    end

    subgraph HTTP Layer
        handlers[handlers/*]
    end

    subgraph Service Layer
        services[services/* - wms, wfs, wcs, wmts]
    end

    subgraph Storage Layer
        store[store/mod.rs - Store trait]
        vector[store/vector - VectorStore trait]
        raster[store/raster - RasterStore trait]
        cache[store/cache - TileCacheBackend + SessionCache]
        sqlite[store/sqlite_store.rs]
        postgres[store/postgres_store.rs]
        vec_local[store/vector/local_dir.rs]
        vec_pg[store/vector/postgres.rs]
        ras_local[store/raster/local.rs]
        cac_tile[store/cache/tile.rs]
        cac_session[store/cache/session.rs]
    end

    subgraph Utility Layer
        utils[utils/*]
        tile_cache[utils/tile_cache.rs]
        rendering[utils/rendering.rs]
        parsers[utils/*_parser.rs]
    end

    main --> config
    main --> state
    main --> auth
    main --> routes
    routes --> handlers
    handlers --> services
    handlers --> store
    handlers --> state
    handlers --> auth
    services --> store
    services --> utils
    store --> sqlite
    store --> postgres
    vector --> vec_local
    vector --> vec_pg
    raster --> ras_local
    cache --> cac_tile
    cache --> cac_session
    state --> store
    state --> vector
    state --> raster
    state --> cache
    state --> tile_cache
    utils --> tile_cache
    tile_cache --> cache
    utils --> rendering
    utils --> parsers
```

## 4. Dual-Mode Deployment

### Standalone (default, local dev)

```mermaid
graph LR
    Browser[Browser - Angular UI]
    App[terrane process]
    SQLite[(SQLite - metadata)]
    LocalDir[(data_dir/business - vector GeoJSON)]
    LocalRaster[(data_dir/rasters - raster files)]
    DiskCache[(data_dir/gwc - tile cache)]
    Mem[(in-memory session cache)]

    Browser --> App
    App --> SQLite
    App --> LocalDir
    App --> LocalRaster
    App --> DiskCache
    App --> Mem
```

### Cloud-Native (production, stateless replicas)

```mermaid
graph LR
    LB[Load Balancer / Ingress]
    R1[terrane replica 1]
    R2[terrane replica N]
    PG[(PostgreSQL / PostGIS - metadata + vector)]
    Redis[(Redis - session + cache)]
    MinIO[(MinIO / S3 - raster + uploads)]

    LB --> R1
    LB --> R2
    R1 --> PG
    R2 --> PG
    R1 --> Redis
    R2 --> Redis
    R1 --> MinIO
    R2 --> MinIO
```

> **Status**: PostgreSQL metadata / vector backends exist today; local (disk / in-memory)
> raster and cache backends exist too. Redis and object-storage backends are on the roadmap
> — see [ROADMAP.md](ROADMAP.md).

## 5. Data Flow

### 5.1 REST CRUD (e.g. create layer)

```mermaid
sequenceDiagram
    participant UI as Angular UI
    participant H as layer_handler
    participant S as Store (SQLite/Postgres)
    participant C as AppState cache

    UI->>H: POST /geoserver/layers
    H->>S: create_layer(request)
    S-->>H: Layer
    H->>C: refresh in-memory layers
    H-->>UI: 201 + Layer JSON
```

### 5.2 WMS GetMap

```mermaid
sequenceDiagram
    participant C as Client
    participant W as WMS service
    participant R as Rendering utils
    participant TC as TileCache
    participant S as Store

    C->>W: GET /wms?SERVICE=WMS&REQUEST=GetMap...
    W->>TC: lookup tile (cache hit?)
    alt cache hit
        TC-->>W: cached PNG
    else cache miss
        W->>S: load layer + features
        W->>R: render features -> PNG
        W->>TC: store tile
    end
    W-->>C: PNG image
```

### 5.3 Authentication & Authorization

```mermaid
sequenceDiagram
    participant UI as Angular UI
    participant H as auth_handler
    participant S as Store
    participant SC as SessionCache
    participant A as auth.rs (JWT)
    participant L as LDAP (optional)

    UI->>H: POST /geoserver/auth/login
    H->>S: validate user (sha256 + salt)
    alt local user missing / password rejected AND [security.ldap] enabled
        H->>L: LDAP bind (user DN or service account + search)
        L-->>H: OK + groups
        H->>S: auto-provision local user (group-mapped role)
    end
    H->>A: sign JWT (jti, role)
    H->>S: create_session (persist jti)
    H->>SC: set(session) (write-through fast path)
    H-->>UI: token
    UI->>H: GET ... (Authorization: Bearer)
    H->>A: verify JWT
    H->>SC: get(jti) (cache hit -> skip store)
    alt cache miss
        H->>S: get_session(jti)
        H->>SC: set(session) (backfill)
    end
    alt [security] geofence_enabled
        H->>S: evaluate /permissions rules (workspace/store/layer, user/role, allow/deny)
    else
        H->>S: check layer permission
    end
    H-->>UI: resource
```

**Security building blocks**:

- **Enterprise identity (LDAP)** — `service/src/utils/ldap.rs`: `[security.ldap]`
  (`url` / `base_dn` / optional service `bind_dn`+`bind_password` /
  `user_filter` template / `admin_group` / `default_role`). Login falls back to
  an LDAP bind when the local user is missing or the password is rejected, then
  auto-provisions the local user with the group-mapped role (RFC 4514 DN
  escaping). Enabled via `GEOSERVER__SECURITY__LDAP__*`.
- **Fine-grained GeoFence ACL** — `service/src/utils/geofence.rs`: per-request
  `workspace / store / layer` rules over the `/permissions` model. Most-specific
  rule wins (user > role, layer > store > workspace > global), deny wins
  equal-priority ties, mode hierarchy admin ⊇ write ⊇ read, open default.
  Opt-in via `[security] geofence_enabled`; enforced in WMS GetMap/GetFeatureInfo,
  WFS GetFeature(+WithLock/LockFeature/GetPropertyValue/GetGmlObject), WCS
  GetCoverage and OGC API Maps; admin bypass; 403 on deny.
- **Credential management** — `service/src/utils/secrets.rs`: data-source passwords /
  S3 keys may reference `${ENV_VAR}` (K8s Secrets style); resolved at
  connection-build time (postgres/mysql/mongo/redis/S3) and never persisted or
  logged (`redact` masks values).

### 5.4 WFS Transaction (WFS-T) — planned

WFS-T (Insert / Update / Delete via `POST /wfs Transaction`) is **not implemented
yet** (currently returns 501); it is planned for a later milestone. The intended
data flow once implemented:

```mermaid
flowchart LR
    C[Client] -->|POST /wfs Transaction| W[WFS service]
    W -->|parse XML| P[models / parsers]
    P -->|save_features| B[VectorStore]
    B -->|local_dir / postgres / metadata| F[(features)]
```

## 6. API Contract Design

### 6.1 Base paths

- **OGC services** on the root path: `/wms`, `/wfs`, `/wcs`, `/wmts`
- **REST API** under the configurable context path (default `/geoserver`)
- **Probes & metrics** on the root path, decoupled from the context: `/health/live`, `/health/ready`, `/metrics`

> See [PROTOCOLS.md](PROTOCOLS.md) for the full protocol adaptation matrix (versions,
> operations, output formats, and pending protocols).

### 6.2 REST endpoint groups

All endpoints below live under `/geoserver` (the configurable `api_context`).

| Group         | Endpoints                                                                                          |
|---------------|----------------------------------------------------------------------------------------------------|
| Layers        | `/layers`, `/layers/{name}`, `/layers/{name}/preview`, `/layers/{name}/feature-type`, `/layers/{name}/features[/{id}]`, `/layers/{name}/style` |
| Workspaces    | `/workspaces`, `/workspaces/{name}`                                                                |
| Namespaces    | `/namespaces`, `/namespaces/{prefix}`                                                              |
| Data sources  | `/data-sources`, `/data-sources/test`, `/data-sources/{name}`, `/data-sources/{name}/tables`, `/data-sources/{name}/test` |
| Stores        | `/stores`, `/stores/{name}`, `/workspaces/{ws}/stores`, `/workspaces/{ws}/stores/{name}`           |
| Styles        | `/styles`, `/styles/{name}` (SLD / CSS / YSLD / MBStyle)                                           |
| Layer groups  | `/layer-groups`, `/layer-groups/{name}`                                                            |
| SQL views     | `/sql-views`, `/sql-views/preview`, `/sql-views/{name}`                                            |
| Tiles         | `/tiles/{layer}/{z}/{x}/{y}`, `/tiles/{layer}/{z}/{x}/{y}.pbf`, `/mvt/{layer}/{z}/{x}/{y}`, `/wmts/{layer}/{tileMatrixSet}/{tileMatrix}/{tileCol}/{tileRow}`, `/tiles/cache/clear/{layer}`, `/tiles/cache/stats` |
| Auth          | `/auth/login`, `/auth/logout`, `/auth/verify`, `/auth/change-password`, `/auth/users`, `/auth/users/{username}` |
| Permissions   | `/permissions`, `/permissions/{id}`, `/permissions/check/{type}/{name}`                            |
| Monitoring    | `/server/status`, `/monitor/stats`, `/monitor/requests`, `/monitor/logs`, `/monitor/reset`         |
| Backup        | `/backup/export`, `/backup/import`                                                                 |
| Uploads       | `/data/upload`, `/data/upload/shapefile`, `/data/upload/geotiff`                                   |

### 6.3 Layer ↔ database mapping

- `layer.store` = the data source (store) name
- `layer.native_name` = the database table name
- Boundaries: GET `/layers/{name}` returns `native_bounds.bounds.{minx,miny,maxx,maxy}`; the list endpoint returns `bounds` at the top level.

### 6.4 Error format

Errors are returned as JSON with an HTTP status code; error mapping is centralized in
`service/src/error.rs` and `service/src/store/error.rs`.

### 6.5 Configuration contract

All options are externalized via `service/terrane.toml` + `GEOSERVER__<SECTION>__<KEY>` env
overrides (precedence: CLI > env > file > defaults). See
[DEVELOPMENT.md](DEVELOPMENT.md) for the full variable reference.
