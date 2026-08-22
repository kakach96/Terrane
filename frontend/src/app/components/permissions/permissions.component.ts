import { Component, ChangeDetectionStrategy, inject, signal } from '@angular/core';
import { toSignal, toObservable } from '@angular/core/rxjs-interop';
import { TranslateService } from '@ngx-translate/core';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { Permission, CreatePermissionRequest } from '../../models/geoserver.models';
import { switchMap, tap, startWith, catchError, of } from 'rxjs';

@Component({
  standalone: false,
  selector: 'app-permissions',
  templateUrl: './permissions.component.html',
  styleUrls: ['./permissions.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class PermissionsComponent {
  private geoserver = inject(GeoserverService);
  private notificationService = inject(NotificationService);
  private translate = inject(TranslateService);

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

  private refreshTrigger = signal(0);
  loading = signal(false);

  private permissions$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    tap(() => this.loading.set(true)),
    switchMap(() =>
      this.geoserver.getPermissions().pipe(
        catchError(() => {
          this.notificationService.error(this.translate.instant('permissions.loadFail'));
          return of([] as Permission[]);
        }),
      ),
    ),
    tap(() => this.loading.set(false)),
  );

  permissions = toSignal(this.permissions$, { initialValue: [] as Permission[] });

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
        this.refreshTrigger.update((v) => v + 1);
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
        this.refreshTrigger.update((v) => v + 1);
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