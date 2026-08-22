import { Component, OnInit, ChangeDetectorRef } from '@angular/core';
import { TranslateService } from '@ngx-translate/core';
import { GeoserverService } from '../../services/geoserver.service';
import { AuthService } from '../../services/auth.service';
import { NotificationService } from '../../services/notification.service';
import { User } from '../../models/geoserver.models';

@Component({
  standalone: false,
  selector: 'app-users',
  templateUrl: './users.component.html',
  styleUrls: ['./users.component.scss'],
})
export class UsersComponent implements OnInit {
  users: User[] = [];
  loading = false;
  error = '';

  // 创建用户
  showCreateForm = false;
  newUsername = '';
  newPassword = '';
  newRole = 'user';

  // 修改密码
  showPasswordForm = false;
  oldPassword = '';
  newPassword1 = '';
  newPassword2 = '';

  constructor(
    private geoserver: GeoserverService,
    private auth: AuthService,
    private notificationService: NotificationService,
    private translate: TranslateService,
    private cdr: ChangeDetectorRef,
  ) {}

  ngOnInit(): void {
    this.loadUsers();
  }

  loadUsers(): void {
    this.loading = true;
    this.geoserver.listUsers().subscribe({
      next: (users) => {
        this.users = users;
        this.loading = false;
        this.cdr.detectChanges();
      },
      error: () => {
        this.error = this.translate.instant('users.loadFail');
        this.loading = false;
        this.cdr.detectChanges();
      },
    });
  }

  createUser(): void {
    if (!this.newUsername || !this.newPassword) return;
    this.geoserver.createUser(this.newUsername, this.newPassword, this.newRole).subscribe({
      next: () => {
        this.notificationService.success(this.translate.instant('users.createSuccess'));
        this.showCreateForm = false;
        this.newUsername = '';
        this.newPassword = '';
        this.newRole = 'user';
        this.loadUsers();
      },
      error: (e) => this.notificationService.error(this.notificationService.fromError(e)),
    });
  }

  deleteUser(username: string): void {
    if (username === 'admin') {
      this.notificationService.warning(this.translate.instant('users.cannotDeleteAdmin'));
      return;
    }
    if (!confirm(this.translate.instant('users.deleteUserConfirm', { username }))) return;
    this.geoserver.deleteUser(username).subscribe({
      next: () => {
        this.notificationService.success(this.translate.instant('users.deleteSuccess'));
        this.loadUsers();
      },
      error: (e) => this.notificationService.error(this.notificationService.fromError(e)),
    });
  }

  changePassword(): void {
    if (this.newPassword1 !== this.newPassword2) {
      this.notificationService.warning(this.translate.instant('users.passwordMismatch'));
      return;
    }
    this.geoserver.changePassword(this.oldPassword, this.newPassword1).subscribe({
      next: () => {
        this.notificationService.success(this.translate.instant('users.passwordChangeSuccess'));
        this.showPasswordForm = false;
        this.oldPassword = '';
        this.newPassword1 = '';
        this.newPassword2 = '';
      },
      error: (e) => this.notificationService.error(this.notificationService.fromError(e)),
    });
  }

  getRoleColor(role: string): string {
    switch (role) {
      case 'admin':
        return 'warn';
      case 'manager':
        return 'primary';
      case 'user':
        return 'accent';
      default:
        return '';
    }
  }

  /** Localized role label for a user role. */
  roleLabel(role: string): string {
    switch (role) {
      case 'admin':
        return this.translate.instant('users.roleAdmin');
      case 'manager':
        return this.translate.instant('users.roleManager');
      case 'guest':
        return this.translate.instant('users.roleGuest');
      default:
        return this.translate.instant('users.roleUser');
    }
  }

  /** Localized enabled/disabled label. */
  statusLabel(enabled: boolean): string {
    return enabled
      ? this.translate.instant('common.enabled')
      : this.translate.instant('common.disabled');
  }
}
