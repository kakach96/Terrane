import { Component, OnInit } from '@angular/core';
import { MatDialog } from '@angular/material/dialog';
import { NamespaceDialogComponent } from './namespace-dialog.component';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import {
  Namespace,
  CreateNamespaceRequest,
  UpdateNamespaceRequest,
} from '../../models/geoserver.models';

@Component({
  selector: 'app-namespaces',
  templateUrl: './namespaces.component.html',
  styleUrls: ['./namespaces.component.scss'],
})
export class NamespacesComponent implements OnInit {
  namespaces: Namespace[] = [];
  loading = false;
  displayedColumns = ['prefix', 'uri', 'workspace', 'isolated', 'actions'];

  constructor(
    private geoserverService: GeoserverService,
    private notificationService: NotificationService,
    private dialog: MatDialog,
  ) {}

  ngOnInit(): void {
    this.loadNamespaces();
  }

  loadNamespaces(): void {
    this.loading = true;
    this.geoserverService.getNamespaces().subscribe({
      next: (data) => {
        this.namespaces = data;
        this.loading = false;
      },
      error: (error) => {
        console.error('Failed to load namespaces:', error);
        this.loading = false;
      },
    });
  }

  openCreateDialog(): void {
    const dialogRef = this.dialog.open(NamespaceDialogComponent, {
      width: '520px',
    });

    dialogRef.afterClosed().subscribe((result: CreateNamespaceRequest) => {
      if (result) {
        this.createNamespace(result);
      }
    });
  }

  openEditDialog(ns: Namespace): void {
    const dialogRef = this.dialog.open(NamespaceDialogComponent, {
      width: '520px',
      data: { namespace: ns },
    });

    dialogRef.afterClosed().subscribe((result: UpdateNamespaceRequest) => {
      if (result) {
        this.updateNamespace(ns.prefix, result);
      }
    });
  }

  createNamespace(request: CreateNamespaceRequest): void {
    this.loading = true;
    this.geoserverService.createNamespace(request).subscribe({
      next: (ns) => {
        this.namespaces.push(ns);
        this.notificationService.success('命名空间创建成功');
        this.loading = false;
      },
      error: (error) => {
        console.error('Failed to create namespace:', error);
        this.notificationService.error('创建失败');
        this.loading = false;
      },
    });
  }

  updateNamespace(prefix: string, request: UpdateNamespaceRequest): void {
    this.loading = true;
    this.geoserverService.updateNamespace(prefix, request).subscribe({
      next: () => {
        const index = this.namespaces.findIndex((n) => n.prefix === prefix);
        if (index !== -1) {
          this.namespaces[index] = { ...this.namespaces[index], ...request };
        }
        this.notificationService.success('命名空间更新成功');
        this.loading = false;
      },
      error: (error) => {
        console.error('Failed to update namespace:', error);
        this.notificationService.error('更新失败');
        this.loading = false;
      },
    });
  }

  deleteNamespace(ns: Namespace): void {
    this.notificationService
      .confirm('确认删除', `确定要删除命名空间 "${ns.prefix}" 吗？`)
      .subscribe((confirmed) => {
        if (confirmed) {
          this.loading = true;
          this.geoserverService.deleteNamespace(ns.prefix).subscribe({
            next: () => {
              this.namespaces = this.namespaces.filter((n) => n.prefix !== ns.prefix);
              this.notificationService.success('命名空间删除成功');
              this.loading = false;
            },
            error: (error) => {
              console.error('Failed to delete namespace:', error);
              this.notificationService.error('删除失败');
              this.loading = false;
            },
          });
        }
      });
  }

  toggleIsolated(ns: Namespace): void {
    const newStatus = !ns.isolated;
    this.loading = true;
    this.geoserverService.updateNamespace(ns.prefix, { isolated: newStatus }).subscribe({
      next: () => {
        ns.isolated = newStatus;
        this.notificationService.success(`命名空间已${newStatus ? '启用隔离' : '关闭隔离'}`);
        this.loading = false;
      },
      error: (error) => {
        console.error('Failed to update namespace:', error);
        this.notificationService.error('操作失败');
        this.loading = false;
      },
    });
  }
}
