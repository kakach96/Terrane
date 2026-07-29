import { Component } from '@angular/core';
import { AuthService } from '../../services/auth.service';
import { NotificationService } from '../../services/notification.service';
import { MatDialogRef } from '@angular/material/dialog';

@Component({
  selector: 'app-login',
  templateUrl: './login.component.html',
  styleUrls: ['./login.component.scss']
})
export class LoginComponent {
  username = '';
  password = '';
  showPassword = false;
  loading = false;
  errorMessage = '';

  constructor(
    private authService: AuthService,
    private notificationService: NotificationService,
    private dialogRef: MatDialogRef<LoginComponent>,
  ) {
    if (this.authService.isLoggedIn()) {
      this.dialogRef.close();
    }
  }

  onSubmit(): void {
    if (!this.username || !this.password) return;
    this.loading = true;
    this.errorMessage = '';

    this.authService.login(this.username, this.password).subscribe({
      next: () => {
        this.notificationService.success('登录成功');
        this.dialogRef.close();
      },
      error: (err) => {
        this.errorMessage = err.error?.message || '登录失败，请检查用户名和密码';
        this.loading = false;
      }
    });
  }
}
