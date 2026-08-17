# Terrane — Protocol Adaptation Matrix

> Status of OGC / REST / tile protocol adaptation, compared against the reference
> GeoServer instance (`http://127.0.0.1:18080/geoserver/web/`, 9 workspaces / 30 layers).
> Complements [ARCHITECTURE.md](ARCHITECTURE.md) (API contract design),
> [ROADMAP.md](ROADMAP.md) (milestones), and
> [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) (feature-gap analysis).

Per the technical roadmap, Terrane is a **protocol adapter**: it speaks the standard
OGC interfaces (WMS / WFS / WCS / WMTS / REST) while all state lives in external stores.
This document tracks *which protocol surfaces are already adapted*, *which are partial*,
and *which remain to be adapted*.

## 1. Status summary

| Protocol            | Version(s)                     | Status  | Notes |
|---------------------|--------------------------------|---------|-------|
| **WMS**             | 1.1.1 / 1.3.0                  | ✅ Core | GetCapabilities / GetMap / GetFeatureInfo / DescribeLayer / GetLegendGraphic / GetStyles / PutStyles; multi-format + vendor params |
| **WFS**             | 1.0.0 / 1.1.0 / 2.0.0          | ✅ Core | GetCapabilities / DescribeFeatureType / GetFeature / GetFeatureWithLock / LockFeature (WFS-T planned for later) |
| **WCS**             | 1.0.0 / 1.1.x / 2.0.1          | ✅ Core | GetCapabilities / DescribeCoverage / GetCoverage; WCS 2.0 subsetting |
| **WMTS**            | 1.0.0                          | ✅ Core | GetCapabilities / GetTile / GetFeatureInfo; KVP + RESTful tile template |
| **MVT (vector tiles)** | —                          | ✅      | Pure-Rust protobuf encoder; `/tiles/{layer}/{z}/{x}/{y}.pbf`, `/mvt/{layer}/{z}/{x}/{y}` |
| **REST API**        | —                              | ✅      | Full CRUD under `/geoserver` (see §6) |
| **TMS**             | 1.0.0                          | ✅      | GetCapabilities / TileMap / GetTile under `/gwc/service/tms` (global-geodetic + global-mercator, PNG/JPEG) |
| **WMS-C**           | 1.1.1                          | ✅      | GetCapabilities / GetMap with `TILED=true` under `/gwc/service/wms` |
| **Tile cache (GWC-like)** | —                         | ⚠️      | Basic `/tiles` + local disk cache; no seeding / metastore / full GWC |
| **SLD styling**     | 1.0.0                          | ⚠️      | Basic CRUD + rendering; no CSS / YSLD / MBStyle, limited SLD features |
| **WMS output formats** | —                          | ✅      | PNG/JPEG/GIF/WebP/SVG/KML/GeoJSON/PDF/GeoRSS |
| **WFS output formats** | —                          | ✅      | GML 2.1.2 / GML 3.1.1 / GML 3.2 / GeoJSON / CSV / KML / Shapefile (SHAPE-ZIP) |
| **WPS**             | 1.0.0                          | ✅      | GetCapabilities / DescribeProcess / Execute (KVP + XML POST); built-in processes vec:Centroid / vec:Buffer / gs:Bounds |
| **CSW**             | 2.0.2                          | ✅      | GetCapabilities / DescribeRecord / GetRecords / GetRecordById / GetDomain; catalog = Terrane layers as Dublin Core records (KVP + XML POST) |
| **OGC API series**  | Features (Core) / Tiles / Maps / Coverages / Processes / Styles | ⚠️ | Features Core ✅ at `/ogc/features`; Tiles ✅ at `/ogc/tiles` (tileMatrixSets + raster tiles, PNG/JPEG); **Maps ✅ at `/ogc/maps`** (map operation reuses the WMS GetMap pipeline); **Processes ✅ at `/ogc/processes`** (synchronous jobs over the WPS engine); **Coverages ✅ at `/ogc/coverages`** (coverage collections = raster data sources, `coverage` operation reuses the WCS GetCoverage pipeline, GeoTIFF/PNG/JPEG + bbox crop); **Styles ✅ at `/ogc/styles`** (style list/create/get/replace/delete + metadata + layer collection linkage, SLD/CSS/YSLD/MBStyle native media types) |

## 2. WMS — Web Map Service

Endpoint `/wms` (`src/services/wms.rs`, `src/handlers/wms_handler.rs`).

**Versions**: capabilities advertise **1.3.0**; **1.1.1** requests are handled
(including the 1.3.0 lat/lon axis-order rule for geographic CRS).

**Operations**:

| Operation           | Status | Notes |
|---------------------|--------|-------|
| GetCapabilities     | ✅     | Layers from the catalog |
| GetMap              | ✅     | Raster (PNG/JPEG/GIF/WebP), vector (SVG), KML, GeoJSON, GeoRSS, PDF, OpenLayers preview; CascadedWms proxy |
| GetFeatureInfo      | ✅     | `text/plain`, `text/html`, `application/json`, **`application/vnd.ogc.gml`** (GML 3.1.1 feature collection) |
| DescribeLayer       | ✅     | WMS 1.1.1 DescribeLayerResponse |
| GetLegendGraphic    | ✅     | SLD-based legend |
| GetStyles / PutStyles | ✅   | SLD style read/write |

**Vendor parameters**: `TRANSPARENT`, `CQL_FILTER` (ECQL, multi-layer `;`-separated),
`TIME` (ISO 8601), `ELEVATION`, `SRS/CRS`, `ENV` (style env substitution),
`featureId` (feature-id filter), `angle`, scale-denominator-aware styling.

**Gaps vs reference**: full SLD dynamic-styling feature
set (rendering transforms out of scope).

## 3. WFS — Web Feature Service

Endpoint `/wfs` (`src/services/wfs.rs`, `src/handlers/wfs_handler.rs`). GET + POST.

**Versions**: 1.0.0 / 1.1.0 / 2.0.0.

**Operations**:

| Operation           | Status | Notes |
|---------------------|--------|-------|
| GetCapabilities     | ✅     | Advertises 2.0.0 (incl. GetFeatureWithLock / LockFeature / GetPropertyValue / GetGmlObject) |
| DescribeFeatureType | ✅     | XSD schema (real typed columns for published GeoPackage layers) |
| GetFeature          | ✅     | GML 2.1.2 / GML 3.1.1 / GML 3.2, GeoJSON, CSV, KML 2.2, Shapefile (SHAPE-ZIP) |
| GetFeatureWithLock  | ✅     | Real locking (in-memory lock registry, TTL expiry, `lockAction=SOME` skips locked features), response carries `lockId` |
| LockFeature         | ✅     | Acquire / renew (LOCKID+EXPIRY) / release (LOCKID+RELEASEACTION=ALL), `lockAction` ALL/SOME; `wfs:LockFeatureResponse` (1.1/2.0 namespaces) |
| GetPropertyValue    | ✅     | WFS 2.0 core operation (OGC 09-025r2): `PROPERTYNAME` selection → `wfs:ValueCollection` (geometry properties as GML) |
| GetGmlObject        | ✅     | WFS 2.0 core operation (OGC 09-025r2): lookup by `GMLOBJECTID` → `wfs:GMLObjectCollection` (GML 3.2) |
| Transaction         | ⏳     | WFS-T writes not implemented yet (currently 501); planned for a later milestone |

**Gaps vs reference**: deeper GML 3.2 schema fidelity.

## 4. WCS — Web Coverage Service

Endpoint `/wcs` (`src/services/wcs.rs`, `src/handlers/wcs_handler.rs`).

**Versions**: 1.0.0 / 1.1.x / **2.0.1** (capabilities advertise 2.0.1).

**Operations**:

| Operation           | Status | Notes |
|---------------------|--------|-------|
| GetCapabilities     | ✅     | |
| DescribeCoverage    | ✅     | |
| GetCoverage         | ✅     | GeoTIFF (`image/tiff`), ArcGrid (`application/x-arcgrid`); spatial subset + **range band subset** (`SUBSET=Band(a:b)`, r/g/b/a axes) + **`INTERPOLATION`** (nearest / bilinear / cubic / lanczos) |

**Subsetting**: WCS 2.0 `SUBSET` (intervals / position, with CRS + resolution,
comma `x(10,20)` or colon `Band(1:2)` syntax), `SUBSETTINGCRS`.

**Gaps vs reference**: no full WCS 1.1 range-subset / interpolation *semantics*
(band subset and interpolation are implemented on 2.0 GetCoverage, but the
1.1 semantics are not fully modelled), no
Web Coverage Transaction (WCS-T, deferred).

## 5. Tiling — WMTS / MVT / TMS / WMS-C

### 5.1 WMTS 1.0.0 ✅
Endpoint `/wmts` (`src/services/wmts.rs`, `src/handlers/wmts_handler.rs`).

- **Operations**: GetCapabilities, GetTile, GetFeatureInfo
- **Access styles**: KVP (`?SERVICE=WMTS&REQUEST=GetTile`) and RESTful
  `/wmts/{layer}/{tileMatrixSet}/{tileMatrix}/{tileCol}/{tileRow}`

### 5.2 MVT vector tiles ✅
- `/tiles/{layer}/{z}/{x}/{y}.pbf`, `/mvt/{layer}/{z}/{x}/{y}` — pure-Rust protobuf
  encoder (`src/utils/mvt.rs`, `src/handlers/mvt_handler.rs`)

### 5.3 Tile cache (GWC-like) ✅

- `/tiles/{layer}/{z}/{x}/{y}` with a local disk cache (`<data_dir>/gwc`,
  `TileCacheBackend` trait, `src/store/cache/`) + layer-level Redis data-source
  backends (`Layer.cache_store` → `DataSourceType::Redis`)
- **Seeding / truncate** (`src/utils/tile_seed.rs` + `/tiles/seed` REST): background
  seed jobs over a zoom range with progress + cooperative cancel; truncate clears
  a layer's cached tiles
- **Metastore**: per-`(layer, gridset)` JSON tile registry (`meta_dir`) — fast
  stats, seed resume, quota bookkeeping
- **Disk quota**: `[cache] layer_quota_bytes` with mtime-LRU eviction per layer
- **Conditional requests**: tile responses carry `ETag` (content hash) +
  `Last-Modified`; `If-None-Match` / `If-Modified-Since` → `304`
- **Custom gridsets**: runtime registration (per-zoom resolutions), honoured by
  the shared grid math (`src/utils/tile_grid.rs`)
- No GWC metastore/disk-quota *fidelity* (e.g. per-gridset quotas, blob stores) — 
  see ROADMAP known debt

### 5.4 TMS 1.0.0 ✅ & WMS-C 1.1.1 ✅

Both are served under `/gwc/` (mirroring the reference GeoServer GeoWebCache
paths) and reuse the shared tile engine (`src/handlers/tile_common.rs`) and grid
math (`src/utils/tile_grid.rs`):
- **TMS 1.0.0** — `/gwc/service/tms` (KVP) + `/gwc/service/tms/1.0.0[/…]`
  (RESTful): GetCapabilities (`TileMapService`), per-layer `TileMap` documents,
  and tiles at `/1.0.0/{layer}@{gridset}@{format}/{z}/{x}/{y}.{ext}`. TMS rows
  are bottom-up, so `y` is flipped before rendering. Gridsets: `EPSG:4326`
  (global-geodetic, 2^(z+1)×2^z) and `EPSG:3857`/`EPSG:900913`
  (global-mercator). Formats: PNG + JPEG.
- **WMS-C 1.1.1** — `/gwc/service/wms`: GetCapabilities (`WMT_MS_Capabilities`
  version 1.1.1) and GetMap with `TILED=true`, which resolves the grid-aligned
  BBOX to a single cached tile (gridset derived from SRS, zoom from the
  horizontal resolution). Without `TILED`, it delegates to the normal WMS 1.1.1
  GetMap pipeline.

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

## 7. WPS — Web Processing Service

Endpoint `/wps` (`src/services/wps.rs`, `src/handlers/wps_handler.rs`). First WPS
surface (the reference GeoServer has WPS disabled, so this follows the OGC WPS
1.0.0 schema, OGC 05-007r7).

- **GetCapabilities** — WPS 1.0.0 `wps:Capabilities` with ServiceIdentification /
  OperationsMetadata / `wps:ProcessOfferings` / Languages.
- **DescribeProcess** — `wps:ProcessDescriptions` with DataInputs / ProcessOutputs
  (ComplexData `application/json` + `xsd:double` literals).
- **Execute** — KVP (`response=raw` → raw GeoJSON, else ExecuteResponse XML) and
  POST XML (`<wps:DataInputs>` with LiteralData / ComplexData / Reference, where
  `xlink:href="layer:name"` is a Terrane extension resolving a local layer).
- **Built-in processes** (pure Rust): `vec:Centroid` (geo `Centroid`),
  `vec:Buffer` (point-buffer: a circle around every coordinate), `gs:Bounds`
  (bounding-box rectangle polygon). Outputs are proper GeoJSON FeatureCollections.

## 8. Data sources

PostGIS, **MySQL** (spatial DB connector: MBRIntersects filtering + ST_AsGeoJSON output,
pooled connections), **MongoDB** (GeoJSON document connector: `$geoWithin` bbox filtering,
`ping` connection test, pooled client), Shapefile, GeoTIFF, GeoPackage, WorldImage,
CascadedWms, ArcGrid, Redis (cache backend), GeoJson, **ImageMosaic** (raster-directory
mosaic: GeoTIFF / WorldImage / ArcGrid / PNG / JPEG granules composited into one coverage;
WCS GetCoverage + WMS GetMap + tile pipelines), **ImagePyramid** (pyramid: numeric level
subdirs `0/1/2/…`, level chosen by requested resolution; same WCS / WMS / tile surfaces).

### 8.1 WMS / WMS-C / WMTS rendering engine

- **Vector rendering**: Point / LineString / Polygon **and** MultiPoint / MultiLineString /
  MultiPolygon / GeometryCollection; `fill-opacity` / `stroke-opacity` alpha compositing
  (source-over), polygon interior-ring holes (even-odd scanline), z-order (polygon → line →
  point within a layer, SLD `z-index` vendor option on top), label rendering from
  `TextSymbolizer` (built-in 5×7 bitmap font, halo + greedy collision avoidance).
- **Compositing / blend modes**: SLD `VendorOption name="composite"` / CSS `composite`
  (multiply / screen / overlay / darken / lighten); non-default modes draw the feature
  onto an offscreen layer first, then composite with the mode (SVG/PDF layer semantics).
- **SVG output** honors per-feature SLD/CSS styles (fill/stroke colors + opacity + width +
  dash array, point markers, `fill-rule="evenodd"` holes, labels with halo as text stroke).
- **KML output** honors per-feature styles: deduplicated `<Style>` definitions referenced
  by `styleUrl`, KML `aabbggrr` colors, label text as Placemark name + `<LabelStyle>`;
  Multi-geometry support (MultiPoint → first point, MultiLineString → joined coords,
  MultiPolygon / GeometryCollection → `<MultiGeometry>`).
- **Raster rendering**: GeoTIFF / WorldImage / ArcGrid / ImageMosaic layers render in
  WMS GetMap and in the shared tile pipeline (crop to BBOX + resample + source-over).
- **Style selection**: WMS GetMap `STYLES` parameter selects the per-layer style
  (comma-separated, empty entry = layer default), matching GeoServer semantics.
- **Map rotation**: WMS GetMap `ANGLE` vendor parameter rotates the vector geometry
  around the request BBOX center (labels stay horizontal, matching GeoServer);
  passed through to cascaded WMS upstreams; the OpenLayers preview passes `ANGLE`
  through and sets the view `rotation` so the preview matches GetMap.
- **GetLegendGraphic**: per-rule color swatches with point-marker icons, optional
  `SCALE` scale-denominator rule filtering, `WIDTH` control, and rule-name labels
  rendered with the built-in bitmap font.
- **Output safety**: WMS GetFeatureInfo `text/html` escapes layer/feature-id/property
  values (XSS); WFS CSV output quotes+escapes headers and values (CSV injection-safe).
- **Style formats**: SLD / CSS / YSLD / MBStyle all dispatch to the same rule pipeline in
  WMS GetMap (previously only SLD reached the WMS renderer).
- **SLD parsing**: TextSymbolizer (`Label` property/literal, `Font` size, `Fill`, `Halo`
  radius/color), `ogc:Filter` comparisons + `And`/`Or`/`Not` nesting, `VendorOption
  name="z-index"` / `name="composite"`, Min/MaxScaleDenominator.

## 9. Pending protocols (to adapt)

Prioritized by [ROADMAP.md](ROADMAP.md) and [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md):

| Priority | Protocol / surface                  | Effort |
|----------|-------------------------------------|--------|
| P2       | TMS 1.0.0 + WMS-C 1.1.1 (GWC)       | small — reuse tile engine |
| P2       | WMS PDF / GeoRSS output             | small — reuse render pipeline |
| P2       | WFS KML / Shapefile output          | ✅ done |
| P4       | WPS (processing)                    | ✅ first surface (Centroid/Buffer/Bounds) |
| P4       | CSW (catalog)                       | ✅ first surface (GetCapabilities/DescribeRecord/GetRecords/GetRecordById/GetDomain) |
| P4       | OGC API Features / Tiles / Maps / Coverages / Processes / Styles | ✅ Features Core + Tiles (batch 19/20) + Maps + Processes (batch 21) + Coverages + Styles (batch 22) |
| P4       | Printing / Importer / GeoFence      | enterprise |
| P4       | CSS / YSLD / MBStyle styling        | medium |

## 10. Verification checklist

Hand-verified smoke path against the reference console (`http://127.0.0.1:18080/geoserver/web/`):

- [x] `GET /wms?SERVICE=WMS&VERSION=1.3.0&REQUEST=GetCapabilities`
- [x] `GET /wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetCapabilities`
- [x] `GET /wfs?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetCapabilities`
- [x] `GET /wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetCapabilities`
- [x] `GET /wfs?SERVICE=WFS&VERSION=1.0.0&REQUEST=GetCapabilities`
- [x] `GET /wcs?SERVICE=WCS&VERSION=2.0.1&REQUEST=GetCapabilities`
- [x] `GET /wcs?SERVICE=WCS&VERSION=1.1.1&REQUEST=GetCapabilities`
- [x] `GET /wmts?SERVICE=WMTS&VERSION=1.0.0&REQUEST=GetCapabilities`
- [x] `GET /gwc/service/tms/1.0.0`  (TMS — adapted)
- [x] `GET /gwc/service/wms?...&tiled=true` (WMS-C — adapted)
- [x] `GET /wps?SERVICE=WPS&REQUEST=GetCapabilities` (WPS — adapted)
- [x] `GET /wps?SERVICE=WPS&REQUEST=Execute&IDENTIFIER=vec:Centroid&...` (WPS Execute)

## 11. Automated test coverage

`cargo test` currently green: **289 lib unit tests + 200 integration tests**, plus
**7 `#[ignore]`-marked live tests** (PostGIS metadata/vector + PostGIS data-source
HTTP + 3× S3 + 2× cascaded-WMS live) that require
running services and are verified with `cargo test -- --ignored`.

The integration tests are split by protocol into separate test crates (Rust
convention: every file directly under `tests/` is its own binary), sharing a
`tests/common/mod.rs` helper (`create_test_config` + `build_test_app!` macro):

| Test crate          | Tests | Scope                                                     |
|---------------------|-------|-----------------------------------------------------------|
| `tests/wms_test.rs` | 30 (+2 ignored live) | WMS (all operations, formats incl. GeoRSS/PDF, **GetFeatureInfo GML output**, vendor params incl. ANGLE → OpenLayers preview with **local assets only, no CDN** (regression: jsdelivr blanked the iframe on restricted networks); + cascaded WMS live proxy incl. CQL_FILTER / TIME vendor-param pass-through) + WMS-C (GetCapabilities, GetMap `TILED=true` geodetic/mercator, plain GetMap) |
| `tests/wfs_test.rs` | 15    | WFS (all operations + **LockFeature acquire/conflict/renew/release** + **GetFeatureWithLock lockId** + **GetPropertyValue** + **GetGmlObject** + WFS-T 501 (not implemented, planned) contract + FILTER= OGC XML / ECQL + CQL_FILTER + XML `ogc:Function` (strToLowerCase) + spatial `Intersects` + GeoPackage DescribeFeatureType typed columns + KML 2.2 + Shapefile SHAPE-ZIP) |
| `tests/rest_test.rs`| 49 (+1 ignored live) | health / probes / metrics, REST CRUD (layers / workspaces / namespaces / **stores full CRUD** / **layer-groups PUT** / styles / sql-views / **users PUT**), **workspace-dimension endpoints** (`/workspaces/{ws}/layers|datastores|coveragestores`), **service settings** (`/services/wms/settings` + GetCapabilities title), **`/about/version` + `/about/system-status`**, **`/resources`** (list/upload/delete + path-traversal protection), **feature-type PUT** (GeoPackage columns), **tile seed/truncate REST** (`/tiles/seed` create/list/progress/cancel + truncate), **tile conditional requests** (ETag/Last-Modified → 304), **GeoFence ACL** (deny anonymous WMS/WFS → forbidden, admin bypass, allow rule for role user), **S3 upload validation** (`storage=s3` without bucket → 400), **production-stack smoke test** (full middleware + routes + static files serves index/API/OGC, TraceId echoed), MVT (incl. `.pbf` route), auth, backup, `/tiles` + tile cache, **GeoPackage data source over REST + `/layers/{layer}/feature-type` typed columns**; + PostGIS data source HTTP (live) |
| `tests/wcs_test.rs` | 14    | WCS (DescribeCoverage / GetCoverage incl. real GeoTIFF / ArcGrid + SUBSET / SIZE, **range band subset Band(a:b)**, **INTERPOLATION** bilinear/nearest/unknown fallback, JPEG / netCDF) |
| `tests/wmts_test.rs`| 4     | WMTS (GetCapabilities / GetTile / GetFeatureInfo)         |
| `tests/tms_test.rs` | 7     | TMS (GetCapabilities RESTful + KVP, TileMap document, GetTile geodetic/mercator PNG + JPEG, KVP GetTile) |
| `tests/wps_test.rs` | 6     | WPS (GetCapabilities, DescribeProcess, Execute KVP raw centroid/buffer/bounds + XML POST) |
| `tests/csw_test.rs` | 10    | CSW (GetCapabilities, DescribeRecord, GetRecords KVP + XML POST summary/brief/full + CQL constraint + paging/hits, GetRecordById, GetDomain) |
| `tests/ogc_api_test.rs` | 8 | OGC API Features (landing, conformance, collections, collection, items with limit/offset/bbox, item by id, 404) |
| `tests/ogc_tiles_test.rs` | 7 | OGC API Tiles (landing, conformance, tileMatrixSets + definition, collections/tilesets, tile PNG/JPEG, 404) |
| `tests/ogc_maps_test.rs` | 8 | OGC API Maps (landing, conformance, collections, collection, styles, map PNG/JPEG + size, 400/404) |
| `tests/ogc_processes_test.rs` | 9 | OGC API Processes (landing, conformance, process list, description, execute centroid/buffer, jobs list, results, cancel 409, 400/404) |
| `tests/ogc_coverages_test.rs` | 9 | OGC API Coverages (landing, conformance, collections empty/with real GeoTIFF, collection detail with real 8x8 metadata + bounds, coverage GeoTIFF/PNG/JPEG, bbox crop 2x2 + resize 8x8, disjoint bbox 400, 404) |
| `tests/ogc_styles_test.rs` | 7 | OGC API Styles (landing, conformance, style list + SLD content + 404, metadata, create 401/201, get/put(CSS content-type)/delete lifecycle, collections + layer styles) |
| `tests/resilience_test.rs` | 6 | Resilience middleware (rate limit 429, per-client X-Forwarded-For buckets, request timeout 504, fast requests pass) + **TraceId echo regression** (X-Trace-Id passthrough + generated) + **full middleware stack** (TraceId + RateLimit + RequestTimeout + Compress serves without panic) |

Coverage is **protocol-surface level** — each adapted OGC operation / REST group
has at least one request/response test validating status codes, content types,
and basic payload structure. It still does **not** validate full protocol
semantics (XML schema conformance of capabilities, GML 3.2 fidelity, etc.).
The test config uses in-memory SQLite, disables the tile cache, and reuses the
metadata store for vectors, so tests write nothing to `./data`. The live tests
(`#[ignore]`) talk to a local PostGIS (env `GEOSERVER_TEST_PG_*`, defaults
matching the dev compose `build/docker-compose.yml`: 127.0.0.1:5433
terrane/`terrane`) and the reference GeoServer at :18080.

### 11.1 What IS covered

| Layer              | Coverage                                                                 |
|--------------------|--------------------------------------------------------------------------|
| WMS                | GetCapabilities, GetMap (PNG / JPEG / GIF / SVG / KML / GeoJSON, 1.1.1 + 1.3.0 axis-order), GetFeatureInfo (JSON / text/html / text/plain / **GML application/vnd.ogc.gml**), DescribeLayer, GetLegendGraphic, GetStyles, vendor params CQL_FILTER / TIME / ELEVATION / ENV / ANGLE / FEATUREID — integration |
| WFS                | GetCapabilities, DescribeFeatureType (real typed columns for published GeoPackage layers), GetFeature (GeoJSON / GML 2.1.2 / GML 3.1.1 / GML 3.2 / CSV / KML 2.2 / Shapefile SHAPE-ZIP), GetFeatureWithLock (real locking + `lockId` response), LockFeature (acquire / lockAction ALL/SOME conflict / renew / RELEASEACTION release), GetPropertyValue (`wfs:ValueCollection` + FEATUREID filter + missing PROPERTYNAME → 400), GetGmlObject (`wfs:GMLObjectCollection` + unknown id → empty collection + missing GMLOBJECTID → 400), Transaction (501 not-implemented contract, planned for later), URL `FILTER=` (OGC XML PropertyIsEqualTo / PropertyIsGreaterThan + ECQL `name='x'` / `bbox(...)` / `LIKE`+`AND`) and `CQL_FILTER` (ECQL) — integration |
| WPS                | GetCapabilities (ServiceIdentification / OperationsMetadata / ProcessOfferings / Languages), DescribeProcess (DataInputs / ProcessOutputs + xsd:double), Execute (KVP `response=raw` → GeoJSON + document XML, POST XML with `Reference xlink:href="layer:…"`), built-in `vec:Centroid` / `vec:Buffer` / `gs:Bounds` — integration + 6 unit tests (`services/wps.rs`: KVP DataInputs, operation parse, Execute XML, capabilities structure, features_to_geojson, run_process) |
| OGC API Maps       | Landing / conformance / collections / collection / styles, `map` operation (`bbox` + `width`/`height`, PNG/JPEG via `?f=`, `transparent` / `bgcolor` / `datetime` / `cql_filter` pass-through) reusing the shared WMS GetMap pipeline — integration + 8 unit tests (`services/ogc_maps.rs`: landing, conformance, collections, collection links, styles, bbox parse, map href formats) |
| OGC API Processes  | Landing / conformance / process list / process description; synchronous job surface (`POST /jobs` → 201 status document, `GET /jobs`, `GET /jobs/{id}`, `GET /jobs/{id}/results`, `DELETE /jobs/{id}`) over the WPS engine, inputs as `layer:` reference / inline GeoJSON / OGC API href / literals — integration + 9 unit tests (`services/ogc_processes.rs`: landing, conformance, process list/description, job status/results, job request parse) |
| OGC API Coverages | Landing / conformance / collections (empty + with a real 8x8 georeferenced GeoTIFF data source) / collection detail (real bounds + grid scale + band fields) / `coverage` operation (GeoTIFF default, PNG + JPEG via `?f=`, bbox crop 2×2 + width/height resample 8×8, disjoint bbox 400, unknown collection 404) — integration + 7 unit tests (`services/ogc_coverages.rs`: landing, conformance, collections, collection links/dimensions, bbox parse, coverage hrefs) |
| OGC API Styles     | Landing / conformance / style list (built-in `default` SLD) + style content with native media type + 404 / metadata / create (401 without token, 201 with JWT) / get–put (CSS content-type after replace)–delete lifecycle / collections + layer styles — integration + 8 unit tests (`services/ogc_styles.rs`: mime mapping, landing, conformance, list, metadata, collections, layer-style resolution, hrefs) |
| Resilience (middleware) | Rate limit (429 over limit, per-client `X-Forwarded-For` buckets) + request timeout (504 over a real HTTP server, fast requests pass) + **TraceId echo** (X-Trace-Id passthrough / generated) + **full production middleware stack** (TraceId + RateLimit + RequestTimeout + Compress serves without panic — regression for the `HeaderName::from_static` uppercase panic) — integration + 5 unit tests (`middleware.rs`: allow/deny, disabled limiter, per-client independence, window slide, count after prune) |
| WCS                | GetCapabilities, DescribeCoverage (incl. real GeoTIFF / ArcGrid / WorldImage metadata enrichment), GetCoverage (TIFF / PNG / JPEG / default-format + netCDF→TIFF fallback, real GeoTIFF 8×8 bytes, real ArcGrid 4×3 bytes, SUBSET/SIZE on real ArcGrid AND real georeferenced GeoTIFF: crop→2×2 + resize→8×8, **range band subset `Band(a:b)` constant/gradient output assertions**, **INTERPOLATION=bilinear/nearest/unknown fallback**) — integration |
| WMTS               | GetCapabilities, GetTile (KVP + RESTful template), GetFeatureInfo — integration |
| TMS                | GetCapabilities (RESTful + KVP), TileMap document (SRS / BoundingBox / Origin / TileFormat / TileSets + units-per-pixel), GetTile (global-geodetic + global-mercator, PNG + JPEG, TMS bottom-up y flip) — integration |
| WMS-C              | GetCapabilities (`WMT_MS_Capabilities` 1.1.1), GetMap `TILED=true` (geodetic + mercator grid-aligned BBOX → cached tile), GetMap without TILED (plain WMS 1.1.1) — integration |
| Tile grid (util)   | 5 unit tests (`utils/tile_grid.rs`: geodetic matrix/bounds, mercator matrix/bounds, TMS y-flip, zoom-for-resolution, bbox→tile) |
| MVT                | `/mvt/{layer}/{z}/{x}/{y}` endpoint integration + 5 encoder unit tests    |
| REST               | health, split probes `/health/live` + `/health/ready` + `/metrics`, `/server/status`, layers, workspaces CRUD, namespaces CRUD, styles CRUD, layer-groups CRUD, features CRUD (+ single get / update / delete), sql-views CRUD, data-sources CRUD, auth (login / verify / users CRUD), permissions CRUD, backup export + import round-trip, GeoJSON upload, `/tiles` PNG tile, tile cache stats / clear / HIT, live PostGIS data source HTTP (`/data-sources/{name}/tables` + `/layers/{layer}/feature-type` against a real schema) — integration |
| Tile cache (util)  | 15 unit tests (`utils/tile_cache.rs` + `store/cache/tile.rs`: gridset, bounds, hit-rate, **metastore write + fast-path stats**, **layer quota LRU eviction**, put/get/clear) + 2 `tile_seed` (serde, status) + 7 `tile_grid` (geodetic/mercator + **custom gridset matrix/bounds/zoom**) |
| CQL / ECQL         | 8 unit tests (`utils/cql_filter.rs`) — incl. bug8 regression: `A AND B` is a real AND, not OR |
| Projection         | 7 unit tests (`utils/projection.rs`)                                     |
| Geometry           | 3 unit tests (`utils/geometry.rs`)                                       |
| Shapefile          | 3 unit tests (`utils/shapefile.rs`: PRJ + coordinates)                   |
| Shapefile export   | 4 unit tests (`utils/shapefile_export.rs`: .shp/.shx structure, .dbf fields/values, .prj, ZIP package, **round-trip through the reader**, polyline/polygon shape types) |
| GeoTIFF            | 3 unit tests (`utils/geotiff.rs`: format, crop-no-geo, georeferencing-tags → bounds + crop) |
| Config             | 3 unit tests (`config.rs`, incl. legacy `[business]`/`[gwc]` alias)      |
| Auth               | 8 unit tests (`auth.rs`: salt, hash, verify, JWT)                        |
| Upload             | 1 unit test (`upload_handler.rs`: filename sanitize)                     |
| Store (cache)      | 4 `LocalSessionCache` unit tests (set/get/remove/remove_user/TTL) + 4 `LocalTileCacheBackend` (put/get/clear/stats/gridset-path sanitize) |
| Store (vector/raster) | 4 `LocalVectorStore` (save/load/delete/list/sanitize) + 4 `LocalRasterStore` (put/get/delete/list/.tif) + 8 `sqlite_store` (workspace / namespace / layer+features / user+permission / session / styles CRUD / layer-groups CRUD / audit logs) + 2 live `PostgresStore` (metadata CRUD) / `PostgresVectorStore` (feature round-trip) — `#[ignore]` |
| Data-source adapters | 4 `arcgrid` (read/meta/errors) + 5 `worldimage` (ext/meta/crop) + 5 `cascaded` (config extract + **vendor-param URL encoding** + live `fetch_cascaded_map` via WMS proxy: CQL_FILTER valid/invalid + TIME pass-through) + 8 `geopackage` (layers / validation / features read round-trip / limit + **write→read round-trip** for Point & LineString + **typed attributes: INTEGER/REAL/BOOLEAN/TEXT inference + round-trip** + **stable feature ids `table.rowid`**) + 2 `data_source` (serde type round-trip incl. `cascaded_wms`, postgis connection constructor) + 4 `wkb` (Point/LineString/Polygon round-trip + **Multi*/GeometryCollection round-trip** + byte lengths + big-endian decode, all 7 WKB types now parse) + 7 `wfs_lock` (acquire / conflict ALL / SOME skip / expiry / never-expire / renew+release / per-layer) + 3 `gml` (escape / point / feature GML 3.2) |
| Security           | 12 `geofence` (no-rules allow / anonymous deny / allow rule / workspace + store scopes / qualified layer name / specific-beats-wildcard / deny-wins-tie / priority tiebreak / mode hierarchy / service-global) + 7 `ldap` (filter substitution / DN escaping / DN building / role mapping / parse-role / disabled config) + 6 `secrets` (no-placeholder / env expand / missing env / unterminated / connection resolve / redact) + 3 `config` (ldap defaults / ldap parse / geofence parse) |

### 11.2 Coverage gaps (adapted but untested)

| Protocol / surface | Missing tests                                                              |
|--------------------|----------------------------------------------------------------------------|
| **WFS**            | OGC XML `FILTER=` also handles `ogc:Function` (e.g. `strToLowerCase` → case-insensitive equality) and spatial `Intersects` / `Within` / `DWithin` with GML Point / Polygon / Envelope (delegated to the CQL engine) — a superset of the reference KVP `FILTER=`, which rejects `ogc:`-prefixed tags and GML geometry. LockFeature / GetPropertyValue / GetGmlObject are now implemented (see §3) — remaining gap is deeper GML 3.2 schema fidelity |
| **WCS**            | native netCDF output (falls back to TIFF); elevation / time subsetting on real rasters (only recorded) |
| **Data sources**   | GeoPackage attributes are typed on write (INTEGER/REAL/BOOLEAN/TEXT by value inference) and typed on read, but no GeoPackage **update** / append; GeoPackage writer still only emits Point / LineString geometry types |
| **WKB**            | decoder now handles all 7 WKB types (2D); EWKB Z/M/SRID flags are masked for routing but Z/M coordinates are not yet parsed; no GeometryCollection-in-GeoPackage round-trip test (GeoPackage write supports Point/LineString only) |

### 11.3 Bugs found by the new tests

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

### 11.4 Recommended next tests (small → large)

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
pass-through), TIME → PNG.

Batch 12 completed: OGC XML `FILTER=` edge cases — `ogc:Function` (e.g.
`strToLowerCase` maps to case-insensitive equality via a new `CaseInsensitiveEq`
CQL expression) and spatial `Intersects` / `Within` / `DWithin` with GML Point /
Polygon / Envelope (GML→WKT, delegated to the CQL engine). Note: the reference
GeoServer's KVP `FILTER=` parser rejects `ogc:`-prefixed tags and GML geometry,
so this is a Terrane superset.

Batch 13 completed: WMS **PDF / GeoRSS output** — `render_to_georss` emits RSS 2.0
with the GeoRSS namespace (`<georss:point>` / `<georss:line>` / `<georss:polygon>`
in `lat lon` order; multi-geometries fall back to the first member) and
`render_to_pdf` embeds the `MapRenderer` raster in a single-page PDF (FlateDecode-
compressed RGB image, hand-written xref/trailer). Verified against the reference
GeoServer: `FORMAT=application/rss+xml` → `application/rss+xml`, `FORMAT=application/
pdf` → `%PDF-1.5`. Both formats are now advertised in GetMap capabilities.

Batch 14 completed: GeoPackage `FeatureType` via WFS **`DescribeFeatureType`** —
`handle_describe_feature_type` now resolves the layer's store and, for GeoPackage
data sources, reports the real typed columns (`PRAGMA table_info` via a new
`geopackage_table_columns` helper in `src/utils/geopackage.rs`) instead of the
hardcoded id/name/geometry stub. The SQLite→XSD mapping (`sqlite_type_to_xsd` in
`src/services/wfs.rs`) follows the reference GeoServer: INTEGER→`xsd:long`, REAL→
`xsd:double`, BOOLEAN→`xsd:boolean`, geometry→`gml:GeometryPropertyType`, BLOB→
`xsd:base64Binary`; the internal `id` primary key is skipped (fid is not a regular
attribute). Verified against the reference GeoServer `DescribeFeatureType` on
`sf:archsites` (`cat`→xsd:long, `str1`→xsd:string, `the_geom`→gml:PointPropertyType).
Batch 15 completed: **TMS 1.0.0 + WMS-C 1.1.1 (GeoWebCache)** — the tile
surface now shares one grid helper (`src/utils/tile_grid.rs`: EPSG:4326
global-geodetic 2^(z+1)×2^z with 0.703125/2^z deg/px, EPSG:3857/900913
global-mercator with 156543.03/2^z m/px) and one render pipeline
(`src/handlers/tile_common.rs`, PNG + JPEG, cache on PNG only), which the WMTS
handler was refactored onto (fixing its EPSG:4326 grid to match the advertised
matrix). TMS is served at `/gwc/service/tms` (+ RESTful `/1.0.0`): GetCapabilities
(`TileMapService`), per-layer `TileMap` docs, and bottom-up-y tiles. WMS-C at
`/gwc/service/wms`: 1.1.1 GetCapabilities + GetMap `TILED=true` (snaps the BBOX
to a cached tile; verified against the reference, which rejects off-grid BBOXes
over a 10% threshold). New unit tests: 5 (tile_grid) + 5 (tms) + 2 (wmsc); new
integration tests: 7 (tms_test) + 4 (WMS-C in wms_test).
Batch 16 completed: **WFS KML / Shapefile output** — `GetFeature` now emits
**KML 2.2** (`application/vnd.google-earth.kml+xml`, aliases `KML` and the
GeoServer `application/vnd.google-earth.kml xml` quirk): a `<Document>` with a
`<Schema>` of the layer attributes and one `<Placemark>` per feature
(`<ExtendedData>/<SchemaData>/<SimpleData>` + `lon,lat` geometry, LineString /
Polygon / Multi* supported). **Shapefile** (`SHAPE-ZIP`, `application/zip`): a
new hand-written in-memory exporter `src/utils/shapefile_export.rs` produces
`.shp` / `.shx` / `.dbf` / `.prj` (ESRI Shapefile Technical Description +
dBASE III; Point / MultiPoint / PolyLine / Polygon with parts, dBASE field
types inferred from property values) and zips them (STORE). The output
round-trips through the existing `read_shapefile` reader (verified by unit +
integration tests). GetFeature capabilities now advertise KML / SHAPE-ZIP /
CSV. Verified against the reference GeoServer: `outputFormat=application/vnd.
google-earth.kml+xml` → KML 2.2 Document with Schema + Placemarks,
`outputFormat=SHAPE-ZIP` → `application/zip` with .shp/.shx/.dbf/.prj/.cst.
New unit tests: 4 (shapefile_export); new integration: 5 (KML + SHAPE-ZIP in
wfs_test).
Batch 17 completed: **WPS (Web Processing Service) 1.0.0 first surface** —
`src/services/wps.rs` + `src/handlers/wps_handler.rs` served at `/wps`
(GetCapabilities / DescribeProcess / Execute, KVP + POST XML). GetCapabilities
emits `wps:Capabilities` (ServiceIdentification / OperationsMetadata /
ProcessOfferings / Languages); DescribeProcess emits `wps:ProcessDescriptions`
with DataInputs / ProcessOutputs. Execute accepts KVP (`response=raw` → raw
GeoJSON, else ExecuteResponse XML) and POST XML (`<wps:DataInputs>` with
LiteralData / ComplexData / `<wps:Reference xlink:href="layer:name">`, a Terrane
extension resolving a local layer). Built-in processes (pure Rust): `vec:Centroid`
(geo `Centroid`), `vec:Buffer` (point-buffer — a circle around every coordinate;
geo 0.27 does not ship the Buffer algorithm), `gs:Bounds` (bounding-box
rectangle). The reference GeoServer has WPS disabled, so this follows the OGC
WPS 1.0.0 schema directly. New unit tests: 6 (wps.rs); new integration: 6
(wps_test.rs).
Next candidates:

Batch 18 completed: **CSW (Catalog Service for the Web) 2.0.2 first surface** —
`src/services/csw.rs` + `src/handlers/csw_handler.rs` served at `/csw`
(GetCapabilities / DescribeRecord / GetRecords / GetRecordById / GetDomain,
KVP + XML POST). GetCapabilities emits `csw:Capabilities` (ServiceIdentification /
ServiceProvider / OperationsMetadata / FilterCapabilities); DescribeRecord emits
`csw:DescribeRecordResponse` with a simplified inline schema per type name.
GetRecords returns `csw:GetRecordsResponse` with `csw:SearchResults` — catalog
records are derived from the Terrane layer catalog and rendered as Dublin Core
`csw:SummaryRecord` / `csw:BriefRecord` / `csw:Record` (identifier / title /
subject / type / format / references + WGS84 `ows:BoundingBox`), with paging
(`startPosition` / `maxRecords`), `resultType=hits`, `elementSetName`, and a
minimal CQL constraint (`Title` / `Identifier` / `Subject`, `=` and `like`).
GetRecordById returns matching records; GetDomain reports result-type values.
The reference GeoServer has CSW disabled, so this follows the OGC CSW 2.0.2
schema directly (OGC 07-006r1). Also fixed: `create_layer` ignored the
user-provided bounds (the in-memory catalog kept the world extent) — REST layer
creation now applies them to `native_bounds` / `lat_lon_bounds`. New unit
tests: 9 (csw.rs); new integration: 10 (csw_test.rs).
Next candidates:

Batch 19 completed: **OGC API - Features Part 1 Core (OGC 17-069r3) first
surface** — `src/services/ogc_features.rs` + `src/handlers/ogc_api_handler.rs`
served at `/ogc/features` (JSON). Resources: landing page, `/conformance`
(conformsTo: core / oas30 / geojson), `/collections` and `/collections/{id}`
(collection = Terrane layer: id / title / description / WGS84 extent +
self/items links), `/collections/{id}/items` (GeoJSON FeatureCollection with
`numberMatched` / `numberReturned` + `links`, `bbox` filter via
`calculate_bounds` intersection, `limit` / `offset` paging + `next` link) and
`/collections/{id}/items/{featureId}`. The reference GeoServer does not ship
the OGC API extension (404), so this follows the OGC API - Features Core
schema directly. New unit tests: 7 (ogc_features.rs); new integration: 8
(ogc_api_test.rs).
Next candidates:

Batch 20 completed: **OGC API - Tiles first surface (OGC 19-069)** —
`src/services/ogc_tiles.rs` + `src/handlers/ogc_tiles_handler.rs` served at
`/ogc/tiles` (JSON). Resources: landing page, `/conformance` (conformsTo:
core / tileset / tilesets-list / tilematrixset / dataset-tileset),
`/tileMatrixSets` (+ `/tileMatrixSets/{id}` definitions for EPSG:4326
global-geodetic and EPSG:3857 global-mercator — OGC 17-083r2 TileMatrixSet
JSON with cellSize / pointOfOrigin / tileWidth/Height / matrixWidth/Height per
zoom 0..MAX_ZOOM), `/collections` and `/collections/{id}/tiles` tileset
listings (2 TileMatrixSets + item links), and raster tiles at
`/collections/{id}/tiles/{tileMatrixSetId}/{tileMatrix}/{tileRow}/{tileCol}`
(PNG default, JPEG via `?f=image/jpeg`) — rendered through the shared tile
engine (`render_tile_bytes`), so OGC API - Tiles serves the same tiles as
WMTS / TMS / WMS-C. New unit tests: 7 (ogc_tiles.rs); new integration: 7
(ogc_tiles_test.rs).
Next candidates:

Batch 21 completed: **OGC API - Maps (OGC 20-058) + OGC API - Processes
(OGC 18-062) first surfaces** — `src/services/ogc_maps.rs` +
`src/handlers/ogc_maps_handler.rs` served at `/ogc/maps` and
`src/services/ogc_processes.rs` + `src/handlers/ogc_processes_handler.rs`
served at `/ogc/processes` (both JSON; the reference GeoServer at :18080 does
not ship the OGC API extension, so both follow the OGC API schema directly).
**Maps**: landing page, `/conformance` (conformsTo: core / oas30 / html / map /
collections / collection-map), `/collections` and `/collections/{id}` (map
collection = Terrane layer with WGS84 extent + self/styles/map links),
`/collections/{id}/styles` and the `map` operation at
`/collections/{id}/map` (`bbox` + `width`/`height`, PNG default / JPEG via
`?f=image/jpeg`, `transparent` / `bgcolor` / `datetime` / `cql_filter`
pass-through) — rendered through the shared WMS GetMap pipeline (new public
`render_ogc_map` helper in `wms_handler.rs`), so OGC API - Maps serves the
same PNG/JPEG maps as the WMS 1.1.1/1.3.0 interface. **Processes**: landing
page, `/conformance` (conformsTo: core / ogc-process-description / json),
`/processes` and `/processes/{processId}` (process summary / full description
with typed inputs & outputs), and a **synchronous job surface** —
`POST /ogc/processes/jobs` executes a built-in process immediately (inputs as
`layer:<name>` reference, inline GeoJSON FeatureCollection, OGC API href to a
collection, or literal) and returns `201` with a status document; `GET /jobs` /
`GET /jobs/{jobId}` / `GET /jobs/{jobId}/results` / `DELETE /jobs/{jobId}`
complete the lifecycle (in-memory job store in `AppState.ogc_jobs`). The
built-in processes reuse the WPS 1.0.0 engine (`vec:Centroid` / `vec:Buffer` /
`gs:Bounds`). New unit tests: 8 (ogc_maps.rs) + 9 (ogc_processes.rs); new
integration: 8 (ogc_maps_test.rs) + 9 (ogc_processes_test.rs).
Next candidates:

Batch 22 completed: **OGC API - Coverages (OGC 19-088) + OGC API - Styles
(OGC 21-009) first surfaces + resilience hardening** — `src/services/
ogc_coverages.rs` + `src/handlers/ogc_coverages_handler.rs` served at
`/ogc/coverages` and `src/services/ogc_styles.rs` + `src/handlers/
ogc_styles_handler.rs` served at `/ogc/styles` (both JSON; the reference
GeoServer at :18080 does not ship the OGC API extension, so both follow the
OGC API schema directly). **Coverages**: landing page, `/conformance`
(conformsTo: core / oas30 / html / collections / coverage), `/collections`
and `/collections/{id}` (coverage collection = one per raster data source —
GeoTIFF / WorldImage / ArcGrid — with real bounds + grid size + band range
fields read from the files, mirroring WCS 2.0), and the `coverage` operation
at `/collections/{id}/coverage` (GeoTIFF default, PNG / JPEG via `?f=`,
optional `bbox` crop + `width`/`height` rescale) — rendered through the same
raster readers as the WCS 2.0 GetCoverage pipeline. **Styles**: landing page,
`/conformance` (conformsTo: core / oas30 / html / styles-list / style-info /
style-metadata / style-create-update-delete / style-search / collections),
`/styles` (GET list + POST create, write ops require JWT auth), `/styles/
{styleId}` (GET content in native media type — SLD XML / CSS / YSLD YAML /
MBStyle JSON — PUT replace, DELETE), `/styles/{styleId}/metadata` and the
collection linkage `/collections` + `/collections/{id}/styles`. **Resilience**:
`src/middleware.rs` adds a sliding-window per-client rate limiter (HTTP 429,
keyed by IP / X-Forwarded-For) and a request timeout (HTTP 504), both opt-in
via `[server]` config (`rate_limit_max_requests` / `rate_limit_window_secs` /
`request_timeout_secs`) and wired in `main.rs` around the whole app. New unit
tests: 7 (ogc_coverages.rs) + 7 (ogc_styles.rs) + 5 (middleware.rs); new
integration: 9 (ogc_coverages_test.rs) + 7 (ogc_styles_test.rs) + 4
(resilience_test.rs). CI: `.github/workflows/ci.yml` (fmt + clippy + test +
frontend build + GHCR image push on main).
Next candidates:

Batch 23 completed: **cascaded WMS resilience (retry/backoff + circuit breaking),
structured JSON logs + request `trace_id`, Redis cache data source** — (1)
`src/utils/cascaded.rs` adds exponential-backoff retry for cascaded WMS upstreams
(`cascaded_max_retries` / `cascaded_retry_base_ms`; transient failures = timeout /
connection failure / 429 / 5xx) and per-upstream circuit breakers
(`cascaded_circuit_threshold` / `cascaded_circuit_reset_secs`, open → half-open
probe → closed/reopen), driven by `[server]` config, with state held in
`AppState.cascaded_circuits`. (2) `[logging] format = "json"` emits structured
logs; the `TraceId` middleware assigns each request a `trace_id` (pass-through
`X-Trace-Id`/`X-Request-Id`, otherwise generates a UUID), attaches it to the
tracing span and echoes it in the `X-Trace-Id` response header — JSON/text logs
can be correlated across replicas. (3) Redis as a **data source**
(`DataSourceType::Redis`, persisted in metadata `data_sources`, connection fields
host/port/database/username/password); tile layers select a cache backend via
`Layer.cache_store` — empty uses the default in-memory/local cache, pointing at a
Redis data source routes that layer's tile cache through Redis
(`src/store/cache/redis.rs` + `RedisTileCacheBackend`, resolved per layer in
`render_tile_bytes` / `get_tile`); data-source connection test supports Redis
PING; backup/restore and REST layer CRUD both carry `cache_store`. Session
management stays simple JWT (no Redis session cache). New unit tests: 10
(cascaded.rs) + 6 (cache/redis.rs) + 1 (sqlite_store cache_store CRUD); frontend:
data-source dialog adds a Redis type, layer detail adds a tile-cache backend
selector.
Next candidates:

1. ImageMosaic / ImagePyramid data sources (P2)
