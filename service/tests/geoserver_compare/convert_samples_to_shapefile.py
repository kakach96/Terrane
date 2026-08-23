#!/usr/bin/env python3
"""Convert the Terrane built-in sample GeoJSON files to Shapefile format so
they can be published through GeoServer's built-in Shapefile datastore.

GeoServer 2.28 does not ship the GeoJSON datastore extension, so the curated
sample datasets are converted to Shapefile before publishing.

Usage: python service/tests/convert_samples_to_shapefile.py
Output: <service>/samples/shp/<name>.shp (+ .shx/.dbf/.prj)
"""
import json
import os
import shapefile

SAMPLES_DIR = os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "..", "samples"
)
OUT_DIR = os.path.join(SAMPLES_DIR, "shp")

# (file, shape_type, field spec)
# field spec: list of (name, pyshp_type, size, decimal)
DATASETS = [
    (
        "major_cities.geojson",
        shapefile.POINT,
        [
            ("name", "C", 40, 0),
            ("country", "C", 40, 0),
            ("population", "N", 18, 2),
            ("capital", "L", 1, 0),
        ],
    ),
    (
        "sample_routes.geojson",
        shapefile.POLYLINE,
        [
            ("route", "C", 40, 0),
            ("from", "C", 40, 0),
            ("to", "C", 40, 0),
            ("distance_km", "N", 18, 2),
        ],
    ),
    (
        "world_countries.geojson",
        shapefile.POLYGON,
        [
            ("name", "C", 40, 0),
            ("continent", "C", 40, 0),
            ("area_km2", "N", 18, 2),
            ("population", "N", 18, 2),
        ],
    ),
]


def convert(geojson_path, out_base, shape_type, fields):
    with open(geojson_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    w = shapefile.Writer(out_base, shapeType=shape_type)
    for name, ftype, size, dec in fields:
        w.field(name, ftype, size=size, decimal=dec)

    for feat in data["features"]:
        geom = feat["geometry"]
        props = feat.get("properties", {})
        if shape_type == shapefile.POINT:
            w.point(*geom["coordinates"][:2])
        elif shape_type == shapefile.POLYLINE:
            w.line([geom["coordinates"]])
        elif shape_type == shapefile.POLYGON:
            # GeoJSON polygon: list of rings; pyshp expects [ [ring], ... ]
            w.poly(geom["coordinates"])
        w.record(*[props.get(name) for name, *_ in fields])

    w.close()

    # Write a .prj (EPSG:4326 WGS84)
    prj = (
        'GEOGCS["WGS 84",DATUM["WGS_1984",SPHEROID["WGS 84",6378137,'
        "298.257223563]],PRIMEM[\"Greenwich\",0],UNIT[\"degree\","
        "0.0174532925199433]]"
    )
    with open(out_base + ".prj", "w") as f:
        f.write(prj)

    print(f"converted {os.path.basename(geojson_path)} -> {os.path.basename(out_base)}.shp")


def main():
    os.makedirs(OUT_DIR, exist_ok=True)
    for geojson, shape_type, fields in DATASETS:
        src = os.path.join(SAMPLES_DIR, geojson)
        out_base = os.path.join(OUT_DIR, os.path.splitext(geojson)[0])
        convert(src, out_base, shape_type, fields)


if __name__ == "__main__":
    main()