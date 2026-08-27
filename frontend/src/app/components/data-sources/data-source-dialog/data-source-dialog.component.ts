import { Component, Inject, inject, computed } from '@angular/core';
import { FormBuilder, FormGroup, Validators } from '@angular/forms';
import { MAT_DIALOG_DATA, MatDialog, MatDialogRef } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { GeoserverService } from '../../../services/geoserver.service';
import { NotificationService } from '../../../services/notification.service';
import { LanguageService } from '../../../services/language.service';
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
import { catchError, of } from 'rxjs';

@Component({
  standalone: false,
  selector: 'app-data-source-dialog',
  templateUrl: './data-source-dialog.component.html',
  styleUrls: ['./data-source-dialog.component.scss'],
})
export class DataSourceDialogComponent {
  private dialogRef = inject(MatDialogRef<DataSourceDialogComponent>);
  private dialog = inject(MatDialog);
  private fb = inject(FormBuilder);
  private geoserverService = inject(GeoserverService);
  private notificationService = inject(NotificationService);
  private translate = inject(TranslateService);
  private languageService = inject(LanguageService);

  form: FormGroup;
  mode: 'create' | 'edit';
  dataSource?: DataSource;
  // Computed so labels re-translate on language switch; reading currentLang()
  // makes the computed re-evaluate when the language changes.
  dataSourceTypes = computed(() => {
    this.languageService.currentLang();
    return [
      { value: 'postgis', label: 'PostGIS' },
      { value: 'mysql', label: 'MySQL' },
      { value: 'mongo', label: 'MongoDB' },
      { value: 'shapefile', label: 'Shapefile' },
      { value: 'geotiff', label: 'GeoTIFF' },
      { value: 'geopackage', label: 'GeoPackage' },
      { value: 'geojson', label: 'GeoJSON' },
      { value: 'worldimage', label: 'WorldImage' },
      { value: 'cascaded_wms', label: this.translate.instant('dataSources.cascadedWms') },
      { value: 'arcgrid', label: 'ArcGrid' },
      { value: 'image_mosaic', label: 'ImageMosaic' },
      { value: 'image_pyramid', label: 'ImagePyramid' },
      { value: 'redis', label: 'Redis Cache' },
    ];
  });
  fileStorageTypes = computed(() => {
    this.languageService.currentLang();
    return [
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
  });
  isTesting = false;
  selectedFile: File | null = null;

  // ── Signal pipeline: workspaces ───────────────────────────────────
  private workspaces$ = this.geoserverService
    .getAllWorkspaces()
    .pipe(catchError(() => of([] as Workspace[])));

  workspaces = toSignal(this.workspaces$, { initialValue: [] as Workspace[] });

  constructor(
    @Inject(MAT_DIALOG_DATA) public data: { mode: 'create' | 'edit'; dataSource?: DataSource },
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

  /** Dialog title; re-evaluated on language switch. */
  title = computed(() => {
    this.languageService.currentLang();
    return this.mode === 'create'
      ? this.translate.instant('dataSources.dialogTitleCreate')
      : this.translate.instant('dataSources.dialogTitleEdit');
  });

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
      // Auto-fill the name from the file name (without extension)
      const nameCtrl = this.form.get('name');
      if (nameCtrl && !nameCtrl.value) {
        nameCtrl.setValue(this.selectedFile.name.replace(/\.[^.]+$/, ''));
      }
    }
  }

  /** Open the local server directory picker */
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

  /** Open the S3 bucket directory picker (carrying the filled-in connection config) */
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

  /** Derive the initial browse directory from the current file_path (its parent dir/prefix) */
  private initialBrowsePath(): string {
    const raw = (this.form.get('file_path')?.value as string) || '';
    if (!raw) {
      return '';
    }
    if (this.isS3) {
      // S3: a prefix ending in '/' is a directory; otherwise take its parent
      // (compatible with both / and \ separators), and strip root prefixes like
      // './' so leftover local path segments are not treated as S3 prefixes
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
      this.selectedType !== 'mysql' &&
      this.selectedType !== 'mongo' &&
      this.selectedType !== 'redis' &&
      this.selectedType !== 'image_mosaic' &&
      this.selectedType !== 'image_pyramid'
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

  /** Build a host/port/database/username/password connection (shared by PostGIS and Redis) */
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

    if (type === 'postgis' || type === 'mysql' || type === 'mongo' || type === 'redis') {
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

    if (type === 'postgis' || type === 'mysql' || type === 'mongo' || type === 'redis') {
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

    // GeoJSON data sources go through normal creation (file_path + file_storage_type),
    // not file upload; S3 storage also skips local file upload (references an existing object)
    if (
      this.mode === 'create' &&
      type !== 'postgis' &&
      type !== 'geojson' &&
      !this.isS3 &&
      this.selectedFile
    ) {
      // File-based data source: create via the upload endpoint
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
  trackByIndex(index: number): number {
    return index;
  }
}
