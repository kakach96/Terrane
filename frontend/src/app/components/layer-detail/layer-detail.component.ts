import { Component, OnInit } from '@angular/core';
import { ActivatedRoute, Router } from '@angular/router';
import { DomSanitizer, SafeResourceUrl } from '@angular/platform-browser';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { Layer, Feature, PropertyDef, StyleInfo } from '../../models/geoserver.models';

@Component({
  selector: 'app-layer-detail',
  templateUrl: './layer-detail.component.html',
  styleUrls: ['./layer-detail.component.scss']
})
export class LayerDetailComponent implements OnInit {
  layer: Layer | null = null;
  features: Feature[] = [];
  properties: PropertyDef[] = [];
  previewUrl = '';
  safePreviewUrl: SafeResourceUrl = '';
  styleNames: string[] = [];
  currentStyleName = '';
  loading = true;

  previewOptions = {
    width: 800,
    height: 400,
    format: 'image/png' as 'image/png' | 'image/jpeg',
    transparent: true,
    crs: 'EPSG:4326'
  };

  previewFormats = [
    { value: 'image/png', label: 'PNG (透明)' },
    { value: 'image/jpeg', label: 'JPEG' }
  ];

  previewCrsOptions = ['EPSG:4326', 'EPSG:3857'];

  constructor(
    private route: ActivatedRoute,
    private router: Router,
    private sanitizer: DomSanitizer,
    private geoserverService: GeoserverService,
    private notificationService: NotificationService
  ) {}

  ngOnInit(): void {
    const layerName = this.route.snapshot.paramMap.get('name');
    if (layerName) {
      this.loadLayer(layerName);
      this.loadFeatures(layerName);
      this.loadFeatureType(layerName);
      this.loadStyleNames();
    }
  }

  loadLayer(name: string): void {
    this.geoserverService.getLayer(name).subscribe({
      next: (layer) => {
        this.layer = layer;
        this.currentStyleName = layer.styles?.[0]?.name || 'default';
        this.previewOptions.crs = (layer as any).native_bounds?.crs || layer.srs || 'EPSG:4326';
        this.refreshPreview();
        this.loading = false;
      },
      error: (error) => {
        this.notificationService.error('加载图层失败');
        this.loading = false;
      }
    });
  }

  refreshPreview(): void {
    if (!this.layer) return;
    this.previewUrl = this.geoserverService.getMapImageUrl(this.layer, {
      width: this.previewOptions.width,
      height: this.previewOptions.height,
      format: this.previewOptions.format,
      transparent: this.previewOptions.transparent,
      crs: this.previewOptions.crs,
    });
    this.safePreviewUrl = this.sanitizer.bypassSecurityTrustResourceUrl(this.previewUrl);
  }

  updatePreviewParam(param: string, value: number | boolean | string): void {
    (this.previewOptions as Record<string, number | boolean | string>)[param] = value;
    this.refreshPreview();
  }

  loadFeatures(layerName: string): void {
    this.geoserverService.getLayerFeatures(layerName).subscribe({
      next: (collection) => {
        this.features = collection.features;
      }
    });
  }

  loadFeatureType(layerName: string): void {
    this.geoserverService.getLayerFeatureType(layerName).subscribe({
      next: (props) => {
        this.properties = props;
      },
      error: () => {
        if (this.features.length > 0) {
          this.properties = this.deriveProperties(this.features);
        }
      }
    });
  }

  loadStyleNames(): void {
    this.geoserverService.getStyles().subscribe({
      next: (data) => {
        this.styleNames = data.map(s => s.name);
      }
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
            error: () => this.notificationService.error('样式切换失败')
          });
        }
      },
      error: () => this.notificationService.error('加载样式失败')
    });
  }

  deriveProperties(features: Feature[]): PropertyDef[] {
    const keyTypes = new Map<string, string>();
    for (const feature of features) {
      for (const [key, value] of Object.entries(feature.properties)) {
        if (!keyTypes.has(key)) {
          keyTypes.set(key, this.inferType(value));
        }
      }
    }
    return Array.from(keyTypes.entries()).map(([name, type]) => ({ name, type, nullable: true }));
  }

  inferType(value: any): string {
    if (value === null || value === undefined) return 'string';
    if (typeof value === 'number') return Number.isInteger(value) ? 'integer' : 'float';
    if (typeof value === 'boolean') return 'boolean';
    if (typeof value === 'string') {
      if (/^\d{4}-\d{2}-\d{2}/.test(value)) return 'date';
      return 'string';
    }
    return typeof value;
  }

  getGeometryTypes(): string[] {
    return [...new Set(this.features.map(f => f.geometry.type))];
  }

  onPreviewError(): void {
    console.warn('地图预览加载失败，可能图层暂无数据');
  }

  onTransparentChange(event: any): void {
    this.previewOptions.transparent = event.checked;
    this.refreshPreview();
  }

  goBack(): void {
    this.router.navigate(['/layers']);
  }
}
