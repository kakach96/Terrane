import { Component, OnInit, OnDestroy } from '@angular/core';
import { TranslateService } from '@ngx-translate/core';
import { GeoserverService } from '../../services/geoserver.service';
import { MonitorStats, RequestRecord, AuditLogEntry } from '../../models/geoserver.models';

@Component({
  selector: 'app-monitor',
  templateUrl: './monitor.component.html',
  styleUrls: ['./monitor.component.scss'],
})
export class MonitorComponent implements OnInit, OnDestroy {
  stats: MonitorStats | null = null;
  recentRequests: RequestRecord[] = [];
  auditLogs: AuditLogEntry[] = [];
  loading = true;
  error = '';
  activeTab: 'overview' | 'requests' | 'audit' = 'overview';
  refreshInterval: ReturnType<typeof setInterval> | null = null;

  // 图表数据
  requestHistory: number[] = [];
  requestLabels: string[] = [];

  constructor(
    private geoserver: GeoserverService,
    private translate: TranslateService,
  ) {}

  ngOnInit(): void {
    this.loadData();
    // 每 10 秒自动刷新
    this.refreshInterval = setInterval(() => this.loadData(), 10000);
  }

  ngOnDestroy(): void {
    if (this.refreshInterval) {
      clearInterval(this.refreshInterval);
    }
  }

  loadData(): void {
    this.loading = true;
    this.geoserver.getMonitorStats().subscribe({
      next: (s) => {
        this.stats = s;
        this.loading = false;
      },
      error: (e) => {
        this.error = this.translate.instant('monitor.loadFail', { message: e.message });
        this.loading = false;
      },
    });

    this.geoserver.getRecentRequests(50).subscribe({
      next: (r) => (this.recentRequests = r),
      error: () => {},
    });

    this.geoserver.getAuditLogs(50, 0).subscribe({
      next: (l) => (this.auditLogs = l),
      error: () => {},
    });
  }

  get uptimeFormatted(): string {
    if (!this.stats) return '';
    const s = this.stats.uptime_seconds;
    const d = Math.floor(s / 86400);
    const h = Math.floor((s % 86400) / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = s % 60;
    const parts: string[] = [];
    if (d > 0) parts.push(this.translate.instant('monitor.uptimeDay', { count: d }));
    if (h > 0) parts.push(this.translate.instant('monitor.uptimeHour', { count: h }));
    if (m > 0) parts.push(this.translate.instant('monitor.uptimeMinute', { count: m }));
    parts.push(this.translate.instant('monitor.uptimeSecond', { count: sec }));
    return parts.join(' ');
  }

  get errorRateFormatted(): string {
    if (!this.stats) return '0%';
    return this.stats.error_rate.toFixed(2) + '%';
  }

  get topEndpoints(): { name: string; count: number; avgDuration: number }[] {
    if (!this.stats) return [];
    return Object.entries(this.stats.endpoints)
      .sort((a, b) => b[1].count - a[1].count)
      .slice(0, 10)
      .map(([name, data]) => ({
        name,
        count: data.count,
        avgDuration: data.avg_duration_ms,
      }));
  }

  get methodChart(): { method: string; count: number }[] {
    if (!this.stats) return [];
    return Object.entries(this.stats.methods)
      .sort((a, b) => b[1] - a[1])
      .map(([method, count]) => ({ method, count }));
  }

  get statusChart(): { code: string; count: number }[] {
    if (!this.stats) return [];
    return Object.entries(this.stats.status_codes)
      .sort((a, b) => b[1] - a[1])
      .map(([code, count]) => ({ code, count }));
  }

  resetStats(): void {
    if (confirm(this.translate.instant('monitor.confirmReset'))) {
      this.geoserver.resetMonitorStats().subscribe({
        next: () => this.loadData(),
        error: (e) =>
          (this.error = this.translate.instant('monitor.resetFail', { message: e.message })),
      });
    }
  }

  switchTab(tab: 'overview' | 'requests' | 'audit'): void {
    this.activeTab = tab;
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
    if (!this.stats) return 1;
    const counts = Object.values(this.stats.methods);
    return Math.max(...counts, 1);
  }
}
