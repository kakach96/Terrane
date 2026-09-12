import { Component, ChangeDetectionStrategy, inject, Signal } from '@angular/core';
import { Router } from '@angular/router';
import { FormBuilder, FormGroup, Validators } from '@angular/forms';
import { TranslateService } from '@ngx-translate/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { TerraneService } from '../../services/terrane.service';
import { NotificationService } from '../../services/notification.service';
import { Workspace, DataSource } from '../../models/terrane.models';
import { Observable, switchMap, tap, map, distinctUntilChanged, catchError, of } from 'rxjs';

@Component({
  standalone: false,
  selector: 'app-layer-create',
  templateUrl: './layer-create.component.html',
  styleUrls: ['./layer-create.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class LayerCreateComponent {
  private fb = inject(FormBuilder);
  private terraneService = inject(TerraneService);
  private notificationService = inject(NotificationService);
  private router = inject(Router);
  private translate = inject(TranslateService);

  layerForm!: FormGroup;
  loading = false;
  /** When the metadata built-in data source has no existing business tables: auto-use the layer name as the table */
  metadataNewTable = false;
  /** Selected data source is file-based: the cascade lists directory files instead of DB tables */
  fileTableMode = false;

  /** File-based data-source types publish per-file layers from a directory.
   * ImageMosaic / ImagePyramid are excluded: their file_path is the raster
   * directory itself, no file-level cascade applies. */
  private isFileBasedType(type: string): boolean {
    return ['geojson', 'shapefile', 'geopackage', 'geotiff', 'worldimage', 'arcgrid'].includes(
      type,
    );
  }

  /**
   * Mark the table/file control required only when the selected data source
   * actually needs a native_name (PostGIS table / directory-level data file).
   * Single-file and directory-semantics sources (ImageMosaic / ImagePyramid)
   * publish layers without a native_name.
   */
  private setTableRequired(required: boolean): void {
    const ctrl = this.layerForm.get('table');
    if (!ctrl) return;
    ctrl.setValidators(required ? [Validators.required] : null);
    ctrl.updateValueAndValidity();
  }

  // ── Signal pipeline: workspaces ───────────────────────────────────
  private workspaces$ = this.terraneService
    .getAllWorkspaces()
    .pipe(catchError(() => of([] as Workspace[])));

  workspaces = toSignal(this.workspaces$, { initialValue: [] as Workspace[] });

  // ── Signal pipelines: form-driven cascading data ──────────────────
  // NOTE: these are initialized in the constructor AFTER layerForm is built,
  // because they subscribe to layerForm controls' valueChanges. Declaring them
  // as class-field initializers would run before the constructor body, when
  // layerForm is still undefined, so the cascading would never fire.
  private dataSources$!: Observable<DataSource[]>;
  private tables$!: Observable<string[]>;
  dataSources!: Signal<DataSource[]>;
  tables!: Signal<string[]>;

  constructor() {
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
      maxy: [90],
    });

    // Cascading data sources: reload when the workspace changes.
    this.dataSources$ = this.layerForm.get('workspace')!.valueChanges.pipe(
      distinctUntilChanged(),
      tap(() => {
        this.layerForm.get('dataSource')?.setValue('');
        this.layerForm.get('table')?.setValue('');
      }),
      switchMap((workspaceName: string) => {
        if (!workspaceName) return of([] as DataSource[]);
        return this.terraneService.getDataSources().pipe(
          map((dataSources) =>
            dataSources.filter((ds) => ds.workspace === workspaceName || ds.name === 'metadata'),
          ),
          catchError(() => of([] as DataSource[])),
        );
      }),
    );
    this.dataSources = toSignal(this.dataSources$, { initialValue: [] as DataSource[] });

    // Cascading tables: reload when the data source changes.
    this.tables$ = this.layerForm.get('dataSource')!.valueChanges.pipe(
      distinctUntilChanged(),
      switchMap((dataSourceName: string) => {
        const tableCtrl = this.layerForm.get('table')!;
        if (!dataSourceName) {
          this.metadataNewTable = false;
          this.fileTableMode = false;
          tableCtrl.setValue('');
          this.setTableRequired(false);
          return of([] as string[]);
        }
        const ds = this.dataSources().find((d) => d.name === dataSourceName);
        // 表/文件级联: PostGIS 走数据库表列表; 文件类数据源走目录文件列表
        // (目录级数据源按文件发布, native_name = 目录内的文件名)
        const fileBased = !!ds && this.isFileBasedType(ds.type);
        if (!ds || (ds.type !== 'postgis' && !fileBased)) {
          this.metadataNewTable = false;
          this.fileTableMode = false;
          tableCtrl.setValue('');
          this.setTableRequired(false);
          return of([] as string[]);
        }
        return this.terraneService.getDataSourceTables(dataSourceName).pipe(
          tap((tables) => {
            // 仅目录级文件数据源有文件列表; 单文件 / 其他类型无级联
            this.fileTableMode = fileBased && tables.length > 0;
            if (ds.name === 'metadata') {
              if (tables.length > 0) {
                this.metadataNewTable = false;
                tableCtrl.setValue('');
              } else {
                this.metadataNewTable = true;
                const layerName = this.layerForm.get('name')?.value;
                tableCtrl.setValue(layerName || '');
              }
              this.setTableRequired(true);
            } else {
              tableCtrl.setValue('');
              // PostGIS → 表必填; 文件类 → 仅目录级有文件列表时必填
              this.setTableRequired(fileBased ? this.fileTableMode : true);
            }
          }),
          catchError(() => {
            this.metadataNewTable = false;
            this.fileTableMode = false;
            this.setTableRequired(false);
            return of([] as string[]);
          }),
        );
      }),
    );
    this.tables = toSignal(this.tables$, { initialValue: [] as string[] });

    // When the metadata built-in data source has no existing business tables: auto-use the layer name as the table
    this.layerForm.get('name')?.valueChanges.subscribe((name: string) => {
      if (this.metadataNewTable) {
        this.layerForm.get('table')?.setValue(name || '');
      }
    });
  }

  // ── Actions ───────────────────────────────────────────────────────
  onSubmit(): void {
    if (this.layerForm.invalid) {
      this.notificationService.error(this.translate.instant('layerCreate.formInvalid'));
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
        maxy: formValue.maxy,
      },
    };

    this.terraneService.createLayer(layerData).subscribe({
      next: (layer) => {
        this.notificationService.success(
          this.translate.instant('layerCreate.success', { name: layer.name }),
        );
        this.loading = false;
        this.router.navigate(['/layers', layer.name]);
      },
      error: (error) => {
        this.notificationService.error(
          this.translate.instant('layerCreate.createFail', {
            message: error.message || this.translate.instant('common.unknown'),
          }),
        );
        this.loading = false;
      },
    });
  }

  goBack(): void {
    this.router.navigate(['/layers']);
  }

  trackByIndex(index: number): number {
    return index;
  }
}
