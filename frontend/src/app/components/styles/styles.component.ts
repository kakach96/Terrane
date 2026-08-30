import { Component, ChangeDetectionStrategy, inject, signal } from '@angular/core';
import { toSignal, toObservable } from '@angular/core/rxjs-interop';
import { MatDialog } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
import { TerraneService } from '../../services/terrane.service';
import { NotificationService } from '../../services/notification.service';
import { StyleInfo } from '../../models/terrane.models';
import { StyleEditorDialogComponent } from './style-editor-dialog.component';
import { switchMap, tap, startWith, catchError, of } from 'rxjs';

@Component({
  standalone: false,
  selector: 'app-styles',
  templateUrl: './styles.component.html',
  styleUrls: ['./styles.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class StylesComponent {
  private terraneService = inject(TerraneService);
  private notificationService = inject(NotificationService);
  private dialog = inject(MatDialog);
  private translate = inject(TranslateService);

  private refreshTrigger = signal(0);
  loading = signal(true);

  private styles$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    tap(() => this.loading.set(true)),
    switchMap(() =>
      this.terraneService.getStyles().pipe(
        catchError(() => {
          this.notificationService.error(this.translate.instant('styles.loadListFail'));
          return of([] as StyleInfo[]);
        }),
      ),
    ),
    tap(() => this.loading.set(false)),
  );

  styles = toSignal(this.styles$, { initialValue: [] as StyleInfo[] });

  createStyle(): void {
    const dialogRef = this.dialog.open(StyleEditorDialogComponent, {
      width: '700px',
      maxWidth: '90vw',
      data: { mode: 'create' },
    });
    dialogRef.afterClosed().subscribe((result) => {
      if (result) this.refreshTrigger.update((v) => v + 1);
    });
  }

  editStyle(style: StyleInfo): void {
    this.terraneService.getStyle(style.name).subscribe({
      next: (full) => {
        this.dialog
          .open(StyleEditorDialogComponent, {
            width: '700px',
            maxWidth: '90vw',
            data: { mode: 'edit', style: full },
          })
          .afterClosed()
          .subscribe((result) => {
            if (result) this.refreshTrigger.update((v) => v + 1);
          });
      },
      error: () => this.notificationService.error(this.translate.instant('styles.loadFail')),
    });
  }

  deleteStyle(style: StyleInfo): void {
    if (style.is_builtin) {
      this.notificationService.info(this.translate.instant('styles.builtinNotDeletable'));
      return;
    }
    this.notificationService
      .confirm(
        this.translate.instant('common.confirm'),
        this.translate.instant('styles.deleteConfirm', { title: style.title }),
      )
      .subscribe((confirmed: boolean) => {
        if (!confirmed) return;
        this.terraneService.deleteStyle(style.name).subscribe({
          next: () => {
            this.notificationService.success(this.translate.instant('styles.deleteSuccess'));
            this.refreshTrigger.update((v) => v + 1);
          },
          error: () => this.notificationService.error(this.translate.instant('styles.deleteFail')),
        });
      });
  }
}
