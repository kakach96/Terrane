import { Component, Inject, OnInit } from '@angular/core';
import { FormBuilder, FormGroup, Validators } from '@angular/forms';
import { MAT_DIALOG_DATA, MatDialog, MatDialogRef } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
import { GeoserverService } from '../../../services/geoserver.service';
import { NotificationService } from '../../../services/notification.service';
import {
  DataSource,
  DataSourceConnection,
  CreateDataSourceRequest,
  UpdateDataSourceRequest,
  ConnectionTestResult,
  Workspace,
  S3BrowseRequest,
} from '../../../models/geoserver.models';
import {
  DirectoryBrowserComponent,
  DirectoryBrowserResult,
} from '../../shared/directory-browser/directory-browser.component';

@Component({
  selector: 'app-data-source-dialog',
  templateUrl: './data-source-dialog.component.html',
  styleUrls: ['./data-source-dialog.component.scss'],
})
export class DataSourceDialogComponent implements OnInit {
  form: FormGroup;
  mode: 'create' | 'edit';
  dataSource?: DataSource;
  workspaces: Workspace[] = [];
  dataSourceTypes = [
    { value: 'postgis', label: 'PostGIS' },
    { value: 'shapefile', label: 'Shapefile' },
    { value: 'geotiff', label: 'GeoTIFF' },
    { value: 'geopackage', label: 'GeoPackage' },
    { value: 'geojson', label: 'GeoJSON' },
    { value: 'worldimage', label: 'WorldImage' },
    { value: 'cascaded_wms', label: 'Cascaded WMS' },
    { value: 'arcgrid', label: 'ArcGrid' },
    { value: 'image_mosaic', label: 'ImageMosaic' },
    { value: 'redis', label: 'Redis Cache' },
  ];
  fileStorageTypes = [
    { value: 'local', label: 'Local', description: 'Server-local directory' },
    { value: 's3', label: 'S3', description: 'Object storage directory' },
  ];
  isTesting = false;
  selectedFile: File | null = null;

  constructor(
    @Inject(MAT_DIALOG_DATA) public data: { mode: 'create' | 'edit'; dataSource?: DataSource },
    private dialogRef: MatDialogRef<DataSourceDialogComponent>,
    private dialog: MatDialog,
    private fb: FormBuilder,
    private geoserverService: GeoserverService,
    private notificationService: NotificationService,
    private translate: TranslateService,
  ) {
    this.mode = data.mode;
    this.dataSource = data.dataSource;
    this.form = this.fb.group({
      name: ['', Validators.required],
      type: ['postgis', Validators.required],
      workspace: ['', Validators.required],
      host: ['localhost'],
      port: ['5432'],
      database: [''],
      schema: ['public'],
      username: [''],
      password: [''],
      file_path: [''],
      file_storage_type: ['local'],
      s3_endpoint: [''],
      s3_region: ['us-east-1'],
      s3_bucket: [''],
      s3_access_key: [''],
      s3_secret_key: [''],
      enabled: [true],
    });
  }

  ngOnInit(): void {
    this.loadWorkspaces();
    this.dataSourceTypes = [
      { value: 'postgis', label: 'PostGIS' },
      { value: 'shapefile', label: 'Shapefile' },
      { value: 'geotiff', label: 'GeoTIFF' },
      { value: 'geopackage', label: 'GeoPackage' },
      { value: 'geojson', label: 'GeoJSON' },
      { value: 'worldimage', label: 'WorldImage' },
      { value: 'cascaded_wms', label: this.translate.instant('dataSources.cascadedWms') },
      { value: 'arcgrid', label: 'ArcGrid' },
      { value: 'image_mosaic', label: 'ImageMosaic' },
      { value: 'redis', label: 'Redis Cache' },
    ];
    this.fileStorageTypes = [
      {
        value: 'local',
        label: this.translate.instant('dataSources.storageTypeLocal'),
        description: this.translate.instant('dataSources.storageTypeLocalDesc'),
      },
      {
        value: 's3',
        label: this.translate.instant('dataSources.storageTypeS3'),
        description: this.translate.instant('dataSources.storageTypeS3Desc'),
      },
    ];

    if (this.mode === 'edit' && this.dataSource) {
      this.form.patchValue({
        name: this.dataSource.name,
        type: this.dataSource.type,
        workspace: this.dataSource.workspace || '',
        host: this.dataSource.connection?.host || 'localhost',
        port: this.dataSource.connection?.port || '5432',
        database: this.dataSource.connection?.database || '',
        schema: this.dataSource.connection?.schema || 'public',
        username: this.dataSource.connection?.username || '',
        file_path: this.dataSource.connection?.file_path || '',
        file_storage_type: this.dataSource.connection?.file_storage_type || 'local',
        s3_endpoint: this.dataSource.connection?.s3_endpoint || '',
        s3_region: this.dataSource.connection?.s3_region || 'us-east-1',
        s3_bucket: this.dataSource.connection?.s3_bucket || '',
        s3_access_key: this.dataSource.connection?.s3_access_key || '',
        s3_secret_key: this.dataSource.connection?.s3_secret_key || '',
        enabled: this.dataSource.enabled,
      });
      this.form.get('name')?.disable();
    }
  }

  loadWorkspaces(): void {
    this.geoserverService.getAllWorkspaces().subscribe({
      next: (workspaces: Workspace[]) => {
        this.workspaces = workspaces;
      },
      error: (err) => {
        console.error('Failed to load workspaces:', err);
      },
    });
  }

  get title(): string {
    return this.mode === 'create'
      ? this.translate.instant('dataSources.dialogTitleCreate')
      : this.translate.instant('dataSources.dialogTitleEdit');
  }

  get selectedType(): string {
    return this.form.get('type')?.value;
  }

  get selectedStorageType(): string {
    return this.form.get('file_storage_type')?.value || 'local';
  }

  get isS3(): boolean {
    return this.selectedStorageType === 's3';
  }

  onFileSelected(event: Event): void {
    const input = event.target as HTMLInputElement;
    if (input.files && input.files.length > 0) {
      this.selectedFile = input.files[0];
      // 自动填充名称为文件名（不含扩展名）
      const nameCtrl = this.form.get('name');
      if (nameCtrl && !nameCtrl.value) {
        nameCtrl.setValue(this.selectedFile.name.replace(/\.[^.]+$/, ''));
      }
    }
  }

  /** 打开本地服务器目录选择器 */
  browseLocal(): void {
    const dialogRef = this.dialog.open(DirectoryBrowserComponent, {
      width: '580px',
      data: {
        mode: 'local',
        initialPath: this.initialBrowsePath(),
      },
    });
    dialogRef.afterClosed().subscribe((result?: DirectoryBrowserResult) => {
      if (result) {
        this.form.get('file_path')?.setValue(result.path);
      }
    });
  }

  /** 打开 S3 bucket 目录选择器 (携带已填写的连接配置) */
  browseS3(): void {
    const connection: S3BrowseRequest = {
      s3_endpoint: this.form.get('s3_endpoint')?.value || undefined,
      s3_region: this.form.get('s3_region')?.value || undefined,
      s3_bucket: this.form.get('s3_bucket')?.value || undefined,
      s3_access_key: this.form.get('s3_access_key')?.value || undefined,
      s3_secret_key: this.form.get('s3_secret_key')?.value || undefined,
    };
    if (!connection.s3_bucket) {
      this.notificationService.warning(this.translate.instant('dataSources.s3BucketWarning'));
      return;
    }
    const dialogRef = this.dialog.open(DirectoryBrowserComponent, {
      width: '580px',
      data: {
        mode: 's3',
        initialPath: this.initialBrowsePath(),
        s3Connection: connection,
      },
    });
    dialogRef.afterClosed().subscribe((result?: DirectoryBrowserResult) => {
      if (result) {
        this.form.get('file_path')?.setValue(result.path);
      }
    });
  }

  /** 从当前 file_path 推导初始浏览目录 (取其所在目录/前缀) */
  private initialBrowsePath(): string {
    const raw = (this.form.get('file_path')?.value as string) || '';
    if (!raw) {
      return '';
    }
    if (this.isS3) {
      // S3: 前缀以 '/' 结尾视为目录, 否则取其所在目录 (兼容 / 与 \ 分隔符),
      // 并去掉 ./ 等根前缀, 避免把本地路径残留当作 S3 前缀
      if (raw.endsWith('/')) {
        return raw;
      }
      const idx = Math.max(raw.lastIndexOf('/'), raw.lastIndexOf('\\'));
      const base = idx >= 0 ? raw.slice(0, idx + 1) : '';
      return base.replace(/^[.\\/]+/, '');
    }
    const trimmed = raw.replace(/[\\/]+$/, '');
    const idx = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'));
    return idx >= 0 ? trimmed.slice(0, idx) : '';
  }

  testConnection(): void {
    if (
      this.selectedType !== 'postgis' &&
      this.selectedType !== 'redis' &&
      this.selectedType !== 'image_mosaic'
    ) {
      this.notificationService.info(this.translate.instant('dataSources.postgisOnlyTest'));
      return;
    }
    if (!this.form.valid) {
      this.notificationService.warning(this.translate.instant('dataSources.requiredFields'));
      return;
    }

    this.isTesting = true;
    const request = this.buildCreateRequest();
    if (!request.connection) {
      this.notificationService.error(this.translate.instant('dataSources.missingConnection'));
      this.isTesting = false;
      return;
    }
    this.geoserverService.testConnection(request).subscribe({
      next: (result: ConnectionTestResult) => {
        this.isTesting = false;
        if (result.success) {
          this.notificationService.success(this.translate.instant('dataSources.testSuccess'));
        } else {
          this.notificationService.error(
            this.translate.instant('dataSources.testFailWithMessage', { message: result.message }),
          );
        }
      },
      error: (err) => {
        this.isTesting = false;
        console.error('Connection test failed:', err);
        this.notificationService.error(this.translate.instant('dataSources.testFail'));
      },
    });
  }

  /** 构建 host/port/database/username/password 连接 (PostGIS 与 Redis 通用) */
  private buildTcpConnection(): DataSourceConnection {
    return {
      host: this.form.get('host')?.value,
      port: parseInt(this.form.get('port')?.value, 10),
      database: this.form.get('database')?.value,
      schema: this.form.get('schema')?.value,
      username: this.form.get('username')?.value,
      password: this.form.get('password')?.value,
    };
  }

  buildCreateRequest(): CreateDataSourceRequest {
    const type = this.form.get('type')?.value;
    const request: CreateDataSourceRequest = {
      name: this.form.get('name')?.value,
      type: type,
      workspace: this.form.get('workspace')?.value,
      enabled: this.form.get('enabled')?.value ?? true,
    };

    if (type === 'postgis' || type === 'redis') {
      request.connection = this.buildTcpConnection();
    } else {
      const filePath = this.form.get('file_path')?.value || this.selectedFile?.name;
      if (filePath || this.isS3) {
        request.connection = {
          file_path: filePath,
          file_storage_type: this.selectedStorageType,
          ...(this.isS3
            ? {
                s3_endpoint: this.form.get('s3_endpoint')?.value || undefined,
                s3_region: this.form.get('s3_region')?.value || undefined,
                s3_bucket: this.form.get('s3_bucket')?.value || undefined,
                s3_access_key: this.form.get('s3_access_key')?.value || undefined,
                s3_secret_key: this.form.get('s3_secret_key')?.value || undefined,
              }
            : {}),
        };
      }
    }

    return request;
  }

  buildUpdateRequest(): UpdateDataSourceRequest {
    const type = this.form.get('type')?.value;
    const request: UpdateDataSourceRequest = {
      type: type,
      workspace: this.form.get('workspace')?.value,
      enabled: this.form.get('enabled')?.value ?? true,
    };

    if (type === 'postgis' || type === 'redis') {
      request.connection = this.buildTcpConnection();
    } else {
      const filePath = this.form.get('file_path')?.value;
      if (filePath || this.isS3) {
        request.connection = {
          file_path: filePath,
          file_storage_type: this.selectedStorageType,
          ...(this.isS3
            ? {
                s3_endpoint: this.form.get('s3_endpoint')?.value || undefined,
                s3_region: this.form.get('s3_region')?.value || undefined,
                s3_bucket: this.form.get('s3_bucket')?.value || undefined,
                s3_access_key: this.form.get('s3_access_key')?.value || undefined,
                s3_secret_key: this.form.get('s3_secret_key')?.value || undefined,
              }
            : {}),
        };
      }
    }

    return request;
  }

  onSubmit(): void {
    if (!this.form.valid) {
      this.notificationService.warning(this.translate.instant('dataSources.requiredFields'));
      return;
    }

    const type = this.form.get('type')?.value;

    // geojson 数据源走普通创建 (指定 file_path + file_storage_type), 不走文件上传;
    // S3 存储同样不走本地文件上传 (直接引用已存在的对象)
    if (
      this.mode === 'create' &&
      type !== 'postgis' &&
      type !== 'geojson' &&
      !this.isS3 &&
      this.selectedFile
    ) {
      // 文件型数据源：通过上传接口创建
      const dsName = this.form.get('name')?.value;
      const upload$ =
        type === 'shapefile'
          ? this.geoserverService.uploadShapefile(this.selectedFile, dsName)
          : this.geoserverService.uploadGeoTiff(this.selectedFile, dsName);

      upload$.subscribe({
        next: () => {
          this.notificationService.success(
            this.translate.instant('dataSources.uploadCreateSuccess', {
              type: type === 'shapefile' ? 'Shapefile' : 'GeoTIFF',
            }),
          );
          this.dialogRef.close(true);
        },
        error: (err) => {
          console.error('Upload failed:', err);
          this.notificationService.error(this.notificationService.fromError(err));
        },
      });
    } else if (this.mode === 'create') {
      this.geoserverService.createDataSource(this.buildCreateRequest()).subscribe({
        next: () => {
          this.notificationService.success(this.translate.instant('dataSources.createSuccess'));
          this.dialogRef.close(true);
        },
        error: (err) => {
          console.error('Create failed:', err);
          this.notificationService.error(this.translate.instant('dataSources.createFail'));
        },
      });
    } else {
      this.geoserverService
        .updateDataSource(this.dataSource!.name, this.buildUpdateRequest())
        .subscribe({
          next: () => {
            this.notificationService.success(this.translate.instant('dataSources.updateSuccess'));
            this.dialogRef.close(true);
          },
          error: (err) => {
            console.error('Update failed:', err);
            this.notificationService.error(this.translate.instant('dataSources.updateFail'));
          },
        });
    }
  }

  onCancel(): void {
    this.dialogRef.close(false);
  }
}
