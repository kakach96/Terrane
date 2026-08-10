import { Component } from '@angular/core';

export interface ServiceVersion {
  /** Implemented version label, e.g. "1.3.0" / "WMS 1.3.0". */
  version: string;
  /** GetCapabilities / capability-document URL opened in a new tab. */
  url: string;
  /** Optional doc-type note (e.g. "能力文档" for RESTful OGC API). */
  note?: string;
}

export interface ServiceEntry {
  id: string;
  /** Service acronym, e.g. "WMS". */
  name: string;
  /** Full display name (EN + CN). */
  fullName: string;
  /** Material icon name. */
  icon: string;
  /** Short description. */
  description: string;
  /** Endpoint path(s). */
  endpoint: string;
  /** Supported operations / request types. */
  operations: string[];
  /** Implemented versions (clicking opens GetCapabilities in a new tab). */
  versions?: ServiceVersion[];
  /** Clean URL used by the "打开端点" button (when endpoint has placeholders). */
  testUrl?: string;
}

export interface ServiceGroup {
  id: string;
  title: string;
  icon: string;
  items: ServiceEntry[];
}

@Component({
  selector: 'app-services',
  templateUrl: './services.component.html',
  styleUrls: ['./services.component.scss'],
})
export class ServicesComponent {
  private _groups: ServiceGroup[] | null = null;

  /** Lazy-built service groups (localized at first access). */
  get groups(): ServiceGroup[] {
    return this._groups ?? (this._groups = this.buildGroups());
  }

  private buildGroups(): ServiceGroup[] {
    return [
      {
        id: 'ogc',
        title: 'services.group.ogc',
        icon: 'public',
        items: [
          {
            id: 'wms',
            name: 'WMS',
            fullName: 'services.wms.fullName',
            icon: 'map',
            description: 'services.wms.description',
            endpoint: '/wms',
            operations: ['GetCapabilities', 'GetMap', 'GetFeatureInfo', 'GetLegendGraphic'],
            testUrl: '/wms?service=WMS&version=1.3.0&request=GetCapabilities',
            versions: [
              {
                version: '1.1.1',
                url: '/wms?service=WMS&version=1.1.1&request=GetCapabilities',
              },
              {
                version: '1.3.0',
                url: '/wms?service=WMS&version=1.3.0&request=GetCapabilities',
              },
            ],
          },
          {
            id: 'wmts',
            name: 'WMTS',
            fullName: 'services.wmts.fullName',
            icon: 'grid_on',
            description: 'services.wmts.description',
            endpoint: '/wmts',
            operations: [
              'GetCapabilities',
              'GetTile',
              'GetFeatureInfo',
              'services.operationsRESTfulTiles',
            ],
            testUrl: '/wmts?service=WMTS&version=1.0.0&request=GetCapabilities',
            versions: [
              {
                version: '1.0.0',
                url: '/wmts?service=WMTS&version=1.0.0&request=GetCapabilities',
              },
            ],
          },
          {
            id: 'wfs',
            name: 'WFS',
            fullName: 'services.wfs.fullName',
            icon: 'table_rows',
            description: 'services.wfs.description',
            endpoint: '/wfs',
            operations: ['GetCapabilities', 'DescribeFeatureType', 'GetFeature'],
            testUrl: '/wfs?service=WFS&version=2.0.0&request=GetCapabilities',
            versions: [
              {
                version: '2.0.0',
                url: '/wfs?service=WFS&version=2.0.0&request=GetCapabilities',
              },
            ],
          },
          {
            id: 'wcs',
            name: 'WCS',
            fullName: 'services.wcs.fullName',
            icon: 'layers',
            description: 'services.wcs.description',
            endpoint: '/wcs',
            operations: ['GetCapabilities', 'DescribeCoverage', 'GetCoverage'],
            testUrl: '/wcs?service=WCS&version=2.0.1&request=GetCapabilities',
            versions: [
              {
                version: '2.0.1',
                url: '/wcs?service=WCS&version=2.0.1&request=GetCapabilities',
              },
            ],
          },
          {
            id: 'wps',
            name: 'WPS',
            fullName: 'services.wps.fullName',
            icon: 'science',
            description: 'services.wps.description',
            endpoint: '/wps',
            operations: ['GetCapabilities', 'DescribeProcess', 'Execute'],
            testUrl: '/wps?service=WPS&version=1.0.0&request=GetCapabilities',
            versions: [
              {
                version: '1.0.0',
                url: '/wps?service=WPS&version=1.0.0&request=GetCapabilities',
              },
            ],
          },
          {
            id: 'csw',
            name: 'CSW',
            fullName: 'services.csw.fullName',
            icon: 'folder_open',
            description: 'services.csw.description',
            endpoint: '/csw',
            operations: ['GetCapabilities', 'GetRecords', 'GetRecordById', 'GetDomain'],
            testUrl: '/csw?service=CSW&version=2.0.2&request=GetCapabilities',
            versions: [
              {
                version: '2.0.2',
                url: '/csw?service=CSW&version=2.0.2&request=GetCapabilities',
              },
            ],
          },
          {
            id: 'ows',
            name: 'OWS',
            fullName: 'services.ows.fullName',
            icon: 'hub',
            description: 'services.ows.description',
            endpoint: '/geoserver/ows',
            operations: ['service=WMS / WFS / WCS / WPS / CSW'],
            testUrl: '/geoserver/ows?service=WMS&version=1.3.0&request=GetCapabilities',
            versions: [
              {
                version: 'WMS 1.3.0',
                url: '/geoserver/ows?service=WMS&version=1.3.0&request=GetCapabilities',
              },
              {
                version: 'WFS 2.0.0',
                url: '/geoserver/ows?service=WFS&version=2.0.0&request=GetCapabilities',
              },
              {
                version: 'WCS 2.0.1',
                url: '/geoserver/ows?service=WCS&version=2.0.1&request=GetCapabilities',
              },
              {
                version: 'WPS 1.0.0',
                url: '/geoserver/ows?service=WPS&version=1.0.0&request=GetCapabilities',
              },
              {
                version: 'CSW 2.0.2',
                url: '/geoserver/ows?service=CSW&version=2.0.2&request=GetCapabilities',
              },
            ],
          },
        ],
      },
      {
        id: 'ogcapi',
        title: 'services.group.ogcapi',
        icon: 'api',
        items: [
          {
            id: 'ogc-features',
            name: 'OGC API - Features',
            fullName: 'services.ogcFeatures.fullName',
            icon: 'dataset',
            description: 'services.ogcFeatures.description',
            endpoint: '/ogc/features/',
            operations: ['Landing', 'Conformance', 'Collections', 'Items'],
            testUrl: '/ogc/features/',
            versions: [
              {
                version: '1.0',
                url: '/ogc/features/',
                note: 'services.noteDoc',
              },
            ],
          },
          {
            id: 'ogc-tiles',
            name: 'OGC API - Tiles',
            fullName: 'services.ogcTiles.fullName',
            icon: 'grid_view',
            description: 'services.ogcTiles.description',
            endpoint: '/ogc/tiles/',
            operations: ['TileMatrixSets', 'Collections', 'Tiles'],
            testUrl: '/ogc/tiles/',
            versions: [
              {
                version: '1.0',
                url: '/ogc/tiles/',
                note: 'services.noteDoc',
              },
            ],
          },
          {
            id: 'ogc-maps',
            name: 'OGC API - Maps',
            fullName: 'services.ogcMaps.fullName',
            icon: 'map',
            description: 'services.ogcTiles.description',
            endpoint: '/ogc/maps/',
            operations: ['Collections', 'Styles', 'Map'],
            testUrl: '/ogc/maps/',
            versions: [
              {
                version: '1.0',
                url: '/ogc/maps/',
                note: 'services.noteDoc',
              },
            ],
          },
          {
            id: 'ogc-processes',
            name: 'OGC API - Processes',
            fullName: 'services.ogcProcesses.fullName',
            icon: 'auto_awesome',
            description: 'services.ogcProcesses.description',
            endpoint: '/ogc/processes/',
            operations: ['Processes', 'Execute', 'Jobs'],
            testUrl: '/ogc/processes/',
            versions: [
              {
                version: '1.0',
                url: '/ogc/processes/',
                note: 'services.noteDoc',
              },
            ],
          },
        ],
      },
      {
        id: 'tile',
        title: 'services.group.tile',
        icon: 'grid_view',
        items: [
          {
            id: 'tms',
            name: 'TMS',
            fullName: 'services.tms.fullName',
            icon: 'view_module',
            description: 'services.tms.description',
            endpoint: '/gwc/service/tms/1.0.0',
            operations: ['TileMapService', 'TileMap', 'Tile'],
            testUrl: '/gwc/service/tms/1.0.0',
            versions: [{ version: '1.0.0', url: '/gwc/service/tms/1.0.0' }],
          },
          {
            id: 'wmsc',
            name: 'WMS-C',
            fullName: 'services.wmsc.fullName',
            icon: 'photo_library',
            description: 'services.wmsc.description',
            endpoint: '/gwc/service/wms',
            operations: ['GetCapabilities', 'GetMap'],
            testUrl: '/gwc/service/wms?SERVICE=WMS&REQUEST=GetCapabilities',
            versions: [
              {
                version: '1.1.1',
                url: '/gwc/service/wms?SERVICE=WMS&REQUEST=GetCapabilities',
              },
            ],
          },
          {
            id: 'mvt',
            name: 'MVT',
            fullName: 'services.mvt.fullName',
            icon: 'view_in_ar',
            description: 'services.mvt.description',
            endpoint: '/geoserver/mvt/{layer}/{z}/{x}/{y}',
            operations: ['services.operationsPbfTiles'],
          },
          {
            id: 'tile-rest',
            name: 'services.tileRest.name',
            fullName: 'services.tileRest.fullName',
            icon: 'grid_4x4',
            description: 'services.tileRest.description',
            endpoint: '/geoserver/tiles/{layer}/{z}/{x}/{y}',
            operations: ['services.operationsRasterTiles'],
          },
        ],
      },
    ];
  }

  get totalCount(): number {
    return this.groups.reduce((sum, g) => sum + g.items.length, 0);
  }

  groupCount(groupId: string): number {
    const group = this.groups.find((g) => g.id === groupId);
    return group ? group.items.length : 0;
  }

  /** Full URL for an endpoint path, resolved against the current origin. */
  fullUrl(path: string): string {
    return window.location.origin + path;
  }

  /** Smooth-scroll to a service group section (used by the stat cards). */
  scrollToGroup(groupId: string): void {
    const el = document.getElementById(`group-${groupId}`);
    el?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
}
