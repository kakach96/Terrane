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
  RequestRecord,
  AuditLogEntry,
  TileCacheStats,
} from '../../models/terrane.models';
import { switchMap, map, startWith, catchError, of, combineLatest, interval } from 'rxjs';

interface MonitorData {
  stats: MonitorStats | null;
  requests: RequestRecord[];
  audit: AuditLogEntry[];
  tileCache: TileCacheStats | null;
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

  activeTab: 'overview' | 'requests' | 'audit' = 'overview';
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
        this.terrane.getRecentRequests(50).pipe(catchError(() => of([] as RequestRecord[]))),
        this.terrane.getAuditLogs(50, 0).pipe(catchError(() => of([] as AuditLogEntry[]))),
        this.terrane
          .getTileCacheStats()
          .pipe(catchError(() => of(null as TileCacheStats | null))),
      ]).pipe(
        map(
          ([stats, requests, audit, tileCache]) =>
            ({ stats, requests, audit, tileCache }) as MonitorData,
        ),
      ),
    ),
  );

  private data = toSignal(this.monitorData$, {
    initialValue: { stats: null, requests: [], audit: [], tileCache: null } as MonitorData,
  });

  // Derived signals
  stats = computed(() => this.data().stats);
  recentRequests = computed(() => this.data().requests);
  auditLogs = computed(() => this.data().audit);
  tileCacheStats = computed(() => this.data().tileCache);
  loading = computed(() => this.data().stats === null && !this.error);

  // ── Computed chart data ───────────────────────────────────────────
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

  errorRateFormatted = computed(() => {
    const s = this.stats();
    return s ? s.error_rate.toFixed(2) + '%' : '0%';
  });

  topEndpoints = computed(() => {
    const s = this.stats();
    if (!s) return [];
    return Object.entries(s.endpoints)
      .sort((a, b) => b[1].count - a[1].count)
      .slice(0, 10)
      .map(([name, data]) => ({
        name,
        count: data.count,
        avgDuration: data.avg_duration_ms,
      }));
  });

  methodChart = computed(() => {
    const s = this.stats();
    if (!s) return [];
    return Object.entries(s.methods)
      .sort((a, b) => b[1] - a[1])
      .map(([method, count]) => ({ method, count }));
  });

  statusChart = computed(() => {
    const s = this.stats();
    if (!s) return [];
    return Object.entries(s.status_codes)
      .sort((a, b) => b[1] - a[1])
      .map(([code, count]) => ({ code, count }));
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

  resetStats(): void {
    if (confirm(this.translate.instant('monitor.confirmReset'))) {
      this.terrane.resetMonitorStats().subscribe({
        next: () => this.refreshTrigger.update((v) => v + 1),
        error: (e) =>
          (this.error = this.translate.instant('monitor.resetFail', { message: e.message })),
      });
    }
  }

  switchTab(tab: 'overview' | 'requests' | 'audit'): void {
    this.activeTab = tab;
  }

  /** Map a mat-tab index back to the active tab id. */
  onTabChange(event: MatTabChangeEvent): void {
    const tabs = ['overview', 'requests', 'audit'] as const;
    this.activeTab = tabs[event.index] ?? tabs[0];
  }

  formatTimestamp(ts: string): string {
    try {
      const d = new Date(ts);
      return d.toLocaleString('zh-CN');
    } catch {
      return ts;
    }
  }

  getMaxMethodCount(): number {
    const s = this.stats();
    if (!s) return 1;
    const counts = Object.values(s.methods);
    return Math.max(...counts, 1);
  }
}
