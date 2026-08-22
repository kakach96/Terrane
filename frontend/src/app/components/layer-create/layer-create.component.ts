import { Component, ChangeDetectionStrategy, inject } from '@angular/core';
import { Router } from '@angular/router';
import { FormBuilder, FormGroup, Validators } from '@angular/forms';
import { TranslateService } from '@ngx-translate/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { Workspace, DataSource } from '../../models/geoserver.models';
import { switchMap, tap, map, distinctUntilChanged, catchError, of } from 'rxjs';

@Component({
  standalone: false,
  selector: 'app-layer-create',
  templateUrl: './layer-create.component.html',
  styleUrls: ['./layer-create.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class LayerCreateComponent {
  private fb = inject(FormBuilder);
  private geoserverService = inject(GeoserverService);
  private notificationService = inject(NotificationService);
  private router = inject(Router);
  private translate = inject(TranslateService);

  layerForm!: FormGroup;
  loading = false;
  /** metadata 内置数据源且无已有业务表时: 数据表自动用图层名 */
  metadataNewTable = false;

  // ── Signal pipeline: workspaces ───────────────────────────────────
  private workspaces$ = this.geoserverService.getAllWorkspaces().pipe(
    catchError(() => of([] as Workspace[])),
  );

  workspaces = toSignal(this.workspaces$, { initialValue: [] as Workspace[] });

  // ── Signal pipelines: form-driven cascading data ──────────────────
  private dataSources$ = this.layerForm?.get('workspace')?.valueChanges.pipe(
    distinctUntilChanged(),
    tap(() => {
      this.layerForm.get('dataSource')?.setValue('');
      this.layerForm.get('table')?.setValue('');
    }),
    switchMap((workspaceName: string) => {
      if (!workspaceName) return of([] as DataSource[]);
      return this.geoserverService.getDataSources().pipe(
        map((dataSources) =>
          dataSources.filter(
            (ds) => ds.workspace === workspaceName || ds.name === 'metadata',
          ),
        ),
        catchError(() => of([] as DataSource[])),
      );
    }),
  ) ?? of([] as DataSource[]);

  dataSources = toSignal(this.dataSources$, { initialValue: [] as DataSource[] });

  private tables$ = this.layerForm?.get('dataSource')?.valueChanges.pipe(
    distinctUntilChanged(),
    switchMap((dataSourceName: string) => {
      if (!dataSourceName) {
        this.metadataNewTable = false;
        this.layerForm.get('table')?.setValue('');
        return of([] as string[]);
      }
      const ds = this.dataSources().find((d) => d.name === dataSourceName);
      if (!ds || ds.type !== 'postgis') {
        this.metadataNewTable = false;
        this.layerForm.get('table')?.setValue('');
        return of([] as string[]);
      }
      return this.geoserverService.getDataSourceTables(dataSourceName).pipe(
        tap((tables) => {
          if (ds.name === 'metadata') {
            if (tables.length > 0) {
              this.metadataNewTable = false;
              this.layerForm.get('table')?.setValue('');
            } else {
              this.metadataNewTable = true;
              const layerName = this.layerForm.get('name')?.value;
              this.layerForm.get('table')?.setValue(layerName || '');
            }
          } else {
            this.layerForm.get('table')?.setValue('');
          }
        }),
        catchError(() => {
          this.metadataNewTable = false;
          return of([] as string[]);
        }),
      );
    }),
  ) ?? of([] as string[]);

  tables = toSignal(this.tables$, { initialValue: [] as string[] });

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

    // metadata 内置数据源且无已有业务表时: 数据表自动用图层名
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

    this.geoserverService.createLayer(layerData).subscribe({
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
      maxy: 90,
    });
  }

  goBack(): void {
    this.router.navigate(['/layers']);
  }

  trackByIndex(index: number): number {
    return index;
  }
}