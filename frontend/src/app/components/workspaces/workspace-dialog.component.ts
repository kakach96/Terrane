import { Component, Inject } from '@angular/core';
import { FormBuilder, FormGroup, Validators } from '@angular/forms';
import { MAT_DIALOG_DATA, MatDialogRef } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
import {
  Workspace,
  CreateWorkspaceRequest,
  UpdateWorkspaceRequest,
} from '../../models/terrane.models';

@Component({
  standalone: false,
  selector: 'app-workspace-dialog',
  templateUrl: './workspace-dialog.component.html',
  styleUrls: ['./workspace-dialog.component.scss'],
})
export class WorkspaceDialogComponent {
  form: FormGroup;
  isEdit: boolean;
  title: string;

  constructor(
    private fb: FormBuilder,
    public dialogRef: MatDialogRef<WorkspaceDialogComponent>,
    @Inject(MAT_DIALOG_DATA) public data: { workspace?: Workspace },
    private translate: TranslateService,
  ) {
    this.isEdit = !!data?.workspace;
    this.title = this.isEdit
      ? this.translate.instant('workspaces.dialogTitleEdit')
      : this.translate.instant('workspaces.dialogTitleCreate');
    this.form = this.fb.group({
      name: [
        { value: data?.workspace?.name || '', disabled: this.isEdit },
        [Validators.required, Validators.pattern('^[a-zA-Z0-9_-]+$')],
      ],
      title: [data?.workspace?.title || ''],
      description: [data?.workspace?.description || ''],
    });
  }

  get name() {
    return this.form.get('name');
  }

  onSubmit(): void {
    if (this.form.invalid) return;

    const value = this.form.getRawValue();
    const request: CreateWorkspaceRequest | UpdateWorkspaceRequest = {
      name: value.name,
      title: value.title || undefined,
      description: value.description || undefined,
    };

    this.dialogRef.close(request);
  }

  onCancel(): void {
    this.dialogRef.close();
  }
}
