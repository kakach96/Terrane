import { Component, ChangeDetectionStrategy, computed, inject, signal } from '@angular/core';
import { ActivatedRoute, Router } from '@angular/router';
import { DomSanitizer, SafeResourceUrl } from '@angular/platform-browser';
import { TranslateService } from '@ngx-translate/core';
import { toSignal, toObservable } from '@angular/core/rxjs-interop';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { LanguageService } from '../../services/language.service';
import { Layer, FeatureCollection, StyleInfo, DataSource } from '../../models/geoserver.models';
import { switchMap, tap, filter, map, startWith, catchError, of } from 'rxjs';

@Component({
  standalone: false,
  selector: 'app-layer-detail',
  templateUrl: './layer-detail.component.html',
  styleUrls: ['./layer-detail.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class LayerDetailComponent {
  private route = inject(ActivatedRoute);
  private router = inject(Router);
  private sanitizer = inject(DomSanitizer);
  private geoserverService = inject(GeoserverService);
  private notificationService = inject(NotificationService);
  private translate = inject(TranslateService);
  private languageService = inject(LanguageService);

  previewUrl = '';
  safePreviewUrl: SafeResourceUrl = '';
  currentStyleName = '';
  currentCacheStore = '';
  private refreshTrigger = signal(0);

  previewOptions = {
    width: 800,
    height: 400,
    format: 'application/openlayers' as string,
    transparent: true,
    crs: 'EPSG:4326',
  };

  // Computed so labels re-translate on language switch. Reading currentLang()
  // makes the computed re-evaluate when the language changes; the array is
  // memoized so *ngFor does not rebuild mat-option nodes on every CD cycle.
  previewFormats = computed<{ value: string; label: string }[]>(() => {
    this.languageService.currentLang();
    return [
      {
        value: 'application/openlayers',
        label: this.translate.instant('layerDetail.formatOpenLayers'),
      },
      { value: 'image/png', label: this.translate.instant('layerDetail.formatPng') },
      { value: 'image/jpeg', label: this.translate.instant('layerDetail.formatJpeg') },
    ];
  });

  previewCrsOptions = ['EPSG:4326', 'EPSG:3857', 'EPSG:4490'];

  /** Layer name from route – read once. */
  private layerName = this.route.snapshot.paramMap.get('name') ?? '';

  // ── Signal pipelines ──────────────────────────────────────────────
  private layer$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    filter(() => !!this.layerName),
    switchMap(() =>
      this.geoserverService.getLayer(this.layerName).pipe(
        catchError(() => {
          this.notificationService.error(this.translate.instant('layerDetail.loadFail'));
          return of(null as Layer | null);
        }),
      ),
    ),
    tap((layer) => {
      if (layer) {
        this.currentStyleName = layer.styles?.[0]?.name || 'default';
        this.currentCacheStore = layer.cache_store || '';
        this.previewOptions.crs = this.displayCrs();
        this.refreshPreview();
      }
    }),
  );

  layer = toSignal(this.layer$, { initialValue: null as Layer | null });

  private featureCount$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    filter(() => !!this.layerName),
    switchMap(() =>
      this.geoserverService.getLayerFeatures(this.layerName).pipe(
        catchError(() => of({ type: 'FeatureCollection', features: [] } as FeatureCollection)),
      ),
    ),
  );

  featureCount = toSignal(
    this.featureCount$.pipe(map((c) => c.features?.length ?? 0)),
    { initialValue: 0 },
  );

  private styleNames$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    switchMap(() =>
      this.geoserverService.getStyles().pipe(
        catchError(() => of([] as StyleInfo[])),
      ),
    ),
  );

  styleNames = toSignal(
    this.styleNames$.pipe(map((data) => data.map((s) => s.name))),
    { initialValue: [] as string[] },
  );

  private redisCacheSources$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    switchMap(() =>
      this.geoserverService.getDataSources().pipe(
        catchError(() => of([] as DataSource[])),
      ),
    ),
  );

  redisCacheSources = toSignal(
    this.redisCacheSources$.pipe(
      map((sources) => sources.filter((s) => s.type === 'redis' && s.enabled)),
    ),
    { initialValue: [] as DataSource[] },
  );

  // ── Computed signals ──────────────────────────────────────────────
  /** Display bounds (prefer native_bounds, fallback to bounds) */
  displayBounds = computed(() => {
    const l = this.layer();
    const b = l?.native_bounds?.bounds || l?.bounds;
    return b || { minx: -180, miny: -90, maxx: 180, maxy: 90 };
  });

  /** Display CRS */
  displayCrs = computed(() => {
    const l = this.layer();
    return l?.native_bounds?.crs || l?.srs || 'EPSG:4326';
  });

  /** Whether the bounds are the default world extent */
  isDefaultBounds = computed(() => {
    const b = this.displayBounds();
    const is4326Default = b.minx === -180 && b.miny === -90 && b.maxx === 180 && b.maxy === 90;
    const is3857Default =
      Math.abs(b.minx - -20037508.34) < 0.01 &&
      Math.abs(b.miny - -20037508.34) < 0.01 &&
      Math.abs(b.maxx - 20037508.34) < 0.01 &&
      Math.abs(b.maxy - 20037508.34) < 0.01;
    return is4326Default || is3857Default;
  });

  /** Whether the preview format is a static image (not OpenLayers iframe) */
  isStaticPreview = computed(() => this.previewOptions.format !== 'application/openlayers');

  // ── Imperative methods ────────────────────────────────────────────
  refreshPreview(): void {
    const l = this.layer();
    if (!l) return;
    if (this.previewOptions.format === 'application/openlayers') {
      this.previewUrl = this.geoserverService.getWmsPreviewUrl(l, {
        width: this.previewOptions.width,
        height: this.previewOptions.height,
        crs: this.previewOptions.crs,
        format: 'application/openlayers',
        transparent: true,
      });
    } else {
      const b = this.displayBounds();
      const bbox = `${b.minx},${b.miny},${b.maxx},${b.maxy}`;
      this.previewUrl = this.geoserverService.getMapImageUrl(l, {
        width: this.previewOptions.width,
        height: this.previewOptions.height,
        format: this.previewOptions.format as 'image/png' | 'image/jpeg',
        transparent: this.previewOptions.transparent,
        crs: this.previewOptions.crs,
        bbox,
      });
    }
    this.safePreviewUrl = this.sanitizer.bypassSecurityTrustResourceUrl(this.previewUrl);
  }

  updatePreviewParam(param: string, value: number | boolean | string): void {
    (this.previewOptions as Record<string, number | boolean | string>)[param] = value;
    this.refreshPreview();
  }

  /** Toggle tile cache backend (Redis data source / default in-memory) */
  onCacheStoreChange(cacheStore: string): void {
    const l = this.layer();
    if (!l) return;
    const value = cacheStore || null;
    this.geoserverService.updateLayer(l.name, { cache_store: value }).subscribe({
      next: () => {
        this.currentCacheStore = cacheStore;
        this.notificationService.success(
          this.translate.instant('layerDetail.cacheStoreSuccess'),
        );
      },
      error: () =>
        this.notificationService.error(this.translate.instant('layerDetail.cacheStoreFail')),
    });
  }

  /** Switch layer style (nested subscribe: getStyle → updateLayerStyle) */
  onStyleChange(styleName: string): void {
    const l = this.layer();
    if (!l) return;
    this.geoserverService.getStyle(styleName).subscribe({
      next: (style) => {
        if (style.content) {
          this.geoserverService.updateLayerStyle(l.name, style.content).subscribe({
            next: () => {
              this.currentStyleName = styleName;
              this.notificationService.success(
                this.translate.instant('layerDetail.styleSwitchSuccess', { title: style.title }),
              );
              this.refreshPreview();
            },
            error: () =>
              this.notificationService.error(this.translate.instant('layerDetail.styleSwitchFail')),
          });
        }
      },
      error: () =>
        this.notificationService.error(this.translate.instant('layerDetail.styleLoadFail')),
    });
  }

  onPreviewError(): void {
    console.warn('地图预览加载失败，可能图层暂无数据');
  }

  onTransparentChange(event: { checked: boolean }): void {
    this.previewOptions.transparent = event.checked;
    this.refreshPreview();
  }

  goBack(): void {
    this.router.navigate(['/layers']);
  }

  downloadGeoJson(): void {
    const l = this.layer();
    if (!l) return;
    this.geoserverService.exportFeaturesGeoJson(l.name).subscribe({
      next: (blob) => {
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `${l.name}.geojson`;
        a.click();
        window.URL.revokeObjectURL(url);
        this.notificationService.success(
          this.translate.instant('layerDetail.downloadGeoJsonSuccess'),
        );
      },
      error: () =>
        this.notificationService.error(this.translate.instant('layerDetail.downloadGeoJsonFail')),
    });
  }

  downloadCsv(): void {
    const l = this.layer();
    if (!l) return;
    this.geoserverService.exportFeaturesCsv(l.name).subscribe({
      next: (blob) => {
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `${l.name}.csv`;
        a.click();
        window.URL.revokeObjectURL(url);
        this.notificationService.success(this.translate.instant('layerDetail.downloadCsvSuccess'));
      },
      error: () =>
        this.notificationService.error(this.translate.instant('layerDetail.downloadCsvFail')),
    });
  }

  openInNewWindow(): void {
    if (this.previewUrl) {
      window.open(this.previewUrl, '_blank');
    }
  }

  trackByIndex(index: number): number {
    return index;
  }
}
