import { Component, OnInit, ChangeDetectorRef } from '@angular/core';
import { MatDialog } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { Layer } from '../../models/geoserver.models';
import { ConfirmDialogComponent } from '../../shared/components/confirm-dialog.component';

@Component({
  standalone: false,
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
    private translate: TranslateService,
    private cdr: ChangeDetectorRef,
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
        this.cdr.detectChanges();
      },
      error: () => {
        this.notificationService.error(this.translate.instant('layers.loadFail'));
        this.loading = false;
        this.cdr.detectChanges();
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
        title: this.translate.instant('layers.deleteTitle'),
        message: this.translate.instant('layers.deleteMessage', { name: layer.name }),
      },
    });

    dialogRef.afterClosed().subscribe((result) => {
      if (result) {
        this.geoserverService.deleteLayer(layer.name).subscribe({
          next: () => {
            this.notificationService.success(this.translate.instant('layers.deleteSuccess'));
            this.loadLayers();
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
    this.loadLayers();
  }

  trackByIndex(index: number): number {
    return index;
  }
}
