import { Component, OnInit } from '@angular/core';
import { MatDialog } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { StyleInfo } from '../../models/geoserver.models';
import { StyleEditorDialogComponent } from './style-editor-dialog.component';

@Component({
  selector: 'app-styles',
  templateUrl: './styles.component.html',
  styleUrls: ['./styles.component.scss'],
})
export class StylesComponent implements OnInit {
  styles: StyleInfo[] = [];
  loading = true;

  constructor(
    private geoserverService: GeoserverService,
    private notificationService: NotificationService,
    private dialog: MatDialog,
    private translate: TranslateService,
  ) {}

  ngOnInit(): void {
    this.loadStyles();
  }

  loadStyles(): void {
    this.loading = true;
    this.geoserverService.getStyles().subscribe({
      next: (data) => {
        this.styles = data;
        this.loading = false;
      },
      error: () => {
        this.notificationService.error(this.translate.instant('styles.loadListFail'));
        this.loading = false;
      },
    });
  }

  createStyle(): void {
    const dialogRef = this.dialog.open(StyleEditorDialogComponent, {
      width: '700px',
      maxWidth: '90vw',
      data: { mode: 'create' },
    });
    dialogRef.afterClosed().subscribe((result) => {
      if (result) this.loadStyles();
    });
  }

  editStyle(style: StyleInfo): void {
    this.geoserverService.getStyle(style.name).subscribe({
      next: (full) => {
        this.dialog
          .open(StyleEditorDialogComponent, {
            width: '700px',
            maxWidth: '90vw',
            data: { mode: 'edit', style: full },
          })
          .afterClosed()
          .subscribe((result) => {
            if (result) this.loadStyles();
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
    if (!confirm(this.translate.instant('styles.deleteConfirm', { title: style.title }))) return;
    this.geoserverService.deleteStyle(style.name).subscribe({
      next: () => {
        this.notificationService.success(this.translate.instant('styles.deleteSuccess'));
        this.loadStyles();
      },
      error: () => this.notificationService.error(this.translate.instant('styles.deleteFail')),
    });
  }
}
