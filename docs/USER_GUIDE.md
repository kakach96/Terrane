# Terrane — User Guide

> Hands-on manual for data publishers and GIS administrators: install, publish
> data, consume it over OGC web services, secure it, and operate the server.
>
> For local development setup see [DEVELOPMENT.md](DEVELOPMENT.md); for design
> rationale see [ARCHITECTURE.md](ARCHITECTURE.md).

---

## 1. Installation & Quick Start

### 1.1 Run locally

```bash
# Full build (frontend + backend), then start
./build/build.sh          # Windows: ./build/build.bat
cd service && cargo run   # serves http://127.0.0.1:8080
```

Open **http://localhost:8080** — the Angular admin console is served by the
backend itself. Log in with the default administrator:

| Username | Password   |
|----------|------------|
| `admin`  | `terrane` |

### 1.2 Run with Docker

```bash
docker build -t terrane:latest .
docker compose -f build/docker-compose.yml --profile terrane up -d
```

The image listens on `0.0.0.0:8080`; inject `TERRANE_JWT_SECRET` in any
multi-user / multi-replica deployment.

### 1.3 Where things live

| Path                        | Purpose                                        |
|-----------------------------|------------------------------------------------|
| `http://localhost:8080`     | Admin console (UI)                             |
| `http://localhost:8080/terrane/...` | REST API (`api_context`)             |
| `http://localhost:8080/wms|wfs|wcs|wmts|wps|csw` | OGC services (root paths) |
| `/health/live`, `/health/ready`, `/metrics` | Probes & Prometheus metrics     |

---

## 2. Core Concepts

Terrane is a **data publishing platform**: business data stays in external
stores (PostGIS tables, MySQL, MongoDB, or files on disk / object storage),
while Terrane handles cataloging, styling, caching and OGC protocol
adaptation.

```
workspace ──▶ data source ──▶ layer ──▶ style ──▶ publish ──▶ OGC clients
 (namespace)   (connection)    (map view)  (SLD/CSS/…)   (preview/tiles)
```

| Concept       | Meaning                                                                 |
|---------------|-------------------------------------------------------------------------|
| **Workspace** | Namespace grouping layers and stores (like GeoServer workspaces)        |
| **Data source** | A registered connection: a database, or a vector/raster file          |
| **Layer**     | A published view of one table/file (`store` = data source name, `native_name` = table name) |
| **Style**     | How the layer is drawn — SLD, CSS, YSLD or MBStyle                      |
| **Layer group** | A set of layers rendered together under one name                      |
| **Tile cache** | GWC-style tile cache (local disk or Redis) with seeding & quotas       |

Supported data source types: PostGIS · MySQL · MongoDB · GeoJSON · Shapefile ·
GeoPackage · GeoTIFF · WorldImage · ArcGrid · ImageMosaic · ImagePyramid ·
CascadedWMS · Redis (cache backend).

---

## 3. Publishing Your First Layer

### 3.1 Upload a file (fastest path)

Drag-free, REST-only example — upload a GeoJSON FeatureCollection; Terrane
creates the data source **and** you then publish the layer:

```bash
curl -X POST http://localhost:8080/terrane/data/upload \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "type": "FeatureCollection",
    "total_count": 1,
    "features": [{
      "id": "poi-1", "type": "Feature",
      "geometry": {"type": "Point", "coordinates": [2.35, 48.85]},
      "properties": {"name": "Paris", "layer_name": "pois"}
    }]
  }'

curl -X POST http://localhost:8080/terrane/layers \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "name": "pois", "title": "Points of Interest",
    "workspace": "default",
    "store": "pois", "native_name": "pois",
    "srs": "EPSG:4326",
    "minx": 2.2, "miny": 48.7, "maxx": 2.5, "maxy": 49.0
  }'
```

> The data source name is taken from the features' `layer_name` property.
> Shapefile (`.zip`) and GeoTIFF uploads use multipart form-data:
> `POST /terrane/data/upload/shapefile?layer=myshp` and
> `POST /terrane/data/upload/geotiff?layer=mytif`.
> Add `&storage=s3&bucket=my-bucket` to the GeoTIFF upload to store the raster
> in S3/MinIO so all replicas can read it.

In the admin UI the same flow lives under **Data Sources → Upload**, then
**Layers → Add layer**.

### 3.2 Publish a database table

1. Register the connection — **Data Sources → Add** (or
   `POST /terrane/data-sources`) with `type: postgis` (or `mysql`, `mongo`)
   plus host/port/database/user/password.
2. Use `GET /terrane/data-sources/{name}/tables` to list publishable tables.
3. Publish: create a layer whose `store` = data source name and
   `native_name` = table name.
4. Test connectivity anytime via `POST /terrane/data-sources/{name}/test`.

For virtual layers built from parameterized SQL, see **SQL views**
(`POST /terrane/sql-views`, preview at `/terrane/sql-views/preview`).

### 3.3 Style it

Styles support four syntaxes; pick per layer via
`PUT /terrane/layers/{name}/style`:

```bash
curl -X PUT http://localhost:8080/terrane/layers/pois/style \
  -H "Content-Type: application/json" -H "Authorization: Bearer $TOKEN" \
  -d '{"style": "points"}'
```

Manage styles under **Styles** in the UI or `GET/POST /terrane/styles`
(formats: `sld`, `css`, `ysld`, `mbstyle`). Built-in templates are listed by
the styles API.

### 3.4 Preview

- UI: open the layer → **Preview** (OpenLayers map).
- Plain image:
  `GET /terrane/layers/pois/preview?width=512&height=512&format=png`

---

## 4. Consuming Data (OGC Services)

All examples assume host `http://localhost:8080` and layer `pois`.

### 4.1 WMS — rendered maps

```bash
# Map image (WMS 1.1.1)
curl "http://localhost:8080/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap\
&LAYERS=pois&BBOX=-180,-90,180,90&WIDTH=768&HEIGHT=384\
&SRS=EPSG:4326&FORMAT=image/png&STYLES=" -o map.png

# Capabilities document (advertises every layer)
curl "http://localhost:8080/wms?SERVICE=WMS&REQUEST=GetCapabilities"
```

Useful vendor parameters: `cql_filter=` (attribute/spatial pre-filter),
`featureId=`, `angle=`, `env=` (style variable substitution), plus standard
`TIME=` / `ELEVATION=` dimensions. Non-PNG outputs: `image/svg+xml`,
`application/kml`, `application/geojson`.

Feature info at a pixel:

```bash
curl "http://localhost:8080/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetFeatureInfo\
&LAYERS=pois&QUERY_LAYERS=pois&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256\
&SRS=EPSG:4326&I=128&J=128&INFO_FORMAT=application/json"
```

Legend: `GET /wms?SERVICE=WMS&REQUEST=GetLegendGraphic&LAYER=pois&FORMAT=image/png`.

### 4.2 WFS — vector features

```bash
# GeoJSON
curl "http://localhost:8080/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=pois&OUTPUTFORMAT=application/json"

# Other formats: csv, text/xml; subtype=gml/3.2, shape-zip …
curl "http://localhost:8080/wfs?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetPropertyValue\
&TYPENAMES=pois&PROPERTYNAME=name"
```

WFS 1.0/1.1/2.0 operations include DescribeFeatureType, GetFeature (with
CQL/OGC filtering), LockFeature / GetFeatureWithLock, GetPropertyValue and
GetGmlObject.

### 4.3 WCS — raster coverages

```bash
curl "http://localhost:8080/wcs?SERVICE=WCS&VERSION=2.0.1&REQUEST=GetCoverage\
&COVERAGEID=dem&FORMAT=image/tiff" -o coverage.tiff
```

Supports spatial subsets, range-band subsets and interpolation
(`INTERPOLATION=nearest|bilinear|cubic|lanczos`).

### 4.4 WMTS & tiled access

```bash
# Capabilities + KVP GetTile
curl "http://localhost:8080/wmts?SERVICE=WMTS&REQUEST=GetCapabilities"
curl "http://localhost:8080/wmts?SERVICE=WMTS&REQUEST=GetTile&LAYER=pois\
&STYLE=default&TILEMATRIXSET=EPSG:4326&TILEMATRIX=EPSG:4326:3&TILEROW=2&TILECOL=6&FORMAT=image/png"

# RESTful template
curl "http://localhost:8080/terrane/wmts/pois/EPSG:4326/3/6/2"

# Simple XYZ tiles (slippy) and vector tiles (.pbf / MVT)
curl "http://localhost:8080/terrane/tiles/pois/3/6/2"
curl "http://localhost:8080/terrane/tiles/pois/3/6/2.pbf"
```

GeoWebCache-compatible endpoints: `/terrane/gwc/service/tms` (TMS 1.0.0) and
`/terrane/gwc/service/wms` (WMS-C).

### 4.5 WPS · CSW · OGC API

```bash
curl "http://localhost:8080/wps?SERVICE=WPS&REQUEST=GetCapabilities"      # processing (Buffer/Centroid/Bounds…)
curl "http://localhost:8080/csw?SERVICE=CSW&REQUEST=GetCapabilities"      # catalog/metadata search

# OGC API building blocks (Features / Maps / Tiles / Coverages / Styles / Processes)
curl "http://localhost:8080/ogc/features/collections"                     # feature collections
curl "http://localhost:8080/ogc/features/collections/pois/items?limit=10" # items as GeoJSON
curl "http://localhost:8080/ogc/maps/collections"                         # renderable map collections
```

---

## 5. Tile Caching & Seeding

Tiles are cached per layer (local disk by default; select a Redis data source
via the layer's `cache_store` for multi-replica sharing).

```bash
# Seed a zoom range in the background
curl -X POST http://localhost:8080/terrane/tiles/seed \
  -H "Content-Type: application/json" -H "Authorization: Bearer $TOKEN" \
  -d '{"layer": "pois", "z_min": 0, "z_max": 10}'

curl http://localhost:8080/terrane/tiles/seed            # list jobs / progress
curl -X DELETE http://localhost:8080/terrane/tiles/seed/<job-id>   # cancel

curl http://localhost:8080/terrane/tiles/cache/stats     # hit rate & size
curl -X DELETE http://localhost:8080/terrane/tiles/cache/clear/pois
curl -X POST http://localhost:8080/terrane/tiles/seed/truncate \
  -H "Content-Type: application/json" -d '{"layer": "pois"}'
```

Disk quota (LRU eviction) is configured per layer via `layer_quota_bytes`;
conditional requests return `304` when a cached tile is fresh (ETag /
Last-Modified).

---

## 6. Security

### 6.1 Authentication (JWT)

```bash
TOKEN=$(curl -s -X POST http://localhost:8080/terrane/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "terrane"}' \
  | sed -n 's/.*"token":"\([^"]*\)".*/\1/p')

curl http://localhost:8080/terrane/auth/verify -H "Authorization: Bearer $TOKEN"
```

Send `Authorization: Bearer $TOKEN` for administrative calls (writes,
user management, permissions, backup). OGC read endpoints stay open unless
locked down with permissions.

### 6.2 Users & roles

```bash
curl -X POST http://localhost:8080/terrane/auth/users \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"username": "alice", "password": "secret123", "role": "user"}'
curl http://localhost:8080/terrane/auth/users -H "Authorization: Bearer $TOKEN"
```

Roles: `admin` ⊇ `write` ⊇ `read`. Enterprise deployments can delegate login to
LDAP (`[security.ldap]`: url/base_dn/admin_group; users auto-provision on
first login).

### 6.3 Permissions (layer-level ACL)

Rules match user/role against workspace/store/layer with allow/deny effect
(most specific wins, deny beats ties):

```bash
curl -X POST http://localhost:8080/terrane/permissions \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"username": "*", "role": "user", "resource_type": "layer", "resource_name": "pois", \
       "access_mode": "read", "effect": "allow", "priority": 10}'

curl http://localhost:8080/terrane/permissions/check/layer/pois -H "Authorization: Bearer $TOKEN"
```

Enable the fine-grained **GeoFence** engine (per-request enforcement on
WMS/WFS/WCS) with `[security] geofence_enabled = true` in `service/terrane.toml`.

---

## 7. Operations

| Task               | How                                                              |
|--------------------|------------------------------------------------------------------|
| Server status      | `GET /terrane/server/status` · `/about/version`                |
| Monitoring         | `GET /terrane/monitor/stats` · `/monitor/requests` · `/monitor/logs` |
| Metrics (Prometheus)| `GET /metrics`                                                  |
| Health probes      | `GET /health/live` · `GET /health/ready`                         |
| Backup / restore   | `GET /terrane/backup/export` · `POST /terrane/backup/import` |
| Audit logs         | `GET /terrane/monitor/logs`                                    |

Graceful shutdown drains in-flight requests (`shutdown_timeout_secs`);
structured JSON logs + `trace_id` via `[logging] format = "json"`.

Performance validation: run the bundled benchmarks and HTTP load harness
(`cd service && cargo bench`, `cd service && cargo test --test perf_test -- --ignored --nocapture`) —
see [DEVELOPMENT.md](DEVELOPMENT.md) §7.

---

## 8. Configuration Pointers

Configuration precedence: **CLI flags > environment variables > service/terrane.toml >
defaults**. Common knobs:

| Setting              | Env var / config key                              |
|----------------------|---------------------------------------------------|
| Host / port          | `TERRANE__SERVER__HOST` / `_PORT`               |
| Metadata store       | `[metadata] kind = sqlite \| postgres`            |
| JWT secret           | `TERRANE__SECURITY__JWT_SECRET` (**set in prod**) |
| Rate limiting / timeouts | `[server] rate_limit_*` / `request_timeout_secs` |
| Tile cache dir       | `TERRANE__DATA_DIR` + `[cache]`                 |

The full variable table, Docker guidance and troubleshooting live in
[DEVELOPMENT.md](DEVELOPMENT.md).

---

## 9. Troubleshooting

| Symptom                            | Fix                                                                  |
|------------------------------------|----------------------------------------------------------------------|
| UI blank / assets 404              | Ensure `service/static/` contains a frontend build (or dev-mode via `npm start`) |
| Preview shows empty map            | Check layer bounds vs data extent; verify CRS matches request SRS     |
| WMS returns ServiceException XML   | Read the exception text — usually bad BBOX order or unknown layer    |
| Login rejected after restart       | Sessions persist in the metadata DB; check SQLite/PostgreSQL path     |
| Tiles stale after data change      | `DELETE /terrane/tiles/cache/clear/{layer}` or truncate + reseed    |
| 401 on admin REST call             | Missing/expired Bearer token → log in again                          |

---

## 10. Upgrading from pre-1.2 (`geoserver` → `terrane` naming)

v1.2 renamed the product identifiers from `geoserver` to `terrane`. This is a
**breaking change** — review the items below before upgrading an existing
installation.

| What changed                                   | Old (pre-1.2)                    | New (1.2+)                          | Migration                                                                 |
|------------------------------------------------|----------------------------------|-------------------------------------|---------------------------------------------------------------------------|
| Default metadata SQLite path                   | `geoserver.sqlite`               | `terrane.sqlite`                    | Rename the file, or set `[metadata] sqlite_path` / `TERRANE__METADATA__SQLITE_PATH` to the old path |
| Default API context                            | `/geoserver`                     | `/terrane`                          | Update clients/proxies, or set `[server] api_context = "/geoserver"` temporarily |
| Environment variable prefix                    | `GEOSERVER__*`                   | `TERRANE__*`                        | `GEOSERVER__*` still works as a deprecated alias; `TERRANE__*` wins when both are set. Migrate scripts/manifests at your own pace |
| Default admin password (fresh installs only)   | `geoserver`                      | `terrane`                           | Existing databases keep their users; only brand-new catalogs get the new default |
| Default PostgreSQL database name               | `geoserver`                      | `terrane`                           | Set `[metadata.postgres] instance` if your DB keeps the old name |
| Frontend localStorage keys                     | `geoserver_user` / `geoserver_token` | `terrane_user` / `terrane_token` | Users are logged out once after upgrade; no action needed                |
| Docker compose volume                          | `geoserver-data`                 | `terrane-data`                      | Re-attach the old volume or copy its contents to the new one             |

> **Warning**: if you upgrade without migrating the SQLite path, the backend
> starts with an empty catalog and (with `[samples] enabled`, the default)
> auto-seeds the demo workspace — this can look like "data loss". The old
> `geoserver.sqlite` file is never deleted; point `sqlite_path` back at it to
> recover.

GML / XML namespace URIs such as `http://geoserver.org/feature` and
`http://geoserver.org/{workspace}` in WFS output are **intentionally kept** for
GeoServer wire compatibility — they are not part of the rename (see
[PROTOCOLS.md](PROTOCOLS.md) §3).
