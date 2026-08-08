import { Component, OnInit } from '@angular/core';
import { GeoserverService } from '../../services/geoserver.service';
import { ServerStatus } from '../../models/geoserver.models';

@Component({
  selector: 'app-server-status',
  templateUrl: './server-status.component.html',
  styleUrls: ['./server-status.component.scss'],
})
export class ServerStatusComponent implements OnInit {
  status: ServerStatus = {
    uptime: '0天 0小时 0分钟',
    memory: { used: 0, total: 512, percent: 0 },
    cpu: 0,
    requests: 0,
    errors: 0,
    layerCount: 0,
    enabledLayers: 0,
    workspaceCount: 0,
  };
  loading = false;

  constructor(private geoserverService: GeoserverService) {}

  ngOnInit(): void {
    this.loadStatus();
  }

  loadStatus(): void {
    this.loading = true;
    this.geoserverService.getServerStatus().subscribe({
      next: (data) => {
        this.status = data;
        this.loading = false;
      },
      error: (error) => {
        console.error('Failed to load server status:', error);
        this.loading = false;
      },
    });
  }
}
