import { Component, Inject } from '@angular/core';
import { MatDialogRef, MAT_DIALOG_DATA } from '@angular/material/dialog';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { Layer } from '../../models/geoserver.models';

@Component({
  template: `
    <h2 mat-dialog-title>新建图层组</h2>
    <mat-dialog-content>
      <mat-form-field appearance="outline" class="full-width">
        <mat-label>名称</mat-label>
        <input matInput [(ngModel)]="name" placeholder="my-group" />
      </mat-form-field>

      <mat-form-field appearance="outline" class="full-width">
        <mat-label>标题</mat-label>
        <input matInput [(ngModel)]="title" placeholder="我的图层组" />
      </mat-form-field>

      <mat-form-field appearance="outline" class="full-width">
        <mat-label>选择图层</mat-label>
        <mat-select [(ngModel)]="selectedLayers" multiple>
          <mat-option *ngFor="let layer of data.layers" [value]="layer.name">
            {{ layer.title || layer.name }}
          </mat-option>
        </mat-select>
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button (click)="cancel()">取消</button>
      <button mat-raised-button color="primary" (click)="save()" [disabled]="!name">创建</button>
    </mat-dialog-actions>
  `,
  styles: [
    `
      .full-width {
        width: 100%;
        margin-bottom: 16px;
      }
    `,
  ],
})
export class CreateLayerGroupDialogComponent {
  name = '';
  title = '';
  selectedLayers: string[] = [];

  constructor(
    public dialogRef: MatDialogRef<CreateLayerGroupDialogComponent>,
    @Inject(MAT_DIALOG_DATA) public data: { layers: Layer[] },
    private geoserverService: GeoserverService,
    private notificationService: NotificationService,
  ) {}

  save(): void {
    this.geoserverService
      .createLayerGroup({
        name: this.name,
        title: this.title || this.name,
        layers: this.selectedLayers,
      })
      .subscribe({
        next: () => {
          this.notificationService.success('图层组创建成功');
          this.dialogRef.close(true);
        },
        error: (e) =>
          this.notificationService.error('创建失败: ' + (e.error?.message || e.message)),
      });
  }

  cancel(): void {
    this.dialogRef.close(false);
  }
}
