# 🌍 Rust GeoServer

A lightweight geospatial data server built with Rust + Actix-web, featuring a modern Angular + Material admin interface.

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

**Current status**

- ✅ Configuration supports environment variable overrides (`GEOSERVER__<section>__<key>`, double-underscore separator, see `src/config.rs`)
- ✅ `/health` health check endpoint, stdout logs (tracing)
- ⚠️ TODO: Dockerfile / docker-compose / CI pipeline (not yet created)
- ⚠️ TODO: JWT secret hardcoded in `src/auth.rs`, needs env injection
- ✅ Storage split: `[metadata]` (workspaces / data sources / layers / styles, default SQLite) + `[business]` (layer features; local dir / reuse metadata / PostgreSQL), see `src/config.rs`
- ⚠️ TODO: in-memory caches (`src/state.rs`); multi-replica requires shared storage or migration to PostgreSQL / object storage
- ⚠️ TODO: tile cache / uploaded data on local disk, needs PVC or object storage
- ⚠️ TODO: no graceful shutdown (SIGTERM drain)

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

### Or use the start script

```bash
cd frontend
START.bat
```

## 📁 Project Structure

```
rust-geoserver/
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
└── Cargo.toml             # Rust dependencies
```

## 🌐 API Endpoints

### REST API

| Method | Endpoint | Description |
|------|------|------|
| GET | `/api/layers` | Get all layers |
| POST | `/api/layers` | Create a layer |
| GET | `/api/layers/:name` | Get layer details |
| PUT | `/api/layers/:name` | Update a layer |
| DELETE | `/api/layers/:name` | Delete a layer |
| GET | `/api/layers/:name/preview` | Get layer preview image |
| GET | `/api/layers/:name/features` | Get layer features |
| POST | `/api/layers/:name/features` | Add a feature |

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

Configuration file: `geoserver.toml`

```toml
[server]
host = "127.0.0.1"
port = 8080
workers = 12

[workspaces]
[[workspaces.stores.layers]]
name = "world"
title = "World"
srs = "EPSG:4326"
```

> **Environment variable overrides**: all configuration options can be overridden via `GEOSERVER__<section>__<key>` (e.g. `GEOSERVER__SERVER__PORT=9090`). In container deployments, use K8s ConfigMap / Secret injection.

## 📚 Documentation

- [Integration build notes](BUILD_INTEGRATION.md)
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

**Made with ❤️ using Rust + Angular**
