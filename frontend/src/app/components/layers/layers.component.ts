import { Component, OnInit } from '@angular/core';
import { MatDialog } from '@angular/material/dialog';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { Layer } from '../../models/geoserver.models';
import { ConfirmDialogComponent } from '../../shared/components/confirm-dialog.component';

@Component({
  selector: 'app-layers',
  templateUrl: './layers.component.html',
  styleUrls: ['./layers.component.scss'],
})
export class LayersComponent implements OnInit {
  layers: Layer[] = [];
  loading = true;
  searchQuery = '';
  selectedWorkspace = '';
  workspaces: string[] = [];

  constructor(
    private geoserverService: GeoserverService,
    private notificationService: NotificationService,
    private dialog: MatDialog,
  ) {}

  ngOnInit(): void {
    this.loadLayers();
  }

  loadLayers(): void {
    this.loading = true;
    this.geoserverService.getLayers().subscribe({
      next: (layers) => {
        this.layers = layers;
        this.workspaces = [...new Set(layers.map((l) => l.workspace))];
        this.loading = false;
      },
      error: () => {
        this.notificationService.error('加载图层失败');
        this.loading = false;
      },
    });
  }

  get filteredLayers(): Layer[] {
    return this.layers.filter((layer) => {
      const matchesSearch =
        !this.searchQuery ||
        layer.name.toLowerCase().includes(this.searchQuery.toLowerCase()) ||
        layer.title.toLowerCase().includes(this.searchQuery.toLowerCase());
      const matchesWorkspace =
        !this.selectedWorkspace || layer.workspace === this.selectedWorkspace;
      return matchesSearch && matchesWorkspace;
    });
  }

  deleteLayer(layer: Layer): void {
    const dialogRef = this.dialog.open(ConfirmDialogComponent, {
      width: '400px',
      data: {
        title: '删除图层',
        message: `确定要删除图层 "${layer.name}" 吗？此操作不可撤销。`,
      },
    });

    dialogRef.afterClosed().subscribe((result) => {
      if (result) {
        this.geoserverService.deleteLayer(layer.name).subscribe({
          next: () => {
            this.notificationService.success('图层删除成功');
            this.loadLayers();
          },
          error: (error) => {
            this.notificationService.error('删除失败: ' + error.message);
          },
        });
      }
    });
  }

  refresh(): void {
    this.loadLayers();
  }
}
