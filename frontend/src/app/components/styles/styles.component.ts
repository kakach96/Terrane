import { Component, OnInit } from '@angular/core';
import { MatDialog } from '@angular/material/dialog';
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
        this.notificationService.error('加载样式列表失败');
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
      error: () => this.notificationService.error('加载样式失败'),
    });
  }

  deleteStyle(style: StyleInfo): void {
    if (style.is_builtin) {
      this.notificationService.info('内置样式不能删除');
      return;
    }
    if (!confirm(`确定要删除样式 "${style.title}" 吗？`)) return;
    this.geoserverService.deleteStyle(style.name).subscribe({
      next: () => {
        this.notificationService.success('样式已删除');
        this.loadStyles();
      },
      error: () => this.notificationService.error('删除失败'),
    });
  }
}
