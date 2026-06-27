import { Component, Inject, OnInit } from '@angular/core';
import { MatDialogRef, MAT_DIALOG_DATA } from '@angular/material/dialog';
import { Namespace } from '../../models/geoserver.models';

@Component({
  selector: 'app-namespace-dialog',
  template: `
    <h2 mat-dialog-title>{{ data.namespace ? '编辑命名空间' : '创建命名空间' }}</h2>
    <mat-dialog-content>
      <div class="dialog-form">
        <mat-form-field appearance="outline" class="full-width">
          <mat-label>前缀 (Prefix)</mat-label>
          <input matInput [(ngModel)]="formData.prefix" [disabled]="isEdit" required
                 placeholder="例如: default, ogc, custom">
          <mat-hint>命名空间前缀，创建后不可修改</mat-hint>
        </mat-form-field>

        <mat-form-field appearance="outline" class="full-width">
          <mat-label>URI</mat-label>
          <input matInput [(ngModel)]="formData.uri" required
                 placeholder="例如: http://geoserver.org/default">
          <mat-hint>命名空间唯一标识 URI</mat-hint>
        </mat-form-field>

        <mat-form-field appearance="outline" class="full-width">
          <mat-label>关联工作空间 (可选)</mat-label>
          <input matInput [(ngModel)]="formData.workspace"
                 placeholder="关联的工作空间名称">
        </mat-form-field>

        <mat-checkbox [(ngModel)]="formData.isolated">
          隔离命名空间 (Isolated)
        </mat-checkbox>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>取消</button>
      <button mat-raised-button color="primary" (click)="onSubmit()"
              [disabled]="!formData.prefix || !formData.uri">
        {{ isEdit ? '更新' : '创建' }}
      </button>
    </mat-dialog-actions>
  `,
  styles: [`
    .dialog-form {
      display: flex;
      flex-direction: column;
      gap: 16px;
      padding: 8px 0;
    }
    .full-width {
      width: 100%;
    }
  `]
})
export class NamespaceDialogComponent implements OnInit {
  isEdit = false;
  formData = {
    prefix: '',
    uri: '',
    workspace: '',
    isolated: false
  };

  constructor(
    public dialogRef: MatDialogRef<NamespaceDialogComponent>,
    @Inject(MAT_DIALOG_DATA) public data: { namespace?: Namespace }
  ) {}

  ngOnInit(): void {
    if (this.data?.namespace) {
      this.isEdit = true;
      const ns = this.data.namespace;
      this.formData.prefix = ns.prefix;
      this.formData.uri = ns.uri;
      this.formData.workspace = ns.workspace || '';
      this.formData.isolated = ns.isolated;
    }
  }

  onSubmit(): void {
    if (!this.formData.prefix || !this.formData.uri) return;

    if (this.isEdit) {
      this.dialogRef.close({
        uri: this.formData.uri,
        isolated: this.formData.isolated,
        workspace: this.formData.workspace || undefined
      });
    } else {
      this.dialogRef.close({
        prefix: this.formData.prefix,
        uri: this.formData.uri,
        isolated: this.formData.isolated,
        workspace: this.formData.workspace || undefined
      });
    }
  }
}
