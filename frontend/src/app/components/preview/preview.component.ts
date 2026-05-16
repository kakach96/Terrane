import { Component, Input } from '@angular/core';

@Component({
  selector: 'app-preview',
  template: `
    <div class="preview-container">
      <img [src]="imageUrl" [alt]="alt">
    </div>
  `,
  styles: [`
    .preview-container {
      width: 100%;
      height: 100%;
      display: flex;
      align-items: center;
      justify-content: center;
      background: #f5f5f7;
      border-radius: 8px;
      
      img {
        max-width: 100%;
        max-height: 100%;
        border-radius: 4px;
      }
    }
  `]
})
export class PreviewComponent {
  @Input() imageUrl = '';
  @Input() alt = 'Preview';
}
