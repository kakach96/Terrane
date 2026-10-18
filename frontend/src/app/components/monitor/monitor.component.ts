import {
  Component,
  ChangeDetectionStrategy,
  computed,
  inject,
  signal,
  DestroyRef,
} from '@angular/core';
import { toSignal, toObservable } from '@angular/core/rxjs-interop';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { TranslateService } from '@ngx-translate/core';
import { MatTabChangeEvent } from '@angular/material/tabs';
import { TerraneService } from '../../services/terrane.service';
import { LanguageService } from '../../services/language.service';
import {
  MonitorStats,
  AuditLogEntry,
  TileCacheStats,
  MonitorHealth,
  CatalogStats,
  SeedJob,
} from '../../models/terrane.models';
import { switchMap, map, startWith, catchError, of, combineLatest, interval } from 'rxjs';

interface MonitorData {
  stats: MonitorStats | null;
  audit: AuditLogEntry[];
  tileCache: TileCacheStats | null;
  health: MonitorHealth | null;
  catalog: CatalogStats | null;
  seedJobs: SeedJob[];
}

@Component({
  standalone: false,
  selector: 'app-monitor',
  templateUrl: './monitor.component.html',
  styleUrls: ['./monitor.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class MonitorComponent {
  private terrane = inject(TerraneService);
  private translate = inject(TranslateService);
  private languageService = inject(LanguageService);
  private destroyRef = inject(DestroyRef);

  activeTab: 'overview' | 'cache' | 'audit' = 'overview';
  error = '';
  private refreshTrigger = signal(0);

  // ── Signal pipeline ───────────────────────────────────────────────
  private monitorData$ = toObservable(this.refreshTrigger).pipe(
    startWith(0),
    switchMap(() =>
      combineLatest([
        this.terrane.getMonitorStats().pipe(
          catchError((e) => {
            this.error = this.translate.instant('monitor.loadFail', { message: e.message });
            return of(null as MonitorStats | null);
          }),
        ),
        this.terrane.getAuditLogs(50, 0).pipe(catchError(() => of([] as AuditLogEntry[]))),
        this.terrane
          .getTileCacheStats()
          .pipe(catchError(() => of(null as TileCacheStats | null))),
        this.terrane.getMonitorHealth().pipe(catchError(() => of(null as MonitorHealth | null))),
        this.terrane.getCatalogStats().pipe(catchError(() => of(null as CatalogStats | null))),
        this.terrane.getSeedJobs().pipe(catchError(() => of([] as SeedJob[]))),
      ]).pipe(
        map(
          ([stats, audit, tileCache, health, catalog, seedJobs]) =>
            ({ stats, audit, tileCache, health, catalog, seedJobs }) as MonitorData,
        ),
      ),
    ),
  );

  private data = toSignal(this.monitorData$, {
    initialValue: {
      stats: null,
      audit: [],
      tileCache: null,
      health: null,
      catalog: null,
      seedJobs: [],
    } as MonitorData,
  });

  // Derived signals
  stats = computed(() => this.data().stats);
  auditLogs = computed(() => this.data().audit);
  tileCacheStats = computed(() => this.data().tileCache);
  health = computed(() => this.data().health);
  catalog = computed(() => this.data().catalog);
  seedJobs = computed(() => this.data().seedJobs);
  loading = computed(() => this.data().stats === null && !this.error);

  // ── Computed helpers ──────────────────────────────────────────────
  uptimeFormatted = computed(() => {
    // Read currentLang to re-translate on language switch.
    this.languageService.currentLang();
    const s = this.stats();
    if (!s) return '';
    const sec = s.uptime_seconds;
    const d = Math.floor(sec / 86400);
    const h = Math.floor((sec % 86400) / 3600);
    const m = Math.floor((sec % 3600) / 60);
    const parts: string[] = [];
    if (d > 0) parts.push(this.translate.instant('monitor.uptimeDay', { count: d }));
    if (h > 0) parts.push(this.translate.instant('monitor.uptimeHour', { count: h }));
    if (m > 0) parts.push(this.translate.instant('monitor.uptimeMinute', { count: m }));
    parts.push(this.translate.instant('monitor.uptimeSecond', { count: sec % 60 }));
    return parts.join(' ');
  });

  constructor() {
    // Auto-refresh every 10 seconds
    interval(10000)
      .pipe(takeUntilDestroyed(this.destroyRef))
      .subscribe(() => this.refreshTrigger.update((v) => v + 1));
  }

  // ── Actions ───────────────────────────────────────────────────────
  refreshData(): void {
    this.refreshTrigger.update((v) => v + 1);
  }

  switchTab(tab: 'overview' | 'cache' | 'audit'): void {
    this.activeTab = tab;
  }

  /** Map a mat-tab index back to the active tab id. */
  onTabChange(event: MatTabChangeEvent): void {
    const tabs = ['overview', 'cache', 'audit'] as const;
    this.activeTab = tabs[event.index] ?? tabs[0];
  }

  // ── Seed jobs ─────────────────────────────────────────────────────
  /** Seed job progress percent (0..100), falls back to 0 when total is 0. */
  seedProgress(job: SeedJob): number {
    return job.total > 0 ? Math.round((job.done / job.total) * 100) : 0;
  }

  /** Cancel a running seed job (cooperative). */
  cancelSeedJob(job: SeedJob): void {
    this.terrane.cancelSeedJob(job.id).subscribe({
      next: () => this.refreshTrigger.update((v) => v + 1),
      error: () => this.refreshTrigger.update((v) => v + 1),
    });
  }

  formatTimestamp(ts: string): string {
    try {
      const d = new Date(ts);
      return d.toLocaleString('zh-CN');
    } catch {
      return ts;
    }
  }
}
