# Terrane — Feature Gap Analysis & Implementation Plan

> Comparison analysis based on the GeoServer official documentation (https://docs.geoserver.org/latest/en/user/)
>
> For the product roadmap, milestones and known technical debt, see [ROADMAP.md](ROADMAP.md).
> For design rationale and architecture, see [ARCHITECTURE.md](ARCHITECTURE.md).

---

## 📊 Implementation Overview

| Feature Area | Implemented | Partial | Not Implemented | Progress |
|---------|:-----:|:--------:|:-----:|:-----:|
| OGC Core Services | 7/7 | 0 | 0 | **100%** |
| REST API | 15/16 | 0 | 1 | **94%** |
| Data Source Types | 12/15 | 0 | 3 | **80%** |
| Styling System | 5/5 | 0 | 0 | **100%** |
| Tile Caching | 6/6 | 0 | 0 | **100%** |
| Security | 7/7 | 0 | 0 | **100%** |
| Extensions | 8/14 | 0 | 6 | **57%** |
| Cloud-Native | 7/7 | 0 | 0 | **100%** |
| **Overall Progress** | | | | **~87%** |

```
OGC services     ██████████████████  100%
REST API         █████████████████░  94%
Data sources     ██████████████░░░░  80%
Styling system   ██████████████████  100%
Tile caching     ██████████████████  100%
Security         ██████████████████  100%
Extensions       ██████████░░░░░░░░  57%
Cloud-Native     ██████████████████  100%
──────────────────────────────
Overall progress ████████████████░░  87%
```

> **Statistics basis**: the historical "REST API 11/16" and "Tile Caching 3/6"
> numbers came from an original 16/6-item list that is no longer traceable
> (document archaeology confirms the earliest version already carried those
> figures without a breakdown). The REST/Tile, Security and Cloud-Native counts
> below are re-defined on an auditable basis (see
> [REST/Tile gap-closure plan](#4-phased-implementation-roadmap) and §6.1):
>
> - **REST API 16 items** = original 11 implemented + 5 groups closed this round
>   (full Stores CRUD, workspace-dimension endpoints, `/services/settings`,
>   `/about` + `/resources`, feature-type PUT); the 1 remaining gap is **Importer**
>   (bulk-import workflow, deferred as a standalone large item).
> - **Tile Caching 6 items** = basic tile service ✅ + cache engine (disk+TTL+Gridset) ✅ +
>   multi-backend (local + Redis) ✅ + **Seeding/Truncate ✅** + **Metastore ✅** +
>   **Disk Quota + conditional refresh (304) ✅** (plus: custom Gridset registration ✅).
> - **Security 7 items** (auditable, see §P3): CORS/CSRF ✅, user/group/role ✅,
>   REST API auth ✅, layer-level permissions ✅, frontend login ✅ —
>   **LDAP enterprise identity ✅** (`service/src/utils/ldap.rs`, `[security.ldap]`,
>   login fallback + auto-provision), **GeoFence fine-grained ACL ✅**
>   (`service/src/utils/geofence.rs`, `[security] geofence_enabled`, per-request
>   workspace/store/layer rules enforced on WMS/WFS/WCS) → 7/7 (100%).
> - **Cloud-Native 7 items** (auditable, see §6.1): containerization ✅,
>   12-Factor config ✅, observability ✅, lifecycle ✅, CI/CD ✅, resilience ✅ —
>   **statelessness/scalability ✅** (env-injectable credentials via
>   `service/src/utils/secrets.rs` — K8s Secrets style, never persisted/logged; S3/MinIO
>   object-storage uploads for shared multi-replica data) → 7/7 (100%).

---

## 1. ✅ Implemented Features

### 1.1 OGC Standard Services

> OGC core services **7/7 (100%)**: WMS / WFS / WCS / WMTS / WPS / CSW / OGC API series.
> The earlier "2 not implemented" (WPS, CSW) are now complete; this plan also closed
> operation-level gaps (WFS locks + GetPropertyValue + GetGmlObject, WMS GetFeatureInfo
> GML, WCS band subset + interpolation).

| Service | Operation | Status |
|------|------|:----:|
| **WMS 1.1.1/1.3.0** | GetCapabilities | ✅ |
| | GetMap | ✅ |
| | GetFeatureInfo (text/plain · text/html · application/json · **application/vnd.ogc.gml**) | ✅ |
| | DescribeLayer | ✅ |
| | GetLegendGraphic | ✅ |
| | GetStyles / PutStyles | ✅ |
| **WFS 1.0/1.1/2.0** | GetCapabilities | ✅ |
| | DescribeFeatureType | ✅ |
| | GetFeature (GML 2/3.1.1/3.2 · GeoJSON · CSV · KML · SHAPE-ZIP) | ✅ |
| | GetFeatureWithLock (real locking, response carries `lockId`) | ✅ |
| | LockFeature (acquire / renew / RELEASEACTION release, lockAction ALL/SOME, EXPIRY) | ✅ |
| | GetPropertyValue (WFS 2.0, `wfs:ValueCollection`) | ✅ |
| | GetGmlObject (WFS 2.0, `wfs:GMLObjectCollection`) | ✅ |
| | Transaction (Insert/Update/Delete) | ⏳ planned later (currently 501) |
| **WCS 1.0/1.1/2.0** | GetCapabilities | ✅ |
| | DescribeCoverage | ✅ |
| | GetCoverage (spatial subset + **range band subset** + **INTERPOLATION** nearest/bilinear/cubic/lanczos) | ✅ |

### 1.2 REST API

| Endpoint | Function | Status |
|------|------|:----:|
| `/layers` | CRUD | ✅ |
| `/layers/{name}/preview` | Preview | ✅ |
| `/layers/{name}/features` | Feature CRUD | ✅ |
| `/layers/{name}/feature-type` | Attribute schema | ✅ |
| `/layers/{name}/style` | Layer style binding | ✅ |
| `/workspaces` | CRUD | ✅ |
| `/data-sources` | CRUD + connection test | ✅ |
| `/styles` | CRUD + multi-format (SLD/CSS/YSLD/MBStyle) | ✅ |
| `/layer-groups` | CRUD | ✅ |
| `/data/upload` | GeoJSON upload | ✅ |
| `/data/upload/shapefile` | Shapefile upload | ✅ |
| `/data/upload/geotiff` | GeoTIFF upload | ✅ |
| `/server/status` | Server status | ✅ |
| `/health` | Health check | ✅ |
| `/tiles/{layer}/{z}/{x}/{y}` | Tile service | ✅ |

> **REST endpoints closed in this round** (batches after the Phased Roadmap):
>
> | Endpoint | Function | Batch |
> |------|------|:----:|
> | `/stores` POST、`/stores/{name}` PUT/DELETE | Store CRUD (data-source view) | R1 |
> | `/workspaces/{ws}/stores` POST | Create store under a workspace | R1 |
> | `/layer-groups/{name}` PUT | Update layer group | R1 |
> | `/auth/users/{username}` PUT | Update user (role/enabled/password reset) | R1 |
> | `/workspaces/{ws}/layers\|datastores\|coveragestores` | Workspace-dimension endpoints (GeoServer standard paths) | R2 |
> | `/services/{wms,wfs,wcs,wmts,wps,csw}/settings` GET/PUT | Service title/abstract/keywords (effective in WMS capabilities) | R2 |
> | `/about/version`、`/about/system-status` | System info (GeoServer compatible) | R3 |
> | `/resources` GET/POST/DELETE | Data-dir resource management (path-traversal protected) | R3 |
> | `/layers/{name}/feature-type` PUT | GeoPackage attribute column extension | R3 |
> | `/tiles/seed` POST/GET、`/tiles/seed/{id}` GET/DELETE、`/tiles/seed/truncate` POST | GWC-style seed jobs / cancel / truncate | T1 |

### 1.3 Data Source Types

| Type | Description | Status |
|------|------|:----:|
| **PostGIS** | PostgreSQL/PostGIS database | ✅ |
| **MySQL** | MySQL spatial database — MBR spatial filtering + ST_AsGeoJSON geometry output, pooled connections | ✅ **New** |
| **MongoDB** | MongoDB GeoJSON document database — GeoJSON geometries in a collection, $geoWithin bbox filtering, cached clients | ✅ **New** |
| **Shapefile** | ESRI Shapefile vector format | ✅ |
| **GeoTIFF** | GeoTIFF raster format | ✅ |
| **GeoPackage** | OGC GeoPackage vector format (WKB) | ✅ **P2** |
| **WorldImage** | Image + world file (.pgw/.jgw/.tfw) | ✅ **P2** |
| **CascadedWms** | Cascaded external WMS service | ✅ **P2** |
| **ArcGrid** | ESRI ASCII Grid raster format | ✅ **P2** |
| **ImageMosaic** | Raster-directory mosaic — multiple GeoTIFF/WorldImage/ArcGrid/PNG/JPEG granules under one directory served as a single coverage (WCS GetCoverage / WMS GetMap / tile pipelines) | ✅ **New** |
| **ImagePyramid** | Pyramid imagery — numeric level subdirs `0/1/2/…` each holding one level of granules, best-matching level selected by request resolution (WCS GetCoverage / WMS GetMap / tile pipelines) | ✅ **New** |
| **Redis** | Redis cache data source — tile-layer cache backend (selected via `Layer.cache_store`) | ✅ **New** |

### 1.4 Extended REST API

| Endpoint | Function | Status |
|------|------|:----:|
| `/namespaces` | Namespace CRUD | ✅ **New** |
| `/stores` | Store management (DataStore/CoverageStore) | ✅ **New** |
| `/workspaces/{ws}/stores` | List stores per workspace | ✅ **New** |
| `/sql-views` | SQL view CRUD | ✅ **New** |
| `/sql-views/preview` | SQL preview execution | ✅ **New** |
| `/tiles/cache/clear/{layer}` | Clear tile cache | ✅ **New** |
| `/tiles/cache/stats` | Cache statistics | ✅ **New** |

### 1.5 New OGC Services

| Service | Operations | Status |
|------|------|:----:|
| **WMTS 1.0.0** | GetCapabilities / GetTile / GetFeatureInfo | ✅ **New** |
| **CQL/ECQL Filter** | Comparison/logical/spatial/IN/BETWEEN/LIKE | ✅ **New** |

### 1.6 New Output Formats

- ✅ **WMS multi-format**: SVG (vector) / KML / GeoJSON
- ✅ **WFS multi-format**: CSV / GML 2.1.2 / GML 3.2.1
- ✅ **WMS Vendor parameters**: cql_filter / env / angle / featureId
- ✅ **WMS TIME/ELEVATION**: ISO 8601 time filtering + numeric elevation filtering

### 1.7 Infrastructure

- ✅ **GeoWebCache tile caching**: disk cache + expiry + Gridset + **Seeding/Truncate** (`/tiles/seed`) + **Metastore** (tile-metadata JSON: fast stats / resume) + **per-layer disk-quota LRU eviction** (`layer_quota_bytes`) + **ETag/Last-Modified 304 conditional requests** + **custom Gridset registration** (runtime, per-resolution)
- ✅ **SQL views**: parameterized SQL → virtual layers
- ✅ **WCS subset enhancements**: spatial clipping + resolution resampling

### 1.8 Frontend Features

- ✅ Dashboard
- ✅ Layer list / create / detail / preview
- ✅ Feature CRUD
- ✅ Workspace management
- ✅ Namespace management (NamespacesComponent) — **New**
- ✅ Store management (StoresComponent) — **New**
- ✅ Data source management (PostGIS/Shapefile/GeoTIFF/ImageMosaic/Redis)
- ✅ Style management (multi-format SLD/CSS/YSLD/MBStyle + templates)
- ✅ Layer group management
- ✅ Tile layers + GeoWebCache statistics (TileLayersComponent revamp) — **New**
- ✅ Server status page
- ✅ File upload support (GeoJSON/Shapefile/GeoTIFF)
- ✅ Login page (LoginComponent) + JWT token management — **New**

---

## 2. ⚠️ Partially Implemented Features

### 2.1 Style Rendering Enhancements ✅ Completed

- ✅ SLD/CSS/YSLD/MBStyle parsers all implemented (multi-format dispatch in WMS GetMap)
- ✅ **Label rendering (TextSymbolizer)**: Label property/literal, Font size, Fill color,
  Halo radius/color + greedy collision avoidance (built-in 5×7 bitmap font, zero deps)
- ✅ **Geometry / rendering correctness**: MultiPoint / MultiLineString / MultiPolygon /
  GeometryCollection rendering; `fill-opacity` / `stroke-opacity` alpha compositing;
  polygon interior-ring holes; z-order (polygon → line → point + SLD `z-index`)
- ✅ **SVG style-aware output**: WMS `image/svg+xml` honors per-feature styles
  (colors/opacity/width/dash + labels with halo); WMS GetMap `STYLES` parameter selects
  the per-layer style
- ✅ **WMS ANGLE rotation**: GetMap `ANGLE` rotates vector geometry around the request
  BBOX center (labels stay horizontal); passed through to cascaded upstreams;
  OpenLayers preview passes `ANGLE` + sets view `rotation`
- ✅ **GetLegendGraphic enhancements**: point-marker icons, `SCALE` rule filtering,
  `WIDTH` control, rule-name labels (bitmap font)
- ✅ **KML style-aware output**: deduplicated `<Style>` definitions + `styleUrl`
  references, KML `aabbggrr` colors, label as Placemark name
- ✅ **Compositing / blend modes**: SLD `VendorOption name="composite"` / CSS `composite`
  (multiply / screen / overlay / darken / lighten) with offscreen-layer compositing
- ✅ **WMS raster rendering**: GeoTIFF / WorldImage / ArcGrid / ImageMosaic layers render
  in WMS GetMap and the shared tile pipeline (BBOX crop + resample + source-over)
- ⏳ Rendering transforms (still out of scope)

---

## 3. ❌ Not Implemented Features (sorted by priority)

### P0 — Core Foundations ✅ Completed

- ✅ WMTS standard service
- ✅ GeoWebCache caching engine
- ✅ Namespace management
- ✅ Independent Store management
- ✅ SQL views

### P1 — OGC Service Enhancements ✅ Completed

- ✅ WMS time & elevation support
- ✅ WMS multi-format output (SVG/KML/GeoJSON)
- ✅ WMS vendor parameters (cql_filter/env/angle/featureId)
- ✅ WFS multi-format output (CSV/GML2/GML3.2)
- ✅ WCS range subsets
- ✅ ECQL/CQL filters

### P2 — Data Source Extensions ✅ Partially Completed

- ✅ **GeoPackage support** — vector + WKB geometry parsing
- ✅ **WorldImage** — world image format (.pgw/.jgw/.tfw)
- ✅ **ArcGrid** — ESRI ASCII Grid raster format
- ✅ **Cascaded WMS service** — HTTP proxy of WMS upstream services
- ✅ **ImageMosaic** — raster-directory mosaic (composites GeoTIFF/WorldImage/ArcGrid/PNG/JPEG granules)
- ✅ **ImagePyramid** — pyramid imagery (numeric level subdirs, resolution-based level selection)
- ✅ **MySQL** — MySQL spatial database connector (MBR filtering + ST_AsGeoJSON, connection test / pooled clients)
- ✅ **MongoDB** — MongoDB GeoJSON document connector ($geoWithin bbox filtering, ping test / cached clients)
- ❌ Oracle / SQL Server — additional database support

### P3 — Security (7/7)

- ✅ **CORS/CSRF protection** — `actix-cors` middleware + configurable whitelist
- ✅ **User/group/role system** — SHA-256+salt password hashing + JWT tokens + audit logs; users CRUD + PUT (role / enabled / password reset) via `/auth/users/{username}`
- ✅ **REST API authentication** — Bearer token + `require_auth()` middleware
- ✅ **Layer-level permissions** — Permission model + CRUD + matching rule engine
- ✅ **Frontend security** — login page (LoginComponent) + AuthInterceptor + default admin `admin / geoserver`
- ✅ **Enterprise identity (LDAP)** — `service/src/utils/ldap.rs`: `[security.ldap]` (url/base_dn/bind/user_filter/admin_group/default_role); login falls back to an LDAP bind when the local user is missing or the password is rejected, auto-provisioning the local user with the group-mapped role; RFC 4514 DN escaping; live test `#[ignore]`
- ✅ **Fine-grained GeoFence ACL** — `service/src/utils/geofence.rs`: per-request `workspace / store / layer` rules over the `/permissions` model (most-specific rule wins, deny overrides ties, mode hierarchy admin ⊇ write ⊇ read, open default); opt-in via `[security] geofence_enabled`, enforced on WMS GetMap/GetFeatureInfo, WFS GetFeature/GetFeatureWithLock/LockFeature/GetPropertyValue/GetGmlObject, WCS GetCoverage and OGC API Maps; admin bypass; 403 on deny

### P4 — Extensions

| # | Feature | Description | Est. Effort |
|---|------|------|:---------:|
| 25 | **WPS (Web Processing Service)** | Geoprocessing services: buffer, union/intersection/difference, coordinate transforms, etc. | ✅ **Completed** (first surface: vec:Centroid / vec:Buffer / gs:Bounds) |
| 26 | **CSW (Catalog Service)** | Catalog service: data discovery and metadata management | ✅ **Completed** (first surface: GetCapabilities / DescribeRecord / GetRecords / GetRecordById / GetDomain) |
| 27 | **OGC API series** | Features / Tiles / Maps / Coverages / Processes / Styles | ⏳ Features Core + Tiles ✅ (batch 19/20); Maps + Processes ✅ (batch 21); **Coverages + Styles ✅ (batch 22)** |
| 28 | **Vector Tiles** | MVT (Mapbox Vector Tile) format output | ✅ **Completed** |
| 29 | **KML output** | Map/feature export in KML/KMZ format | 1-2 weeks |
| 30 | **Printing module** | PDF map printing service | 3-4 weeks |
| 31 | **Monitoring** | Request statistics, performance monitoring, audit logs | ✅ **Completed** |
| 32 | **Importer** | Batch data import workflows | 3-4 weeks |
| 33 | **CSS/YSLD/MBStyle styling** | Style language support replacing SLD | ✅ **Completed** |
| 34 | **Backup/Restore** | Data directory backup and restore | ✅ **Completed** |
| 35 | **GeoFence** | Fine-grained access control | 3-4 weeks |

### P5 — v1.2 Planned Items (see [ROADMAP.md](ROADMAP.md) "v1.2")

> New work items added after a full codebase review; tracked in the v1.2 milestone.

| # | Feature | Description | Status |
|---|------|------|:-----:|
| 36 | **Layer preview format parity** | Frontend preview offers only OpenLayers/PNG/JPEG while backend WMS already emits SVG/KML/GeoJSON/GeoRSS/PDF/GIF/WebP; add TIFF/Atom/UTFGrid/GML to WMS GetMap and wire MVT (`.pbf`) preview for full GeoServer parity | ⏳ planned |
| 37 | **Built-in sample data** | Curated `service/samples/` set (GeoJSON point/line/polygon + simplified world map) + first-startup seeding into a `demo` workspace (`[samples] enabled`, default true); Shapefile/GeoTIFF samples and reusing samples in integration tests remain deferred | ✅ GeoJSON set done |
| 38 | **Database cluster connections** | PostGIS multi-host + read/write replica separation, MySQL multi-host, MongoDB replica-set URI; frontend dialog cluster fields + cluster-aware connection test | ⏳ planned |
| 39 | **`geoserver` → `terrane` naming migration** | Type names (`GeoServerConfig`/`GeoServerError`/`GeoServerBackup`), `GEOSERVER__` env prefix (keep alias), default `/geoserver` API context, defaults (admin password/DB name/namespace/`geoserver.sqlite`), frontend files, tests, docs, Docker/CI env vars | ⏳ planned (breaking) |

---

## 4. 📋 Phased Implementation Roadmap

### Phase 1: Core Enhancements (1-2 months)
**Goal**: complete the OGC core services and fill in essential data management features

```
📅 Week 1-2:  Namespace management + Independent Store management
📅 Week 3-4:  Full WMTS + GeoWebCache engine
📅 Week 5-6:  SQL views + WMS time/elevation support
📅 Week 7-8:  WFS 2.0 enhancements + multi-format output + ECQL filters
```

### Phase 2: Data Source Extensions ✅ Partially Completed (8/8)

```
📅 GeoPackage    ✅ Completed
📅 WorldImage    ✅ Completed
📅 ArcGrid       ✅ Completed
📅 Cascaded WMS  ✅ Completed
📅 ImageMosaic   ✅ Completed
📅 ImagePyramid  ✅ Completed
📅 MySQL         ✅ Completed
📅 MongoDB       ✅ Completed
```

> Note: Oracle / SQL Server from the plan's "More databases" section are not part
> of this 8-item list; they can be evaluated later via tiberius (SQL Server) /
> native driver (Oracle) if needed.

### Phase 3: Security & Permissions (7/7)
**Goal**: build a complete security system

```
📅 CORS/CSRF protection         ✅ Completed
📅 User/group/role system       ✅ Completed (incl. users PUT)
📅 Layer-level permissions      ✅ Completed
📅 REST API authentication      ✅ Completed
📅 Frontend login               ✅ Completed
📅 Enterprise identity (LDAP)   ✅ Completed ([security.ldap], login fallback + auto-provision)
📅 GeoFence fine-grained ACL    ✅ Completed ([security] geofence_enabled, per-request layer rules)
```

### Phase 4: Advanced Extensions (3-6 months)
**Goal**: implement enterprise-level advanced features

```
📅 Week 1-4:   WPS processing service
📅 Week 5-8:   CSW catalog service + OGC API series
📅 Week 9-12:  Vector tiles ✅ + KML + CSS/YSLD/MBStyle styling ✅
📅 Week 13-16: Printing module + Monitoring + Importer
📅 Week 17-20: GeoFence + Backup/Restore
```

---

## 5. 📝 Technical Recommendations

### 5.1 Architecture Improvements

1. **Plugin architecture**: following GeoServer's extension mechanism, design a trait-based plugin system for dynamically loading data sources and handlers
2. **Layer/data-source separation**: layers and data sources are currently tightly coupled; abstract `DataStore` / `CoverageStore` interfaces
3. **Cache layer abstraction**: design a unified tile cache interface supporting memory/disk/Redis backends
4. **Async streaming**: use async streaming responses for large datasets to reduce memory pressure

### 5.2 Recommended Crates

| Requirement | Recommended Crate |
|------|--------|
| Vector tiles (MVT) | `tilejson` / `mvt` crate |
| GeoPackage | `geopackage` crate or use SQLite directly |
| Projection enhancements | `proj` crate (PROJ bindings) |
| WPS processing | `geo` crate + `geos` crate (GEOS bindings) |
| Excel output | `calamine` / `rust_xlsxwriter` |
| PDF printing | `printpdf` / `genpdf` |
| JWT authentication | `jsonwebtoken` crate |
| LDAP authentication | `ldap3` crate |

### 5.3 Testing Strategy

- Unit + integration suites are in place (native Rust `#[cfg(test)]` unit tests;
  `actix-rt` HTTP integration tests split by protocol under `service/tests/` — see
  [DEVELOPMENT.md](DEVELOPMENT.md) §7). Remaining plans:
  - **OGC CITE tests**: reference the GeoServer CITE test suite to verify standards compliance
  - **Performance test suite ✅ (v1.1)**:
    - *Micro-benchmarks* (`cargo bench`, `service/benches/core_paths.rs` — criterion) over
      hot paths: GML serialization (`service/src/utils/gml.rs`), CQL/ECQL filter parsing,
      coordinate transforms (`service/src/utils/geometry.rs`), label rendering
      (`service/src/utils/bitmap_font.rs`), map rendering (`render_map`) and MVT encoding
    - *HTTP load harness* ✅: `#[ignore]` perf test `service/tests/perf_test.rs`
      (invoked like the live PostGIS/CascadedWMS tests) driving REST / WMS
      GetMap / WFS GetFeature / WMTS tiles against a real embedded server,
      reporting throughput + p50/p95/p99 latency; tunable via `PERF_REQUESTS` /
      `PERF_CONCURRENCY` / `PERF_WARMUP`
    - *Regression tracking*: criterion compares each re-run against the previous
      baseline (`service/target/criterion/`) so performance regressions surface during review

### 5.4 Documentation Plan

- **User guide** (`docs/USER_GUIDE.md`) — **done**; audience is data publishers /
  GIS administrators rather than code contributors:
  - Installation & quick start (local run / Docker, default admin account)
  - Data publishing workflow: workspace → data source → layer → style → preview → tile seeding
  - OGC service usage examples with copy-paste request URLs
    (WMS / WFS / WCS / WMTS / WPS / CSW / OGC API)
  - Security operations: users & roles, layer permissions, GeoFence ACL, LDAP login
  - Configuration & deployment pointers (env vars, health probes, `/metrics`),
    deferring details to [DEVELOPMENT.md](DEVELOPMENT.md)
- Linked from README and `AGENTS.md` document lists.

---

## 6. ☁️ Cloud-Native Evolution Roadmap

> **Goal**: equip the application with **containerization, 12-Factor configuration, observability, horizontal scalability, and automated delivery** so it can run on modern infrastructure such as Docker / Kubernetes.
>
> See [ARCHITECTURE.md](ARCHITECTURE.md) for the target architecture and [ROADMAP.md](ROADMAP.md) for the milestone timeline.

### 6.1 Cloud-Native Readiness Assessment

| Dimension | Current State | Gap | Priority |
|------|------|------|:-----:|
| **Containerization** | Multi-stage `Dockerfile` + `.dockerignore` + `build/docker-compose.yml` (dev deps: PostGIS + Redis + MinIO; app via `--profile terrane`); image `HEALTHCHECK` based on `/health/ready` | CI image build/push/scan not wired into a real repo yet | **P0 ✅** |
| **12-Factor config** | `service/terrane.toml` + `GEOSERVER__` env var prefix; `load_from_file()` mounts `config::Environment` | Default `host=127.0.0.1` (Dockerfile env sets 0.0.0.0); JWT secret default hardcoded in `service/src/auth.rs` | **P0 ✅** |
| **Statelessness / scalability** | Config keeps only `[metadata]` (SQLite/PostgreSQL); vector/raster data sources registered per data source (persisted in metadata store, `file_path` + `file_storage_type`); cache local (`service/src/store/cache/`); layers/styles cached in memory `Arc<RwLock<...>>` (`service/src/state.rs`); **env-injectable credentials** (`service/src/utils/secrets.rs`: `${ENV_VAR}` interpolation at connection build — K8s Secrets style, never persisted/logged); **S3/MinIO uploads** (`/data/upload/geotiff?storage=s3` → `file_storage_type=s3`, shared across replicas) | In-memory state still diverges across replicas (bounded by periodic + event-driven catalog refresh); SQLite remains single-writer for standalone mode — production HA uses PostgreSQL metadata + Redis cache + shared object storage | **P1 ✅** |
| **Observability** | stdout logs (tracing); split probes `/health/live` & `/health/ready`; Prometheus `/metrics` (requests/errors, method/status/endpoint, tile cache hit rate, PG pool watermarks, system); **structured JSON logs (`[logging] format = "json"`) + request-level `trace_id` done** | OpenTelemetry tracing pending | **P1 ✅** |
| **Lifecycle** | SIGTERM/SIGINT graceful shutdown + `shutdown_timeout_secs` draining in-flight requests (`main.rs`) | — | **P1 ✅** |
| **CI/CD & security** | GitHub Actions CI created (`.github/workflows/ci.yml`: fmt + clippy + test + frontend build + GHCR push); **Trivy image scan + Dependabot auto-update added** | Not yet verified against a real repository; GitLab CI optional | **P2 ✅** |
| **Resilience** | CORS defaults to `["*"]`; rate limiting / request-timeout middleware **done** (`service/src/middleware.rs`, `[server]` config, HTTP 429/504); **cascaded WMS retry/backoff + circuit breaking done** (`service/src/utils/cascaded.rs`, `[server]` config `cascaded_max_retries` / `cascaded_retry_base_ms` / `cascaded_circuit_threshold` / `cascaded_circuit_reset_secs`, per-upstream circuit breakers) | — | **P2 ✅** |

### 6.2 Phased Roadmap

#### Phase 0: Containerization Foundations (~1 week) ✅

- ✅ Multi-stage `Dockerfile`: `node` stage builds the frontend → `rust` stage `cargo build --release` → debian-slim runtime image (binary + `service/static/` only, non-root)
- ✅ `.dockerignore` (excludes `service/target/`, `frontend/node_modules/`, `service/static/`, `service/data/`, etc.)
- ✅ Built-in image `HEALTHCHECK` (based on `/health/ready`)
- ✅ `build/docker-compose.yml`: one command brings up dev deps (PostGIS + Redis + MinIO); app optional via `--profile terrane`
- ✅ Runtime default `host=0.0.0.0`; `static_dir` / `data_dir` / `sqlite_path` / `gwc` overridable via env (Dockerfile `ENV`)

#### Phase 1: 12-Factor Configuration & Observability ✅

- ✅ Unified config loading: `load_from_file()` mounts `config::Environment` (`GEOSERVER__` prefix); env overrides file config
- ⚠️ JWT secret: `GEOSERVER__SECURITY__JWT_SECRET` injection supported; default still hardcoded in `service/src/auth.rs` — production must inject explicitly
- ✅ Split health probes: `/health/live` (liveness) + `/health/ready` (metadata/business stores ready, 200/503)
- ✅ Structured JSON logs (tracing JSON layer, `[logging] format = "json"`) with request-level `trace_id` (middleware generates/pass-throughs `X-Trace-Id`/`X-Request-Id`, echoed in response headers; default `text` stays human-readable)
- ✅ Prometheus `/metrics`: request/error counts, method/status/endpoint distribution, tile cache hit rate, PG pool watermarks, system resources (hand-written pure-Rust text format, zero external deps)
- ✅ Graceful shutdown: SIGTERM/SIGINT captured + `shutdown_timeout_secs` drains in-flight requests (`main.rs::shutdown_signal` + `HttpServer::shutdown_signal`/`shutdown_timeout`)

> Note: this phase was folded into round 9 of implementation (monitoring + container build support).

#### Phase 2: State Convergence & Scalability

- ✅ Storage: config keeps only `[metadata]` (SQLite/PostgreSQL); vector/raster file data sources registered per data source (persisted in metadata store) with `file_path` + `file_storage_type` (local / s3 / oss); cache stays built-in local (`TileCacheBackend` + `SessionCache` traits in `service/src/store/cache/`) — local backends in place
- ✅ **Redis cache data source** (redesigned): Redis as a data source (`DataSourceType::Redis`, persisted in metadata `data_sources`, host/port/database/username/password); tile layers select a cache backend via `Layer.cache_store` (default in-memory/local, or a named Redis data source); `service/src/store/cache/redis.rs` provides `RedisConn` + `redis_url_from_connection`; `RedisTileCacheBackend` (`service/src/store/cache/tile.rs`) keyed by data-source URL; tile render paths (`render_tile_bytes` / `get_tile`) resolve per layer; connection test supports Redis PING
- ✅ **In-memory catalog refresh mechanism**: periodically reloads layers/styles/layer-groups from the metadata store into the in-memory cache to converge replicas (`[server] catalog_refresh_secs`, 0 = disabled; `state.rs::refresh_catalog_from_store`, background tokio task, update/add by name without delete); **event-driven refresh**: REST layer update/delete immediately reloads the in-memory catalog via `AppState::refresh_catalog`, eliminating the write-after-read-stale window for WMS/WMTS/tile paths
- ✅ **Object-storage uploads (s3 / minio)**: `/data/upload/geotiff?storage=s3&bucket=…` writes the object through `S3FileStore` and registers the data source with `file_storage_type = "s3"`, so uploaded rasters are shared across replicas (**no S3 tile-cache backend** — cache backends remain local + Redis data source only)
- ✅ **Credential management (K8s Secrets style)**: `service/src/utils/secrets.rs` resolves `${ENV_VAR}` references in data-source passwords / S3 keys at connection-build time (postgres/mysql/mongo/redis/S3) — secrets are injected via env (K8s Secrets) and are never persisted in plaintext nor logged (`redact` masks them)
- ⏳ Session management: **no Redis session cache** by design — simple JWT + metadata-store sessions (`SessionCache` is only a local fast-path)
- ⏳ `data_dir` / upload file storage abstraction: shared PVC / object storage for arbitrary uploads (pending; GeoTIFF S3 upload covers the raster path)

#### Phase 3: CI/CD & Security Hardening

- ⚠️ CI (GitHub Actions) created (`.github/workflows/ci.yml`): `cargo fmt + clippy + test` + frontend build + docker build/push (ghcr, tagged by git sha); not yet verified against a real repository
- ✅ Images tagged by git sha; Trivy image scanning + Dependabot dependency updates added
- ✅ **Credential management**: data source passwords / S3 keys injectable via `${ENV_VAR}` env references (K8s Secrets), resolved at connection build, never logged (`service/src/utils/secrets.rs`); direct K8s Secret volume-mount integration is deployment-side
- ✅ Resilience middleware: request timeout (504) + sliding-window rate limiting (429, per client IP / X-Forwarded-For) — `service/src/middleware.rs`, enabled via `[server]` config (`request_timeout_secs` / `rate_limit_max_requests` / `rate_limit_window_secs`)
- ✅ Cascaded WMS resilience: exponential-backoff retry on transient failures (timeout/conn-fail/429/5xx) (`cascaded_max_retries` / `cascaded_retry_base_ms`) + per-upstream circuit breaker (`cascaded_circuit_threshold` / `cascaded_circuit_reset_secs`, open → half-open probe → closed/reopen) — `service/src/utils/cascaded.rs`, `AppState.cascaded_circuits`
- ✅ Dependency & image security: CI docker job runs Trivy image vulnerability scan (CRITICAL/HIGH, SARIF uploaded to GitHub Security tab); `.github/dependabot.yml` auto-updates cargo / npm / GitHub Actions deps

### 6.3 Target Deployment Architecture

```
                    ┌──────────────────────────────────┐
   Ingress / TLS ─▶ │  K8s Deployment (N stateless     │
                    │      replicas) — terrane         │
                    │      API service                 │
                    └──────────┬───────────────────────┘
                               │
        ┌──────────────┬───────┴───────────┬────────────────┐
        ▼              ▼                   ▼                ▼
   PostgreSQL      PVC / MinIO         PVC / S3           Prometheus /
   (metadata +     (uploaded data)    (tile cache)        OTel Collector
    data source                                     (logs / metrics /
    connections)                                          traces)
```

- **Cross-replica state convergence**: metadata → PostgreSQL; tiles → object storage; uploaded files → shared volume
- **Rolling releases** rely on the `/health/ready` probe and graceful shutdown for availability
- The frontend is baked in as a **build artifact** in the runtime image, served from the backend `static/` directory — no separate frontend service required

---
