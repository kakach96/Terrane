import { Component, OnInit } from '@angular/core';
import { GeoserverService } from '../../services/geoserver.service';
import { NotificationService } from '../../services/notification.service';

interface Store {
  name: string;
  type: 'DataStore' | 'CoverageStore';
  workspace: string | null;
  enabled: boolean;
  connection?: any;
  created?: string;
  modified?: string;
}

@Component({
  selector: 'app-stores',
  templateUrl: './stores.component.html',
  styleUrls: ['./stores.component.scss']
})
export class StoresComponent implements OnInit {
  stores: Store[] = [];
  loading = false;
  displayedColumns = ['name', 'type', 'workspace', 'enabled', 'actions'];
  filterType: string = 'all';

  constructor(
    private geoserverService: GeoserverService,
    private notificationService: NotificationService,
  ) {}

  ngOnInit(): void {
    this.loadStores();
  }

  get filteredStores(): Store[] {
    if (this.filterType === 'all') return this.stores;
    return this.stores.filter(s => s.type === this.filterType);
  }

  loadStores(): void {
    this.loading = true;
    this.geoserverService.getStores().subscribe({
      next: (data) => {
        this.stores = data;
        this.loading = false;
      },
      error: (error) => {
        console.error('Failed to load stores:', error);
        this.loading = false;
      }
    });
  }

  getStoreTypeIcon(type: string): string {
    return type === 'DataStore' ? 'storage' : 'image';
  }

  getStoreTypeColor(type: string): string {
    return type === 'DataStore' ? 'primary' : 'accent';
  }

  getConnectionSummary(store: Store): string {
    if (!store.connection) return '-';
    const conn = store.connection;
    if (conn.host) {
      return `${conn.host}:${conn.port || 5432}/${conn.database || ''}`;
    }
    if (conn.file_path) {
      return conn.file_path.split(/[\\/]/).pop() || conn.file_path;
    }
    return '-';
  }
}
