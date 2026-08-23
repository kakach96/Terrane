# Built-in sample data

Curated sample datasets that ship with Terrane so the product demos
out-of-the-box. On first startup (when the catalog has no layers yet) the
backend copies these files into `<data_dir>/samples/` and auto-registers a
`demo` workspace with one GeoJSON data source + layer per dataset.

| File                    | Geometry | Layer            | Description                                        |
| ----------------------- | -------- | ---------------- | -------------------------------------------------- |
| `major_cities.geojson`  | Point    | `major_cities`   | 25 major world cities (name / country / population / capital) |
| `sample_routes.geojson` | Line     | `sample_routes`  | 14 sample flight routes between major cities (route / from / to / distance_km) |
| `world_countries.geojson` | Polygon | `world_countries` | 6 simplified country outlines (name / continent / area_km2 / population) |

All files are in EPSG:4326 (WGS84 lon/lat). The country polygons are heavily
simplified for demo purposes — they are not cartographically accurate.

## Configuration

Seeding is controlled by the `[samples]` section:

```toml
[samples]
# Auto-register built-in sample data on first startup (default: true)
enabled = true
# Directory that holds the curated sample files (default: "./samples")
source_dir = "./samples"
```

Set `enabled = false` (or `GEOSERVER__SAMPLES__ENABLED=false`) to skip seeding.
Seeding only runs when the catalog contains no layers, so existing installs are
never modified.

### Source directory resolution

`source_dir` is resolved relative to the current working directory. If the
configured directory does not contain the sample files, the backend falls back
to these locations (in order):

1. `<exe_dir>/samples` — samples bundled next to the executable (the Docker
   image layout `/app/terrane` + `/app/samples`, and the release package
   layout `release-package/terrane.exe` + `samples/`).
2. Ancestor directories of the executable that contain a `samples` dir with
   the sample files — covers running the binary from a packaged subdirectory
   while the samples live in the source tree (e.g.
   `service/target/release/release-package/` → `service/samples/`).

A candidate only counts when it actually holds all curated sample files, so an
unrelated `samples` directory is never picked up by accident.

## Adding a dataset

1. Drop a `.geojson` file into this directory.
2. Add a `SampleDataset` entry in `service/src/utils/samples.rs`
   (`SAMPLE_DATASETS`).
3. On the next fresh startup the file is copied to `<data_dir>/samples/` and
   registered as a GeoJSON data source + layer in the `demo` workspace.