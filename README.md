<p align="center">
  <img src="docs/logo/terrane.svg" alt="Terrane" width="160">
</p>

# 🦀 Terrane

> **Cloud-native, high-performance spatial data server powered by Rust.**

## ✨ Features

### Backend (Rust)
- 🌐 **WMS** - Web Map Service
- 📍 **WFS** - Web Feature Service
- 🛰️ **WCS** - Web Coverage Service
- 🔌 **REST API** - Complete data management interface
- �️ **Data source management** - Per-datasource storage (PostGIS tables / data files; local dir / s3 / oss reserved)
- �🗺️ **Map rendering** - Supports points, lines, and polygons

### Frontend (Angular)
- 📊 **Dashboard** - System overview and statistics
- 🗺️ **Layer management** - Visual layer management
- ➕ **Create layer** - Form wizard
- 🔍 **Layer detail** - Info and preview
- 📍 **Feature browsing** - Read-only view & export (GeoJSON / CSV)
- 🎨 **Material Design** - UI components

## ☁️ Cloud-Native

**Goal**: containerization + 12-Factor configuration + observability + horizontal scaling, suitable for Docker / Kubernetes deployment.

**Dual-mode**: one codebase, two deployment profiles — standalone (SQLite metadata, local data files, in-memory session & cache) and cloud-native (PostgreSQL/PostGIS metadata, object-storage file backends, Redis session & cache, stateless replicas). Terrane is a **data publishing platform**: business data lives in external stores (PostGIS tables / data files) registered per data source, and Terrane focuses on publishing & OGC protocol adaptation (WFS-T / WCS-T writes not implemented yet). See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full design.

**Current status

- ✅ Configuration supports environment variable overrides (`GEOSERVER__<section>__<key>`, double-underscore separator, see `src/config.rs`)
- ✅ Multi-stage container image: `Dockerfile` + `.dockerignore` + `build/docker-compose.yml` (dev deps: PostGIS + Redis + MinIO; app via `--profile terrane`)
- ✅ Built-in image `HEALTHCHECK` + split probes: `/health/live` (liveness) & `/health/ready` (readiness)
- ✅ Prometheus `/metrics` endpoint (requests/errors, tile cache hit rate, PG pool watermarks, system resources)
- ✅ Graceful shutdown on SIGTERM/SIGINT with `shutdown_timeout_secs` in-flight drain
- ⚠️ TODO: JWT secret default is hardcoded in `src/auth.rs`; use `GEOSERVER__SECURITY__JWT_SECRET` env injection in prod
- ✅ Storage: config keeps only `[metadata]` (workspaces / data sources / layers / styles, default SQLite / PostgreSQL); vector/raster file data sources registered per data source (`file_storage_type`: local / s3 / oss, object storage reserved); tile + session cache stay built-in local, see `src/config.rs`
- ⚠️ TODO: in-memory caches (`src/state.rs`); multi-replica requires shared storage or migration to PostgreSQL / object storage
- ⚠️ TODO: tile cache / uploaded data on local disk, needs PVC or object storage
- ✅ CI pipeline: `.github/workflows/ci.yml` (fmt + clippy + test + frontend build + GHCR image push on main) + Trivy image scan + Dependabot — created, not yet exercised against a real repository

**Cloud-native roadmap**: containerization → 12-Factor/observability → state convergence → CI/CD. See section 6 of [IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md).

## 🚀 Quick Start

### Requirements

- Rust 1.95+
- Node.js 18+
- npm 9+

### Option 1: One-Click Start (recommended)

```bash
# Automatically builds the frontend and starts the server
cargo run

# Visit http://127.0.0.1:8080
```

### Option 2: Development Mode (separated frontend/backend)

```bash
# Terminal 1 - start the backend (skips frontend build, faster)
$env:SKIP_FRONTEND=1
cargo run

# Terminal 2 - start the frontend dev server (hot reload)
cd frontend
npm install
npm start

# Visit http://localhost:4200
```

### Option 3: Full Build

```bash
# cargo build automatically builds the frontend
cargo build

# Run
cargo run
```

### Option 4: Docker (containerized)

```bash
# Start the local dev dependencies (PostgreSQL/PostGIS + Redis + MinIO)
docker compose -f build/docker-compose.yml up -d

# Also start the terrane app itself (builds the image, wires it to PostGIS)
docker compose -f build/docker-compose.yml --profile terrane up -d

# Build the image manually (BuildKit is used by default; cache mounts keep
# npm/cargo dependency downloads across builds)
#   docker build -t terrane:latest .

# Build with domestic base-image mirrors (China):
#   docker build \
#     --build-arg NODE_IMAGE=docker.1ms.run/library/node:20-alpine \
#     --build-arg RUST_IMAGE=docker.1ms.run/library/rust:1.85-bookworm \
#     --build-arg RUNTIME_IMAGE=docker.1ms.run/library/debian:bookworm-slim \
#     -t terrane:latest .
#   (npm/cargo dependency registries default to npmmirror.com / rsproxy.cn;
#    override via --build-arg NPM_REGISTRY / CARGO_MIRROR if needed)

# Use PostgreSQL (metadata + business data reuse): the app service in
# build/docker-compose.yml already wires this up, just start it with:
#   docker compose -f build/docker-compose.yml --profile terrane up -d

# Visit: http://127.0.0.1:8080  (frontend is baked into the image; no separate frontend service)
```

**Cloud-native monitoring endpoints** (all registered on the root path, decoupled from `api_context`):

| Endpoint | Purpose |
|------|------|
| `/health/live` | Liveness probe - 200 when the process is alive |
| `/health/ready` | Readiness probe - 200 when metadata/business stores are ready, otherwise 503 |
| `/metrics` | Prometheus metrics (text format) for scraping / K8s HPA |

> The image ships a built-in `HEALTHCHECK` and SIGTERM graceful shutdown (`shutdown_timeout_secs`, default 30s).

### Or use the start script

```bash
cd frontend
START.bat
```

## 📁 Project Structure

```
terrane/
├── src/                    # Rust backend source code
│   ├── handlers/          # HTTP request handlers
│   ├── services/          # OGC service implementations
│   ├── models/            # Data models
│   ├── utils/             # Utility functions
│   └── main.rs            # Application entry point
├── frontend/              # Angular frontend
│   ├── src/
│   │   └── app/
│   │       ├── components/ # Page components
│   │       ├── services/  # API services
│   │       └── models/    # Data models
│   └── ...
├── static/                 # Backend static files (fallback)
├── docs/                   # Architecture / Roadmap / Development guides
└── Cargo.toml             # Rust dependencies
```

## 🌐 API Endpoints

### REST API

All REST endpoints live under the configurable context path (default `/geoserver`, see `[server] api_context` in `terrane.toml`):

| Method | Endpoint | Description |
|------|------|------|
| GET | `/geoserver/layers` | Get all layers |
| POST | `/geoserver/layers` | Create a layer |
| GET | `/geoserver/layers/:name` | Get layer details |
| PUT | `/geoserver/layers/:name` | Update a layer |
| DELETE | `/geoserver/layers/:name` | Delete a layer |
| GET | `/geoserver/layers/:name/preview` | Get layer preview image |
| GET | `/geoserver/layers/:name/features` | Get layer features (read-only) |
| GET/POST | `/geoserver/stores`, `/geoserver/stores/:name` (PUT/DELETE) | Store management (data-source view) |
| GET/POST | `/geoserver/workspaces/:ws/layers` `/datastores` `/coveragestores` | Workspace-dimension listings |
| GET/PUT | `/geoserver/services/:service/settings` | OGC service settings (WMS/WFS/WCS/…) |
| GET/POST/DELETE | `/geoserver/resources` | Data-directory resource management |
| GET/PUT | `/geoserver/layers/:name/feature-type` | Attribute schema (GeoPackage columns) |
| POST/GET/DELETE | `/geoserver/tiles/seed`, `/tiles/seed/:id`, `/tiles/seed/truncate` | Tile seeding / cancel / truncate |
| GET | `/geoserver/about/version`, `/geoserver/about/system-status` | System information |

### OGC Services

| Service | Endpoint | Operations |
|------|------|------|
| WMS | `/wms` | GetCapabilities, GetMap, GetFeatureInfo |
| WFS | `/wfs` | GetCapabilities, DescribeFeatureType, GetFeature |
| WCS | `/wcs` | GetCapabilities, DescribeCoverage, GetCoverage |

## 🎨 UI Preview

### Dashboard
- System statistics cards
- Recent layer list
- Quick actions

### Layer Management
- Card-based layer display
- Search and filtering
- One-click delete

### Layer Detail
- Detailed information display
- Live map preview
- Feature table management

## 🛠️ Tech Stack

### Backend
- **Rust** - System programming language
- **Actix-web** - Web framework
- **Tokio** - Async runtime
- **Geo** - Geometry computation
- **Image** - Image processing
- **Serde** - Serialization

### Frontend
- **Angular 17** - Frontend framework
- **Angular Material** - UI component library
- **TypeScript** - Type safety
- **RxJS** - Reactive programming
- **SCSS** - Style preprocessor

## 📦 Data Formats

### Layer

```json
{
  "name": "world_cities",
  "title": "World Cities",
  "workspace": "default",
  "store": "shapes",
  "srs": "EPSG:4326",
  "bounds": {
    "minx": -180,
    "miny": -90,
    "maxx": 180,
    "maxy": 90
  }
}
```

### Feature

```json
{
  "geometry": {
    "type": "Point",
    "coordinates": [116.4, 39.9]
  },
  "properties": {
    "name": "Beijing",
    "population": 21540000
  }
}
```

## 🔧 Configuration

Configuration file: `terrane.toml`

```toml
[server]
host = "127.0.0.1"
port = 8080
workers = 12
shutdown_timeout_secs = 30   # Graceful shutdown drain timeout for in-flight requests (container rolling updates)

[workspaces]
[[workspaces.stores.layers]]
name = "world"
title = "World"
srs = "EPSG:4326"
```

> **Environment variable overrides**: all configuration options can be overridden via `GEOSERVER__<section>__<key>` (e.g. `GEOSERVER__SERVER__PORT=9090`). In container deployments, use K8s ConfigMap / Secret injection.

## 📚 Documentation

- [Architecture](docs/ARCHITECTURE.md) — design rationale, module dependency graph, data flows, API contracts
- [Protocols](docs/PROTOCOLS.md) — protocol adaptation matrix (adapted vs pending)
- [Roadmap](docs/ROADMAP.md) — milestones, quarterly plan, known technical debt, future vision
- [Development guide](docs/DEVELOPMENT.md) — local setup, environment variables, git conventions
- [Feature gap analysis & implementation plan](docs/IMPLEMENTATION_PLAN.md)
- [Frontend documentation](frontend/README.md)
- [Frontend project summary](frontend/PROJECT_SUMMARY.md)

## 🤝 Contributing

Issues and Pull Requests are welcome!

## 📄 License

MIT License

## 🙏 Acknowledgements

- GeoServer community
- OGC standards organizations
- Rust community
- Angular team

---

**Made with ❤️ — Terrane, powered by Rust + Angular**
