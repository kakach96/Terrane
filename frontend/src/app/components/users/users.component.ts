import { Component, OnInit } from '@angular/core';
import { MatSnackBar } from '@angular/material/snack-bar';
import { GeoserverService } from '../../services/geoserver.service';
import { AuthService } from '../../services/auth.service';
import { User } from '../../models/geoserver.models';

@Component({
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
    private snackBar: MatSnackBar,
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
      },
      error: () => {
        this.error = '加载用户列表失败';
        this.loading = false;
      },
    });
  }

  createUser(): void {
    if (!this.newUsername || !this.newPassword) return;
    this.geoserver.createUser(this.newUsername, this.newPassword, this.newRole).subscribe({
      next: () => {
        this.snackBar.open('用户创建成功', '关闭', { duration: 3000 });
        this.showCreateForm = false;
        this.newUsername = '';
        this.newPassword = '';
        this.newRole = 'user';
        this.loadUsers();
      },
      error: (e) => this.snackBar.open(e.error?.message || '创建失败', '关闭', { duration: 5000 }),
    });
  }

  deleteUser(username: string): void {
    if (username === 'admin') {
      this.snackBar.open('不能删除默认管理员', '关闭', { duration: 3000 });
      return;
    }
    if (!confirm(`确认删除用户「${username}」？`)) return;
    this.geoserver.deleteUser(username).subscribe({
      next: () => {
        this.snackBar.open('用户已删除', '关闭', { duration: 3000 });
        this.loadUsers();
      },
      error: (e) => this.snackBar.open(e.error?.message || '删除失败', '关闭', { duration: 5000 }),
    });
  }

  changePassword(): void {
    if (this.newPassword1 !== this.newPassword2) {
      this.snackBar.open('两次密码不一致', '关闭', { duration: 3000 });
      return;
    }
    this.geoserver.changePassword(this.oldPassword, this.newPassword1).subscribe({
      next: () => {
        this.snackBar.open('密码修改成功', '关闭', { duration: 3000 });
        this.showPasswordForm = false;
        this.oldPassword = '';
        this.newPassword1 = '';
        this.newPassword2 = '';
      },
      error: (e) => this.snackBar.open(e.error?.message || '修改失败', '关闭', { duration: 5000 }),
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
}
