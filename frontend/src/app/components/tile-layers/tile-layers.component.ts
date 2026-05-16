import { Component, OnInit } from '@angular/core';

@Component({
  selector: 'app-tile-layers',
  templateUrl: './tile-layers.component.html',
  styleUrls: ['./tile-layers.component.scss']
})
export class TileLayersComponent implements OnInit {
  tileLayers = [
    { name: 'osm_base', format: 'PNG', minZoom: 0, maxZoom: 18, enabled: true },
    { name: 'satellite_tiles', format: 'JPEG', minZoom: 5, maxZoom: 16, enabled: true },
    { name: 'terrain', format: 'PNG', minZoom: 8, maxZoom: 14, enabled: false }
  ];
  loading = false;

  ngOnInit(): void {}
}
