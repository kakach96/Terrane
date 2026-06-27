import { Component } from '@angular/core';
import { Router } from '@angular/router';
import { AuthService } from '../../services/auth.service';
import { NotificationService } from '../../services/notification.service';

@Component({
  selector: 'app-login',
  template: `
    <div class="login-container">
      <mat-card class="login-card">
        <mat-card-header>
          <div class="login-logo">
            <mat-icon class="logo-icon">public</mat-icon>
          </div>
          <mat-card-title>Rust GeoServer</mat-card-title>
          <mat-card-subtitle>登录管理系统</mat-card-subtitle>
        </mat-card-header>

        <mat-card-content>
          <form (ngSubmit)="onSubmit()" #loginForm="ngForm">
            <mat-form-field appearance="outline" class="full-width">
              <mat-label>用户名</mat-label>
              <input matInput [(ngModel)]="username" name="username" required
                     placeholder="admin" autocomplete="username">
              <mat-icon matPrefix>person</mat-icon>
            </mat-form-field>

            <mat-form-field appearance="outline" class="full-width">
              <mat-label>密码</mat-label>
              <input matInput [(ngModel)]="password" name="password"
                     [type]="showPassword ? 'text' : 'password'" required
                     autocomplete="current-password">
              <mat-icon matPrefix>lock</mat-icon>
              <button mat-icon-button matSuffix (click)="showPassword = !showPassword" type="button">
                <mat-icon>{{ showPassword ? 'visibility_off' : 'visibility' }}</mat-icon>
              </button>
            </mat-form-field>

            <div class="error-message" *ngIf="errorMessage">
              <mat-icon>error</mat-icon> {{ errorMessage }}
            </div>

            <button mat-raised-button color="primary" class="full-width login-btn"
                    type="submit" [disabled]="loading">
              <mat-spinner diameter="20" *ngIf="loading" class="spinner"></mat-spinner>
              <span *ngIf="!loading">登 录</span>
            </button>
          </form>
        </mat-card-content>

        <mat-card-footer class="login-footer">
          <span>默认管理员: admin / geoserver</span>
        </mat-card-footer>
      </mat-card>
    </div>
  `,
  styles: [`
    .login-container {
      display: flex;
      justify-content: center;
      align-items: center;
      min-height: 100vh;
      background: linear-gradient(135deg, #1565c0 0%, #0d47a1 100%);
    }
    .login-card {
      width: 400px;
      padding: 32px 24px;
      text-align: center;
    }
    .login-logo { margin-bottom: 8px; }
    .logo-icon { font-size: 48px; width: 48px; height: 48px; color: #1565c0; }
    .full-width { width: 100%; margin-bottom: 16px; }
    .login-btn { height: 44px; font-size: 16px; }
    .spinner { display: inline-block; }
    .error-message {
      color: #f44336;
      font-size: 13px;
      margin-bottom: 12px;
      display: flex;
      align-items: center;
      gap: 4px;
      justify-content: center;
    }
    .login-footer {
      padding: 16px;
      font-size: 12px;
      color: rgba(0,0,0,0.4);
    }
  `]
})
export class LoginComponent {
  username = '';
  password = '';
  showPassword = false;
  loading = false;
  errorMessage = '';

  constructor(
    private authService: AuthService,
    private router: Router,
    private notificationService: NotificationService,
  ) {
    if (this.authService.isLoggedIn()) {
      this.router.navigate(['/dashboard']);
    }
  }

  onSubmit(): void {
    if (!this.username || !this.password) return;
    this.loading = true;
    this.errorMessage = '';

    this.authService.login(this.username, this.password).subscribe({
      next: () => {
        this.notificationService.success('登录成功');
        this.router.navigate(['/dashboard']);
      },
      error: (err) => {
        this.errorMessage = err.error?.message || '登录失败，请检查用户名和密码';
        this.loading = false;
      }
    });
  }
}
