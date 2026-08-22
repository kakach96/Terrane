import { Component } from '@angular/core';
import { TranslateService } from '@ngx-translate/core';
import { AuthService } from '../../services/auth.service';
import { NotificationService } from '../../services/notification.service';
import { MatDialogRef } from '@angular/material/dialog';

@Component({
  standalone: false,
  selector: 'app-login',
  templateUrl: './login.component.html',
  styleUrls: ['./login.component.scss'],
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
    private translate: TranslateService,
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
        this.notificationService.success(this.translate.instant('login.success'));
        this.dialogRef.close();
      },
      error: (err) => {
        this.errorMessage =
          this.notificationService.fromError(err) || this.translate.instant('login.fail');
        this.loading = false;
      },
    });
  }
}
