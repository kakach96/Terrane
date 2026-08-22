import { Component, Inject, OnInit } from '@angular/core';
import { MAT_DIALOG_DATA, MatDialogRef } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
import { GeoserverService } from '../../../services/geoserver.service';
import { FileEntry, S3BrowseRequest } from '../../../models/geoserver.models';

export interface DirectoryBrowserData {
  mode: 'local' | 's3';
  /** 起始路径/前缀 (local: 绝对路径; s3: 对象 key 前缀) */
  initialPath?: string;
  /** S3 模式下的连接配置 (endpoint/bucket/密钥等) */
  s3Connection?: S3BrowseRequest;
}

export interface DirectoryBrowserResult {
  path: string;
  is_dir: boolean;
  name?: string;
}

@Component({
  standalone: false,
  selector: 'app-directory-browser',
  templateUrl: './directory-browser.component.html',
  styleUrls: ['./directory-browser.component.scss'],
})
export class DirectoryBrowserComponent implements OnInit {
  mode: 'local' | 's3';
  currentPath = '';
  breadcrumbs: { label: string; path: string }[] = [];
  entries: FileEntry[] = [];
  loading = false;
  error = '';
  selectedPath = '';

  constructor(
    @Inject(MAT_DIALOG_DATA) public data: DirectoryBrowserData,
    private dialogRef: MatDialogRef<DirectoryBrowserComponent>,
    private geoserverService: GeoserverService,
    private translate: TranslateService,
  ) {
    this.mode = data.mode;
    this.currentPath = data.initialPath || '';
  }

  ngOnInit(): void {
    this.load(this.currentPath);
  }

  get title(): string {
    return this.mode === 'local'
      ? this.translate.instant('directoryBrowser.titleLocal')
      : this.translate.instant('directoryBrowser.titleS3');
  }

  get rootHint(): string {
    return this.mode === 'local'
      ? this.translate.instant('directoryBrowser.rootHintLocal')
      : this.translate.instant('directoryBrowser.rootHintS3');
  }

  load(path: string): void {
    this.loading = true;
    this.error = '';
    const request: S3BrowseRequest = { ...this.data.s3Connection, prefix: path };
    const observable =
      this.mode === 'local'
        ? this.geoserverService.browseLocalDirectory(path)
        : this.geoserverService.browseS3Directory(request);

    observable.subscribe({
      next: (entries) => {
        this.entries = entries;
        this.currentPath = path;
        this.selectedPath = '';
        this.buildBreadcrumbs(path);
        this.loading = false;
      },
      error: (err) => {
        this.loading = false;
        this.error = err.error?.message || this.translate.instant('directoryBrowser.loadFail');
      },
    });
  }

  private buildBreadcrumbs(path: string): void {
    const crumbs: { label: string; path: string }[] = [];
    if (path) {
      const sep = this.mode === 'local' ? /[\\/]+/ : /\//;
      const parts = path.split(sep).filter((p) => p.length > 0);
      let acc = '';
      parts.forEach((part) => {
        acc = acc ? `${acc}/${part}` : part;
        crumbs.push({ label: part, path: acc });
      });
      if (this.mode === 'local') {
        crumbs.forEach((c) => {
          c.path = c.path.split('/').join('\\');
        });
      }
    }
    this.breadcrumbs = crumbs;
  }

  goUp(): void {
    if (!this.currentPath) {
      return;
    }
    const sep = this.mode === 'local' ? /[\\/]+/ : /\//;
    const parts = this.currentPath.split(sep).filter(Boolean);
    parts.pop();
    const parent =
      this.mode === 'local' ? parts.join('\\') : parts.length ? `${parts.join('/')}/` : '';
    this.load(parent);
  }

  navigate(entry: FileEntry): void {
    if (entry.is_dir) {
      this.load(entry.path);
    } else {
      this.selectedPath = entry.path;
    }
  }

  select(entry: FileEntry): void {
    this.selectedPath = entry.path;
  }

  selectCurrent(): void {
    if (this.selectedPath) {
      const entry = this.entries.find((e) => e.path === this.selectedPath);
      this.dialogRef.close({
        path: this.selectedPath,
        is_dir: entry?.is_dir ?? false,
        name: entry?.name,
      } as DirectoryBrowserResult);
    } else if (this.currentPath) {
      // 未选中条目时, 选择当前目录/前缀
      this.dialogRef.close({
        path: this.currentPath,
        is_dir: true,
        name: this.currentPath,
      } as DirectoryBrowserResult);
    }
  }

  onCancel(): void {
    this.dialogRef.close();
  }
}
