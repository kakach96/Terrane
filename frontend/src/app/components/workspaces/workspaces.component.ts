import { Component, ChangeDetectionStrategy, inject, signal } from '@angular/core';
import { toSignal, toObservable } from '@angular/core/rxjs-interop';
import { MatDialog } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
import { WorkspaceDialogComponent } from './workspace-dialog.component';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import {
  Workspace,
  CreateWorkspaceRequest,
  UpdateWorkspaceRequest,
} from '../../models/geoserver.models';
import { switchMap, tap, startWith, catchError, of } from 'rxjs';

@Component({
  standalone: false,
  selector: 'app-workspaces',
  templateUrl: './workspaces.component.html',
  styleUrls: ['./workspaces.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class WorkspacesComponent {
  private geoserverService = inject(GeoserverService);
  private notificationService = inject(NotificationService);
  private dialog = inject(MatDialog);
  private translate = inject(TranslateService);

  displayedColumns = ['name', 'title', 'layerCount', 'status', 'actions'];

  private refreshTrigger = signal(0);
  loading = signal(false);

  private workspaces$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    tap(() => this.loading.set(true)),
    switchMap(() =>
      this.geoserverService.getAllWorkspaces().pipe(
        catchError(() => {
          return of(this.getDefaultWorkspaces());
        }),
      ),
    ),
    tap(() => this.loading.set(false)),
  );

  workspaces = toSignal(this.workspaces$, { initialValue: [] as Workspace[] });

  getDefaultWorkspaces(): Workspace[] {
    return [
      {
        name: 'default',
        title: this.translate.instant('workspaces.defaultWsTitle'),
        enabled: true,
        layerCount: 5,
        description: this.translate.instant('workspaces.defaultWsDesc'),
      },
      {
        name: 'demo',
        title: this.translate.instant('workspaces.demoWsTitle'),
        enabled: true,
        layerCount: 3,
        description: this.translate.instant('workspaces.demoWsDesc'),
      },
      {
        name: 'test',
        title: this.translate.instant('workspaces.testWsTitle'),
        enabled: false,
        layerCount: 0,
        description: this.translate.instant('workspaces.testWsDesc'),
      },
    ];
  }

  openCreateDialog(): void {
    const dialogRef = this.dialog.open(WorkspaceDialogComponent, {
      width: '480px',
    });

    dialogRef.afterClosed().subscribe((result: CreateWorkspaceRequest) => {
      if (result) {
        this.createWorkspace(result);
      }
    });
  }

  openEditDialog(workspace: Workspace): void {
    const dialogRef = this.dialog.open(WorkspaceDialogComponent, {
      width: '480px',
      data: { workspace },
    });

    dialogRef.afterClosed().subscribe((result: UpdateWorkspaceRequest) => {
      if (result) {
        this.updateWorkspace(workspace.name, result);
      }
    });
  }

  createWorkspace(request: CreateWorkspaceRequest): void {
    this.loading.set(true);
    this.geoserverService.createWorkspace(request).subscribe({
      next: () => {
        this.notificationService.success(this.translate.instant('workspaces.createSuccess'));
        this.loading.set(false);
        this.refreshTrigger.update((v) => v + 1);
      },
      error: (error) => {
        console.error('Failed to create workspace:', error);
        this.notificationService.error(this.translate.instant('workspaces.createFail'));
        this.loading.set(false);
      },
    });
  }

  updateWorkspace(name: string, request: UpdateWorkspaceRequest): void {
    this.loading.set(true);
    this.geoserverService.updateWorkspace(name, request).subscribe({
      next: () => {
        this.notificationService.success(this.translate.instant('workspaces.updateSuccess'));
        this.loading.set(false);
        this.refreshTrigger.update((v) => v + 1);
      },
      error: (error) => {
        console.error('Failed to update workspace:', error);
        this.notificationService.error(this.translate.instant('workspaces.updateFail'));
        this.loading.set(false);
      },
    });
  }

  deleteWorkspace(workspace: Workspace): void {
    if (workspace.layerCount > 0) {
      this.notificationService.warning(this.translate.instant('workspaces.warningHasLayers'));
      return;
    }

    this.notificationService
      .confirm(
        this.translate.instant('workspaces.deleteConfirmTitle'),
        this.translate.instant('workspaces.deleteConfirmMessage', { name: workspace.name }),
      )
      .subscribe((confirmed) => {
        if (confirmed) {
          this.loading.set(true);
          this.geoserverService.deleteWorkspace(workspace.name).subscribe({
            next: () => {
              this.notificationService.success(this.translate.instant('workspaces.deleteSuccess'));
              this.loading.set(false);
              this.refreshTrigger.update((v) => v + 1);
            },
            error: (error) => {
              console.error('Failed to delete workspace:', error);
              this.notificationService.error(this.translate.instant('workspaces.deleteFail'));
              this.loading.set(false);
            },
          });
        }
      });
  }

  toggleStatus(workspace: Workspace): void {
    const newStatus = !workspace.enabled;
    this.loading.set(true);
    this.geoserverService.updateWorkspace(workspace.name, { enabled: newStatus }).subscribe({
      next: () => {
        this.notificationService.success(
          this.translate.instant('workspaces.toggleStatusSuccess', {
            status: newStatus
              ? this.translate.instant('common.enabled')
              : this.translate.instant('common.disabled'),
          }),
        );
        this.loading.set(false);
        this.refreshTrigger.update((v) => v + 1);
      },
      error: (error) => {
        console.error('Failed to update workspace status:', error);
        this.notificationService.error(this.translate.instant('workspaces.operationFail'));
        this.loading.set(false);
      },
    });
  }
}
