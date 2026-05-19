import { Component, OnInit } from '@angular/core';
import { Router } from '@angular/router';
import { FormBuilder, FormGroup, Validators } from '@angular/forms';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { Workspace, DataSource } from '../../models/geoserver.models';

@Component({
  selector: 'app-layer-create',
  templateUrl: './layer-create.component.html',
  styleUrls: ['./layer-create.component.scss']
})
export class LayerCreateComponent implements OnInit {
  layerForm!: FormGroup;
  loading = false;
  workspaces: Workspace[] = [];
  dataSources: DataSource[] = [];
  tables: string[] = [];
  loadingDataSources = false;
  loadingTables = false;

  constructor(
    private fb: FormBuilder,
    private geoserverService: GeoserverService,
    private notificationService: NotificationService,
    private router: Router
  ) {}

  ngOnInit(): void {
    this.initForm();
    this.loadWorkspaces();
  }

  initForm(): void {
    this.layerForm = this.fb.group({
      name: ['', [Validators.required, Validators.pattern(/^[a-z][a-z0-9_]*$/)]],
      title: ['', Validators.required],
      workspace: ['', Validators.required],
      dataSource: ['', Validators.required],
      table: ['', Validators.required],
      srs: ['EPSG:4326', Validators.required],
      abstract: [''],
      minx: [-180],
      miny: [-90],
      maxx: [180],
      maxy: [90]
    });

    this.layerForm.get('workspace')?.valueChanges.subscribe(workspaceName => {
      if (workspaceName) {
        this.loadDataSourcesForWorkspace(workspaceName);
      } else {
        this.dataSources = [];
        this.tables = [];
        this.layerForm.get('dataSource')?.setValue('');
        this.layerForm.get('table')?.setValue('');
      }
    });

    this.layerForm.get('dataSource')?.valueChanges.subscribe(dataSourceName => {
      if (dataSourceName) {
        this.loadTablesForDataSource(dataSourceName);
      } else {
        this.tables = [];
        this.layerForm.get('table')?.setValue('');
      }
    });
  }

  loadWorkspaces(): void {
    this.geoserverService.getAllWorkspaces().subscribe({
      next: (workspaces) => {
        this.workspaces = workspaces;
      }
    });
  }

  loadDataSourcesForWorkspace(workspaceName: string): void {
    this.loadingDataSources = true;
    this.geoserverService.getDataSources().subscribe({
      next: (dataSources) => {
        this.dataSources = dataSources.filter(ds => ds.workspace === workspaceName);
        this.loadingDataSources = false;
        this.layerForm.get('dataSource')?.setValue('');
        this.tables = [];
        this.layerForm.get('table')?.setValue('');
      },
      error: (err) => {
        console.error('Failed to load data sources:', err);
        this.loadingDataSources = false;
        this.dataSources = [];
      }
    });
  }

  loadTablesForDataSource(dataSourceName: string): void {
     const dataSource = this.dataSources.find(ds => ds.name === dataSourceName);
     if (!dataSource || dataSource.type !== 'postgis') {
       this.tables = [];
       this.layerForm.get('table')?.setValue('');
       return;
     }

    this.loadingTables = true;
    this.geoserverService.getDataSourceTables(dataSourceName).subscribe({
      next: (tables) => {
        this.tables = tables;
        this.loadingTables = false;
        this.layerForm.get('table')?.setValue('');
      },
      error: (err) => {
        console.error('Failed to load tables:', err);
        this.loadingTables = false;
        this.tables = [];
      }
    });
  }

  onSubmit(): void {
    if (this.layerForm.invalid) {
      this.notificationService.error('请检查表单填写');
      return;
    }

    this.loading = true;
    const formValue = this.layerForm.value;
    const layerData = {
      name: formValue.name,
      title: formValue.title,
      workspace: formValue.workspace,
      store: formValue.dataSource,
      native_name: formValue.table,
      srs: formValue.srs,
      abstract: formValue.abstract,
      bounds: {
        minx: formValue.minx,
        miny: formValue.miny,
        maxx: formValue.maxx,
        maxy: formValue.maxy
      }
    };

    this.geoserverService.createLayer(layerData).subscribe({
      next: (layer) => {
        this.notificationService.success(`图层 "${layer.name}" 创建成功`);
        this.loading = false;
        this.router.navigate(['/layers', layer.name]);
      },
      error: (error) => {
        this.notificationService.error('创建失败: ' + (error.message || '未知错误'));
        this.loading = false;
      }
    });
  }

  resetForm(): void {
    this.layerForm.reset({
      name: '',
      title: '',
      workspace: '',
      dataSource: '',
      table: '',
      srs: 'EPSG:4326',
      abstract: '',
      minx: -180,
      miny: -90,
      maxx: 180,
      maxy: 90
    });
    this.dataSources = [];
    this.tables = [];
  }

  goBack(): void {
    this.router.navigate(['/layers']);
  }
}
