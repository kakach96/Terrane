import { Injectable } from '@angular/core';
import { MatSnackBar } from '@angular/material/snack-bar';
import { TranslateService } from '@ngx-translate/core';
import { Observable, of } from 'rxjs';

@Injectable({
  providedIn: 'root',
})
export class NotificationService {
  constructor(
    private snackBar: MatSnackBar,
    private translate: TranslateService,
  ) {}

  success(message: string, duration = 3000): void {
    this.open(message, duration, 'snackbar-success');
  }

  error(message: string, duration = 5000): void {
    this.open(message, duration, 'snackbar-error');
  }

  info(message: string, duration = 3000): void {
    this.open(message, duration, 'snackbar-info');
  }

  warning(message: string, duration = 3000): void {
    this.open(message, duration, 'snackbar-warning');
  }

  private open(message: string, duration: number, panelClass: string): void {
    this.snackBar.open(message, this.translate.instant('common.close'), {
      duration,
      panelClass: [panelClass],
      horizontalPosition: 'end',
      verticalPosition: 'bottom',
    });
  }

  /**
   * Resolve a user-facing error message from an HTTP error response.
   * Backend error responses carry a stable `code`; map it to a localized
   * message first, then fall back to the backend `message` and finally a
   * generic translated error.
   */
  fromError(err: unknown): string {
    const e = err as { error?: { code?: string; message?: string }; message?: string } | null;
    const code = e?.error?.code;
    if (code) {
      const key = `error.${code}`;
      const translated = this.translate.instant(key);
      if (translated && translated !== key) {
        return translated;
      }
    }
    if (e?.error?.message) {
      return e.error.message;
    }
    if (e?.message) {
      return e.message;
    }
    return this.translate.instant('error.INTERNAL_ERROR');
  }

  confirm(title: string, message: string): Observable<boolean> {
    const confirmed = window.confirm(`${title}\n\n${message}`);
    return of(confirmed);
  }
}
