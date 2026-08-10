import { Component, OnInit } from '@angular/core';
import { ActivatedRoute, Router } from '@angular/router';
import { DomSanitizer, SafeResourceUrl } from '@angular/platform-browser';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { Layer } from '../../models/geoserver.models';

@Component({
  selector: 'app-layer-detail',
  templateUrl: './layer-detail.component.html',
  styleUrls: ['./layer-detail.component.scss'],
})
export class LayerDetailComponent implements OnInit {
  layer: Layer | null = null;
  featureCount = 0;
  previewUrl = '';
  safePreviewUrl: SafeResourceUrl = '';
  styleNames: string[] = [];
  currentStyleName = '';
  loading = true;

  /** 显示的边界（优先 native_bounds，回退到 bounds） */
  get displayBounds(): { minx: number; miny: number; maxx: number; maxy: number } {
    const b = this.layer?.native_bounds?.bounds || this.layer?.bounds;
    return b || { minx: -180, miny: -90, maxx: 180, maxy: 90 };
  }

  /** 显示的坐标系 */
  get displayCrs(): string {
    return this.layer?.native_bounds?.crs || this.layer?.srs || 'EPSG:4326';
  }

  /** 是否为默认世界范围 */
  get isDefaultBounds(): boolean {
    const b = this.displayBounds;
    const is4326Default = b.minx === -180 && b.miny === -90 && b.maxx === 180 && b.maxy === 90;
    const is3857Default =
      Math.abs(b.minx - -20037508.34) < 0.01 &&
      Math.abs(b.miny - -20037508.34) < 0.01 &&
      Math.abs(b.maxx - 20037508.34) < 0.01 &&
      Math.abs(b.maxy - 20037508.34) < 0.01;
    return is4326Default || is3857Default;
  }

  previewOptions = {
    width: 800,
    height: 400,
    format: 'application/openlayers' as string,
    transparent: true,
    crs: 'EPSG:4326',
  };

  previewFormats = [
    { value: 'application/openlayers', label: 'OpenLayers 交互地图' },
    { value: 'image/png', label: 'PNG (透明)' },
    { value: 'image/jpeg', label: 'JPEG' },
  ];

  previewCrsOptions = ['EPSG:4326', 'EPSG:3857', 'EPSG:4490'];

  get isStaticPreview(): boolean {
    return this.previewOptions.format !== 'application/openlayers';
  }

  constructor(
    private route: ActivatedRoute,
    private router: Router,
    private sanitizer: DomSanitizer,
    private geoserverService: GeoserverService,
    private notificationService: NotificationService,
  ) {}

  ngOnInit(): void {
    const layerName = this.route.snapshot.paramMap.get('name');
    if (layerName) {
      this.loadLayer(layerName);
      this.loadFeatures(layerName);
      this.loadStyleNames();
    }
  }

  loadLayer(name: string): void {
    this.geoserverService.getLayer(name).subscribe({
      next: (layer) => {
        this.layer = layer;
        this.currentStyleName = layer.styles?.[0]?.name || 'default';
        this.previewOptions.crs = this.displayCrs;
        this.refreshPreview();
        this.loading = false;
      },
      error: () => {
        this.notificationService.error('加载图层失败');
        this.loading = false;
      },
    });
  }

  refreshPreview(): void {
    if (!this.layer) return;
    if (this.previewOptions.format === 'application/openlayers') {
      this.previewUrl = this.geoserverService.getWmsPreviewUrl(this.layer, {
        width: this.previewOptions.width,
        height: this.previewOptions.height,
        crs: this.previewOptions.crs,
        format: 'application/openlayers',
        transparent: true,
      });
    } else {
      const bbox = `${this.displayBounds.minx},${this.displayBounds.miny},${this.displayBounds.maxx},${this.displayBounds.maxy}`;
      this.previewUrl = this.geoserverService.getMapImageUrl(this.layer, {
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

  loadFeatures(layerName: string): void {
    this.geoserverService.getLayerFeatures(layerName).subscribe({
      next: (collection) => {
        this.featureCount = collection.features.length;
      },
      error: () => (this.featureCount = 0),
    });
  }

  loadStyleNames(): void {
    this.geoserverService.getStyles().subscribe({
      next: (data) => {
        this.styleNames = data.map((s) => s.name);
      },
    });
  }

  onStyleChange(styleName: string): void {
    if (!this.layer) return;
    this.geoserverService.getStyle(styleName).subscribe({
      next: (style) => {
        if (style.content) {
          this.geoserverService.updateLayerStyle(this.layer!.name, style.content).subscribe({
            next: () => {
              this.currentStyleName = styleName;
              this.notificationService.success(`样式已切换为 ${style.title}`);
              this.refreshPreview();
            },
            error: () => this.notificationService.error('样式切换失败'),
          });
        }
      },
      error: () => this.notificationService.error('加载样式失败'),
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
    if (!this.layer) return;
    this.geoserverService.exportFeaturesGeoJson(this.layer.name).subscribe({
      next: (blob) => {
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `${this.layer!.name}.geojson`;
        a.click();
        window.URL.revokeObjectURL(url);
        this.notificationService.success('GeoJSON 已下载');
      },
      error: () => this.notificationService.error('下载 GeoJSON 失败'),
    });
  }

  downloadCsv(): void {
    if (!this.layer) return;
    this.geoserverService.exportFeaturesCsv(this.layer.name).subscribe({
      next: (blob) => {
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `${this.layer!.name}.csv`;
        a.click();
        window.URL.revokeObjectURL(url);
        this.notificationService.success('CSV 已下载');
      },
      error: () => this.notificationService.error('下载 CSV 失败'),
    });
  }

  openInNewWindow(): void {
    if (this.previewUrl) {
      window.open(this.previewUrl, '_blank');
    }
  }
}
