#!/usr/bin/env python3
"""Compare response times between Terrane (release) and GeoServer across many
OGC request types. Outputs only timing results (avg/min/max over N runs).
"""
import urllib.request
import time

TERRANE = "http://127.0.0.1:8080"
GEOSERVER = "http://127.0.0.1:18080/geoserver"


def timeit(url, n=15):
    ts = []
    for _ in range(n):
        s = time.perf_counter()
        urllib.request.urlopen(url, timeout=30).read()
        ts.append((time.perf_counter() - s) * 1000)
    ts.sort()
    return sum(ts) / len(ts), ts[0], ts[-1]


CASES = [
    ("WMS GetCapabilities 1.1.1",
     "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetCapabilities",
     "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetCapabilities"),
    ("WMS GetMap PNG world_countries",
     "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=world_countries&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&FORMAT=image/png&SRS=EPSG:4326",
     "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=demo:world_countries&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&FORMAT=image/png&SRS=EPSG:4326"),
    ("WMS GetMap PNG major_cities",
     "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=major_cities&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&FORMAT=image/png&SRS=EPSG:4326",
     "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=demo:major_cities&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&FORMAT=image/png&SRS=EPSG:4326"),
    ("WMS GetMap PNG sample_routes",
     "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=sample_routes&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&FORMAT=image/png&SRS=EPSG:4326",
     "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=demo:sample_routes&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&FORMAT=image/png&SRS=EPSG:4326"),
    ("WMS GetMap JPEG major_cities",
     "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=major_cities&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&FORMAT=image/jpeg&SRS=EPSG:4326",
     "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=demo:major_cities&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&FORMAT=image/jpeg&SRS=EPSG:4326"),
    ("WMS GetMap PNG 512 world_countries",
     "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=world_countries&BBOX=-180,-90,180,90&WIDTH=512&HEIGHT=512&FORMAT=image/png&SRS=EPSG:4326",
     "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=demo:world_countries&BBOX=-180,-90,180,90&WIDTH=512&HEIGHT=512&FORMAT=image/png&SRS=EPSG:4326"),
    ("WMS GetFeatureInfo major_cities",
     "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetFeatureInfo&LAYERS=major_cities&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&X=128&Y=128&QUERY_LAYERS=major_cities&INFO_FORMAT=application/json&SRS=EPSG:4326",
     "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetFeatureInfo&LAYERS=demo:major_cities&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&X=128&Y=128&QUERY_LAYERS=demo:major_cities&INFO_FORMAT=application/json&SRS=EPSG:4326"),
    ("WFS GetCapabilities 1.1.0",
     "/wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetCapabilities",
     "/wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetCapabilities"),
    ("WFS GetFeature GeoJSON major_cities",
     "/wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetFeature&TYPENAME=major_cities&OUTPUTFORMAT=application/json",
     "/wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetFeature&TYPENAME=demo:major_cities&OUTPUTFORMAT=application/json"),
    ("WFS GetFeature GeoJSON world_countries",
     "/wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetFeature&TYPENAME=world_countries&OUTPUTFORMAT=application/json",
     "/wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetFeature&TYPENAME=demo:world_countries&OUTPUTFORMAT=application/json"),
    ("WFS DescribeFeatureType major_cities",
     "/wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=DescribeFeatureType&TYPENAME=major_cities",
     "/wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=DescribeFeatureType&TYPENAME=demo:major_cities"),
    ("WCS GetCapabilities 2.0.1",
     "/wcs?SERVICE=WCS&VERSION=2.0.1&REQUEST=GetCapabilities",
     "/wcs?SERVICE=WCS&VERSION=2.0.1&REQUEST=GetCapabilities"),
]


def main():
    hdr = f"{'case':<42s} {'Terrane(rel) avg/min/max':>26s}  {'GeoServer avg/min/max':>26s}  ratio(T/G)"
    print(hdr)
    print("-" * 105)
    for name, t_path, g_path in CASES:
        tt = timeit(TERRANE + t_path)
        gt = timeit(GEOSERVER + g_path)
        r = tt[0] / gt[0] if gt[0] > 0 else 0
        print(f"{name:<42s} {tt[0]:7.1f}/{tt[1]:5.1f}/{tt[2]:5.1f}ms  "
              f"{gt[0]:7.1f}/{gt[1]:5.1f}/{gt[2]:5.1f}ms  {r:5.2f}x")


if __name__ == "__main__":
    main()