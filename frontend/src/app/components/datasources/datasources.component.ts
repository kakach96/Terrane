import { Component, OnInit } from '@angular/core';

@Component({
  selector: 'app-datasources',
  templateUrl: './datasources.component.html',
  styleUrls: ['./datasources.component.scss']
})
export class DatasourcesComponent implements OnInit {
  datasources = [
    { name: 'postgis_main', type: 'PostGIS', workspace: 'default', enabled: true },
    { name: 'shapefile_demo', type: 'Shapefile', workspace: 'demo', enabled: true },
    { name: 'geotiff_data', type: 'GeoTIFF', workspace: 'default', enabled: false }
  ];
  loading = false;

  ngOnInit(): void {}
}
