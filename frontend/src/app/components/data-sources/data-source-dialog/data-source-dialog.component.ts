import { Component, Inject, OnInit } from '@angular/core';
import { FormBuilder, FormGroup, Validators } from '@angular/forms';
import { MAT_DIALOG_DATA, MatDialogRef } from '@angular/material/dialog';
import { GeoserverService } from '../../../services/geoserver.service';
import { NotificationService } from '../../../services/notification.service';
import { DataSource, CreateDataSourceRequest, UpdateDataSourceRequest, ConnectionTestResult, Workspace } from '../../../models/geoserver.models';

@Component({
  selector: 'app-data-source-dialog',
  templateUrl: './data-source-dialog.component.html',
  styleUrls: ['./data-source-dialog.component.scss']
})
export class DataSourceDialogComponent implements OnInit {
  form: FormGroup;
  mode: 'create' | 'edit';
  dataSource?: DataSource;
  workspaces: Workspace[] = [];
  dataSourceTypes = [
    { value: 'postgis', label: 'PostGIS' },
    { value: 'shapefile', label: 'Shapefile' },
    { value: 'geotiff', label: 'GeoTIFF' }
  ];
  isTesting = false;
  selectedFile: File | null = null;

  constructor(
    @Inject(MAT_DIALOG_DATA) public data: { mode: 'create' | 'edit'; dataSource?: DataSource },
    private dialogRef: MatDialogRef<DataSourceDialogComponent>,
    private fb: FormBuilder,
    private geoserverService: GeoserverService,
    private notificationService: NotificationService
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
      enabled: [true]
    });
  }

  ngOnInit(): void {
    this.loadWorkspaces();
    
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
        enabled: this.dataSource.enabled
      });
      this.form.get('name')?.disable();
    }
  }

  loadWorkspaces(): void {
    this.geoserverService.getAllWorkspaces().subscribe({
      next: (workspaces: Workspace[]) => {
        this.workspaces = workspaces;
      },
      error: (err: any) => {
        console.error('Failed to load workspaces:', err);
      }
    });
  }

  get title(): string {
    return this.mode === 'create' ? '创建数据源' : '编辑数据源';
  }

  get selectedType(): string {
    return this.form.get('type')?.value;
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

  testConnection(): void {
    if (this.selectedType !== 'postgis') {
      this.notificationService.info('仅 PostGIS 支持连接测试');
      return;
    }
    if (!this.form.valid) {
      this.notificationService.warning('请填写必填字段');
      return;
    }

    this.isTesting = true;
    const request = this.buildCreateRequest();
    if (!request.connection) {
      this.notificationService.error('缺少连接配置');
      this.isTesting = false;
      return;
    }
    this.geoserverService.testConnection(request).subscribe({
      next: (result: ConnectionTestResult) => {
        this.isTesting = false;
        if (result.success) {
          this.notificationService.success('连接测试成功');
        } else {
          this.notificationService.error(`连接测试失败: ${result.message}`);
        }
      },
      error: (err: any) => {
        this.isTesting = false;
        console.error('Connection test failed:', err);
        this.notificationService.error('连接测试失败');
      }
    });
  }

  buildCreateRequest(): CreateDataSourceRequest {
    const type = this.form.get('type')?.value;
    const request: any = {
      name: this.form.get('name')?.value,
      type: type,
      workspace: this.form.get('workspace')?.value,
      enabled: this.form.get('enabled')?.value ?? true,
    };

    if (type === 'postgis') {
      request.connection = {
        host: this.form.get('host')?.value,
        port: parseInt(this.form.get('port')?.value, 10),
        database: this.form.get('database')?.value,
        schema: this.form.get('schema')?.value,
        username: this.form.get('username')?.value,
        password: this.form.get('password')?.value
      };
    } else {
      const filePath = this.form.get('file_path')?.value || this.selectedFile?.name;
      if (filePath) {
        request.connection = { file_path: filePath };
      }
    }

    return request;
  }

  buildUpdateRequest(): UpdateDataSourceRequest {
    const type = this.form.get('type')?.value;
    const request: any = {
      type: type,
      workspace: this.form.get('workspace')?.value,
      enabled: this.form.get('enabled')?.value ?? true,
    };

    if (type === 'postgis') {
      request.connection = {
        host: this.form.get('host')?.value,
        port: parseInt(this.form.get('port')?.value, 10),
        database: this.form.get('database')?.value,
        schema: this.form.get('schema')?.value,
        username: this.form.get('username')?.value,
        password: this.form.get('password')?.value
      };
    } else {
      const filePath = this.form.get('file_path')?.value;
      if (filePath) {
        request.connection = { file_path: filePath };
      }
    }

    return request;
  }

  onSubmit(): void {
    if (!this.form.valid) {
      this.notificationService.warning('请填写必填字段');
      return;
    }

    const type = this.form.get('type')?.value;

    if (this.mode === 'create' && type !== 'postgis' && this.selectedFile) {
      // 文件型数据源：通过上传接口创建
      const dsName = this.form.get('name')?.value;
      const upload$ = type === 'shapefile'
        ? this.geoserverService.uploadShapefile(this.selectedFile, dsName)
        : this.geoserverService.uploadGeoTiff(this.selectedFile, dsName);

      upload$.subscribe({
        next: () => {
          this.notificationService.success(`${type === 'shapefile' ? 'Shapefile' : 'GeoTIFF'} 上传并创建成功`);
          this.dialogRef.close(true);
        },
        error: (err: any) => {
          console.error('Upload failed:', err);
          this.notificationService.error(err.error?.message || '上传创建失败');
        }
      });
    } else if (this.mode === 'create') {
      this.geoserverService.createDataSource(this.buildCreateRequest()).subscribe({
        next: () => {
          this.notificationService.success('创建成功');
          this.dialogRef.close(true);
        },
        error: (err: any) => {
          console.error('Create failed:', err);
          this.notificationService.error('创建失败');
        }
      });
    } else {
      this.geoserverService.updateDataSource(this.dataSource!.name, this.buildUpdateRequest()).subscribe({
        next: () => {
          this.notificationService.success('更新成功');
          this.dialogRef.close(true);
        },
        error: (err: any) => {
          console.error('Update failed:', err);
          this.notificationService.error('更新失败');
        }
      });
    }
  }

  onCancel(): void {
    this.dialogRef.close(false);
  }
}