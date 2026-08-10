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
  groups: ServiceGroup[] = [
    {
      id: 'ogc',
      title: 'OGC 标准服务',
      icon: 'public',
      items: [
        {
          id: 'wms',
          name: 'WMS',
          fullName: 'Web Map Service · 网络地图服务',
          icon: 'map',
          description:
            '以图片方式渲染地图图层，支持要素查询与图例输出，可生成 OpenLayers 交互预览。',
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
          fullName: 'Web Map Tile Service · 网络地图瓦片服务',
          icon: 'grid_on',
          description: '按瓦片矩阵切分地图，同时支持 KVP 与 RESTful 瓦片模板（含本地瓦片缓存）。',
          endpoint: '/wmts',
          operations: ['GetCapabilities', 'GetTile', 'GetFeatureInfo', 'RESTful 瓦片'],
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
          fullName: 'Web Feature Service · 网络要素服务',
          icon: 'table_rows',
          description: '以 GML / GeoJSON 提供矢量要素的查询与下载，支持 GET 与 POST 请求。',
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
          fullName: 'Web Coverage Service · 网络覆盖服务',
          icon: 'layers',
          description: '发布与检索栅格覆盖数据（GeoTIFF 等），支持切片与重投影。',
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
          fullName: 'Web Processing Service · 网络处理服务',
          icon: 'science',
          description: '内置纯 Rust 处理引擎，支持栅格切片 / 坡度 / 重分类等内置处理进程。',
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
          fullName: 'Catalog Service for the Web · 网络目录服务',
          icon: 'folder_open',
          description: '地理元数据目录的发现与检索，支持 GET 与 POST 请求。',
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
          fullName: '统一 OGC 调度端点',
          icon: 'hub',
          description: '对标 GeoServer /ows 的统一调度端点，按 service 参数分发到各 OGC 服务。',
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
      title: 'OGC API 服务',
      icon: 'api',
      items: [
        {
          id: 'ogc-features',
          name: 'OGC API - Features',
          fullName: 'OGC API 要素',
          icon: 'dataset',
          description:
            '以 RESTful 方式提供要素集合列表与要素查询（landing / conformance / collections / items）。',
          endpoint: '/ogc/features/',
          operations: ['Landing', 'Conformance', 'Collections', 'Items'],
          testUrl: '/ogc/features/',
          versions: [{ version: '1.0', url: '/ogc/features/', note: '能力文档' }],
        },
        {
          id: 'ogc-tiles',
          name: 'OGC API - Tiles',
          fullName: 'OGC API 瓦片',
          icon: 'grid_view',
          description:
            '以 RESTful 方式提供瓦片矩阵集与瓦片访问（tileMatrixSets / collections / tiles）。',
          endpoint: '/ogc/tiles/',
          operations: ['TileMatrixSets', 'Collections', 'Tiles'],
          testUrl: '/ogc/tiles/',
          versions: [{ version: '1.0', url: '/ogc/tiles/', note: '能力文档' }],
        },
        {
          id: 'ogc-maps',
          name: 'OGC API - Maps',
          fullName: 'OGC API 地图',
          icon: 'map',
          description: '以 RESTful 方式提供地图渲染与样式（collections / styles / map）。',
          endpoint: '/ogc/maps/',
          operations: ['Collections', 'Styles', 'Map'],
          testUrl: '/ogc/maps/',
          versions: [{ version: '1.0', url: '/ogc/maps/', note: '能力文档' }],
        },
        {
          id: 'ogc-processes',
          name: 'OGC API - Processes',
          fullName: 'OGC API 处理',
          icon: 'auto_awesome',
          description: '以 RESTful 方式提交并跟踪异步处理任务（processes / jobs）。',
          endpoint: '/ogc/processes/',
          operations: ['Processes', 'Execute', 'Jobs'],
          testUrl: '/ogc/processes/',
          versions: [{ version: '1.0', url: '/ogc/processes/', note: '能力文档' }],
        },
      ],
    },
    {
      id: 'tile',
      title: '瓦片与缓存',
      icon: 'grid_view',
      items: [
        {
          id: 'tms',
          name: 'TMS',
          fullName: 'Tile Map Service · 瓦片地图服务',
          icon: 'view_module',
          description: '对标 GeoWebCache 的 TMS 1.0.0 瓦片服务，支持 RESTful 与 KVP 两种访问方式。',
          endpoint: '/gwc/service/tms/1.0.0',
          operations: ['TileMapService', 'TileMap', 'Tile'],
          testUrl: '/gwc/service/tms/1.0.0',
          versions: [{ version: '1.0.0', url: '/gwc/service/tms/1.0.0' }],
        },
        {
          id: 'wmsc',
          name: 'WMS-C',
          fullName: '缓存 WMS · WMS-C',
          icon: 'photo_library',
          description: '对标 GeoWebCache 的缓存 WMS 瓦片服务，按固定网格切分地图。',
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
          fullName: '矢量瓦片 · Mapbox Vector Tile',
          icon: 'view_in_ar',
          description: '矢量要素瓦片，支持 .pbf 后缀，适用于高可缩放客户端渲染。',
          endpoint: '/geoserver/mvt/{layer}/{z}/{x}/{y}',
          operations: ['.pbf 矢量瓦片'],
        },
        {
          id: 'tile-rest',
          name: '瓦片 REST',
          fullName: '通用栅格瓦片',
          icon: 'grid_4x4',
          description: '本地瓦片缓存提供的通用栅格瓦片端点，作为其他瓦片服务的底层实现。',
          endpoint: '/geoserver/tiles/{layer}/{z}/{x}/{y}',
          operations: ['栅格瓦片'],
        },
      ],
    },
  ];

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
