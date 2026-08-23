#!/usr/bin/env python3
"""Compare OGC core request responses between Terrane and GeoServer.

Sends identical OGC requests (WMS / WFS / WCS GetCapabilities, GetMap,
GetFeature, GetFeatureInfo, DescribeFeatureType) to both servers and captures
status, content-type, size and key content metrics (feature counts, geometry
types) so the two implementations can be compared.

Requires the built-in sample layers to be published on both servers:
- Terrane:  http://127.0.0.1:8080  (layers: major_cities / sample_routes / world_countries)
- GeoServer: http://127.0.0.1:18080/geoserver (layers: demo:major_cities / ...)

Usage: python service/tests/compare_ogc_with_geoserver.py
Output: <this_dir>/ogc_compare_result.json
"""
import json
import os
import sys
import urllib.request
import urllib.error

TERRANE = "http://127.0.0.1:8080"
GEOSERVER = "http://127.0.0.1:18080/geoserver"

# Layer name mapping: Terrane uses short names, GeoServer uses ws:name
LAYERS = {
    "major_cities": {"gs": "demo:major_cities", "title": "Major World Cities"},
    "sample_routes": {"gs": "demo:sample_routes", "title": "Sample Flight Routes"},
    "world_countries": {"gs": "demo:world_countries", "title": "World Countries (Simplified)"},
}

WORLD_BBOX = "-180,-90,180,90"


def fetch(url, timeout=30):
    """Fetch a URL and return (status, content_type, body_bytes)."""
    req = urllib.request.Request(url, headers={"Accept": "*/*"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            body = resp.read()
            return resp.status, resp.headers.get("Content-Type", ""), body
    except urllib.error.HTTPError as e:
        return e.code, e.headers.get("Content-Type", ""), e.read()
    except Exception as e:  # noqa: BLE001
        return 0, "", str(e).encode()


def parse_geojson_count(body):
    """Return (feature_count, geometry_types) from a GeoJSON body."""
    try:
        data = json.loads(body.decode("utf-8", "replace"))
    except Exception:
        return {"feature_count": None, "geometry_types": None}
    feats = data.get("features", [])
    types = sorted({f.get("geometry", {}).get("type") for f in feats if f.get("geometry")})
    return {"feature_count": len(feats), "geometry_types": types}


def parse_wms_cap_layers(body):
    """Return the number of Layer entries in a WMS GetCapabilities XML."""
    text = body.decode("utf-8", "replace")
    # Count <Layer> open tags (rough but consistent for both servers)
    return text.count("<Layer>")


def parse_wfs_cap_types(body):
    """Return the number of FeatureType entries in a WFS GetCapabilities XML."""
    text = body.decode("utf-8", "replace")
    return text.count("<FeatureType")


def run_case(name, path_t, path_g, analyze=None):
    """Run one request against both servers and return a result dict."""
    st, ct, body_t = fetch(TERRANE + path_t)
    sg, cg, body_g = fetch(GEOSERVER + path_g)
    res = {
        "case": name,
        "terrane": {"status": st, "content_type": ct, "size": len(body_t)},
        "geoserver": {"status": sg, "content_type": cg, "size": len(body_g)},
    }
    if analyze:
        res["terrane"].update(analyze(body_t))
        res["geoserver"].update(analyze(body_g))
    return res


def main():
    results = []

    # ---- WMS GetCapabilities ----
    results.append(run_case(
        "WMS GetCapabilities 1.1.1",
        "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetCapabilities",
        "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetCapabilities",
        analyze=lambda b: {"layers": parse_wms_cap_layers(b)},
    ))
    results.append(run_case(
        "WMS GetCapabilities 1.3.0",
        "/wms?SERVICE=WMS&VERSION=1.3.0&REQUEST=GetCapabilities",
        "/wms?SERVICE=WMS&VERSION=1.3.0&REQUEST=GetCapabilities",
        analyze=lambda b: {"layers": parse_wms_cap_layers(b)},
    ))

    # ---- WMS GetMap (PNG) per layer ----
    for short, info in LAYERS.items():
        results.append(run_case(
            f"WMS GetMap PNG {short}",
            f"/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS={short}"
            f"&BBOX={WORLD_BBOX}&WIDTH=256&HEIGHT=256&FORMAT=image/png&SRS=EPSG:4326",
            f"/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS={info['gs']}"
            f"&BBOX={WORLD_BBOX}&WIDTH=256&HEIGHT=256&FORMAT=image/png&SRS=EPSG:4326",
        ))

    # ---- WMS GetFeatureInfo ----
    results.append(run_case(
        "WMS GetFeatureInfo major_cities",
        f"/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetFeatureInfo&LAYERS=major_cities"
        f"&BBOX={WORLD_BBOX}&WIDTH=256&HEIGHT=256&X=128&Y=128&QUERY_LAYERS=major_cities"
        f"&INFO_FORMAT=application/json&SRS=EPSG:4326",
        f"/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetFeatureInfo&LAYERS=demo:major_cities"
        f"&BBOX={WORLD_BBOX}&WIDTH=256&HEIGHT=256&X=128&Y=128&QUERY_LAYERS=demo:major_cities"
        f"&INFO_FORMAT=application/json&SRS=EPSG:4326",
    ))

    # ---- WFS GetCapabilities ----
    results.append(run_case(
        "WFS GetCapabilities 1.1.0",
        "/wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetCapabilities",
        "/wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetCapabilities",
        analyze=lambda b: {"feature_types": parse_wfs_cap_types(b)},
    ))

    # ---- WFS GetFeature (GeoJSON) per layer ----
    for short, info in LAYERS.items():
        results.append(run_case(
            f"WFS GetFeature GeoJSON {short}",
            f"/wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetFeature&TYPENAME={short}"
            f"&OUTPUTFORMAT=application/json",
            f"/wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetFeature&TYPENAME={info['gs']}"
            f"&OUTPUTFORMAT=application/json",
            analyze=parse_geojson_count,
        ))

    # ---- WFS DescribeFeatureType ----
    results.append(run_case(
        "WFS DescribeFeatureType major_cities",
        "/wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=DescribeFeatureType&TYPENAME=major_cities",
        "/wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=DescribeFeatureType&TYPENAME=demo:major_cities",
    ))

    # ---- WCS GetCapabilities ----
    results.append(run_case(
        "WCS GetCapabilities 2.0.1",
        "/wcs?SERVICE=WCS&VERSION=2.0.1&REQUEST=GetCapabilities",
        "/wcs?SERVICE=WCS&VERSION=2.0.1&REQUEST=GetCapabilities",
    ))

    # ---- Output ----
    out = json.dumps(results, indent=2, ensure_ascii=False)
    print(out)
    out_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "ogc_compare_result.json")
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(out)
    print(f"\n[written] {out_path}", file=sys.stderr)


if __name__ == "__main__":
    main()