import { Component, OnInit } from '@angular/core';
import { GeoserverService } from '../../services/geoserver.service';
import { DashboardStats, Layer } from '../../models/geoserver.models';

@Component({
  selector: 'app-dashboard',
  templateUrl: './dashboard.component.html',
  styleUrls: ['./dashboard.component.scss']
})
export class DashboardComponent implements OnInit {
  stats: DashboardStats = {
    layerCount: 0,
    featureCount: 0,
    activeLayerCount: 0,
    workspaceCount: 0
  };
  recentLayers: Layer[] = [];
  loading = true;

  constructor(private geoserverService: GeoserverService) {}

  ngOnInit(): void {
    this.loadData();
  }

  loadData(): void {
    this.loading = true;
    
    this.geoserverService.getDashboardStats().subscribe({
      next: (stats) => {
        this.stats = stats;
        this.loading = false;
      },
      error: (error) => {
        console.error('Failed to load stats:', error);
        this.loading = false;
      }
    });

    this.geoserverService.getLayers().subscribe({
      next: (layers) => {
        this.recentLayers = layers.slice(0, 5);
      }
    });
  }

  refresh(): void {
    this.loadData();
  }
}
