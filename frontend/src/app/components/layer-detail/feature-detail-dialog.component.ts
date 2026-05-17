import { Component, Inject } from '@angular/core';
import { MAT_DIALOG_DATA, MatDialogRef, MatDialogModule } from '@angular/material/dialog';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatChipsModule } from '@angular/material/chips';
import { Feature } from '../../models/geoserver.models';
import { CommonModule } from '@angular/common';

@Component({
  selector: 'app-feature-detail-dialog',
  standalone: true,
  imports: [
    CommonModule,
    MatDialogModule,
    MatButtonModule,
    MatIconModule,
    MatChipsModule
  ],
  template: `
    <h2 mat-dialog-title>要素详情</h2>
    <mat-dialog-content>
      <div class="feature-detail">
        <div class="detail-section">
          <h3>基本信息</h3>
          <div class="info-item">
            <span class="label">ID</span>
            <span class="value mono">{{ feature.id }}</span>
          </div>
          <div class="info-item">
            <span class="label">类型</span>
            <mat-chip>{{ feature.geometry.type }}</mat-chip>
          </div>
        </div>

        <div class="detail-section">
          <h3>几何信息</h3>
          <div class="geometry-display mono">
            {{ formatGeometry(feature.geometry) }}
          </div>
        </div>

        <div class="detail-section" *ngIf="hasProperties">
          <h3>属性信息</h3>
          <div class="properties-grid">
            <div class="property-item" *ngFor="let key of propertyKeys">
              <span class="property-key">{{ key }}</span>
              <span class="property-value">{{ feature.properties[key] }}</span>
            </div>
          </div>
        </div>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>关闭</button>
    </mat-dialog-actions>
  `,
  styles: [`
    mat-dialog-content {
      min-width: 500px;
      max-width: 700px;
    }

    .feature-detail {
      display: flex;
      flex-direction: column;
      gap: 24px;
    }

    .detail-section {
      h3 {
        font-size: 14px;
        font-weight: 600;
        color: rgba(0, 0, 0, 0.6);
        margin-bottom: 12px;
        text-transform: uppercase;
        letter-spacing: 0.5px;
      }
    }

    .info-item {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 12px;

      .label {
        font-size: 13px;
        font-weight: 600;
        color: rgba(0, 0, 0, 0.6);
        min-width: 80px;
      }

      .value {
        font-size: 14px;
        color: rgba(0, 0, 0, 0.87);

        &.mono {
          font-family: 'JetBrains Mono', monospace;
          font-size: 12px;
        }
      }
    }

    .geometry-display {
      background: #f5f5f5;
      padding: 16px;
      border-radius: 8px;
      font-size: 12px;
      line-height: 1.6;
      max-height: 200px;
      overflow-y: auto;
      white-space: pre-wrap;
      word-break: break-all;
    }

    .properties-grid {
      display: grid;
      grid-template-columns: repeat(2, 1fr);
      gap: 12px;
    }

    .property-item {
      display: flex;
      flex-direction: column;
      gap: 4px;
      padding: 12px;
      background: #f9f9f9;
      border-radius: 6px;

      .property-key {
        font-size: 11px;
        font-weight: 600;
        color: rgba(0, 0, 0, 0.6);
        text-transform: uppercase;
      }

      .property-value {
        font-size: 14px;
        color: rgba(0, 0, 0, 0.87);
      }
    }
  `]
})
export class FeatureDetailDialogComponent {
  constructor(
    public dialogRef: MatDialogRef<FeatureDetailDialogComponent>,
    @Inject(MAT_DIALOG_DATA) public feature: Feature
  ) {}

  get propertyKeys(): string[] {
    return Object.keys(this.feature.properties);
  }

  get hasProperties(): boolean {
    return this.propertyKeys.length > 0;
  }

  formatGeometry(geometry: any): string {
    return JSON.stringify(geometry, null, 2);
  }
}
