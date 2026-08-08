import { Injectable } from '@angular/core';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Observable, of } from 'rxjs';

@Injectable({
  providedIn: 'root',
})
export class NotificationService {
  constructor(private snackBar: MatSnackBar) {}

  success(message: string, duration = 3000): void {
    this.snackBar.open(message, '关闭', {
      duration,
      panelClass: ['snackbar-success'],
      horizontalPosition: 'end',
      verticalPosition: 'bottom',
    });
  }

  error(message: string, duration = 5000): void {
    this.snackBar.open(message, '关闭', {
      duration,
      panelClass: ['snackbar-error'],
      horizontalPosition: 'end',
      verticalPosition: 'bottom',
    });
  }

  info(message: string, duration = 3000): void {
    this.snackBar.open(message, '关闭', {
      duration,
      panelClass: ['snackbar-info'],
      horizontalPosition: 'end',
      verticalPosition: 'bottom',
    });
  }

  warning(message: string, duration = 3000): void {
    this.snackBar.open(message, '关闭', {
      duration,
      panelClass: ['snackbar-warning'],
      horizontalPosition: 'end',
      verticalPosition: 'bottom',
    });
  }

  confirm(title: string, message: string): Observable<boolean> {
    const confirmed = window.confirm(`${title}\n\n${message}`);
    return of(confirmed);
  }
}
