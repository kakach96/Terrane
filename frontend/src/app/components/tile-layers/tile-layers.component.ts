import { Component, ChangeDetectionStrategy, inject, signal } from '@angular/core';
import { toSignal, toObservable } from '@angular/core/rxjs-interop';
import { TranslateService } from '@ngx-translate/core';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { TileCacheStats, Layer } from '../../models/geoserver.models';
import { switchMap, tap, map, startWith, catchError, of } from 'rxjs';

@Component({
  standalone: false,
  selector: 'app-tile-layers',
  templateUrl: './tile-layers.component.html',
  styleUrls: ['./tile-layers.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class TileLayersComponent {
  private geoserverService = inject(GeoserverService);
  private notificationService = inject(NotificationService);
  private translate = inject(TranslateService);

  selectedLayer = '';
  private refreshTrigger = signal(0);
  loading = signal(false);

  // ── Signal pipelines ──────────────────────────────────────────────
  private cacheStats$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    tap(() => this.loading.set(true)),
    switchMap(() =>
      this.geoserverService.getTileCacheStats().pipe(
        catchError(() => of(null as TileCacheStats | null)),
      ),
    ),
    tap(() => this.loading.set(false)),
  );

  cacheStats = toSignal(this.cacheStats$, { initialValue: null as TileCacheStats | null });

  private availableLayers$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    switchMap(() =>
      this.geoserverService.getLayers().pipe(catchError(() => of([] as Layer[]))),
    ),
  );

  availableLayers = toSignal(
    this.availableLayers$.pipe(map((layers) => layers.map((l) => l.name))),
    { initialValue: [] as string[] },
  );

  // ── Actions ───────────────────────────────────────────────────────
  clearAllCache(): void {
    this.notificationService
      .confirm(
        this.translate.instant('tileLayers.confirmClearTitle'),
        this.translate.instant('tileLayers.confirmClearMessage'),
      )
      .subscribe((confirmed) => {
        if (confirmed) {
          this.loading.set(true);
          const layers = this.availableLayers();
          const clearNext = (idx: number) => {
            if (idx >= layers.length) {
              this.loading.set(false);
              this.notificationService.success(
                this.translate.instant('tileLayers.clearAllSuccess'),
              );
              this.refreshTrigger.update((v) => v + 1);
              return;
            }
            this.geoserverService.clearTileCache(layers[idx]).subscribe({
              next: () => clearNext(idx + 1),
              error: () => clearNext(idx + 1),
            });
          };
          clearNext(0);
        }
      });
  }

  clearLayerCache(): void {
    if (!this.selectedLayer) return;
    this.loading.set(true);
    this.geoserverService.clearTileCache(this.selectedLayer).subscribe({
      next: (res) => {
        this.notificationService.success(
          res.message || this.translate.instant('tileLayers.cacheCleared'),
        );
        this.refreshTrigger.update((v) => v + 1);
        this.loading.set(false);
      },
      error: () => {
        this.notificationService.error(this.translate.instant('tileLayers.clearFail'));
        this.loading.set(false);
      },
    });
  }

  refreshStats(): void {
    this.refreshTrigger.update((v) => v + 1);
  }

  trackByIndex(index: number): number {
    return index;
  }
}