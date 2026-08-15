# Terrane — Feature Gap Analysis & Implementation Plan

> Comparison analysis based on the GeoServer official documentation (https://docs.geoserver.org/latest/en/user/)
>
> For the product roadmap, milestones and known technical debt, see [ROADMAP.md](ROADMAP.md).
> For design rationale and architecture, see [ARCHITECTURE.md](ARCHITECTURE.md).

---

## 📊 Implementation Overview

| Feature Area | Implemented | Partial | Not Implemented | Progress |
|---------|:-----:|:--------:|:-----:|:-----:|
| OGC Core Services | 5/7 | 0 | 2 | **71%** |
| REST API | 11/16 | 0 | 5 | **69%** |
| Data Source Types | 8/15 | 0 | 7 | **53%** |
| Styling System | 4/5 | 0 | 1 | **80%** |
| Tile Caching | 3/6 | 0 | 3 | **50%** |
| Security | 3/7 | 0 | 4 | **43%** |
| Extensions | 8/14 | 0 | 6 | **57%** |
| Cloud-Native | 4/7 | 0 | 3 | **57%** |
| **Overall Progress** | | | | **~60%** |

```
OGC services     █████████████░░░░  71%
REST API         █████████████░░░░  69%
Data sources     ███████████░░░░░  53%
Styling system   ████████████████░  80%
Tile caching     ██████████░░░░░░  50%
Security         ████████░░░░░░░░  43%
Extensions       █████████░░░░░░░  57%
Cloud-Native     █████████░░░░░░░  57%
──────────────────────────────
Overall progress ████████████░░░░  60%
```

---

## 一、✅ Implemented Features

### 1.1 OGC Standard Services

| Service | Operation | Status |
|------|------|:----:|
| **WMS 1.1.1/1.3.0** | GetCapabilities | ✅ |
| | GetMap | ✅ |
| | GetFeatureInfo | ✅ |
| | DescribeLayer | ✅ |
| | GetLegendGraphic | ✅ |
| | GetStyles / PutStyles | ✅ |
| **WFS 1.0/1.1/2.0** | GetCapabilities | ✅ |
| | DescribeFeatureType | ✅ |
| | GetFeature | ✅ |
| | GetFeatureWithLock | ✅ |
| | Transaction (Insert/Update/Delete) | ✅ |
| | LockFeature (definition) | ✅ |
| **WCS 1.0/1.1/2.0** | GetCapabilities | ✅ |
| | DescribeCoverage | ✅ |
| | GetCoverage | ✅ |

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

### 1.3 Data Source Types

| Type | Description | Status |
|------|------|:----:|
| **PostGIS** | PostgreSQL/PostGIS database | ✅ |
| **Shapefile** | ESRI Shapefile vector format | ✅ |
| **GeoTIFF** | GeoTIFF raster format | ✅ |
| **GeoPackage** | OGC GeoPackage vector format (WKB) | ✅ **P2** |
| **WorldImage** | Image + world file (.pgw/.jgw/.tfw) | ✅ **P2** |
| **CascadedWms** | Cascaded external WMS service | ✅ **P2** |
| **ArcGrid** | ESRI ASCII Grid raster format | ✅ **P2** |
| **Redis** | Redis 缓存数据源 — 切片图层缓存后端 (经 `Layer.cache_store` 选择) | ✅ **New** |

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

- ✅ **GeoWebCache tile caching**: disk cache + expiry + Gridset
- ✅ **SQL views**: parameterized SQL → virtual layers
- ✅ **WCS subset enhancements**: spatial clipping + resolution resampling

### 1.8 Frontend Features

- ✅ Dashboard
- ✅ Layer list / create / detail / preview
- ✅ Feature CRUD
- ✅ Workspace management
- ✅ Namespace management (NamespacesComponent) — **New**
- ✅ Store management (StoresComponent) — **New**
- ✅ Data source management (PostGIS/Shapefile/GeoTIFF)
- ✅ Style management (multi-format SLD/CSS/YSLD/MBStyle + templates)
- ✅ Layer group management
- ✅ Tile layers + GeoWebCache statistics (TileLayersComponent revamp) — **New**
- ✅ Server status page
- ✅ File upload support (GeoJSON/Shapefile/GeoTIFF)
- ✅ Login page (LoginComponent) + JWT token management — **New**

---

## 二、⚠️ Partially Implemented Features

### 2.1 Style Rendering Enhancements

- ✅ SLD/CSS/YSLD/MBStyle parsers all implemented
- **Missing**: rendering transforms, geometry transforms, label collision, z-order, compositing/blend modes

---

## 三、❌ Not Implemented Features (sorted by priority)

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
- ❌ ImageMosaic — raster time series / mosaic datasets
- ❌ ImagePyramid — pyramid imagery
- ❌ Oracle / MySQL / SQL Server — additional database support
- ❌ MongoDB — MongoDB GeoJSON data source

### P3 — Security ✅ Completed

- ✅ **CORS/CSRF protection** — `actix-cors` middleware + configurable whitelist
- ✅ **User/group/role system** — SHA-256+salt password hashing + JWT tokens + audit logs
- ✅ **REST API authentication** — Bearer token + `require_auth()` middleware
- ✅ **Layer-level permissions** — Permission model + CRUD + matching rule engine
- ✅ Frontend login page (LoginComponent) + AuthInterceptor
- ✅ Default admin: `admin / geoserver`
- 🔐 New endpoints: `/auth/login`, `/auth/verify`, `/auth/users`, `/permissions`

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

---

## 四、📋 Phased Implementation Roadmap

### Phase 1: Core Enhancements (1-2 months)
**Goal**: complete the OGC core services and fill in essential data management features

```
📅 Week 1-2:  Namespace management + Independent Store management
📅 Week 3-4:  Full WMTS + GeoWebCache engine
📅 Week 5-6:  SQL views + WMS time/elevation support
📅 Week 7-8:  WFS 2.0 enhancements + multi-format output + ECQL filters
```

### Phase 2: Data Source Extensions ✅ Partially Completed (4/8)

```
📅 GeoPackage    ✅ Completed
📅 WorldImage    ✅ Completed
📅 ArcGrid       ✅ Completed
📅 Cascaded WMS  ✅ Completed
📅 ImageMosaic   ⏳
📅 ImagePyramid  ⏳
📅 More databases ⏳
📅 MongoDB       ⏳
```

### Phase 3: Security & Permissions ✅ Completed
**Goal**: build a complete security system

```
📅 CORS/CSRF protection         ✅ Completed
📅 User/group/role system       ✅ Completed
📅 Layer-level permissions      ✅ Completed
📅 REST API authentication      ✅ Completed
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

## 五、📝 Technical Recommendations

### 5.1 Architecture Improvements

1. **Plugin architecture**: following GeoServer's extension mechanism, design a trait-based plugin system for dynamically loading data sources and handlers
2. **Layer/data-source separation**: layers and data sources are currently tightly coupled; abstract `DataStore` / `CoverageStore` interfaces
3. **Cache layer abstraction**: design a unified tile cache interface supporting memory/disk/S3/Redis backends
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

- No test suite currently; recommended to introduce:
  - **Unit tests**: native Rust `#[cfg(test)]`
  - **Integration tests**: use `actix-rt` to test HTTP endpoints
  - **OGC CITE tests**: reference the GeoServer CITE test suite to verify standards compliance

---

## 六、☁️ Cloud-Native Evolution Roadmap

> **Goal**: equip the application with **containerization, 12-Factor configuration, observability, horizontal scalability, and automated delivery** so it can run on modern infrastructure such as Docker / Kubernetes.
>
> See [ARCHITECTURE.md](ARCHITECTURE.md) for the target architecture and [ROADMAP.md](ROADMAP.md) for the milestone timeline.

### 6.1 Cloud-Native Readiness Assessment

| Dimension | Current State | Gap | Priority |
|------|------|------|:-----:|
| **Containerization** | Multi-stage `Dockerfile` + `.dockerignore` + `build/docker-compose.yml` (开发依赖: PostGIS + Redis + MinIO; app 经 `--profile terrane`); image `HEALTHCHECK` 基于 `/health/ready` | 未接入 CI 镜像构建/推送/扫描 | **P0 ✅** |
| **12-Factor config** | `terrane.toml` + `GEOSERVER__` env var prefix; `load_from_file()` 已挂载 `config::Environment` | 默认 `host=127.0.0.1` (容器内通过 Dockerfile env 设 0.0.0.0); JWT secret 默认值硬编码于 `src/auth.rs` | **P0 ✅** |
| **Statelessness / scalability** | Config keeps only `[metadata]` (SQLite/PostgreSQL); vector/raster data sources registered per data source (persisted in metadata store, `file_path` + `file_storage_type`); cache local (`src/store/cache/`); layers/styles cached in memory `Arc<RwLock<...>>` (`src/state.rs`); uploads on local disk `./data` | In-memory state diverges across replicas; SQLite is single-writer and unsuitable for HA; needs shared volume/PVC or object storage | **P1** |
| **Observability** | stdout logs (tracing); `/health` + 拆分 `/health/live` & `/health/ready`; Prometheus `/metrics` (请求/错误、方法/状态码/端点、瓦片命中率、PG 池水位、系统资源) | 无 structured JSON logs, 无 OpenTelemetry tracing | **P1 ✅** |
| **Lifecycle** | SIGTERM/SIGINT 优雅关闭 + `shutdown_timeout_secs` 在途请求排空 (`main.rs`) | — | **P1 ✅** |
| **CI/CD & security** | GitHub Actions CI created (`.github/workflows/ci.yml`: fmt + clippy + test + frontend build + GHCR push); **Trivy 镜像扫描 + Dependabot 自动更新 added** | 未在真实仓库验证; GitLab CI 可选 | **P2 ✅** |
| **Resilience** | CORS defaults to `["*"]`; rate limiting / request-timeout middleware **done** (`src/middleware.rs`, `[server]` config, HTTP 429/504); **cascaded WMS retry/backoff + circuit breaking done** (`src/utils/cascaded.rs`, `[server]` config `cascaded_max_retries` / `cascaded_retry_base_ms` / `cascaded_circuit_threshold` / `cascaded_circuit_reset_secs`, per-upstream 熔断器) | — | **P2 ✅** |

### 6.2 Phased Roadmap

#### Phase 0: Containerization Foundations (~1 week)

- ✅ 多阶段 `Dockerfile`: `node` stage 构建前端 → `rust` stage `cargo build --release` → debian-slim 运行镜像 (仅二进制 + `static/`, 非 root 运行)
- ✅ `.dockerignore` (排除 `target/`, `frontend/node_modules/`, `static/`, `data/`, 等)
- ✅ 镜像内置 `HEALTHCHECK` (基于 `/health/ready`)
- ✅ `build/docker-compose.yml`: 一次拉起开发依赖 (PostGIS + Redis + MinIO); app 经 `--profile terrane` 可选启动
- ✅ 运行时默认 `host=0.0.0.0`; `static_dir` / `data_dir` / `sqlite_path` / `gwc` 经环境变量覆盖 (Dockerfile `ENV`)

#### Phase 1: 12-Factor Configuration & Observability

- ✅ 统一配置加载: `load_from_file()` 已挂载 `config::Environment` (`GEOSERVER__` 前缀), env 覆盖文件配置
- ⚠️ JWT secret: 支持 `GEOSERVER__SECURITY__JWT_SECRET` 注入; 默认值仍硬编码于 `src/auth.rs`, 生产必须显式注入
- ✅ 拆分健康探针: `/health/live` (liveness) + `/health/ready` (依赖元数据/业务存储就绪, 200/503)
- ✅ 结构化日志 (tracing JSON layer, `[logging] format = "json"`) 与 request-level `trace_id` (中间件生成/透传 `X-Trace-Id`/`X-Request-Id`, 响应头回显; 默认 `text` 保持人类可读)
- ✅ Prometheus `/metrics`: 请求/错误计数、方法/状态码/端点分布、瓦片缓存命中率、PG 连接池水位、系统资源 (纯 Rust 手工生成文本格式, 零外部依赖)
- ✅ 优雅关闭: 捕获 SIGTERM/SIGINT + `shutdown_timeout_secs` 排空在途请求 (`main.rs::shutdown_signal` + `HttpServer::shutdown_signal`/`shutdown_timeout`)

> 注: 当前阶段已并入第 9 轮实现 (监控检查 + 容器构建支持)。

#### Phase 2: State Convergence & Scalability

- ✅ Storage: 配置文件只保留 `[metadata]` (SQLite/PostgreSQL); 矢量/栅格文件数据源按数据源登记 (persisted in metadata store), 记录 `file_path` + `file_storage_type` (local / s3 / oss); 缓存保持内置 local (`TileCacheBackend` + `SessionCache` traits in `src/store/cache/`) — local backends in place
- ✅ **Redis 缓存数据源** (重新设计): Redis 作为数据源 (`DataSourceType::Redis`, 持久化于元数据 `data_sources` 表, host/port/database/username/password), 切片图层经 `Layer.cache_store` 选择缓存后端 (内存/本地默认 或 指定 Redis 数据源); `src/store/cache/redis.rs` 提供 `RedisConn` + `redis_url_from_connection`, `RedisTileCacheBackend` (`src/store/cache/tile.rs`) 按数据源 URL 驱动, 瓦片渲染路径 (`render_tile_bytes` / `get_tile`) 按图层解析; 连接测试支持 Redis PING
- ✅ **In-memory catalog refresh mechanism**: 周期性地从元数据存储重载图层/样式/图层组到内存缓存, 收敛多副本间差异 (`[server] catalog_refresh_secs`, 0 = 禁用; `state.rs::refresh_catalog_from_store`, 后台 tokio 任务, 按名称更新/新增不删除)
- 对象存储后端 (s3 / oss / minio) 落地: `FileStore` trait 已预留 (`src/store/file_store.rs`), 后续实现 S3/MinIO 读取/上传; cache 到 Redis / S3
- 会话管理: **不引入 Redis 会话缓存**, 保持简单 JWT + 元数据存储会话 (`SessionCache` 仅 local 快速层)
- `data_dir` / upload file storage abstraction: shared PVC / object storage (pending)
- Graceful shutdown: catch SIGTERM + `.shutdown_timeout()` to drain in-flight requests

#### Phase 3: CI/CD & Security Hardening

- ⚠️ CI (GitHub Actions) 已创建 (`.github/workflows/ci.yml`): `cargo fmt + clippy + test` + frontend build + docker build/push (ghcr, 按 git sha 打标签); 尚未在真实仓库验证
- Tag images by git sha; image scanning with Trivy; Dependabot / Renovate dependency updates
- Credential management: data source passwords injectable via env, never logged; integrate with K8s Secrets
- ✅ 韧性中间件: 请求超时 (504) + 滑动窗口速率限制 (429, 按客户端 IP / X-Forwarded-For) — `src/middleware.rs`, 经 `[server]` 配置 (`request_timeout_secs` / `rate_limit_max_requests` / `rate_limit_window_secs`) 启用
- ✅ 级联 WMS 韧性: 瞬时故障 (超时/连接失败/429/5xx) 指数退避重试 (`cascaded_max_retries` / `cascaded_retry_base_ms`) + 按上游 URL 隔离的熔断器 (`cascaded_circuit_threshold` / `cascaded_circuit_reset_secs`, 打开→半开试探→关闭/重开) — `src/utils/cascaded.rs`, `AppState.cascaded_circuits`
- ✅ 依赖与镜像安全: `.github/workflows/ci.yml` docker job 增加 Trivy 镜像漏洞扫描 (CRITICAL/HIGH, SARIF 上传 GitHub Security tab); `.github/dependabot.yml` 自动更新 cargo / npm / GitHub Actions 依赖

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
