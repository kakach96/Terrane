import { Component, OnInit } from '@angular/core';
import { MatDialog } from '@angular/material/dialog';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { DataSourceDialogComponent } from './data-source-dialog/data-source-dialog.component';
import { DataSource } from '../../models/geoserver.models';

@Component({
  selector: 'app-data-sources',
  templateUrl: './data-sources.component.html',
  styleUrls: ['./data-sources.component.scss']
})
export class DataSourcesComponent implements OnInit {
  dataSources: DataSource[] = [];
  loading = true;
  displayedColumns: string[] = ['name', 'type', 'workspace', 'enabled', 'actions'];

  constructor(
    private geoserverService: GeoserverService,
    private notificationService: NotificationService,
    private dialog: MatDialog
  ) {}

  ngOnInit(): void {
    this.loadDataSources();
  }

  loadDataSources(): void {
    this.loading = true;
    this.geoserverService.getDataSources().subscribe({
      next: (data: DataSource[]) => {
        this.dataSources = data;
        this.loading = false;
      },
      error: (err: any) => {
        console.error('Failed to load data sources:', err);
        this.loading = false;
        this.notificationService.error('加载数据源失败');
      }
    });
  }

  openCreateDialog(): void {
    const dialogRef = this.dialog.open(DataSourceDialogComponent, {
      width: '600px',
      data: { mode: 'create' }
    });

    dialogRef.afterClosed().subscribe((result) => {
      if (result) {
        this.loadDataSources();
      }
    });
  }

  openEditDialog(dataSource: DataSource): void {
    const dialogRef = this.dialog.open(DataSourceDialogComponent, {
      width: '600px',
      data: { mode: 'edit', dataSource }
    });

    dialogRef.afterClosed().subscribe((result) => {
      if (result) {
        this.loadDataSources();
      }
    });
  }

  deleteDataSource(name: string): void {
    this.notificationService.confirm('确认删除', `确定要删除数据源 "${name}" 吗？`).subscribe((confirmed: boolean) => {
      if (confirmed) {
        this.geoserverService.deleteDataSource(name).subscribe({
          next: () => {
            this.notificationService.success('删除成功');
            this.loadDataSources();
          },
          error: (err: any) => {
            console.error('Failed to delete data source:', err);
            this.notificationService.error('删除失败');
          }
        });
      }
    });
  }

  testConnection(dataSource: DataSource): void {
    this.geoserverService.testDataSourceConnection(dataSource.name).subscribe({
      next: (result: any) => {
        if (result.success) {
          this.notificationService.success('连接测试成功');
        } else {
          this.notificationService.warning(`连接测试失败: ${result.message}`);
        }
      },
      error: (err: any) => {
        console.error('Failed to test connection:', err);
        this.notificationService.error('连接测试失败');
      }
    });
  }

  toggleEnabled(dataSource: DataSource): void {
    const enabled = !dataSource.enabled;
    this.geoserverService.updateDataSource(dataSource.name, { enabled }).subscribe({
      next: () => {
        dataSource.enabled = enabled;
        this.notificationService.success(`数据源已${enabled ? '启用' : '禁用'}`);
      },
      error: (err: any) => {
        console.error('Failed to update data source:', err);
        this.notificationService.error('更新失败');
      }
    });
  }

  getTypeIcon(type: string): string {
    switch (type) {
      case 'postgis':
        return 'database';
      case 'shapefile':
        return 'folder_open';
      case 'geotiff':
        return 'image';
      default:
        return 'storage';
    }
  }

  getTypeLabel(type: string): string {
    switch (type) {
      case 'postgis':
        return 'PostGIS';
      case 'shapefile':
        return 'Shapefile';
      case 'geotiff':
        return 'GeoTIFF';
      default:
        return type;
    }
  }

  getTypeColor(type: string): 'primary' | 'accent' | 'warn' | undefined {
    switch (type) {
      case 'postgis':
        return 'primary';
      case 'shapefile':
        return 'accent';
      case 'geotiff':
        return 'warn';
      default:
        return undefined;
    }
  }
}