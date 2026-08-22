import { Component, Inject } from '@angular/core';
import { MatDialogRef, MAT_DIALOG_DATA } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';
import { Layer } from '../../models/geoserver.models';

@Component({
  standalone: false,
  template: `
    <h2 mat-dialog-title>{{ 'layerGroups.dialogTitle' | translate }}</h2>
    <mat-dialog-content>
      <mat-form-field appearance="outline" class="full-width">
        <mat-label>{{ 'layerGroups.nameLabel' | translate }}</mat-label>
        <input
          matInput
          [(ngModel)]="name"
          placeholder="{{ 'layerGroups.namePlaceholder' | translate }}"
        />
      </mat-form-field>

      <mat-form-field appearance="outline" class="full-width">
        <mat-label>{{ 'layerGroups.titleLabel' | translate }}</mat-label>
        <input
          matInput
          [(ngModel)]="title"
          placeholder="{{ 'layerGroups.titlePlaceholder' | translate }}"
        />
      </mat-form-field>

      <mat-form-field appearance="outline" class="full-width">
        <mat-label>{{ 'layerGroups.selectLayers' | translate }}</mat-label>
        <mat-select [(ngModel)]="selectedLayers" multiple>
          <mat-option *ngFor="let layer of data.layers" [value]="layer.name">
            {{ layer.title || layer.name }}
          </mat-option>
        </mat-select>
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button (click)="cancel()">{{ 'layerGroups.cancel' | translate }}</button>
      <button mat-raised-button color="primary" (click)="save()" [disabled]="!name">
        {{ 'layerGroups.create' | translate }}
      </button>
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
    private translate: TranslateService,
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
          this.notificationService.success(this.translate.instant('layerGroups.createSuccess'));
          this.dialogRef.close(true);
        },
        error: (e) =>
          this.notificationService.error(
            this.translate.instant('layerGroups.createFail', {
              message: this.notificationService.fromError(e),
            }),
          ),
      });
  }

  cancel(): void {
    this.dialogRef.close(false);
  }
}
