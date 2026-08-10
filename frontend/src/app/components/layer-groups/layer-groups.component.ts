import { Component, OnInit } from '@angular/core';
import { MatDialog } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { ConfirmDialogComponent } from '../../shared/components/confirm-dialog.component';
import { LayerGroup, Layer } from '../../models/geoserver.models';
import { CreateLayerGroupDialogComponent } from './create-layer-group-dialog.component';

@Component({
  selector: 'app-layer-groups',
  templateUrl: './layer-groups.component.html',
  styleUrls: ['./layer-groups.component.scss'],
})
export class LayerGroupsComponent implements OnInit {
  groups: LayerGroup[] = [];
  layers: Layer[] = [];
  loading = true;
  displayedColumns = ['name', 'title', 'layerCount', 'actions'];

  constructor(
    private geoserverService: GeoserverService,
    private notificationService: NotificationService,
    private dialog: MatDialog,
    private translate: TranslateService,
  ) {}

  ngOnInit(): void {
    this.loadGroups();
    this.geoserverService.getLayers().subscribe({
      next: (data) => (this.layers = data),
    });
  }

  loadGroups(): void {
    this.loading = true;
    this.geoserverService.getLayerGroups().subscribe({
      next: (data) => {
        this.groups = data;
        this.loading = false;
      },
      error: () => {
        this.notificationService.error(this.translate.instant('layerGroups.loadFail'));
        this.loading = false;
      },
    });
  }

  createGroup(): void {
    const dialogRef = this.dialog.open(CreateLayerGroupDialogComponent, {
      width: '500px',
      data: { layers: this.layers },
    });
    dialogRef.afterClosed().subscribe((result) => {
      if (result) this.loadGroups();
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
          this.loadGroups();
        },
        error: () =>
          this.notificationService.error(this.translate.instant('layerGroups.deleteFail')),
      });
    });
  }
}
