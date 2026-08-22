import { Component, OnInit } from '@angular/core';
import { TranslateService } from '@ngx-translate/core';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { Permission, CreatePermissionRequest } from '../../models/geoserver.models';

@Component({
  standalone: false,
  selector: 'app-permissions',
  templateUrl: './permissions.component.html',
  styleUrls: ['./permissions.component.scss'],
})
export class PermissionsComponent implements OnInit {
  permissions: Permission[] = [];
  loading = false;
  error = '';

  // 新建权限
  showCreateForm = false;
  newPerm: CreatePermissionRequest = {
    resourceType: 'layer',
    resourceName: '',
    accessMode: 'read',
    effect: 'allow',
    priority: 0,
    username: '',
    role: '',
  };

  // 预定义角色选项
  roleOptions = ['admin', 'manager', 'user', 'guest'];
  typeOptions = ['layer', 'workspace', 'namespace', 'layerGroup', 'store'];
  modeOptions = ['read', 'write', 'admin'];
  effectOptions = ['allow', 'deny'];

  constructor(
    private geoserver: GeoserverService,
    private notificationService: NotificationService,
    private translate: TranslateService,
  ) {}

  ngOnInit(): void {
    this.loadPermissions();
  }

  loadPermissions(): void {
    this.loading = true;
    this.geoserver.getPermissions().subscribe({
      next: (perms) => {
        this.permissions = perms;
        this.loading = false;
      },
      error: () => {
        this.error = this.translate.instant('permissions.loadFail');
        this.loading = false;
      },
    });
  }

  createPermission(): void {
    if (!this.newPerm.resourceName) {
      this.notificationService.warning(this.translate.instant('permissions.resourceNameRequired'));
      return;
    }
    this.geoserver.createPermission(this.newPerm).subscribe({
      next: () => {
        this.notificationService.success(this.translate.instant('permissions.createSuccess'));
        this.showCreateForm = false;
        this.resetForm();
        this.loadPermissions();
      },
      error: (e) => this.notificationService.error(this.notificationService.fromError(e)),
    });
  }

  deletePermission(perm: Permission): void {
    if (!perm.id) return;
    const label = `${perm.effect} ${perm.accessMode} ${perm.resourceType}:${perm.resourceName}`;
    if (!confirm(this.translate.instant('permissions.deleteConfirm', { label }))) return;
    this.geoserver.deletePermission(perm.id).subscribe({
      next: () => {
        this.notificationService.success(this.translate.instant('permissions.deleteSuccess'));
        this.loadPermissions();
      },
      error: (e) => this.notificationService.error(this.notificationService.fromError(e)),
    });
  }

  getEffectColor(effect: string): string {
    return effect === 'allow' ? '#2e7d32' : '#c62828';
  }

  private resetForm(): void {
    this.newPerm = {
      resourceType: 'layer',
      resourceName: '',
      accessMode: 'read',
      effect: 'allow',
      priority: 0,
      username: '',
      role: '',
    };
  }
  trackByIndex(index: number): number {
    return index;
  }

}