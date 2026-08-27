import { Component, ChangeDetectionStrategy, computed, inject, signal } from '@angular/core';
import { toSignal, toObservable } from '@angular/core/rxjs-interop';
import { MatDialog } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { Layer } from '../../models/geoserver.models';
import { ConfirmDialogComponent } from '../../shared/components/confirm-dialog.component';
import { switchMap, tap, startWith, catchError, of } from 'rxjs';

@Component({
  standalone: false,
  selector: 'app-layers',
  templateUrl: './layers.component.html',
  styleUrls: ['./layers.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class LayersComponent {
  private geoserverService = inject(GeoserverService);
  private notificationService = inject(NotificationService);
  private dialog = inject(MatDialog);
  private translate = inject(TranslateService);

  searchQuery = '';
  selectedWorkspace = '';

  private refreshTrigger = signal(0);
  loading = signal(true);

  private layers$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    tap(() => this.loading.set(true)),
    switchMap(() =>
      this.geoserverService.getLayers().pipe(
        catchError(() => {
          this.notificationService.error(this.translate.instant('layers.loadFail'));
          return of([] as Layer[]);
        }),
      ),
    ),
    tap(() => this.loading.set(false)),
  );

  layers = toSignal(this.layers$, { initialValue: [] as Layer[] });

  workspaces = computed(() => [...new Set(this.layers().map((l) => l.workspace))]);

  filteredLayers = computed(() => {
    return this.layers().filter((layer) => {
      const q = this.searchQuery.toLowerCase();
      const matchesSearch =
        !this.searchQuery ||
        layer.name.toLowerCase().includes(q) ||
        layer.title.toLowerCase().includes(q);
      const matchesWs = !this.selectedWorkspace || layer.workspace === this.selectedWorkspace;
      return matchesSearch && matchesWs;
    });
  });

  deleteLayer(layer: Layer): void {
    const dialogRef = this.dialog.open(ConfirmDialogComponent, {
      width: '400px',
      data: {
        title: this.translate.instant('layers.deleteTitle'),
        message: this.translate.instant('layers.deleteMessage', { name: layer.name }),
      },
    });

    dialogRef.afterClosed().subscribe((result) => {
      if (result) {
        this.geoserverService.deleteLayer(layer.name).subscribe({
          next: () => {
            this.notificationService.success(this.translate.instant('layers.deleteSuccess'));
            this.refreshTrigger.update((v) => v + 1);
          },
          error: (error) => {
            this.notificationService.error(
              this.translate.instant('layers.deleteFail', { message: error.message }),
            );
          },
        });
      }
    });
  }

  refresh(): void {
    this.refreshTrigger.update((v) => v + 1);
  }

  trackByIndex(index: number): number {
    return index;
  }
}
