import { Component, ChangeDetectionStrategy, inject, signal, computed } from '@angular/core';
import { toSignal, toObservable } from '@angular/core/rxjs-interop';
import { TranslateService } from '@ngx-translate/core';
import { TerraneService } from '../../services/terrane.service';
import { AuthService } from '../../services/auth.service';
import { NotificationService } from '../../services/notification.service';
import { LanguageService } from '../../services/language.service';
import { User } from '../../models/terrane.models';
import { switchMap, tap, startWith, catchError, of } from 'rxjs';

@Component({
  standalone: false,
  selector: 'app-users',
  templateUrl: './users.component.html',
  styleUrls: ['./users.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class UsersComponent {
  private terrane = inject(TerraneService);
  private auth = inject(AuthService);
  private notificationService = inject(NotificationService);
  private translate = inject(TranslateService);
  private languageService = inject(LanguageService);

  /** Localized role labels; re-evaluated on language switch. */
  roleLabels = computed(() => {
    this.languageService.currentLang();
    return {
      admin: this.translate.instant('users.roleAdmin'),
      manager: this.translate.instant('users.roleManager'),
      user: this.translate.instant('users.roleUser'),
      guest: this.translate.instant('users.roleGuest'),
    } as Record<string, string>;
  });

  displayedColumns: string[] = ['username', 'role', 'status', 'created', 'actions'];

  error = '';

  // Create user form
  showCreateForm = false;
  newUsername = '';
  newPassword = '';
  newRole = 'user';

  // Change password form
  showPasswordForm = false;
  oldPassword = '';
  newPassword1 = '';
  newPassword2 = '';

  private refreshTrigger = signal(0);
  loading = signal(false);

  private users$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    tap(() => this.loading.set(true)),
    switchMap(() =>
      this.terrane.listUsers().pipe(
        catchError(() => {
          this.error = this.translate.instant('users.loadFail');
          return of([] as User[]);
        }),
      ),
    ),
    tap(() => this.loading.set(false)),
  );

  users = toSignal(this.users$, { initialValue: [] as User[] });

  createUser(): void {
    if (!this.newUsername || !this.newPassword) return;
    this.terrane.createUser(this.newUsername, this.newPassword, this.newRole).subscribe({
      next: () => {
        this.notificationService.success(this.translate.instant('users.createSuccess'));
        this.showCreateForm = false;
        this.newUsername = '';
        this.newPassword = '';
        this.newRole = 'user';
        this.refreshTrigger.update((v) => v + 1);
      },
      error: (e) => this.notificationService.error(this.notificationService.fromError(e)),
    });
  }

  deleteUser(username: string): void {
    if (username === 'admin') {
      this.notificationService.warning(this.translate.instant('users.cannotDeleteAdmin'));
      return;
    }
    this.notificationService
      .confirm(
        this.translate.instant('common.confirm'),
        this.translate.instant('users.deleteUserConfirm', { username }),
      )
      .subscribe((confirmed: boolean) => {
        if (!confirmed) return;
        this.terrane.deleteUser(username).subscribe({
          next: () => {
            this.notificationService.success(this.translate.instant('users.deleteSuccess'));
            this.refreshTrigger.update((v) => v + 1);
          },
          error: (e) => this.notificationService.error(this.notificationService.fromError(e)),
        });
      });
  }

  changePassword(): void {
    if (this.newPassword1 !== this.newPassword2) {
      this.notificationService.warning(this.translate.instant('users.passwordMismatch'));
      return;
    }
    this.terrane.changePassword(this.oldPassword, this.newPassword1).subscribe({
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
}
