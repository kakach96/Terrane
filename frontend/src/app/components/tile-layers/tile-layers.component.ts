import { Component, OnInit } from '@angular/core';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';

interface CacheStats {
  enabled: boolean;
  hits: number;
  misses: number;
  hitRate: number;
  totalTiles: number;
  cacheSizeBytes: number;
  cacheSizeMb?: string;
}

@Component({
  selector: 'app-tile-layers',
  templateUrl: './tile-layers.component.html',
  styleUrls: ['./tile-layers.component.scss']
})
export class TileLayersComponent implements OnInit {
  cacheStats: CacheStats | null = null;
  loading = false;
  selectedLayer = '';
  availableLayers: string[] = [];

  constructor(
    private geoserverService: GeoserverService,
    private notificationService: NotificationService,
  ) {}

  ngOnInit(): void {
    this.loadCacheStats();
    this.loadLayers();
  }

  loadCacheStats(): void {
    this.loading = true;
    this.geoserverService.getTileCacheStats().subscribe({
      next: (stats) => {
        this.cacheStats = stats;
        this.loading = false;
      },
      error: () => {
        this.loading = false;
      }
    });
  }

  loadLayers(): void {
    this.geoserverService.getLayers().subscribe({
      next: (layers) => {
        this.availableLayers = layers.map(l => l.name);
      },
      error: () => {}
    });
  }

  clearAllCache(): void {
    this.notificationService.confirm('确认清除', '确定要清除所有瓦片缓存吗？此操作不可恢复。').subscribe((confirmed) => {
      if (confirmed) {
        this.loading = true;
        // 遍历所有图层逐个清除
        let count = 0;
        const clearNext = (idx: number) => {
          if (idx >= this.availableLayers.length) {
            this.loading = false;
            this.notificationService.success(`已清除所有缓存`);
            this.loadCacheStats();
            return;
          }
          this.geoserverService.clearTileCache(this.availableLayers[idx]).subscribe({
            next: (res) => { count += res.cleared || 0; clearNext(idx + 1); },
            error: () => { clearNext(idx + 1); }
          });
        };
        clearNext(0);
      }
    });
  }

  clearLayerCache(): void {
    if (!this.selectedLayer) return;
    this.loading = true;
    this.geoserverService.clearTileCache(this.selectedLayer).subscribe({
      next: (res) => {
        this.notificationService.success(res.message || '缓存已清除');
        this.loadCacheStats();
        this.loading = false;
      },
      error: (err) => {
        this.notificationService.error('清除缓存失败');
        this.loading = false;
      }
    });
  }
}
