import { Component, OnInit } from '@angular/core';
import { MatSnackBar } from '@angular/material/snack-bar';
import { GeoserverService } from '../../services/geoserver.service';
import { Permission, CreatePermissionRequest } from '../../models/geoserver.models';

@Component({
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
    private snackBar: MatSnackBar,
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
        this.error = '加载权限列表失败';
        this.loading = false;
      },
    });
  }

  createPermission(): void {
    if (!this.newPerm.resourceName) {
      this.snackBar.open('请填写资源名称', '关闭', { duration: 3000 });
      return;
    }
    this.geoserver.createPermission(this.newPerm).subscribe({
      next: () => {
        this.snackBar.open('权限创建成功', '关闭', { duration: 3000 });
        this.showCreateForm = false;
        this.resetForm();
        this.loadPermissions();
      },
      error: (e) => this.snackBar.open(e.error?.message || '创建失败', '关闭', { duration: 5000 }),
    });
  }

  deletePermission(perm: Permission): void {
    if (!perm.id) return;
    const label = `${perm.effect} ${perm.accessMode} ${perm.resourceType}:${perm.resourceName}`;
    if (!confirm(`确认删除权限「${label}」？`)) return;
    this.geoserver.deletePermission(perm.id).subscribe({
      next: () => {
        this.snackBar.open('权限已删除', '关闭', { duration: 3000 });
        this.loadPermissions();
      },
      error: (e) => this.snackBar.open(e.error?.message || '删除失败', '关闭', { duration: 5000 }),
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
}
