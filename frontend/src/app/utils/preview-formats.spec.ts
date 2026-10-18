import { PREVIEW_FORMATS, previewFormatCategory } from './preview-formats';

describe('previewFormatCategory', () => {
  it('resolves every curated format to a category', () => {
    for (const f of PREVIEW_FORMATS) {
      expect(previewFormatCategory(f.value)).toBe(f.category);
    }
  });

  it('defaults unknown formats to image', () => {
    expect(previewFormatCategory('image/jpeg')).toBe('image');
    expect(previewFormatCategory('application/x-unknown')).toBe('image');
  });

  it('matches GeoServer preview parity: openlayers + image + document + mvt categories', () => {
    const categories = new Set(PREVIEW_FORMATS.map((f) => f.category));
    expect(categories.has('openlayers')).toBe(true);
    expect(categories.has('image')).toBe(true);
    expect(categories.has('document')).toBe(true);
    expect(categories.has('mvt')).toBe(true);
  });

  it('keeps MVT advertised with the Mapbox vector tile mime', () => {
    const mvt = PREVIEW_FORMATS.find((f) => f.category === 'mvt');
    expect(mvt?.value).toBe('application/vnd.mapbox-vector-tile');
  });

  it('has unique format values (i18n keySuffix used per entry)', () => {
    const values = PREVIEW_FORMATS.map((f) => f.value);
    expect(new Set(values).size).toBe(values.length);
    for (const f of PREVIEW_FORMATS) {
      expect(f.keySuffix.startsWith('format')).toBe(true);
    }
  });
});
