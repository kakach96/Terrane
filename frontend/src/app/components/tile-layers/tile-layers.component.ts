import { Component, OnInit } from '@angular/core';
import { TranslateService } from '@ngx-translate/core';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { TileCacheStats } from '../../models/geoserver.models';

@Component({
  selector: 'app-tile-layers',
  templateUrl: './tile-layers.component.html',
  styleUrls: ['./tile-layers.component.scss'],
})
export class TileLayersComponent implements OnInit {
  cacheStats: TileCacheStats | null = null;
  loading = false;
  selectedLayer = '';
  availableLayers: string[] = [];

  constructor(
    private geoserverService: GeoserverService,
    private notificationService: NotificationService,
    private translate: TranslateService,
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
      },
    });
  }

  loadLayers(): void {
    this.geoserverService.getLayers().subscribe({
      next: (layers) => {
        this.availableLayers = layers.map((l) => l.name);
      },
      error: () => {},
    });
  }

  clearAllCache(): void {
    this.notificationService
      .confirm(
        this.translate.instant('tileLayers.confirmClearTitle'),
        this.translate.instant('tileLayers.confirmClearMessage'),
      )
      .subscribe((confirmed) => {
        if (confirmed) {
          this.loading = true;
          // 遍历所有图层逐个清除
          const clearNext = (idx: number) => {
            if (idx >= this.availableLayers.length) {
              this.loading = false;
              this.notificationService.success(
                this.translate.instant('tileLayers.clearAllSuccess'),
              );
              this.loadCacheStats();
              return;
            }
            this.geoserverService.clearTileCache(this.availableLayers[idx]).subscribe({
              next: () => {
                clearNext(idx + 1);
              },
              error: () => {
                clearNext(idx + 1);
              },
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
        this.notificationService.success(
          res.message || this.translate.instant('tileLayers.cacheCleared'),
        );
        this.loadCacheStats();
        this.loading = false;
      },
      error: () => {
        this.notificationService.error(this.translate.instant('tileLayers.clearFail'));
        this.loading = false;
      },
    });
  }
}
