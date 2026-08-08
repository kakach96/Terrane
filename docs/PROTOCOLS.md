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

`cargo test` currently green: **102 lib unit tests + 78 integration tests**, plus
**5 `#[ignore]`-marked live tests** (3× PostGIS + 2× CascadedWms) that require
running services and are verified with `cargo test -- --ignored`.

The integration tests are split by protocol into separate test crates (Rust
convention: every file directly under `tests/` is its own binary), sharing a
`tests/common/mod.rs` helper (`create_test_config` + `build_test_app!` macro):

| Test crate          | Tests | Scope                                                     |
|---------------------|-------|-----------------------------------------------------------|
| `tests/wms_test.rs` | 20 (+2 ignored live) | WMS (all operations, formats, vendor params; + cascaded WMS live proxy incl. CQL_FILTER / TIME vendor-param pass-through) |
| `tests/wfs_test.rs` | 19    | WFS (all operations + WFS-T + LockFeature contract + FILTER= OGC XML / ECQL + CQL_FILTER) |
| `tests/rest_test.rs`| 23 (+1 ignored live) | health / probes / metrics, REST CRUD, MVT, auth, backup, `/tiles` + tile cache, **GeoPackage data source over REST + `/layers/{layer}/feature-type` typed columns**; + PostGIS data source HTTP (live) |
| `tests/wcs_test.rs` | 12    | WCS (DescribeCoverage / GetCoverage incl. real GeoTIFF / ArcGrid + SUBSET / SIZE, JPEG / netCDF) |
| `tests/wmts_test.rs`| 4     | WMTS (GetCapabilities / GetTile / GetFeatureInfo)         |

Coverage is **protocol-surface level** — each adapted OGC operation / REST group
has at least one request/response test validating status codes, content types,
and basic payload structure. It still does **not** validate full protocol
semantics (XML schema conformance of capabilities, GML 3.2 fidelity, etc.).
The test config uses in-memory SQLite, disables the tile cache, and reuses the
metadata store for vectors, so tests write nothing to `./data`. The live tests
(`#[ignore]`) talk to a local PostGIS (env `GEOSERVER_TEST_PG_*`, defaults
127.0.0.1:5432 postgres/`kakach2026`) and the reference GeoServer at :18080.

### 10.1 What IS covered

| Layer              | Coverage                                                                 |
|--------------------|--------------------------------------------------------------------------|
| WMS                | GetCapabilities, GetMap (PNG / JPEG / GIF / SVG / KML / GeoJSON, 1.1.1 + 1.3.0 axis-order), GetFeatureInfo (JSON / text/html / text/plain), DescribeLayer, GetLegendGraphic, GetStyles, vendor params CQL_FILTER / TIME / ELEVATION / ENV / ANGLE / FEATUREID — integration |
| WFS                | GetCapabilities, DescribeFeatureType, GetFeature (GeoJSON / GML 2.1.2 / GML 3.1.1 / GML 3.2 / CSV), GetFeatureWithLock, Transaction (insert round-trip + update + delete by FeatureId), LockFeature (contract: GET returns 400 — declared but unsupported), URL `FILTER=` (OGC XML PropertyIsEqualTo / PropertyIsGreaterThan + ECQL `name='x'` / `bbox(...)` / `LIKE`+`AND`) and `CQL_FILTER` (ECQL) — integration |
| WCS                | GetCapabilities, DescribeCoverage (incl. real GeoTIFF / ArcGrid / WorldImage metadata enrichment), GetCoverage (TIFF / PNG / JPEG / default-format + netCDF→TIFF fallback, real GeoTIFF 8×8 bytes, real ArcGrid 4×3 bytes, SUBSET/SIZE on real ArcGrid AND real georeferenced GeoTIFF: crop→2×2 + resize→8×8) — integration |
| WMTS               | GetCapabilities, GetTile (KVP + RESTful template), GetFeatureInfo — integration |
| MVT                | `/mvt/{layer}/{z}/{x}/{y}` endpoint integration + 5 encoder unit tests    |
| REST               | health, split probes `/health/live` + `/health/ready` + `/metrics`, `/server/status`, layers, workspaces CRUD, namespaces CRUD, styles CRUD, layer-groups CRUD, features CRUD (+ single get / update / delete), sql-views CRUD, data-sources CRUD, auth (login / verify / users CRUD), permissions CRUD, backup export + import round-trip, GeoJSON upload, `/tiles` PNG tile, tile cache stats / clear / HIT, live PostGIS data source HTTP (`/data-sources/{name}/tables` + `/layers/{layer}/feature-type` against a real schema) — integration |
| Tile cache (util)  | 10 unit tests (`utils/tile_cache.rs`: gridset, bounds, hit-rate)         |
| CQL / ECQL         | 8 unit tests (`utils/cql_filter.rs`) — incl. bug8 regression: `A AND B` is a real AND, not OR |
| Projection         | 7 unit tests (`utils/projection.rs`)                                     |
| Geometry           | 3 unit tests (`utils/geometry.rs`)                                       |
| Shapefile          | 3 unit tests (`utils/shapefile.rs`: PRJ + coordinates)                   |
| GeoTIFF            | 3 unit tests (`utils/geotiff.rs`: format, crop-no-geo, georeferencing-tags → bounds + crop) |
| Config             | 3 unit tests (`config.rs`, incl. legacy `[business]`/`[gwc]` alias)      |
| Auth               | 8 unit tests (`auth.rs`: salt, hash, verify, JWT)                        |
| Upload             | 1 unit test (`upload_handler.rs`: filename sanitize)                     |
| Store (cache)      | 4 `LocalSessionCache` unit tests (set/get/remove/remove_user/TTL) + 4 `LocalTileCacheBackend` (put/get/clear/stats/gridset-path sanitize) |
| Store (vector/raster) | 4 `LocalVectorStore` (save/load/delete/list/sanitize) + 4 `LocalRasterStore` (put/get/delete/list/.tif) + 8 `sqlite_store` (workspace / namespace / layer+features / user+permission / session / styles CRUD / layer-groups CRUD / audit logs) + 2 live `PostgresStore` (metadata CRUD) / `PostgresVectorStore` (feature round-trip) — `#[ignore]` |
| Data-source adapters | 4 `arcgrid` (read/meta/errors) + 5 `worldimage` (ext/meta/crop) + 5 `cascaded` (config extract + **vendor-param URL encoding** + live `fetch_cascaded_map` via WMS proxy: CQL_FILTER valid/invalid + TIME pass-through) + 8 `geopackage` (layers / validation / features read round-trip / limit + **write→read round-trip** for Point & LineString + **typed attributes: INTEGER/REAL/BOOLEAN/TEXT inference + round-trip**) + 2 `data_source` (serde type round-trip incl. `cascaded_wms`, postgis connection constructor) + 4 `wkb` (Point/LineString/Polygon round-trip + **Multi*/GeometryCollection round-trip** + byte lengths + big-endian decode, all 7 WKB types now parse) |

### 10.2 Coverage gaps (adapted but untested)

| Protocol / surface | Missing tests                                                              |
|--------------------|----------------------------------------------------------------------------|
| **WFS**            | LockFeature (documented as unsupported — GET returns 400); OGC XML `FILTER=` supports the common operators (And/Or/Not, the 6 PropertyIs* comparisons, Like, Null, Between, BBox, FeatureId) but not `ogc:Function` / spatial `Intersects` XML forms (ECQL path covers those via CQL) |
| **WCS**            | native netCDF output (falls back to TIFF); elevation / time subsetting on real rasters (only recorded) |
| **Data sources**   | GeoPackage attributes are typed on write (INTEGER/REAL/BOOLEAN/TEXT by value inference) and typed on read, but no GeoPackage **update** / append; GeoPackage writer still only emits Point / LineString geometry types |
| **WKB**            | decoder now handles all 7 WKB types (2D); EWKB Z/M/SRID flags are masked for routing but Z/M coordinates are not yet parsed; no GeometryCollection-in-GeoPackage round-trip test (GeoPackage write supports Point/LineString only) |

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
- **`DataSourceType::CascadedWms` serde name was `cascadedwms`** — `#[serde(rename_all
  = "lowercase")]` lowercases the whole variant, but the public name used by the
  frontend, `Display`, and backup import is `cascaded_wms`, so creating a cascaded
  WMS data source over REST failed to deserialize. Fixed in `src/models/data_source.rs`
  with `#[serde(rename = "cascaded_wms")]` on the variant.
- **`PostgresVectorStore::delete_features` always returned 0** — it issued
  `DROP TABLE` (DDL reports 0 affected rows), inconsistent with `SqliteStore`'s
  deleted-row count. Fixed in `src/store/vector/postgres.rs` to `DELETE` rows and
  return the count, keeping the table (matches the "delete all features" contract).
- **GeoTIFF georeferencing tags were never read (bug7)** — `try_read_geotiff_tags_native`
  queried the `tiff` decoder with `Tag::Unknown(33922)` / `Tag::Unknown(33550)`, but
  the decoder stores these tags under the named variants `Tag::ModelTiepointTag` /
  `Tag::ModelPixelScaleTag`, so `get_tag` always returned `RequiredTagNotFound` and
  `bounds` stayed `None`. Consequence: WCS SUBSET on a real GeoTIFF silently returned
  the full image. Fixed in `src/utils/geotiff.rs` by using the named tag variants;
  guarded by a new unit test (`utils/geotiff.rs`) that writes a georeferenced TIFF
  fixture and asserts bounds + 2×2 crop.
- **CQL/ECQL `A AND B` parsed as `A OR B` (bug8)** — `split_top_level` built its
  keyword list from a hardcoded mix of `AND`/`OR` variants regardless of which
  operator was being split, so `parse_or_expr` also split on `AND` and combined with
  `Or`, and vice-versa. Any multi-clause CQL with both a LIKE/compare and an `AND`
  silently became an OR, returning too many features. The existing unit test only
  asserted `true` and never caught it. Fixed in `src/utils/cql_filter.rs` by passing
  only the current operator's keyword into `split_top_level`; guarded by a new unit
  test (`test_and_or_precedence_bug8`). Found while testing WFS `FILTER=name LIKE
  'alp%' AND elevation > 100` (returned 2 instead of 1).
- **MVT `.pbf` route is shadowed** — the generic `/tiles/{layer}/{z}/{x}/{y}` route is
  registered before `/tiles/{layer}/{z}/{x}/{y}.pbf`, so the latter never matches
  (`{y}` captures `0.pbf` and returns a PNG tile). The `/mvt/{layer}/{z}/{x}/{y}`
  route works and is what the test uses. Reordering routes would un-shadow `.pbf`.

### 10.4 Recommended next tests (small → large)

Batch 7 completed the previous list: GeoPackage **write** round-trip (done),
WCS SUBSET/SIZE on a real georeferenced GeoTIFF (done, caught bug7), and
PostGIS data source HTTP integration (done, live).

Batch 9 completed: WKB decoder extended to MultiPoint / MultiLineString /
MultiPolygon / GeometryCollection (cursor-based `WkbReader`, all 7 types, big-
endian too), with encoder→decoder round-trip tests for every type.

Batch 10 completed: GeoPackage **typed attributes** (INTEGER / REAL / BOOLEAN /
TEXT inferred from `PropertyValue`, typed on read) + REST publish flow
(`/data-sources/{name}/tables` lists GeoPackage feature tables, `/layers/{layer}/
feature-type` returns typed columns for a published GeoPackage layer).

Batch 11 completed: cascaded WMS **vendor-param pass-through** — the live proxy
now forwards CQL_FILTER / TIME / ELEVATION / ENV / ANGLE / FEATUREID (URL-
encoded) to the upstream; verified live against the reference GeoServer: valid
CQL_FILTER → PNG, invalid CQL_FILTER → upstream OGC exception (non-PNG, proving
pass-through), TIME → PNG. Next candidates:

1. OGC XML `FILTER=` edge cases: `ogc:Function`, spatial `Intersects` XML form
2. WMS PDF / GeoRSS output (reuse the render pipeline)
3. GeoPackage `FeatureType` describe via WFS `DescribeFeatureType` for a
   published GeoPackage layer
4. TMS 1.0.0 + WMS-C 1.1.1 (GWC, reuse the tile engine)

