import { Component, OnInit } from '@angular/core';
import { ActivatedRoute, Router } from '@angular/router';
import { DomSanitizer, SafeResourceUrl } from '@angular/platform-browser';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { Layer, Feature, PropertyDef } from '../../models/geoserver.models';

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
  loading = true;

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
    }
  }

  loadLayer(name: string): void {
    this.geoserverService.getLayer(name).subscribe({
      next: (layer) => {
        this.layer = layer;
        this.previewUrl = this.geoserverService.getWmsPreviewUrl(layer, 800, 400);
        this.safePreviewUrl = this.sanitizer.bypassSecurityTrustResourceUrl(this.previewUrl);
        this.loading = false;
      },
      error: (error) => {
        this.notificationService.error('加载图层失败');
        this.loading = false;
      }
    });
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

  goBack(): void {
    this.router.navigate(['/layers']);
  }
}
