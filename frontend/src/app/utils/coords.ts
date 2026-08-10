import { LayerBounds } from '../models/geoserver.models';

/**
 * 前端坐标系转换工具 (无第三方依赖)。
 *
 * 当前内置 WGS84 (EPSG:4326) 与 Web Mercator (EPSG:3857 / EPSG:900913) 之间的
 * 数学变换, 与后端 `src/utils/geometry.rs` 的兜底实现保持一致。后续需要支持
 * 其他坐标系时, 在此追加转换分支即可 (后端已通过 proj4rs 支持任意 EPSG)。
 */

const MERCATOR_EXTENT = 20037508.34;
/** Web Mercator 有效纬度上限 (±85.051129°), 避免 ±90° 时对数发散为 Infinity。 */
const MAX_LAT = 85.05112877980659;

/** 归一化坐标系标识 (EPSG:900913 -> EPSG:3857)。 */
export function normalizeCrs(crs: string): string {
  const c = crs.trim().toUpperCase();
  if (c === 'EPSG:900913' || c === '900913') return 'EPSG:3857';
  return c;
}

/** 判断是否为 Web Mercator (EPSG:3857 / 900913)。 */
function isMercator(crs: string): boolean {
  return normalizeCrs(crs) === 'EPSG:3857';
}

/** 经纬度 (EPSG:4326, 度) -> Web Mercator (米)。 */
function lonLatToMercator(lon: number, lat: number): [number, number] {
  const clampedLat = Math.max(-MAX_LAT, Math.min(MAX_LAT, lat));
  const x = (lon * MERCATOR_EXTENT) / 180;
  let y = Math.log(Math.tan((90 + clampedLat) * (Math.PI / 360))) / (Math.PI / 180);
  y = (y * MERCATOR_EXTENT) / 180;
  return [x, y];
}

/** Web Mercator (米) -> 经纬度 (EPSG:4326, 度)。 */
function mercatorToLonLat(x: number, y: number): [number, number] {
  const lon = (x / MERCATOR_EXTENT) * 180;
  let lat = (y / MERCATOR_EXTENT) * 180;
  lat =
    (180 / Math.PI) * (2 * Math.atan(Math.exp((lat * Math.PI) / 180)) - Math.PI / 2);
  return [lon, lat];
}

/**
 * 将边界从 `fromCrs` 转换到 `toCrs`。
 *
 * WMS 的 BBOX 参数始终处于请求 SRS 下, 因此切换预览坐标系时需要用本函数
 * 把图层的原生边界 (通常为 EPSG:4326) 转换到目标坐标系后再传给 WMS。
 */
export function transformBounds(
  bounds: LayerBounds,
  fromCrs: string,
  toCrs: string,
): LayerBounds {
  const from = normalizeCrs(fromCrs);
  const to = normalizeCrs(toCrs);
  if (from === to) return bounds;

  // 统一经 EPSG:4326 中转
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
