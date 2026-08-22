/**
 * Shared layer-preview format definitions.
 *
 * The backend WMS GetMap already emits a wide range of formats (PNG / JPEG /
 * GIF / WebP / TIFF / SVG / KML / GeoJSON / PDF / GeoRSS); the frontend preview
 * surfaces a curated subset aligned with GeoServer's layer-preview page. Each
 * format is categorized so the preview components know how to render it:
 *
 * - `openlayers` — interactive OpenLayers map (iframe)
 * - `image`      — raster output rendered as an <img>
 * - `document`   — vector/document output (SVG / KML / GeoJSON / PDF) rendered
 *                  in an <iframe> (browsers render SVG/PDF natively, raw text
 *                  for KML/GeoJSON)
 * - `mvt`        — Mapbox Vector Tile (binary .pbf), opened in a new tab
 */

export type PreviewFormatCategory = 'openlayers' | 'image' | 'document' | 'mvt';

export interface PreviewFormatDef {
  value: string;
  /** i18n key suffix, e.g. 'formatPng' → 'preview.formatPng' / 'layerDetail.formatPng' */
  keySuffix: string;
  category: PreviewFormatCategory;
}

export const PREVIEW_FORMATS: PreviewFormatDef[] = [
  {
    value: 'application/openlayers',
    keySuffix: 'formatOpenLayers',
    category: 'openlayers',
  },
  { value: 'image/png', keySuffix: 'formatPng', category: 'image' },
  { value: 'image/jpeg', keySuffix: 'formatJpeg', category: 'image' },
  { value: 'image/gif', keySuffix: 'formatGif', category: 'image' },
  { value: 'image/webp', keySuffix: 'formatWebp', category: 'image' },
  { value: 'image/tiff', keySuffix: 'formatTiff', category: 'image' },
  { value: 'image/svg+xml', keySuffix: 'formatSvg', category: 'document' },
  {
    value: 'application/vnd.google-earth.kml+xml',
    keySuffix: 'formatKml',
    category: 'document',
  },
  { value: 'application/geo+json', keySuffix: 'formatGeoJson', category: 'document' },
  { value: 'application/pdf', keySuffix: 'formatPdf', category: 'document' },
  {
    value: 'application/vnd.mapbox-vector-tile',
    keySuffix: 'formatMvt',
    category: 'mvt',
  },
];

/** Resolve the rendering category for a preview format (defaults to `image`). */
export function previewFormatCategory(format: string): PreviewFormatCategory {
  return PREVIEW_FORMATS.find((f) => f.value === format)?.category ?? 'image';
}