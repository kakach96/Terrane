import { LayerBounds } from '../models/geoserver.models';

/**
 * Frontend coordinate transformation utilities (no third-party dependencies).
 *
 * Provides the math transforms between WGS84 (EPSG:4326) and Web Mercator
 * (EPSG:3857 / EPSG:900913), kept in sync with the fallback implementation in
 * the backend `src/utils/geometry.rs`. To support additional CRSs later, add
 * another transform branch here (the backend already supports arbitrary EPSG
 * via proj4rs).
 */

const MERCATOR_EXTENT = 20037508.34;
/** Web Mercator valid latitude limit (±85.051129°), avoids log divergence to Infinity at ±90°. */
const MAX_LAT = 85.05112877980659;

/** Normalize a CRS identifier (EPSG:900913 -> EPSG:3857). */
export function normalizeCrs(crs: string): string {
  const c = crs.trim().toUpperCase();
  if (c === 'EPSG:900913' || c === '900913') return 'EPSG:3857';
  return c;
}

/** Whether the CRS is Web Mercator (EPSG:3857 / 900913). */
function isMercator(crs: string): boolean {
  return normalizeCrs(crs) === 'EPSG:3857';
}

/** Lon/lat (EPSG:4326, degrees) -> Web Mercator (meters). */
function lonLatToMercator(lon: number, lat: number): [number, number] {
  const clampedLat = Math.max(-MAX_LAT, Math.min(MAX_LAT, lat));
  const x = (lon * MERCATOR_EXTENT) / 180;
  let y = Math.log(Math.tan((90 + clampedLat) * (Math.PI / 360))) / (Math.PI / 180);
  y = (y * MERCATOR_EXTENT) / 180;
  return [x, y];
}

/** Web Mercator (meters) -> lon/lat (EPSG:4326, degrees). */
function mercatorToLonLat(x: number, y: number): [number, number] {
  const lon = (x / MERCATOR_EXTENT) * 180;
  let lat = (y / MERCATOR_EXTENT) * 180;
  lat = (180 / Math.PI) * (2 * Math.atan(Math.exp((lat * Math.PI) / 180)) - Math.PI / 2);
  return [lon, lat];
}

/**
 * Transform bounds from `fromCrs` to `toCrs`.
 *
 * The WMS BBOX parameter is always in the request SRS, so when switching the
 * preview CRS this function converts the layer's native bounds (usually
 * EPSG:4326) to the target CRS before passing it to WMS.
 */
export function transformBounds(bounds: LayerBounds, fromCrs: string, toCrs: string): LayerBounds {
  const from = normalizeCrs(fromCrs);
  const to = normalizeCrs(toCrs);
  if (from === to) return bounds;

  // Route everything through EPSG:4326
  const to4326 = (x: number, y: number): [number, number] =>
    isMercator(from) ? mercatorToLonLat(x, y) : [x, y];
  const from4326 = (lon: number, lat: number): [number, number] =>
    isMercator(to) ? lonLatToMercator(lon, lat) : [lon, lat];

  const [minLon, minLat] = to4326(bounds.minx, bounds.miny);
  const [maxLon, maxLat] = to4326(bounds.maxx, bounds.maxy);

  const [x1, y1] = from4326(minLon, minLat);
  const [x2, y2] = from4326(maxLon, maxLat);

  return {
    minx: Math.min(x1, x2),
    miny: Math.min(y1, y2),
    maxx: Math.max(x1, x2),
    maxy: Math.max(y1, y2),
  };
}
