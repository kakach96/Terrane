import {
  Component,
  ChangeDetectionStrategy,
  OnDestroy,
  computed,
  inject,
  signal,
} from '@angular/core';
import { toSignal, toObservable } from '@angular/core/rxjs-interop';
import { MatDialog } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
import { TerraneService } from '../../services/terrane.service';
import { NotificationService } from '../../services/notification.service';
import { Layer, SeedJob } from '../../models/terrane.models';
import {
  SeedJobDialogComponent,
  SeedJobDialogResult,
} from '../seed-job-dialog/seed-job-dialog.component';
import { ConfirmDialogComponent } from '../../shared/components/confirm-dialog.component';
import { switchMap, tap, startWith, catchError, of, interval } from 'rxjs';

@Component({
  standalone: false,
  selector: 'app-tile-layers',
  templateUrl: './tile-layers.component.html',
  styleUrls: ['./tile-layers.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class TileLayersComponent implements OnDestroy {
  private terraneService = inject(TerraneService);
  private notificationService = inject(NotificationService);
  private dialog = inject(MatDialog);
  private translate = inject(TranslateService);

  /** Cached layers table columns (GeoServer CachedLayersPage style). */
  displayedColumns = ['layer', 'cache', 'gridsets', 'enabled', 'actions'];
  /** Seed jobs table columns. */
  jobColumns = ['layer', 'gridset', 'zoom', 'format', 'status', 'progress', 'actions'];

  searchQuery = signal('');
  private refreshTrigger = signal(0);
  loading = signal(false);

  // ── Layers ───────────────────────────────────────────────────────
  private layers$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    tap(() => this.loading.set(true)),
    switchMap(() => this.terraneService.getLayers().pipe(catchError(() => of([] as Layer[])))),
    tap(() => this.loading.set(false)),
  );
  layers = toSignal(this.layers$, { initialValue: [] as Layer[] });

  filteredLayers = computed(() => {
    const q = this.searchQuery().toLowerCase();
    return this.layers().filter(
      (l) =>
        !this.searchQuery() ||
        l.name.toLowerCase().includes(q) ||
        l.title.toLowerCase().includes(q),
    );
  });

  // ── Seed jobs (auto-refresh while any job is running) ────────────
  private seedJobs$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    switchMap(() =>
      this.terraneService.getSeedJobs().pipe(catchError(() => of([] as SeedJob[]))),
    ),
  );
  seedJobs = toSignal(this.seedJobs$, { initialValue: [] as SeedJob[] });

  hasRunningJobs = computed(() =>
    this.seedJobs().some((j) => j.status === 'Pending' || j.status === 'Running'),
  );

  /** Poll every 2s while a job is running so progress stays live. */
  private autoRefresh$ = toObservable(this.hasRunningJobs).pipe(
    switchMap((running) => (running ? interval(2000) : of(0))),
    tap(() => this.refreshTrigger.update((v) => v + 1)),
  );
  private autoRefreshSub = this.autoRefresh$.subscribe();

  ngOnDestroy(): void {
    this.autoRefreshSub.unsubscribe();
  }

  // ── Actions ──────────────────────────────────────────────────────
  openSeedDialog(layer?: Layer): void {
    const dialogRef = this.dialog.open(SeedJobDialogComponent, {
      width: '480px',
      data: {
        layer: layer?.name || '',
        layers: this.layers().map((l) => l.name),
      },
    });
    dialogRef.afterClosed().subscribe((result: SeedJobDialogResult | undefined) => {
      if (!result) return;
      this.runSeedJob(result);
    });
  }

  private runSeedJob(result: SeedJobDialogResult): void {
    const { layer, operation, gridset, z_min, z_max, format } = result;

    if (operation === 'truncate') {
      this.terraneService.truncateTileCache(layer, gridset).subscribe({
        next: (res) => {
          this.notificationService.success(
            res.message || this.translate.instant('tileLayers.truncateSuccess'),
          );
          this.refreshTrigger.update((v) => v + 1);
        },
        error: () =>
          this.notificationService.error(this.translate.instant('tileLayers.truncateFail')),
      });
      return;
    }

    // seed / reseed → POST /tiles/seed (reseed truncates first)
    const doSeed = () => {
      this.terraneService.createSeedJob({ layer, gridset, z_min, z_max, format }).subscribe({
        next: (res) => {
          this.notificationService.success(
            res.message || this.translate.instant('tileLayers.seedStarted'),
          );
          this.refreshTrigger.update((v) => v + 1);
        },
        error: () => this.notificationService.error(this.translate.instant('tileLayers.seedFail')),
      });
    };

    if (operation === 'reseed') {
      this.terraneService.truncateTileCache(layer, gridset).subscribe({
        next: () => doSeed(),
        error: () => doSeed(),
      });
    } else {
      doSeed();
    }
  }

  truncateLayer(layer: Layer): void {
    this.dialog
      .open(ConfirmDialogComponent, {
        width: '400px',
        data: {
          title: this.translate.instant('tileLayers.confirmTruncateTitle'),
          message: this.translate.instant('tileLayers.confirmTruncateMessage', {
            name: layer.name,
          }),
        },
      })
      .afterClosed()
      .subscribe((confirmed) => {
        if (!confirmed) return;
        this.terraneService.truncateTileCache(layer.name).subscribe({
          next: (res) => {
            this.notificationService.success(
              res.message || this.translate.instant('tileLayers.truncateSuccess'),
            );
            this.refreshTrigger.update((v) => v + 1);
          },
          error: () =>
            this.notificationService.error(this.translate.instant('tileLayers.truncateFail')),
        });
      });
  }

  clearLayerCache(layer: Layer): void {
    this.dialog
      .open(ConfirmDialogComponent, {
        width: '400px',
        data: {
          title: this.translate.instant('tileLayers.confirmClearTitle'),
          message: this.translate.instant('tileLayers.confirmClearMessage', {
            name: layer.name,
          }),
        },
      })
      .afterClosed()
      .subscribe((confirmed) => {
        if (!confirmed) return;
        this.terraneService.clearTileCache(layer.name).subscribe({
          next: (res) => {
            this.notificationService.success(
              res.message || this.translate.instant('tileLayers.cacheCleared'),
            );
            this.refreshTrigger.update((v) => v + 1);
          },
          error: () =>
            this.notificationService.error(this.translate.instant('tileLayers.clearFail')),
        });
      });
  }

  clearAllCache(): void {
    this.notificationService
      .confirm(
        this.translate.instant('tileLayers.confirmClearTitle'),
        this.translate.instant('tileLayers.confirmClearMessage'),
      )
      .subscribe((confirmed) => {
        if (!confirmed) return;
        this.loading.set(true);
        const layers = this.layers();
        const clearNext = (idx: number) => {
          if (idx >= layers.length) {
            this.loading.set(false);
            this.notificationService.success(this.translate.instant('tileLayers.clearAllSuccess'));
            this.refreshTrigger.update((v) => v + 1);
            return;
          }
          this.terraneService.clearTileCache(layers[idx].name).subscribe({
            next: () => clearNext(idx + 1),
            error: () => clearNext(idx + 1),
          });
        };
        clearNext(0);
      });
  }

  cancelSeedJob(job: SeedJob): void {
    this.terraneService.cancelSeedJob(job.id).subscribe({
      next: (res) => {
        this.notificationService.info(
          res.message || this.translate.instant('tileLayers.cancelRequested'),
        );
        this.refreshTrigger.update((v) => v + 1);
      },
      error: () => this.notificationService.error(this.translate.instant('tileLayers.cancelFail')),
    });
  }

  refresh(): void {
    this.refreshTrigger.update((v) => v + 1);
  }

  jobProgress(job: SeedJob): number {
    if (job.total <= 0) return 0;
    return Math.min(100, Math.round((job.done / job.total) * 100));
  }

  trackByIndex(index: number): number {
    return index;
  }
}
