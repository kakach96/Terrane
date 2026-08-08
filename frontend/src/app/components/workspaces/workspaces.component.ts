import { Component, OnInit } from '@angular/core';
import { MatDialog } from '@angular/material/dialog';
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
        title: '默认工作空间',
        enabled: true,
        layerCount: 5,
        description: '系统默认工作空间',
      },
      {
        name: 'demo',
        title: '演示工作空间',
        enabled: true,
        layerCount: 3,
        description: '用于演示目的的工作空间',
      },
      {
        name: 'test',
        title: '测试工作空间',
        enabled: false,
        layerCount: 0,
        description: '用于测试的工作空间',
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
        this.notificationService.success('工作空间创建成功');
        this.loading = false;
      },
      error: (error) => {
        console.error('Failed to create workspace:', error);
        this.notificationService.error('创建失败');
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
        this.notificationService.success('工作空间更新成功');
        this.loading = false;
      },
      error: (error) => {
        console.error('Failed to update workspace:', error);
        this.notificationService.error('更新失败');
        this.loading = false;
      },
    });
  }

  deleteWorkspace(workspace: Workspace): void {
    if (workspace.layerCount > 0) {
      this.notificationService.warning('请先删除该工作空间下的所有图层');
      return;
    }

    this.notificationService
      .confirm('确认删除', `确定要删除工作空间 "${workspace.name}" 吗？`)
      .subscribe((confirmed) => {
        if (confirmed) {
          this.loading = true;
          this.geoserverService.deleteWorkspace(workspace.name).subscribe({
            next: () => {
              this.workspaces = this.workspaces.filter((w) => w.name !== workspace.name);
              this.notificationService.success('工作空间删除成功');
              this.loading = false;
            },
            error: (error) => {
              console.error('Failed to delete workspace:', error);
              this.notificationService.error('删除失败');
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
        this.notificationService.success(`工作空间已${newStatus ? '启用' : '禁用'}`);
        this.loading = false;
      },
      error: (error) => {
        console.error('Failed to update workspace status:', error);
        this.notificationService.error('操作失败');
        this.loading = false;
      },
    });
  }
}
