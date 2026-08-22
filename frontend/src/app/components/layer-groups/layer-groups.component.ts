import { Component, ChangeDetectionStrategy, inject, signal } from '@angular/core';
import { toSignal, toObservable } from '@angular/core/rxjs-interop';
import { MatDialog } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { ConfirmDialogComponent } from '../../shared/components/confirm-dialog.component';
import { LayerGroup, Layer } from '../../models/geoserver.models';
import { CreateLayerGroupDialogComponent } from './create-layer-group-dialog.component';
import { switchMap, tap, startWith, catchError, of } from 'rxjs';

@Component({
  standalone: false,
  selector: 'app-layer-groups',
  templateUrl: './layer-groups.component.html',
  styleUrls: ['./layer-groups.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class LayerGroupsComponent {
  private geoserverService = inject(GeoserverService);
  private notificationService = inject(NotificationService);
  private dialog = inject(MatDialog);
  private translate = inject(TranslateService);

  displayedColumns = ['name', 'title', 'layerCount', 'actions'];

  private refreshTrigger = signal(0);
  loading = signal(true);

  private groups$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    tap(() => this.loading.set(true)),
    switchMap(() =>
      this.geoserverService.getLayerGroups().pipe(
        catchError(() => {
          this.notificationService.error(this.translate.instant('layerGroups.loadFail'));
          return of([] as LayerGroup[]);
        }),
      ),
    ),
    tap(() => this.loading.set(false)),
  );

  groups = toSignal(this.groups$, { initialValue: [] as LayerGroup[] });

  private layers$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    switchMap(() =>
      this.geoserverService.getLayers().pipe(
        catchError(() => of([] as Layer[])),
      ),
    ),
  );

  layers = toSignal(this.layers$, { initialValue: [] as Layer[] });

  createGroup(): void {
    const dialogRef = this.dialog.open(CreateLayerGroupDialogComponent, {
      width: '500px',
      data: { layers: this.layers() },
    });
    dialogRef.afterClosed().subscribe((result) => {
      if (result) this.refreshTrigger.update((v) => v + 1);
    });
  }

  deleteGroup(group: LayerGroup): void {
    const dialogRef = this.dialog.open(ConfirmDialogComponent, {
      data: {
        title: this.translate.instant('layerGroups.deleteTitle'),
        message: this.translate.instant('layerGroups.deleteMessage', {
          name: group.title || group.name,
        }),
      },
    });
    dialogRef.afterClosed().subscribe((confirmed) => {
      if (!confirmed) return;
      this.geoserverService.deleteLayerGroup(group.name).subscribe({
        next: () => {
          this.notificationService.success(this.translate.instant('layerGroups.deleteSuccess'));
          this.refreshTrigger.update((v) => v + 1);
        },
        error: () =>
          this.notificationService.error(this.translate.instant('layerGroups.deleteFail')),
      });
    });
  }
}
