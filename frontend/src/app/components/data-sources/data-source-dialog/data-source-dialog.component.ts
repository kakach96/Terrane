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

  testConnection(): void {
    if (!this.form.valid) {
      this.notificationService.warning('请填写必填字段');
      return;
    }

    this.isTesting = true;
    const request = this.buildCreateRequest();
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
      enabled: this.form.get('enabled')?.value,
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
    }

    return request;
  }

  buildUpdateRequest(): UpdateDataSourceRequest {
    const type = this.form.get('type')?.value;
    const request: any = {
      type: type,
      workspace: this.form.get('workspace')?.value,
      enabled: this.form.get('enabled')?.value,
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
    }

    return request;
  }

  onSubmit(): void {
    if (!this.form.valid) {
      this.notificationService.warning('请填写必填字段');
      return;
    }

    if (this.mode === 'create') {
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