import { Component, Inject, OnInit } from '@angular/core';
import { MatDialogRef, MAT_DIALOG_DATA } from '@angular/material/dialog';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { StyleInfo } from '../../models/geoserver.models';

@Component({
  template: `
    <h2 mat-dialog-title>{{ data.mode === 'create' ? '新建样式' : '编辑样式' }}</h2>
    <mat-dialog-content>
      <mat-form-field appearance="outline" class="full-width">
        <mat-label>样式名称</mat-label>
        <input matInput [(ngModel)]="name" placeholder="my-style" [readonly]="data.mode === 'edit'">
      </mat-form-field>

      <mat-form-field appearance="outline" class="full-width">
        <mat-label>标题</mat-label>
        <input matInput [(ngModel)]="title" placeholder="我的样式">
      </mat-form-field>

      <mat-form-field appearance="outline" class="full-width code-editor">
        <mat-label>SLD XML 内容</mat-label>
        <textarea matInput [(ngModel)]="content" rows="20" class="code-textarea"></textarea>
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button (click)="cancel()">取消</button>
      <button mat-raised-button color="primary" (click)="save()" [disabled]="!name || !content">
        {{ data.mode === 'create' ? '创建' : '保存' }}
      </button>
    </mat-dialog-actions>
  `,
  styles: [`
    .full-width { width: 100%; margin-bottom: 16px; }
    .code-editor { margin-bottom: 0; }
    .code-textarea {
      font-family: 'JetBrains Mono', monospace;
      font-size: 12px;
      line-height: 1.5;
      white-space: pre;
      overflow: auto;
    }
  `]
})
export class StyleEditorDialogComponent implements OnInit {
  name = '';
  title = '';
  content = '';

  constructor(
    public dialogRef: MatDialogRef<StyleEditorDialogComponent>,
    @Inject(MAT_DIALOG_DATA) public data: { mode: 'create' | 'edit'; style?: StyleInfo },
    private geoserverService: GeoserverService,
    private notificationService: NotificationService
  ) {}

  ngOnInit(): void {
    if (this.data.mode === 'edit' && this.data.style) {
      this.name = this.data.style.name;
      this.title = this.data.style.title;
      this.content = this.data.style.content || '';
    }
  }

  save(): void {
    if (this.data.mode === 'create') {
      this.geoserverService.createStyle({ name: this.name, title: this.title || this.name, content: this.content }).subscribe({
        next: () => {
          this.notificationService.success('样式创建成功');
          this.dialogRef.close(true);
        },
        error: (e) => this.notificationService.error('创建失败: ' + (e.error?.message || e.message))
      });
    } else {
      this.geoserverService.updateStyle(this.name, { title: this.title || this.name, content: this.content }).subscribe({
        next: () => {
          this.notificationService.success('样式已保存');
          this.dialogRef.close(true);
        },
        error: (e) => this.notificationService.error('保存失败: ' + (e.error?.message || e.message))
      });
    }
  }

  cancel(): void {
    this.dialogRef.close(false);
  }
}
