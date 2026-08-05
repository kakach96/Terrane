<p align="center">
  <img src="docs/logo/geoferris.svg" alt="GeoFerris" width="160">
</p>

# 🦀 GeoFerris

> **Cloud-native, high-performance spatial data server with a modern UI — powered by Rust and Angular.**

## ✨ Features

### Backend (Rust)
- 🌐 **WMS** - Web Map Service
- 📍 **WFS** - Web Feature Service
- 🛰️ **WCS** - Web Coverage Service
- 🔌 **REST API** - Complete data management interface
- 🗺️ **Map rendering** - Supports points, lines, and polygons

### Frontend (Angular)
- 📊 **Dashboard** - System overview and statistics
- 🗺️ **Layer management** - Visual layer management
- ➕ **Create layer** - Form wizard
- 🔍 **Layer detail** - Info and preview
- 📍 **Feature management** - CRUD operations
- 🎨 **Material Design** - Modern UI

## ☁️ Cloud-Native

**Goal**: containerization + 12-Factor configuration + observability + horizontal scaling, suitable for Docker / Kubernetes deployment.

**Dual-mode**: one codebase, two deployment profiles — standalone (SQLite metadata, local/raster files, in-memory session & cache) and cloud-native (PostgreSQL/PostGIS metadata + vector, object-storage raster, Redis session & cache, stateless replicas). See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full design.

**Current status

- ✅ Configuration supports environment variable overrides (`GEOSERVER__<section>__<key>`, double-underscore separator, see `src/config.rs`)
- ✅ Multi-stage container image: `Dockerfile` + `.dockerignore` + `docker-compose.yml` (SQLite standalone / PostgreSQL HA)
- ✅ Built-in image `HEALTHCHECK` + split probes: `/health/live` (liveness) & `/health/ready` (readiness)
- ✅ Prometheus `/metrics` endpoint (requests/errors, tile cache hit rate, PG pool watermarks, system resources)
- ✅ Graceful shutdown on SIGTERM/SIGINT with `shutdown_timeout_secs` in-flight drain
- ⚠️ TODO: JWT secret default is hardcoded in `src/auth.rs`; use `GEOSERVER__SECURITY__JWT_SECRET` env injection in prod
- ✅ Storage split: `[metadata]` (workspaces / data sources / layers / styles, default SQLite) + `[business]` (layer features; local dir / reuse metadata / PostgreSQL), see `src/config.rs`
- ⚠️ TODO: in-memory caches (`src/state.rs`); multi-replica requires shared storage or migration to PostgreSQL / object storage
- ⚠️ TODO: tile cache / uploaded data on local disk, needs PVC or object storage
- ⚠️ TODO: CI pipeline + image registry push (GitHub Actions / GitLab CI)

**Cloud-native roadmap**: containerization → 12-Factor/observability → state convergence → CI/CD. See section 7 of [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md).

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
# Build the image and start the stack (default: SQLite standalone mode)
docker compose up -d

# Build the image manually (BuildKit is used by default; cache mounts keep
# npm/cargo dependency downloads across builds)
#   docker build -t geoferris:latest .

# Build with domestic base-image mirrors (China):
#   docker build \
#     --build-arg NODE_IMAGE=docker.1ms.run/library/node:20-alpine \
#     --build-arg RUST_IMAGE=docker.1ms.run/library/rust:1.85-bookworm \
#     --build-arg RUNTIME_IMAGE=docker.1ms.run/library/debian:bookworm-slim \
#     -t geoferris:latest .
#   (npm/cargo dependency registries default to npmmirror.com / rsproxy.cn;
#    override via --build-arg NPM_REGISTRY / CARGO_MIRROR if needed)

# Use PostgreSQL (metadata + business data reuse):
#   uncomment the postgres config in the geoserver service in docker-compose.yml, then:
#   docker compose --profile postgres up -d

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
geoferris/
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

All REST endpoints live under the configurable context path (default `/geoserver`, see `[server] api_context` in `geoferris.toml`):

| Method | Endpoint | Description |
|------|------|------|
| GET | `/geoserver/layers` | Get all layers |
| POST | `/geoserver/layers` | Create a layer |
| GET | `/geoserver/layers/:name` | Get layer details |
| PUT | `/geoserver/layers/:name` | Update a layer |
| DELETE | `/geoserver/layers/:name` | Delete a layer |
| GET | `/geoserver/layers/:name/preview` | Get layer preview image |
| GET | `/geoserver/layers/:name/features` | Get layer features |
| POST | `/geoserver/layers/:name/features` | Add a feature |

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

Configuration file: `geoferris.toml`

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
- [Roadmap](docs/ROADMAP.md) — milestones, quarterly plan, known technical debt, future vision
- [Development guide](docs/DEVELOPMENT.md) — local setup, environment variables, git conventions
- [Feature gap analysis & implementation plan](IMPLEMENTATION_PLAN.md)
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

**Made with ❤️ — GeoFerris, powered by Rust + Angular**
