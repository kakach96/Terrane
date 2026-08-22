import { Component, OnInit } from '@angular/core';
import { ActivatedRoute } from '@angular/router';
import { DomSanitizer, SafeResourceUrl } from '@angular/platform-browser';
import { TranslateService } from '@ngx-translate/core';
import { GeoserverService } from '../../services/geoserver.service';
import { Layer, LayerGroup } from '../../models/geoserver.models';
import { transformBounds } from '../../utils/coords';

interface Bounds {
  minx: number;
  miny: number;
  maxx: number;
  maxy: number;
}

@Component({
  standalone: false,
  selector: 'app-preview',
  templateUrl: './preview.component.html',
  styleUrls: ['./preview.component.scss'],
})
export class PreviewComponent implements OnInit {
  layers: Layer[] = [];
  groups: LayerGroup[] = [];
  previewMode: 'layer' | 'group' = 'layer';
  searchQuery = '';

  selectedLayer = '';
  selectedGroup = '';
  currentLayer: Layer | null = null;
  currentGroup: LayerGroup | null = null;

  previewUrl = '';
  safePreviewUrl: SafeResourceUrl = '';
  loading = true;

  featureCount = 0;
  geometryTypes: string[] = [];

  previewOptions = {
    width: 800,
    height: 600,
    format: 'application/openlayers' as string,
    transparent: true,
    crs: 'EPSG:4326',
  };

  // Built once in the constructor instead of a getter: a getter returning a fresh
  // array + objects on every change detection forced *ngFor (default identity
  // tracking) to rebuild all mat-options on each CD, which combined with the
  // iframe's continuous change detection caused a render storm that froze the page.
  formatOptions: { value: string; label: string }[] = [];

  constructor(
    private route: ActivatedRoute,
    private geoserverService: GeoserverService,
    private sanitizer: DomSanitizer,
    private translate: TranslateService,
  ) {
    this.formatOptions = [
      {
        value: 'application/openlayers',
        label: this.translate.instant('preview.formatOpenLayers'),
      },
      { value: 'image/png', label: this.translate.instant('preview.formatPng') },
      { value: 'image/jpeg', label: this.translate.instant('preview.formatJpeg') },
    ];
  }

  get isStaticFormat(): boolean {
    return this.previewOptions.format !== 'application/openlayers';
  }

  get filteredLayers(): Layer[] {
    const q = this.searchQuery.trim().toLowerCase();
    return this.layers.filter((l) => {
      if (!q) return true;
      return (
        l.name.toLowerCase().includes(q) ||
        (l.title || '').toLowerCase().includes(q) ||
        l.workspace.toLowerCase().includes(q)
      );
    });
  }

  get filteredGroups(): LayerGroup[] {
    const q = this.searchQuery.trim().toLowerCase();
    return this.groups.filter((g) => {
      if (!q) return true;
      return g.name.toLowerCase().includes(q) || (g.title || '').toLowerCase().includes(q);
    });
  }

  get displayBounds(): Bounds | null {
    if (this.previewMode === 'layer') {
      const l = this.currentLayer;
      if (!l) return null;
      return l.native_bounds?.bounds || l.bounds || null;
    }
    const g = this.currentGroup;
    if (!g || g.layers.length === 0) return null;
    const first = this.layers.find((l) => l.name === g.layers[0]);
    if (!first) return null;
    return first.native_bounds?.bounds || first.bounds || null;
  }

  get displayCrs(): string {
    if (this.previewMode === 'layer') {
      const l = this.currentLayer;
      if (!l) return 'EPSG:4326';
      return l.native_bounds?.crs || l.srs || 'EPSG:4326';
    }
    const g = this.currentGroup;
    if (g && g.layers.length > 0) {
      const first = this.layers.find((l) => l.name === g.layers[0]);
      if (first) {
        return first.native_bounds?.crs || first.srs || 'EPSG:4326';
      }
    }
    return 'EPSG:4326';
  }

  ngOnInit(): void {
    const layerParam = this.route.snapshot.queryParamMap.get('layer');
    const groupParam = this.route.snapshot.queryParamMap.get('group');
    if (groupParam) {
      this.previewMode = 'group';
      this.selectedGroup = groupParam;
    } else if (layerParam) {
      this.previewMode = 'layer';
      this.selectedLayer = layerParam;
    }

    this.geoserverService.getLayers().subscribe({
      next: (data) => {
        this.layers = data.filter((l) => l.enabled);
        if (this.previewMode === 'layer') {
          this.selectLayer(this.selectedLayer || (this.layers[0]?.name ?? ''));
        }
        this.loading = false;
      },
      error: () => (this.loading = false),
    });

    this.geoserverService.getLayerGroups().subscribe({
      next: (data) => {
        this.groups = data;
        if (this.previewMode === 'group') {
          this.selectGroup(this.selectedGroup || (this.groups[0]?.name ?? ''));
        }
      },
    });
  }

  selectLayer(name: string): void {
    if (!name) return;
    this.selectedLayer = name;
    const layer = this.layers.find((l) => l.name === name);
    if (!layer) return;
    this.currentLayer = layer;
    this.currentGroup = null;
    this.previewOptions.crs = this.displayCrs;
    this.refreshPreview();
    this.loadLayerStats(name);
  }

  selectGroup(name: string): void {
    if (!name) return;
    this.selectedGroup = name;
    const group = this.groups.find((g) => g.name === name);
    if (!group) return;
    this.currentGroup = group;
    this.currentLayer = null;
    this.featureCount = 0;
    this.geometryTypes = [];
    this.previewOptions.crs = this.displayCrs;
    this.refreshPreview();
  }

  onModeChange(): void {
    this.previewUrl = '';
    this.safePreviewUrl = '';
    this.currentLayer = null;
    this.currentGroup = null;
    if (this.previewMode === 'layer') {
      this.selectLayer(this.selectedLayer || (this.layers[0]?.name ?? ''));
    } else {
      this.selectGroup(this.selectedGroup || (this.groups[0]?.name ?? ''));
    }
  }

  refreshPreview(): void {
    if (this.previewMode === 'layer') {
      if (!this.currentLayer) return;
      const bounds = this.displayBounds;
      if (!bounds) {
        this.previewUrl = '';
        return;
      }
      if (this.previewOptions.format === 'application/openlayers') {
        this.previewUrl = this.geoserverService.getWmsPreviewUrl(this.currentLayer, {
          width: this.previewOptions.width,
          height: this.previewOptions.height,
          crs: this.previewOptions.crs,
          format: 'application/openlayers',
          transparent: true,
        });
      } else {
        this.previewUrl = this.geoserverService.getMapImageUrl(this.currentLayer, {
          width: this.previewOptions.width,
          height: this.previewOptions.height,
          crs: this.previewOptions.crs,
          format: this.previewOptions.format as 'image/png' | 'image/jpeg',
          transparent: this.previewOptions.transparent,
        });
      }
    } else {
      const group = this.currentGroup;
      if (!group || group.layers.length === 0) return;
      const bounds = this.displayBounds;
      if (!bounds) {
        this.previewUrl = '';
        return;
      }
      const nativeCrs = this.displayCrs;
      // WMS 约定 BBOX 处于请求 SRS 下, 切换坐标系时转换图层原生边界
      const converted = transformBounds(bounds, nativeCrs, this.previewOptions.crs);
      const params = new URLSearchParams({
        service: 'WMS',
        version: '1.1.1',
        request: 'GetMap',
        layers: group.layers.join(','),
        srs: this.previewOptions.crs,
        bbox: `${converted.minx},${converted.miny},${converted.maxx},${converted.maxy}`,
        width: this.previewOptions.width.toString(),
        height: this.previewOptions.height.toString(),
        format: this.previewOptions.format,
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
