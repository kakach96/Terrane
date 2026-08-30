import { Component, Inject, OnInit, inject, computed } from '@angular/core';
import { MatDialogRef, MAT_DIALOG_DATA } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
import { TerraneService } from '../../services/terrane.service';
import { NotificationService } from '../../services/notification.service';
import { LanguageService } from '../../services/language.service';
import { StyleInfo } from '../../models/terrane.models';

@Component({
  standalone: false,
  template: `
    <h2 mat-dialog-title>
      {{
        data.mode === 'create'
          ? ('styles.dialogTitleCreate' | translate)
          : ('styles.dialogTitleEdit' | translate)
      }}
    </h2>
    <mat-dialog-content>
      <div class="form-row">
        <mat-form-field appearance="outline" class="name-field">
          <mat-label>{{ 'styles.nameLabel' | translate }}</mat-label>
          <input
            matInput
            [(ngModel)]="name"
            placeholder="{{ 'styles.namePlaceholder' | translate }}"
            [readonly]="data.mode === 'edit'"
          />
        </mat-form-field>

        <mat-form-field appearance="outline" class="format-field">
          <mat-label>{{ 'styles.formatLabel' | translate }}</mat-label>
          <mat-select [(ngModel)]="format" (selectionChange)="onFormatChange()">
            <mat-option value="SLD">{{ 'styles.formatSldXml' | translate }}</mat-option>
            <mat-option value="CSS">CSS</mat-option>
            <mat-option value="YSLD">{{ 'styles.formatYaml' | translate }}</mat-option>
            <mat-option value="MBStyle">{{ 'styles.formatMb' | translate }}</mat-option>
          </mat-select>
        </mat-form-field>
      </div>

      <mat-form-field appearance="outline" class="full-width">
        <mat-label>{{ 'styles.titleLabel' | translate }}</mat-label>
        <input
          matInput
          [(ngModel)]="title"
          placeholder="{{ 'styles.titlePlaceholder' | translate }}"
        />
      </mat-form-field>

      <div class="template-buttons" *ngIf="data.mode === 'create'">
        <span class="label">{{ 'styles.templateLabel' | translate }}</span>
        <ng-container [ngSwitch]="format">
          <ng-container *ngSwitchCase="'SLD'">
            <button mat-stroked-button (click)="applyTemplate('point')">
              {{ 'styles.point' | translate }}
            </button>
            <button mat-stroked-button (click)="applyTemplate('line')">
              {{ 'styles.line' | translate }}
            </button>
            <button mat-stroked-button (click)="applyTemplate('polygon')">
              {{ 'styles.polygon' | translate }}
            </button>
            <button mat-stroked-button (click)="applyTemplate('raster')">
              {{ 'styles.raster' | translate }}
            </button>
            <button mat-stroked-button (click)="applyTemplate('labeled')">
              {{ 'styles.labeled' | translate }}
            </button>
          </ng-container>
          <ng-container *ngSwitchCase="'CSS'">
            <button mat-stroked-button (click)="applyTemplate('css-point')">
              {{ 'styles.point' | translate }}
            </button>
            <button mat-stroked-button (click)="applyTemplate('css-line')">
              {{ 'styles.line' | translate }}
            </button>
            <button mat-stroked-button (click)="applyTemplate('css-polygon')">
              {{ 'styles.polygon' | translate }}
            </button>
            <button mat-stroked-button (click)="applyTemplate('css-scale')">
              {{ 'styles.scaleFilter' | translate }}
            </button>
          </ng-container>
          <ng-container *ngSwitchCase="'YSLD'">
            <button mat-stroked-button (click)="applyTemplate('ysld-point')">
              {{ 'styles.point' | translate }}
            </button>
            <button mat-stroked-button (click)="applyTemplate('ysld-line')">
              {{ 'styles.line' | translate }}
            </button>
            <button mat-stroked-button (click)="applyTemplate('ysld-polygon')">
              {{ 'styles.polygon' | translate }}
            </button>
            <button mat-stroked-button (click)="applyTemplate('ysld-scale')">
              {{ 'styles.scaleFilter' | translate }}
            </button>
          </ng-container>
          <ng-container *ngSwitchCase="'MBStyle'">
            <button mat-stroked-button (click)="applyTemplate('mb-fill')">
              {{ 'styles.polygon' | translate }}
            </button>
            <button mat-stroked-button (click)="applyTemplate('mb-line')">
              {{ 'styles.line' | translate }}
            </button>
            <button mat-stroked-button (click)="applyTemplate('mb-circle')">
              {{ 'styles.point' | translate }}
            </button>
          </ng-container>
        </ng-container>
      </div>

      <mat-form-field appearance="outline" class="full-width code-editor">
        <mat-label>{{ 'styles.contentLabel' | translate: { format: formatLabel() } }}</mat-label>
        <textarea matInput [(ngModel)]="content" rows="20" class="code-textarea"></textarea>
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button (click)="cancel()">{{ 'styles.cancel' | translate }}</button>
      <button mat-raised-button color="primary" (click)="save()" [disabled]="!name || !content">
        {{ data.mode === 'create' ? ('styles.create' | translate) : ('styles.save' | translate) }}
      </button>
    </mat-dialog-actions>
  `,
  styles: [
    `
      .full-width {
        width: 100%;
        margin-bottom: 16px;
      }
      .form-row {
        display: flex;
        gap: 16px;
      }
      .name-field {
        flex: 1;
        margin-bottom: 16px;
      }
      .format-field {
        width: 200px;
        margin-bottom: 16px;
      }
      .code-editor {
        margin-bottom: 0;
      }
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
          color: var(--text-secondary);
          white-space: nowrap;
        }
        button {
          font-size: 12px;
          line-height: 28px;
        }
      }
    `,
  ],
})
export class StyleEditorDialogComponent implements OnInit {
  name = '';
  title = '';
  content = '';
  format = 'SLD';

  private languageService = inject(LanguageService);

  constructor(
    public dialogRef: MatDialogRef<StyleEditorDialogComponent>,
    @Inject(MAT_DIALOG_DATA) public data: { mode: 'create' | 'edit'; style?: StyleInfo },
    private terraneService: TerraneService,
    private notificationService: NotificationService,
    private translate: TranslateService,
  ) {}

  /** Format label; re-evaluated on language switch. */
  formatLabel = computed(() => {
    this.languageService.currentLang();
    switch (this.format) {
      case 'CSS':
        return 'CSS';
      case 'YSLD':
        return this.translate.instant('styles.formatYaml');
      case 'MBStyle':
        return this.translate.instant('styles.formatMb');
      default:
        return this.translate.instant('styles.formatSld');
    }
  });

  ngOnInit(): void {
    if (this.data.mode === 'edit' && this.data.style) {
      this.name = this.data.style.name;
      this.title = this.data.style.title;
      this.content = this.data.style.content || '';
      this.format = this.data.style.format || 'SLD';
    }
  }

  onFormatChange(): void {
    if (!this.content || this.data.mode === 'create') {
      this.applyDefaultForFormat();
    }
  }

  applyDefaultForFormat(): void {
    switch (this.format) {
      case 'CSS':
        this.content = this.getTemplate('css-default');
        break;
      case 'YSLD':
        this.content = this.getTemplate('ysld-default');
        break;
      case 'MBStyle':
        this.content = this.getTemplate('mb-default');
        break;
      default:
        this.content = this.getTemplate('polygon');
        break;
    }
  }

  save(): void {
    if (this.data.mode === 'create') {
      this.terraneService
        .createStyle({
          name: this.name,
          title: this.title || this.name,
          content: this.content,
          format: this.format,
        })
        .subscribe({
          next: () => {
            this.notificationService.success(this.translate.instant('styles.createSuccess'));
            this.dialogRef.close(true);
          },
          error: (e) =>
            this.notificationService.error(
              this.translate.instant('styles.createFail', {
                message: this.notificationService.fromError(e),
              }),
            ),
        });
    } else {
      this.terraneService
        .updateStyle(this.name, {
          title: this.title || this.name,
          content: this.content,
          format: this.format,
        })
        .subscribe({
          next: () => {
            this.notificationService.success(this.translate.instant('styles.saveSuccess'));
            this.dialogRef.close(true);
          },
          error: (e) =>
            this.notificationService.error(
              this.translate.instant('styles.saveFail', {
                message: this.notificationService.fromError(e),
              }),
            ),
        });
    }
  }

  applyTemplate(type: string): void {
    this.content = this.getTemplate(type);
  }

  private getTemplate(type: string): string {
    switch (type) {
      // ======== SLD ========
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

      // ======== CSS ========
      case 'css-default':
        return `/* Default style */
* {
  fill: #6688aa;
  fill-opacity: 0.6;
  stroke: #334455;
  stroke-width: 1;
  mark: symbol(circle);
  mark-size: 8;
}`;

      case 'css-point':
        return `/* Point style */
* {
  mark: symbol(circle);
  mark-size: 10;
  fill: #FF0000;
  fill-opacity: 0.8;
  stroke: #000000;
  stroke-width: 1;
}`;

      case 'css-line':
        return `/* Line style */
* {
  stroke: #0000FF;
  stroke-width: 2;
  stroke-opacity: 0.8;
}`;

      case 'css-polygon':
        return `/* Polygon style */
* {
  fill: #00CC00;
  fill-opacity: 0.5;
  stroke: #006600;
  stroke-width: 1;
}`;

      case 'css-scale':
        return `/* Scale-based style */
[@scale > 100000] {
  fill: #6688aa;
  fill-opacity: 0.4;
  stroke: #334455;
  stroke-width: 0.5;
}

[@scale between 50000 and 100000] {
  fill: #6688aa;
  fill-opacity: 0.6;
  stroke: #334455;
  stroke-width: 1;
}

[@scale < 50000] {
  fill: #4466aa;
  fill-opacity: 0.8;
  stroke: #223366;
  stroke-width: 2;
  mark: symbol(circle);
  mark-size: 10;
}`;

      // ======== YSLD ========
      case 'ysld-default':
        return `name: "my-style"
title: "My Style"
feature-styles:
- name: default
  rules:
  - symbolizers:
    - polygon:
        fill-color: "#6688aa"
        fill-opacity: 0.6
        stroke-color: "#334455"
        stroke-width: 1
    - line:
        stroke-color: "#334455"
        stroke-width: 1
    - point:
        mark: circle
        mark-size: 8
        fill-color: "#6688aa"
        stroke-color: "#334455"
        stroke-width: 1`;

      case 'ysld-point':
        return `feature-styles:
- name: points
  rules:
  - symbolizers:
    - point:
        mark: circle
        mark-size: 10
        fill-color: "#FF0000"
        fill-opacity: 0.8
        stroke-color: "#000000"
        stroke-width: 1`;

      case 'ysld-line':
        return `feature-styles:
- name: lines
  rules:
  - symbolizers:
    - line:
        stroke-color: "#0000FF"
        stroke-width: 2
        stroke-opacity: 0.8`;

      case 'ysld-polygon':
        return `feature-styles:
- name: polygons
  rules:
  - symbolizers:
    - polygon:
        fill-color: "#00CC00"
        fill-opacity: 0.5
        stroke-color: "#006600"
        stroke-width: 1`;

      case 'ysld-scale':
        return `feature-styles:
- name: detailed
  rules:
  - scale: [0, 50000]
    symbolizers:
    - polygon:
        fill-color: "#4466aa"
        fill-opacity: 0.8
        stroke-color: "#223366"
        stroke-width: 2
  - scale: [50000, 100000]
    symbolizers:
    - polygon:
        fill-color: "#6688aa"
        fill-opacity: 0.6
        stroke-color: "#334455"
        stroke-width: 1
  - scale: [100000, Infinity]
    symbolizers:
    - polygon:
        fill-color: "#8899aa"
        fill-opacity: 0.4
        stroke-color: "#556677"
        stroke-width: 0.5`;

      // ======== MBStyle ========
      case 'mb-default':
        return `{
  "version": 8,
  "name": "my-layer",
  "layers": [
    {
      "id": "polygons",
      "type": "fill",
      "paint": {
        "fill-color": "#6688aa",
        "fill-opacity": 0.6,
        "fill-outline-color": "#334455"
      }
    },
    {
      "id": "lines",
      "type": "line",
      "paint": {
        "line-color": "#334455",
        "line-width": 1
      }
    },
    {
      "id": "points",
      "type": "circle",
      "paint": {
        "circle-color": "#6688aa",
        "circle-radius": 4,
        "circle-stroke-color": "#334455",
        "circle-stroke-width": 1
      }
    }
  ]
}`;

      case 'mb-fill':
        return `{
  "version": 8,
  "layers": [
    {
      "id": "fill-layer",
      "type": "fill",
      "paint": {
        "fill-color": "#00CC00",
        "fill-opacity": 0.5,
        "fill-outline-color": "#006600"
      }
    }
  ]
}`;

      case 'mb-line':
        return `{
  "version": 8,
  "layers": [
    {
      "id": "line-layer",
      "type": "line",
      "paint": {
        "line-color": "#0000FF",
        "line-width": 2,
        "line-opacity": 0.8
      }
    }
  ]
}`;

      case 'mb-circle':
        return `{
  "version": 8,
  "layers": [
    {
      "id": "circle-layer",
      "type": "circle",
      "paint": {
        "circle-color": "#FF0000",
        "circle-radius": 5,
        "circle-opacity": 0.8,
        "circle-stroke-color": "#000000",
        "circle-stroke-width": 1
      }
    }
  ]
}`;

      default:
        return '';
    }
  }

  cancel(): void {
    this.dialogRef.close(false);
  }
}
