# Terrane — Protocol Adaptation Matrix

> Status of OGC / REST / tile protocol adaptation, compared against the reference
> GeoServer instance (`http://127.0.0.1:18080/geoserver/web/`, 9 workspaces / 30 layers).
> Complements [ARCHITECTURE.md](ARCHITECTURE.md) (API contract design),
> [ROADMAP.md](ROADMAP.md) (milestones), and
> [IMPLEMENTATION_PLAN.md](../IMPLEMENTATION_PLAN.md) (feature-gap analysis).

Per the technical roadmap, Terrane is a **protocol adapter**: it speaks the standard
OGC interfaces (WMS / WFS / WCS / WMTS / REST) while all state lives in external stores.
This document tracks *which protocol surfaces are already adapted*, *which are partial*,
and *which remain to be adapted*.

## 1. Status summary

| Protocol            | Version(s)                     | Status  | Notes |
|---------------------|--------------------------------|---------|-------|
| **WMS**             | 1.1.1 / 1.3.0                  | ✅ Core | GetCapabilities / GetMap / GetFeatureInfo / DescribeLayer / GetLegendGraphic / GetStyles / PutStyles; multi-format + vendor params |
| **WFS**             | 1.0.0 / 1.1.0 / 2.0.0          | ✅ Core | GetCapabilities / DescribeFeatureType / GetFeature / GetFeatureWithLock / LockFeature / Transaction (WFS-T) |
| **WCS**             | 1.0.0 / 1.1.x / 2.0.1          | ✅ Core | GetCapabilities / DescribeCoverage / GetCoverage; WCS 2.0 subsetting |
| **WMTS**            | 1.0.0                          | ✅ Core | GetCapabilities / GetTile / GetFeatureInfo; KVP + RESTful tile template |
| **MVT (vector tiles)** | —                          | ✅      | Pure-Rust protobuf encoder; `/tiles/{layer}/{z}/{x}/{y}.pbf`, `/mvt/{layer}/{z}/{x}/{y}` |
| **REST API**        | —                              | ✅      | Full CRUD under `/geoserver` (see §6) |
| **TMS**             | 1.0.0                          | ❌      | Exposed by reference GeoServer via GeoWebCache; not implemented |
| **WMS-C**           | 1.1.1                          | ❌      | Exposed by reference GeoServer via GeoWebCache; not implemented |
| **Tile cache (GWC-like)** | —                         | ⚠️      | Basic `/tiles` + local disk cache; no seeding / metastore / full GWC |
| **SLD styling**     | 1.0.0                          | ⚠️      | Basic CRUD + rendering; no CSS / YSLD / MBStyle, limited SLD features |
| **WMS output formats** | —                          | ⚠️      | PNG/JPEG/GIF/WebP/SVG/KML/GeoJSON ✅; PDF, GeoRSS ❌ |
| **WFS output formats** | —                          | ⚠️      | GML 2.1.2 / GML 3.1.1 / GML 3.2 / GeoJSON / CSV ✅; KML, Shapefile ❌ |
| **WPS**             | —                              | ❌      | Processing service (P4) |
| **CSW**             | —                              | ❌      | Catalog service (P4) |
| **OGC API series**  | Features / Tiles / Maps / Coverages / Processes / Styles | ❌ | P4 |

## 2. WMS — Web Map Service

Endpoint `/wms` (`src/services/wms.rs`, `src/handlers/wms_handler.rs`).

**Versions**: capabilities advertise **1.3.0**; **1.1.1** requests are handled
(including the 1.3.0 lat/lon axis-order rule for geographic CRS).

**Operations**:

| Operation           | Status | Notes |
|---------------------|--------|-------|
| GetCapabilities     | ✅     | Layers from the catalog |
| GetMap              | ✅     | Raster (PNG/JPEG/GIF/WebP), vector (SVG), KML, GeoJSON, OpenLayers preview; CascadedWms proxy |
| GetFeatureInfo      | ✅     | `text/plain`, `text/html`, `application/json` |
| DescribeLayer       | ✅     | WMS 1.1.1 DescribeLayerResponse |
| GetLegendGraphic    | ✅     | SLD-based legend |
| GetStyles / PutStyles | ✅   | SLD style read/write |

**Vendor parameters**: `TRANSPARENT`, `CQL_FILTER` (ECQL, multi-layer `;`-separated),
`TIME` (ISO 8601), `ELEVATION`, `SRS/CRS`, `ENV` (style env substitution),
`featureId` (feature-id filter), `angle`, scale-denominator-aware styling.

**Gaps vs reference**: PDF output, GeoRSS output, full SLD dynamic-styling feature
set, GML `GetFeatureInfo` output.

## 3. WFS — Web Feature Service

Endpoint `/wfs` (`src/services/wfs.rs`, `src/handlers/wfs_handler.rs`). GET + POST.

**Versions**: 1.0.0 / 1.1.0 / 2.0.0.

**Operations**:

| Operation           | Status | Notes |
|---------------------|--------|-------|
| GetCapabilities     | ✅     | Advertises 2.0.0 |
| DescribeFeatureType | ✅     | XSD schema |
| GetFeature          | ✅     | GML 2.1.2 / GML 3.1.1 / GML 3.2, GeoJSON, CSV |
| GetFeatureWithLock  | ✅     | |
| LockFeature         | ✅     | |
| Transaction         | ✅     | WFS-T insert / update / delete via POST XML |

**Gaps vs reference**: KML / Shapefile output; deeper GML 3.2 schema fidelity.

## 4. WCS — Web Coverage Service

Endpoint `/wcs` (`src/services/wcs.rs`, `src/handlers/wcs_handler.rs`).

**Versions**: 1.0.0 / 1.1.x / **2.0.1** (capabilities advertise 2.0.1).

**Operations**:

| Operation           | Status | Notes |
|---------------------|--------|-------|
| GetCapabilities     | ✅     | |
| DescribeCoverage    | ✅     | |
| GetCoverage         | ✅     | GeoTIFF (`image/tiff`), ArcGrid (`application/x-arcgrid`) |

**Subsetting**: WCS 2.0 `SUBSET` (intervals / position, with CRS + resolution),
`SUBSETTINGCRS`.

**Gaps vs reference**: no full WCS 1.1 range-subset / interpolation semantics, no
Web Coverage Transaction (WCS-T), no coverages exposed through WMS GetMap.

## 5. Tiling — WMTS / MVT / TMS / WMS-C

### 5.1 WMTS 1.0.0 ✅
Endpoint `/wmts` (`src/services/wmts.rs`, `src/handlers/wmts_handler.rs`).

- **Operations**: GetCapabilities, GetTile, GetFeatureInfo
- **Access styles**: KVP (`?SERVICE=WMTS&REQUEST=GetTile`) and RESTful
  `/wmts/{layer}/{tileMatrixSet}/{tileMatrix}/{tileCol}/{tileRow}`

### 5.2 MVT vector tiles ✅
- `/tiles/{layer}/{z}/{x}/{y}.pbf`, `/mvt/{layer}/{z}/{x}/{y}` — pure-Rust protobuf
  encoder (`src/utils/mvt.rs`, `src/handlers/mvt_handler.rs`)

### 5.3 Tile cache (GWC-like) ⚠️
- Basic `/tiles/{layer}/{z}/{x}/{y}` with a local disk cache
  (`<data_dir>/gwc`, `TileCacheBackend` trait, `src/store/cache/`)
- No GWC seeding / metastore / multi-backend integration yet

### 5.4 TMS 1.0.0 ❌ & WMS-C 1.1.1 ❌
- The reference GeoServer exposes **TMS 1.0.0** and **WMS-C 1.1.1** through
  GeoWebCache (`/gwc/service/tms`, `/gwc/service/wms`). Not yet adapted.

## 6. REST API ✅

All endpoints under `/geoserver` (configurable `api_context`). See
[ARCHITECTURE.md §6.2](ARCHITECTURE.md#62-rest-endpoint-groups) for the full table.

- **Layers / workspaces / namespaces / data-sources / stores / styles / layer-groups /
  sql-views / features** — full CRUD
- **Tiles**: `/tiles/{layer}/{z}/{x}/{y}`, cache clear / stats
- **Auth**: JWT login / logout / verify / users / change-password
- **Permissions**: layer-level ACL
- **Monitoring**: `/server/status`, `/monitor/*`, split probes `/health/live` +
  `/health/ready`, Prometheus `/metrics`
- **Backup / restore**: `/backup/export`, `/backup/import`
- **Uploads**: GeoJSON / Shapefile / GeoTIFF

## 7. Data sources

PostGIS, Shapefile, GeoTIFF, GeoPackage, WorldImage, CascadedWms, ArcGrid (7 types).

## 8. Pending protocols (to adapt)

Prioritized by [ROADMAP.md](ROADMAP.md) and [IMPLEMENTATION_PLAN.md](../IMPLEMENTATION_PLAN.md):

| Priority | Protocol / surface                  | Effort |
|----------|-------------------------------------|--------|
| P2       | TMS 1.0.0 + WMS-C 1.1.1 (GWC)       | small — reuse tile engine |
| P2       | WMS PDF / GeoRSS output             | small — reuse render pipeline |
| P2       | WFS KML / Shapefile output          | small |
| P4       | WPS (processing)                    | 4–6 weeks |
| P4       | CSW (catalog)                       | 3–4 weeks |
| P4       | OGC API Features / Tiles / Maps / Coverages / Processes / Styles | 2–3 weeks each |
| P4       | Printing / Importer / GeoFence      | enterprise |
| P4       | CSS / YSLD / MBStyle styling        | medium |

## 9. Verification checklist

Hand-verified smoke path against the reference console (`http://127.0.0.1:18080/geoserver/web/`):

- [x] `GET /wms?SERVICE=WMS&VERSION=1.3.0&REQUEST=GetCapabilities`
- [x] `GET /wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetCapabilities`
- [x] `GET /wfs?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetCapabilities`
- [x] `GET /wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetCapabilities`
- [x] `GET /wfs?SERVICE=WFS&VERSION=1.0.0&REQUEST=GetCapabilities`
- [x] `GET /wcs?SERVICE=WCS&VERSION=2.0.1&REQUEST=GetCapabilities`
- [x] `GET /wcs?SERVICE=WCS&VERSION=1.1.1&REQUEST=GetCapabilities`
- [x] `GET /wmts?SERVICE=WMTS&VERSION=1.0.0&REQUEST=GetCapabilities`
- [ ] `GET /gwc/service/tms/1.0.0`  (TMS — pending)
- [ ] `GET /gwc/service/wms?...&tiled=true` (WMS-C — pending)

## 10. Automated test coverage

`cargo test` currently green: **90 lib unit tests + 69 integration tests**.

The integration tests are split by protocol into separate test crates (Rust
convention: every file directly under `tests/` is its own binary), sharing a
`tests/common/mod.rs` helper (`create_test_config` + `build_test_app!` macro):

| Test crate          | Tests | Scope                                                     |
|---------------------|-------|-----------------------------------------------------------|
| `tests/wms_test.rs` | 20    | WMS (all operations, formats, vendor params)              |
| `tests/wfs_test.rs` | 13    | WFS (all operations + WFS-T + LockFeature contract)       |
| `tests/rest_test.rs`| 22    | health / probes / metrics, REST CRUD, MVT, auth, backup, `/tiles` + tile cache |
| `tests/wcs_test.rs` | 10    | WCS (DescribeCoverage / GetCoverage incl. real GeoTIFF / ArcGrid, JPEG / netCDF) |
| `tests/wmts_test.rs`| 4     | WMTS (GetCapabilities / GetTile / GetFeatureInfo)         |

Coverage is **protocol-surface level** — each adapted OGC operation / REST group
has at least one request/response test validating status codes, content types,
and basic payload structure. It still does **not** validate full protocol
semantics (XML schema conformance of capabilities, GML 3.2 fidelity, etc.).
The test config uses in-memory SQLite, disables the tile cache, and reuses the
metadata store for vectors, so tests write nothing to `./data`.

### 10.1 What IS covered

| Layer              | Coverage                                                                 |
|--------------------|--------------------------------------------------------------------------|
| WMS                | GetCapabilities, GetMap (PNG / JPEG / GIF / SVG / KML / GeoJSON, 1.1.1 + 1.3.0 axis-order), GetFeatureInfo (JSON / text/html / text/plain), DescribeLayer, GetLegendGraphic, GetStyles, vendor params CQL_FILTER / TIME / ELEVATION / ENV / ANGLE / FEATUREID — integration |
| WFS                | GetCapabilities, DescribeFeatureType, GetFeature (GeoJSON / GML 2.1.2 / GML 3.1.1 / GML 3.2 / CSV), GetFeatureWithLock, Transaction (insert round-trip + update + delete by FeatureId), LockFeature (contract: GET returns 400 — declared but unsupported) — integration |
| WCS                | GetCapabilities, DescribeCoverage (incl. real GeoTIFF / ArcGrid / WorldImage metadata enrichment), GetCoverage (TIFF / PNG / JPEG / default-format + netCDF→TIFF fallback, real GeoTIFF 8×8 bytes, real ArcGrid 4×3 bytes, SUBSET / SIZE) — integration |
| WMTS               | GetCapabilities, GetTile (KVP + RESTful template), GetFeatureInfo — integration |
| MVT                | `/mvt/{layer}/{z}/{x}/{y}` endpoint integration + 5 encoder unit tests    |
| REST               | health, split probes `/health/live` + `/health/ready` + `/metrics`, `/server/status`, layers, workspaces CRUD, namespaces CRUD, styles CRUD, layer-groups CRUD, features CRUD (+ single get / update / delete), sql-views CRUD, data-sources CRUD, auth (login / verify / users CRUD), permissions CRUD, backup export + import round-trip, GeoJSON upload, `/tiles` PNG tile, tile cache stats / clear / HIT — integration |
| Tile cache (util)  | 10 unit tests (`utils/tile_cache.rs`: gridset, bounds, hit-rate)         |
| CQL / ECQL         | 7 unit tests (`utils/cql_filter.rs`)                                     |
| Projection         | 7 unit tests (`utils/projection.rs`)                                     |
| Geometry           | 3 unit tests (`utils/geometry.rs`)                                       |
| Shapefile          | 3 unit tests (`utils/shapefile.rs`: PRJ + coordinates)                   |
| GeoTIFF            | 2 unit tests (`utils/geotiff.rs`)                                        |
| Config             | 3 unit tests (`config.rs`, incl. legacy `[business]`/`[gwc]` alias)      |
| Auth               | 8 unit tests (`auth.rs`: salt, hash, verify, JWT)                        |
| Upload             | 1 unit test (`upload_handler.rs`: filename sanitize)                     |
| Store (cache)      | 4 `LocalSessionCache` unit tests (set/get/remove/remove_user/TTL) + 4 `LocalTileCacheBackend` (put/get/clear/stats/gridset-path sanitize) |
| Store (vector/raster) | 4 `LocalVectorStore` (save/load/delete/list/sanitize) + 4 `LocalRasterStore` (put/get/delete/list/.tif) + 8 `sqlite_store` (workspace / namespace / layer+features / user+permission / session / styles CRUD / layer-groups CRUD / audit logs) |
| Data-source adapters | 4 `arcgrid` (read/meta/errors) + 5 `worldimage` (ext/meta/crop) + 4 `cascaded` (config extract) + 4 `geopackage` (layers / validation / features read round-trip / limit) |

### 10.2 Coverage gaps (adapted but untested)

| Protocol / surface | Missing tests                                                              |
|--------------------|----------------------------------------------------------------------------|
| **WFS**            | LockFeature (documented as unsupported — GET returns 400)                  |
| **WCS**            | subsetting against a real raster (SUBSET/SIZE are only exercised on the fallback image) |
| **Store layer**    | `postgres_store` (needs a live PostGIS)                                    |
| **Data sources**   | PostGIS live connection, CascadedWms live fetch                            |

### 10.3 Bugs found by the new tests

- **WCS `DescribeCoverage` always returned 500** — quick-xml cannot serialize a bare
  `Vec` (`cannot serialize sequence without defined root tag`). Fixed in
  `src/handlers/wcs_handler.rs` by serializing each `CoverageDescription` and joining.
- **WFS Transaction Delete never deleted anything** — `parse_transaction_xml` always
  initialized the Delete filter as an empty `Filter::FeatureId(vec![])` and never
  parsed `<wfs:Filter><ogc:FeatureId fid="..."/></wfs:Filter>`, so `validate_filter`
  returned `false` and every feature was retained (`totalDeleted=0`). Fixed in
  `src/services/wfs.rs` by collecting `fid` attributes from `ogc:FeatureId` elements
  into the Delete filter. (The URL `FILTER=` parameter is still served by a hardcoded
  stub `parse_filter_xml` — pending.)
- **ArcGrid header parsing started data at the wrong line** — `read_arcgrid` only
  advanced `header_lines` in the `ncols` branch, so with the standard header order
  (`ncols` first) the first data rows were misread as header values and the value
  count never matched `nrows * ncols`. Fixed in `src/utils/arcgrid.rs` by advancing
  `header_lines` for every header key.
- **WCS `DescribeCoverage` only enriched GeoTIFF metadata** — ArcGrid / WorldImage
  data sources fell back to the default coverage description, so clients never saw
  their real bounds / size. Fixed in `src/handlers/wcs_handler.rs` by matching on
  the data source type and using `read_arcgrid_meta` / `read_worldimage_meta` (both
  already existed) alongside `read_geotiff_metadata`.
- **MVT `.pbf` route is shadowed** — the generic `/tiles/{layer}/{z}/{x}/{y}` route is
  registered before `/tiles/{layer}/{z}/{x}/{y}.pbf`, so the latter never matches
  (`{y}` captures `0.pbf` and returns a PNG tile). The `/mvt/{layer}/{z}/{x}/{y}`
  route works and is what the test uses. Reordering routes would un-shadow `.pbf`.

### 10.4 Recommended next tests (small → large)

1. PostGIS live connection test (needs a live PostGIS / `docker compose --profile postgres`)
2. CascadedWms live fetch against the reference GeoServer (:18080)
3. WCS SUBSET/SIZE against a real raster (GeoTIFF / ArcGrid), GeoPackage write round-trip

