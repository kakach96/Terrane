import { Component, OnInit } from '@angular/core';
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

@Component({
  selector: 'app-workspaces',
  templateUrl: './workspaces.component.html',
  styleUrls: ['./workspaces.component.scss'],
})
export class WorkspacesComponent implements OnInit {
  workspaces: Workspace[] = [];
  loading = false;
  displayedColumns = ['name', 'title', 'layerCount', 'status', 'actions'];

  constructor(
    private geoserverService: GeoserverService,
    private notificationService: NotificationService,
    private dialog: MatDialog,
    private translate: TranslateService,
  ) {}

  ngOnInit(): void {
    this.loadWorkspaces();
  }

  loadWorkspaces(): void {
    this.loading = true;
    this.geoserverService.getAllWorkspaces().subscribe({
      next: (data) => {
        this.workspaces = data;
        this.loading = false;
      },
      error: (error) => {
        console.error('Failed to load workspaces:', error);
        this.workspaces = this.getDefaultWorkspaces();
        this.loading = false;
      },
    });
  }

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
    this.loading = true;
    this.geoserverService.createWorkspace(request).subscribe({
      next: (workspace) => {
        this.workspaces.push(workspace);
        this.notificationService.success(this.translate.instant('workspaces.createSuccess'));
        this.loading = false;
      },
      error: (error) => {
        console.error('Failed to create workspace:', error);
        this.notificationService.error(this.translate.instant('workspaces.createFail'));
        this.loading = false;
      },
    });
  }

  updateWorkspace(name: string, request: UpdateWorkspaceRequest): void {
    this.loading = true;
    this.geoserverService.updateWorkspace(name, request).subscribe({
      next: () => {
        const index = this.workspaces.findIndex((w) => w.name === name);
        if (index !== -1) {
          this.workspaces[index] = { ...this.workspaces[index], ...request };
        }
        this.notificationService.success(this.translate.instant('workspaces.updateSuccess'));
        this.loading = false;
      },
      error: (error) => {
        console.error('Failed to update workspace:', error);
        this.notificationService.error(this.translate.instant('workspaces.updateFail'));
        this.loading = false;
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
          this.loading = true;
          this.geoserverService.deleteWorkspace(workspace.name).subscribe({
            next: () => {
              this.workspaces = this.workspaces.filter((w) => w.name !== workspace.name);
              this.notificationService.success(this.translate.instant('workspaces.deleteSuccess'));
              this.loading = false;
            },
            error: (error) => {
              console.error('Failed to delete workspace:', error);
              this.notificationService.error(this.translate.instant('workspaces.deleteFail'));
              this.loading = false;
            },
          });
        }
      });
  }

  toggleStatus(workspace: Workspace): void {
    const newStatus = !workspace.enabled;
    this.loading = true;
    this.geoserverService.updateWorkspace(workspace.name, { enabled: newStatus }).subscribe({
      next: () => {
        workspace.enabled = newStatus;
        this.notificationService.success(
          this.translate.instant('workspaces.toggleStatusSuccess', {
            status: newStatus
              ? this.translate.instant('common.enabled')
              : this.translate.instant('common.disabled'),
          }),
        );
        this.loading = false;
      },
      error: (error) => {
        console.error('Failed to update workspace status:', error);
        this.notificationService.error(this.translate.instant('workspaces.operationFail'));
        this.loading = false;
      },
    });
  }
}
