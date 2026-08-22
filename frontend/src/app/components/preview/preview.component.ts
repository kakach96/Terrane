import { Component, ChangeDetectionStrategy, computed, inject, signal } from '@angular/core';
import { ActivatedRoute } from '@angular/router';
import { DomSanitizer, SafeResourceUrl } from '@angular/platform-browser';
import { TranslateService } from '@ngx-translate/core';
import { toSignal, toObservable } from '@angular/core/rxjs-interop';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { LanguageService } from '../../services/language.service';
import { Layer, LayerGroup, ConnectionTestResult } from '../../models/geoserver.models';
import { transformBounds } from '../../utils/coords';
import { PREVIEW_FORMATS, previewFormatCategory } from '../../utils/preview-formats';
import { switchMap, tap, map, startWith, catchError, of, combineLatest, forkJoin } from 'rxjs';

interface Bounds {
  minx: number;
  miny: number;
  maxx: number;
  maxy: number;
}

interface PreviewData {
  layers: Layer[];
  groups: LayerGroup[];
}

@Component({
  standalone: false,
  selector: 'app-preview',
  templateUrl: './preview.component.html',
  styleUrls: ['./preview.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class PreviewComponent {
  private route = inject(ActivatedRoute);
  private geoserverService = inject(GeoserverService);
  private sanitizer = inject(DomSanitizer);
  private translate = inject(TranslateService);
  private notificationService = inject(NotificationService);
  private languageService = inject(LanguageService);

  previewMode: 'layer' | 'group' = 'layer';
  searchQuery = '';

  selectedLayer = '';
  selectedGroup = '';
  // Signals so the displayBounds()/displayCrs() computeds re-evaluate when the
  // selection changes (a plain field would leave the computed cached at null).
  currentLayer = signal<Layer | null>(null);
  currentGroup = signal<LayerGroup | null>(null);

  previewUrl = '';
  safePreviewUrl: SafeResourceUrl = '';
  // Non-null when the selected layer's data source is unavailable; shows a
  // friendly message instead of a blank/failed preview.
  dataSourceError = signal<string | null>(null);

  featureCount = 0;
  geometryTypes: string[] = [];

  previewOptions = {
    width: 800,
    height: 600,
    format: 'application/openlayers' as string,
    transparent: true,
    crs: 'EPSG:4326',
  };

  // Computed so labels re-translate on language switch. Reading currentLang()
  // makes the computed re-evaluate when the language changes; the array is
  // memoized so *ngFor does not rebuild mat-option nodes on every CD cycle.
  formatOptions = computed<{ value: string; label: string }[]>(() => {
    this.languageService.currentLang();
    return PREVIEW_FORMATS.map((f) => ({
      value: f.value,
      label: this.translate.instant(`preview.${f.keySuffix}`),
    }));
  });

  private refreshTrigger = signal(0);
  loading = signal(true);

  constructor() {
    // Initialise preview mode from query params
    const layerParam = this.route.snapshot.queryParamMap.get('layer');
    const groupParam = this.route.snapshot.queryParamMap.get('group');
    if (groupParam) {
      this.previewMode = 'group';
      this.selectedGroup = groupParam;
    } else if (layerParam) {
      this.previewMode = 'layer';
      this.selectedLayer = layerParam;
    }
  }

  // ── Signal pipeline ───────────────────────────────────────────────
  private previewData$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    tap(() => this.loading.set(true)),
    switchMap(() =>
      combineLatest([
        this.geoserverService.getLayers().pipe(catchError(() => of([] as Layer[]))),
        this.geoserverService.getLayerGroups().pipe(catchError(() => of([] as LayerGroup[]))),
      ]).pipe(map(([layers, groups]) => ({ layers, groups } as PreviewData))),
    ),
    tap((data) => {
      // Apply initial selection after data loads. Use the emitted `data`
      // directly (not the `data()` signal) because the signal is only updated
      // AFTER this tap runs, so reading this.layers()/this.groups() here would
      // still see the empty initial value and never select a layer.
      const enabledLayers = data.layers.filter((l) => l.enabled);
      if (this.previewMode === 'layer' && !this.currentLayer()) {
        this.selectLayer(this.selectedLayer || (enabledLayers[0]?.name ?? ''), data.layers);
      }
      if (this.previewMode === 'group' && !this.currentGroup()) {
        this.selectGroup(this.selectedGroup || (data.groups[0]?.name ?? ''), data.groups);
      }
      this.loading.set(false);
    }),
  );

  private data = toSignal(this.previewData$, {
    initialValue: { layers: [] as Layer[], groups: [] as LayerGroup[] },
  });

  // ── Derived signals ───────────────────────────────────────────────
  layers = computed(() => this.data().layers.filter((l) => l.enabled));
  groups = computed(() => this.data().groups);

  // Plain methods (not computed) because previewOptions.format is a plain field,
  // not a signal — a computed would cache the initial value and never reflect
  // format changes (breaking the img/iframe switch and the size controls).
  /** Raster or document output: show width/height/transparent controls. */
  isStaticFormat(): boolean {
    const c = previewFormatCategory(this.previewOptions.format);
    return c === 'image' || c === 'document';
  }

  /** Raster output rendered as an <img>. */
  isImageFormat(): boolean {
    return previewFormatCategory(this.previewOptions.format) === 'image';
  }

  /** Interactive map or document output rendered in an <iframe>. */
  isIframeFormat(): boolean {
    const c = previewFormatCategory(this.previewOptions.format);
    return c === 'openlayers' || c === 'document';
  }

  /** Mapbox Vector Tile: binary .pbf, opened in a new tab. */
  isMvtFormat(): boolean {
    return previewFormatCategory(this.previewOptions.format) === 'mvt';
  }

  filteredLayers = computed(() => {
    const q = this.searchQuery.trim().toLowerCase();
    return this.layers().filter((l) => {
      if (!q) return true;
      return (
        l.name.toLowerCase().includes(q) ||
        (l.title || '').toLowerCase().includes(q) ||
        l.workspace.toLowerCase().includes(q)
      );
    });
  });

  filteredGroups = computed(() => {
    const q = this.searchQuery.trim().toLowerCase();
    return this.groups().filter((g) => {
      if (!q) return true;
      return g.name.toLowerCase().includes(q) || (g.title || '').toLowerCase().includes(q);
    });
  });

  displayBounds = computed((): Bounds | null => {
    if (this.previewMode === 'layer') {
      const l = this.currentLayer();
      if (!l) return null;
      return l.native_bounds?.bounds || l.bounds || null;
    }
    const g = this.currentGroup();
    if (!g || g.layers.length === 0) return null;
    const first = this.layers().find((l) => l.name === g.layers[0]);
    if (!first) return null;
    return first.native_bounds?.bounds || first.bounds || null;
  });

  displayCrs = computed((): string => {
    if (this.previewMode === 'layer') {
      const l = this.currentLayer();
      if (!l) return 'EPSG:4326';
      return l.native_bounds?.crs || l.srs || 'EPSG:4326';
    }
    const g = this.currentGroup();
    if (g && g.layers.length > 0) {
      const first = this.layers().find((l) => l.name === g.layers[0]);
      if (first) {
        return first.native_bounds?.crs || first.srs || 'EPSG:4326';
      }
    }
    return 'EPSG:4326';
  });

  // ── Actions ───────────────────────────────────────────────────────
  selectLayer(name: string, layers?: Layer[]): void {
    if (!name) return;
    this.selectedLayer = name;
    const source = layers ?? this.layers();
    const layer = source.find((l) => l.name === name);
    if (!layer) return;
    this.currentLayer.set(layer);
    this.currentGroup.set(null);
    this.previewOptions.crs = this.displayCrs();
    this.checkDataSource([layer.store]);
    this.refreshPreview();
    this.loadLayerStats(name);
  }

  selectGroup(name: string, groups?: LayerGroup[]): void {
    if (!name) return;
    this.selectedGroup = name;
    const source = groups ?? this.groups();
    const group = source.find((g) => g.name === name);
    if (!group) return;
    this.currentGroup.set(group);
    this.currentLayer.set(null);
    this.featureCount = 0;
    this.geometryTypes = [];
    this.previewOptions.crs = this.displayCrs();
    // Check availability of every data source backing the group's layers.
    const stores = group.layers
      .map((ln) => this.layers().find((l) => l.name === ln)?.store)
      .filter((s): s is string => !!s);
    this.checkDataSource(stores);
    this.refreshPreview();
  }

  // Probe the data sources backing the selected layer(s). If any is
  // unavailable, surface a friendly message instead of a blank/failed preview.
  private checkDataSource(stores: string[]): void {
    this.dataSourceError.set(null);
    const unique = [...new Set(stores.filter(Boolean))];
    if (unique.length === 0) return;
    forkJoin(
      unique.map((store) =>
        this.geoserverService.testDataSourceConnection(store).pipe(
          catchError(() => of({ success: false, message: '' } as ConnectionTestResult)),
        ),
      ),
    ).subscribe((results) => {
      const failed = results.find((r) => !r.success);
      if (failed) {
        this.dataSourceError.set(failed.message || '');
        this.previewUrl = '';
        this.safePreviewUrl = '';
      }
    });
  }

  onModeChange(): void {
    this.previewUrl = '';
    this.safePreviewUrl = '';
    this.currentLayer.set(null);
    this.currentGroup.set(null);
    if (this.previewMode === 'layer') {
      this.selectLayer(this.selectedLayer || (this.layers()[0]?.name ?? ''));
    } else {
      this.selectGroup(this.selectedGroup || (this.groups()[0]?.name ?? ''));
    }
  }

  refreshPreview(): void {
    if (this.previewMode === 'layer') {
      const layer = this.currentLayer();
      if (!layer) return;
      const bounds = this.displayBounds();
      if (!bounds) {
        this.previewUrl = '';
        return;
      }
      if (this.previewOptions.format === 'application/vnd.mapbox-vector-tile') {
        // MVT: single tile at zoom 0 (whole layer extent) — open in a new tab.
        this.previewUrl = this.geoserverService.getMvtTileUrl(layer, 0, 0, 0);
        this.safePreviewUrl = this.sanitizer.bypassSecurityTrustResourceUrl(this.previewUrl);
        return;
      }
      if (this.previewOptions.format === 'application/openlayers') {
        this.previewUrl = this.geoserverService.getWmsPreviewUrl(layer, {
          width: this.previewOptions.width,
          height: this.previewOptions.height,
          crs: this.previewOptions.crs,
          format: 'application/openlayers',
          transparent: true,
        });
      } else {
        this.previewUrl = this.geoserverService.getMapImageUrl(layer, {
          width: this.previewOptions.width,
          height: this.previewOptions.height,
          crs: this.previewOptions.crs,
          format: this.previewOptions.format,
          transparent: this.previewOptions.transparent,
        });
      }
    } else {
      const group = this.currentGroup();
      if (!group || group.layers.length === 0) return;
      const bounds = this.displayBounds();
      if (!bounds) {
        this.previewUrl = '';
        return;
      }
      const nativeCrs = this.displayCrs();
      // WMS 约定 BBOX 处于请求 SRS 下, 切换坐标系时转换图层原生边界
      const converted = transformBounds(bounds, nativeCrs, this.previewOptions.crs);
      // MVT 是单图层瓦片格式, 图层组预览回退到 PNG。
      const format =
        this.previewOptions.format === 'application/vnd.mapbox-vector-tile'
          ? 'image/png'
          : this.previewOptions.format;
      const params = new URLSearchParams({
        service: 'WMS',
        version: '1.1.1',
        request: 'GetMap',
        layers: group.layers.join(','),
        srs: this.previewOptions.crs,
        bbox: `${converted.minx},${converted.miny},${converted.maxx},${converted.maxy}`,
        width: this.previewOptions.width.toString(),
        height: this.previewOptions.height.toString(),
        format,
        transparent: this.previewOptions.transparent.toString(),
      });
      this.previewUrl = `/wms?${params}`;
    }
    this.safePreviewUrl = this.sanitizer.bypassSecurityTrustResourceUrl(this.previewUrl);
  }

  loadLayerStats(name: string): void {
    this.geoserverService.getLayerFeatures(name).subscribe({
      next: (fc) => {
        this.featureCount = fc.features.length;
        this.geometryTypes = [
          ...new Set(fc.features.map((f) => f.geometry?.type).filter(Boolean) as string[]),
        ];
      },
      error: () => {
        this.featureCount = 0;
        this.geometryTypes = [];
      },
    });
  }

  isActiveLayer(layer: Layer): boolean {
    return layer.name === this.selectedLayer;
  }

  isActiveGroup(group: LayerGroup): boolean {
    return group.name === this.selectedGroup;
  }

  // Track by stable identifier so *ngFor reuses DOM nodes even when the source
  // array is recreated on each change detection (avoids DOM rebuild storms).
  trackByKey(index: number, item: { name?: string; value?: string }): string {
    return item?.value || item?.name || String(index);
  }

  openInNewWindow(): void {
    if (this.previewUrl) {
      window.open(this.previewUrl, '_blank');
    }
  }
}
