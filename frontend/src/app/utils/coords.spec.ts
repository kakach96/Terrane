import { transformBounds, normalizeCrs } from './coords';

describe('normalizeCrs', () => {
  it('normalizes legacy EPSG:900913 to EPSG:3857', () => {
    expect(normalizeCrs('EPSG:900913')).toBe('EPSG:3857');
    expect(normalizeCrs('900913')).toBe('EPSG:3857');
  });

  it('uppercases and trims other CRS identifiers', () => {
    expect(normalizeCrs(' epsg:4326 ')).toBe('EPSG:4326');
    expect(normalizeCrs('EPSG:3857')).toBe('EPSG:3857');
  });
});

describe('transformBounds', () => {
  const wgs84 = { minx: 100, miny: 20, maxx: 110, maxy: 30 };

  it('returns the same bounds when the CRS matches after normalization', () => {
    expect(transformBounds(wgs84, 'EPSG:4326', 'epsg:4326')).toEqual(wgs84);
    expect(transformBounds(wgs84, 'EPSG:3857', 'EPSG:900913')).toEqual(wgs84);
  });

  it('converts WGS84 bounds to Web Mercator meters', () => {
    const m = transformBounds(wgs84, 'EPSG:4326', 'EPSG:3857');
    // 100 deg lon -> 11131949.07...; 30 deg lat -> 3503549.84...
    expect(m.minx).toBeCloseTo((100 / 180) * 20037508.34, 0);
    expect(m.maxx).toBeCloseTo((110 / 180) * 20037508.34, 0);
    expect(m.miny).toBeGreaterThan(2000000);
    expect(m.maxy).toBeLessThan(4000000);
    // min/max are normalized (south-west / north-east)
    expect(m.miny).toBeLessThan(m.maxy);
  });

  it('round-trips WGS84 -> Mercator -> WGS84', () => {
    const m = transformBounds(wgs84, 'EPSG:4326', 'EPSG:3857');
    const back = transformBounds(m, 'EPSG:3857', 'EPSG:4326');
    expect(back.minx).toBeCloseTo(wgs84.minx, 6);
    expect(back.miny).toBeCloseTo(wgs84.miny, 6);
    expect(back.maxx).toBeCloseTo(wgs84.maxx, 6);
    expect(back.maxy).toBeCloseTo(wgs84.maxy, 6);
  });

  it('clamps Mercator latitude conversion near the pole (no Infinity)', () => {
    const polar = { minx: 0, miny: 89, maxx: 1, maxy: 90 };
    const m = transformBounds(polar, 'EPSG:4326', 'EPSG:3857');
    expect(Number.isFinite(m.miny)).toBe(true);
    expect(Number.isFinite(m.maxy)).toBe(true);
    expect(m.maxy).toBeCloseTo(20037508.34, 3);
  });
});
