import { Component, OnInit } from '@angular/core';
import { MatDialog } from '@angular/material/dialog';
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
        this.notificationService.error('加载图层组失败');
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
        title: '删除图层组',
        message: `确定要删除图层组 "${group.title || group.name}" 吗？`,
      },
    });
    dialogRef.afterClosed().subscribe((confirmed) => {
      if (!confirmed) return;
      this.geoserverService.deleteLayerGroup(group.name).subscribe({
        next: () => {
          this.notificationService.success('图层组已删除');
          this.loadGroups();
        },
        error: () => this.notificationService.error('删除失败'),
      });
    });
  }
}
