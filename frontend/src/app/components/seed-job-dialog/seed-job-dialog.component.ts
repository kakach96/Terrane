import { Component, Inject } from '@angular/core';
import { FormBuilder, FormGroup, Validators } from '@angular/forms';
import { MAT_DIALOG_DATA, MatDialogRef } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';

/** Data passed into the seed / truncate dialog. */
export interface SeedJobDialogData {
  /** Pre-selected layer name (when opened from a layer row). */
  layer?: string;
  /** All available layer names for the select. */
  layers: string[];
}

/** Result emitted by the dialog on submit. */
export interface SeedJobDialogResult {
  layer: string;
  operation: 'seed' | 'truncate' | 'reseed';
  gridset: string;
  z_min: number;
  z_max: number;
  format: string;
}

@Component({
  standalone: false,
  selector: 'app-seed-job-dialog',
  templateUrl: './seed-job-dialog.component.html',
  styleUrls: ['./seed-job-dialog.component.scss'],
})
export class SeedJobDialogComponent {
  form: FormGroup;
  title: string;

  constructor(
    private fb: FormBuilder,
    public dialogRef: MatDialogRef<SeedJobDialogComponent>,
    @Inject(MAT_DIALOG_DATA) public data: SeedJobDialogData,
    private translate: TranslateService,
  ) {
    this.title = this.translate.instant('tileLayers.seedDialogTitle');
    this.form = this.fb.group(
      {
        layer: [data?.layer || '', Validators.required],
        operation: ['seed', Validators.required],
        gridset: ['EPSG:4326', Validators.required],
        z_min: [0, [Validators.required, Validators.min(0), Validators.max(22)]],
        z_max: [5, [Validators.required, Validators.min(0), Validators.max(22)]],
        format: ['png', Validators.required],
      },
      {
        validators: (group) =>
          group.get('z_min')!.value > group.get('z_max')!.value ? { range: true } : null,
      },
    );
  }

  onSubmit(): void {
    if (this.form.invalid) return;
    this.dialogRef.close(this.form.getRawValue() as SeedJobDialogResult);
  }

  onCancel(): void {
    this.dialogRef.close();
  }

  trackByIndex(index: number): number {
    return index;
  }
}
