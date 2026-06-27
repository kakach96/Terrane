import { Component, OnInit } from '@angular/core';
import { DomSanitizer, SafeResourceUrl } from '@angular/platform-browser';
import { GeoserverService } from '../../services/geoserver.service';
import { Layer, LayerGroup } from '../../models/geoserver.models';

@Component({
  selector: 'app-preview',
  templateUrl: './preview.component.html',
  styleUrls: ['./preview.component.scss']
})
export class PreviewComponent implements OnInit {
  layers: Layer[] = [];
  groups: LayerGroup[] = [];
  selectedLayer: string = '';
  selectedGroup: string = '';
  mapUrl: SafeResourceUrl = '';
  previewMode: 'layer' | 'group' = 'layer';

  previewOptions = {
    width: 800,
    height: 500,
    format: 'image/png' as string,
    transparent: true,
    crs: 'EPSG:4326'
  };

  formatOptions = [
    { value: 'image/png', label: 'PNG (透明)' },
    { value: 'image/jpeg', label: 'JPEG' },
    { value: 'application/openlayers', label: 'OpenLayers 交互地图' }
  ];

  crsOptions = ['EPSG:4326', 'EPSG:3857', 'EPSG:4269', 'EPSG:900913'];

  constructor(
    private geoserverService: GeoserverService,
    private sanitizer: DomSanitizer
  ) {}

  ngOnInit(): void {
    this.geoserverService.getLayers().subscribe({
      next: (data) => {
        this.layers = data.filter(l => l.enabled);
        if (this.layers.length > 0) {
          this.selectedLayer = this.layers[0].name;
          this.refreshPreview();
        }
      }
    });
    this.geoserverService.getLayerGroups().subscribe({
      next: (data) => this.groups = data
    });
  }

  refreshPreview(): void {
    if (this.previewMode === 'layer' && this.selectedLayer) {
      const layer = this.layers.find(l => l.name === this.selectedLayer);
      if (!layer) return;

      if (this.previewOptions.format === 'application/openlayers') {
        const bounds = (layer as any).native_bounds?.bounds || layer.bounds;
        if (!bounds) return;
        const crs = (layer as any).native_bounds?.crs || layer.srs || 'EPSG:4326';
        // 使用 VERSION=1.1.1 避免 WMS 1.3.0 的 EPSG:4326 轴序问题
        const url = `/wms?service=WMS&version=1.1.1&request=GetMap&layers=${layer.name}&srs=${crs}&bbox=${bounds.minx},${bounds.miny},${bounds.maxx},${bounds.maxy}&width=${this.previewOptions.width}&height=${this.previewOptions.height}&format=application/openlayers`;
        this.mapUrl = this.sanitizer.bypassSecurityTrustResourceUrl(url);
      } else {
        const url = this.geoserverService.getMapImageUrl(layer, {
          width: this.previewOptions.width,
          height: this.previewOptions.height,
          format: this.previewOptions.format as 'image/png' | 'image/jpeg',
          transparent: this.previewOptions.transparent,
          crs: this.previewOptions.crs,
        });
        this.mapUrl = this.sanitizer.bypassSecurityTrustResourceUrl(url);
      }
    } else if (this.previewMode === 'group' && this.selectedGroup) {
      // 图层组预览：使用第一个图层的边界
      const group = this.groups.find(g => g.name === this.selectedGroup);
      if (!group || group.layers.length === 0) return;
      const firstLayer = this.layers.find(l => l.name === group.layers[0]);
      if (!firstLayer) return;
      const bounds = (firstLayer as any).native_bounds?.bounds || firstLayer.bounds;
      if (!bounds) return;
      const crs = (firstLayer as any).native_bounds?.crs || firstLayer.srs || 'EPSG:4326';
      const layersParam = group.layers.join(',');
      const url = `/wms?service=WMS&version=1.3.0&request=GetMap&layers=${layersParam}&crs=${crs}&bbox=${bounds.minx},${bounds.miny},${bounds.maxx},${bounds.maxy}&width=${this.previewOptions.width}&height=${this.previewOptions.height}&format=${this.previewOptions.format}`;
      this.mapUrl = this.sanitizer.bypassSecurityTrustResourceUrl(url);
    }
  }
}
