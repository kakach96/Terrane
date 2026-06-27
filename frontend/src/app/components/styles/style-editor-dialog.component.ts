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

      <div class="template-buttons" *ngIf="data.mode === 'create'">
        <span class="label">快速模板：</span>
        <button mat-stroked-button (click)="applyTemplate('point')">点样式</button>
        <button mat-stroked-button (click)="applyTemplate('line')">线样式</button>
        <button mat-stroked-button (click)="applyTemplate('polygon')">面样式</button>
        <button mat-stroked-button (click)="applyTemplate('raster')">栅格样式</button>
        <button mat-stroked-button (click)="applyTemplate('labeled')">标注样式</button>
      </div>

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
    .template-buttons {
      display: flex;
      gap: 8px;
      align-items: center;
      margin-bottom: 12px;
      flex-wrap: wrap;
      .label {
        font-size: 13px;
        color: var(--text-secondary, #666);
        white-space: nowrap;
      }
      button {
        font-size: 12px;
        line-height: 28px;
      }
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

  applyTemplate(type: string): void {
    this.content = this.getTemplate(type);
  }

  private getTemplate(type: string): string {
    switch (type) {
      case 'point':
        return `<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0"
  xmlns="http://www.opengis.net/sld"
  xmlns:ogc="http://www.opengis.net/ogc"
  xmlns:xlink="http://www.w3.org/1999/xlink">
  <NamedLayer>
    <Name>point_layer</Name>
    <UserStyle>
      <Title>Red Circle Point Style</Title>
      <FeatureTypeStyle>
        <Rule>
          <PointSymbolizer>
            <Graphic>
              <Mark>
                <WellKnownName>circle</WellKnownName>
                <Fill>
                  <CssParameter name="fill">#FF0000</CssParameter>
                  <CssParameter name="fill-opacity">0.8</CssParameter>
                </Fill>
                <Stroke>
                  <CssParameter name="stroke">#000000</CssParameter>
                  <CssParameter name="stroke-width">1</CssParameter>
                </Stroke>
              </Mark>
              <Size>10</Size>
            </Graphic>
          </PointSymbolizer>
        </Rule>
      </FeatureTypeStyle>
    </UserStyle>
  </NamedLayer>
</StyledLayerDescriptor>`;

      case 'line':
        return `<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0"
  xmlns="http://www.opengis.net/sld"
  xmlns:ogc="http://www.opengis.net/ogc"
  xmlns:xlink="http://www.w3.org/1999/xlink">
  <NamedLayer>
    <Name>line_layer</Name>
    <UserStyle>
      <Title>Blue Line Style</Title>
      <FeatureTypeStyle>
        <Rule>
          <LineSymbolizer>
            <Stroke>
              <CssParameter name="stroke">#0000FF</CssParameter>
              <CssParameter name="stroke-width">2</CssParameter>
              <CssParameter name="stroke-opacity">0.8</CssParameter>
              <CssParameter name="stroke-dasharray">5 3</CssParameter>
            </Stroke>
          </LineSymbolizer>
        </Rule>
      </FeatureTypeStyle>
    </UserStyle>
  </NamedLayer>
</StyledLayerDescriptor>`;

      case 'polygon':
        return `<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0"
  xmlns="http://www.opengis.net/sld"
  xmlns:ogc="http://www.opengis.net/ogc"
  xmlns:xlink="http://www.w3.org/1999/xlink">
  <NamedLayer>
    <Name>polygon_layer</Name>
    <UserStyle>
      <Title>Green Polygon Style</Title>
      <FeatureTypeStyle>
        <Rule>
          <PolygonSymbolizer>
            <Fill>
              <CssParameter name="fill">#00CC00</CssParameter>
              <CssParameter name="fill-opacity">0.5</CssParameter>
            </Fill>
            <Stroke>
              <CssParameter name="stroke">#006600</CssParameter>
              <CssParameter name="stroke-width">1</CssParameter>
            </Stroke>
          </PolygonSymbolizer>
        </Rule>
      </FeatureTypeStyle>
    </UserStyle>
  </NamedLayer>
</StyledLayerDescriptor>`;

      case 'raster':
        return `<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0"
  xmlns="http://www.opengis.net/sld"
  xmlns:ogc="http://www.opengis.net/ogc"
  xmlns:xlink="http://www.w3.org/1999/xlink">
  <NamedLayer>
    <Name>raster_layer</Name>
    <UserStyle>
      <Title>Default Raster Style</Title>
      <FeatureTypeStyle>
        <Rule>
          <RasterSymbolizer>
            <Opacity>1.0</Opacity>
            <ColorMap>
              <ColorMapEntry color="#000000" quantity="0" opacity="0"/>
              <ColorMapEntry color="#333333" quantity="50" opacity="1"/>
              <ColorMapEntry color="#666666" quantity="100" opacity="1"/>
              <ColorMapEntry color="#999999" quantity="150" opacity="1"/>
              <ColorMapEntry color="#CCCCCC" quantity="200" opacity="1"/>
              <ColorMapEntry color="#FFFFFF" quantity="255" opacity="1"/>
            </ColorMap>
          </RasterSymbolizer>
        </Rule>
      </FeatureTypeStyle>
    </UserStyle>
  </NamedLayer>
</StyledLayerDescriptor>`;

      case 'labeled':
        return `<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0"
  xmlns="http://www.opengis.net/sld"
  xmlns:ogc="http://www.opengis.net/ogc"
  xmlns:xlink="http://www.w3.org/1999/xlink">
  <NamedLayer>
    <Name>labeled_layer</Name>
    <UserStyle>
      <Title>Labeled Point Style</Title>
      <FeatureTypeStyle>
        <Rule>
          <PointSymbolizer>
            <Graphic>
              <Mark>
                <WellKnownName>circle</WellKnownName>
                <Fill>
                  <CssParameter name="fill">#3366CC</CssParameter>
                  <CssParameter name="fill-opacity">0.8</CssParameter>
                </Fill>
                <Stroke>
                  <CssParameter name="stroke">#FFFFFF</CssParameter>
                  <CssParameter name="stroke-width">1</CssParameter>
                </Stroke>
              </Mark>
              <Size>8</Size>
            </Graphic>
          </PointSymbolizer>
          <TextSymbolizer>
            <Label>
              <ogc:PropertyName>name</ogc:PropertyName>
            </Label>
            <Font>
              <CssParameter name="font-family">Arial</CssParameter>
              <CssParameter name="font-size">12</CssParameter>
              <CssParameter name="font-style">normal</CssParameter>
            </Font>
            <LabelPlacement>
              <PointPlacement>
                <AnchorPoint>
                  <AnchorPointX>0.5</AnchorPointX>
                  <AnchorPointY>1.5</AnchorPointY>
                </AnchorPoint>
              </PointPlacement>
            </LabelPlacement>
            <Fill>
              <CssParameter name="fill">#000000</CssParameter>
            </Fill>
          </TextSymbolizer>
        </Rule>
      </FeatureTypeStyle>
    </UserStyle>
  </NamedLayer>
</StyledLayerDescriptor>`;

      default:
        return '';
    }
  }

  cancel(): void {
    this.dialogRef.close(false);
  }
}
