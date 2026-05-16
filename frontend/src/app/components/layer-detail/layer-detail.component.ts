import { Component, OnInit } from '@angular/core';
import { ActivatedRoute, Router } from '@angular/router';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { Layer, Feature } from '../../models/geoserver.models';

@Component({
  selector: 'app-layer-detail',
  templateUrl: './layer-detail.component.html',
  styleUrls: ['./layer-detail.component.scss']
})
export class LayerDetailComponent implements OnInit {
  layer: Layer | null = null;
  features: Feature[] = [];
  loading = true;
  previewUrl = '';
  showPreview = false;
  previewSize = { width: 800, height: 400 };
  displayedColumns = ['id', 'type', 'coordinates', 'properties', 'actions'];

  constructor(
    private route: ActivatedRoute,
    private router: Router,
    private geoserverService: GeoserverService,
    private notificationService: NotificationService
  ) {}

  ngOnInit(): void {
    const layerName = this.route.snapshot.paramMap.get('name');
    if (layerName) {
      this.loadLayer(layerName);
      this.loadFeatures(layerName);
      this.updatePreviewUrl();
    }
  }

  loadLayer(name: string): void {
    this.geoserverService.getLayer(name).subscribe({
      next: (layer) => {
        this.layer = layer;
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

  updatePreviewUrl(): void {
    const layerName = this.route.snapshot.paramMap.get('name');
    if (layerName) {
      this.previewUrl = this.geoserverService.getPreviewUrl(layerName, {
        width: this.previewSize.width,
        height: this.previewSize.height
      });
    }
  }

  togglePreview(): void {
    this.showPreview = !this.showPreview;
    if (this.showPreview) {
      this.updatePreviewUrl();
    }
  }

  refreshPreview(): void {
    this.updatePreviewUrl();
  }

  deleteFeature(feature: Feature): void {
    if (!this.layer) return;

    if (confirm(`确定要删除要素 "${feature.id}" 吗？`)) {
      this.geoserverService.deleteFeature(this.layer.name, feature.id).subscribe({
        next: () => {
          this.notificationService.success('要素删除成功');
          this.loadFeatures(this.layer!.name);
        },
        error: (error) => {
          this.notificationService.error('删除失败');
        }
      });
    }
  }

  goBack(): void {
    this.router.navigate(['/layers']);
  }

  getCoordinatesDisplay(feature: Feature): string {
    const coords = feature.geometry.coordinates;
    if (Array.isArray(coords[0])) {
      return `${(coords[0] as number[]).slice(0, 2).join(', ')}...`;
    }
    return (coords as number[]).slice(0, 2).join(', ');
  }

  getPropertyKeys(feature: Feature): string[] {
    return Object.keys(feature.properties).slice(0, 3);
  }
}
