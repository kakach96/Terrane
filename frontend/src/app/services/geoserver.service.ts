import { Injectable } from '@angular/core';
import { HttpClient, HttpParams } from '@angular/common/http';
import { Observable } from 'rxjs';
import { map } from 'rxjs/operators';
import {
  Layer,
  LayerBounds,
  Feature,
  FeatureCollection,
  CreateLayerRequest,
  UpdateLayerRequest,
  CreateFeatureRequest,
  PropertyDef,
  StyleInfo,
  ApiResponse,
  DashboardStats,
  PreviewOptions,
  Workspace,
  CreateWorkspaceRequest,
  UpdateWorkspaceRequest,
  Namespace,
  CreateNamespaceRequest,
  UpdateNamespaceRequest,
  SqlView,
  CreateSqlViewRequest,
  UpdateSqlViewRequest,
  ServerStatus,
  DataSource,
  CreateDataSourceRequest,
  UpdateDataSourceRequest,
  ConnectionTestResult,
  UploadResult,
  LayerGroup
} from '../models/geoserver.models';

@Injectable({
  providedIn: 'root'
})
export class GeoserverService {
  private readonly apiUrl = '/geoserver';

  constructor(private http: HttpClient) {}

  getLayers(): Observable<Layer[]> {
    return this.http.get<ApiResponse<Layer[]>>(`${this.apiUrl}/layers`)
      .pipe(map(response => response.data || []));
  }

  getLayer(name: string): Observable<Layer> {
    return this.http.get<ApiResponse<Layer>>(`${this.apiUrl}/layers/${name}`)
      .pipe(map(response => response.data as Layer));
  }

  createLayer(layer: CreateLayerRequest): Observable<Layer> {
    return this.http.post<ApiResponse<Layer>>(`${this.apiUrl}/layers`, layer)
      .pipe(map(response => response.data as Layer));
  }

  updateLayer(name: string, updates: UpdateLayerRequest): Observable<void> {
    return this.http.put<ApiResponse<void>>(`${this.apiUrl}/layers/${name}`, updates)
      .pipe(map(() => void 0));
  }

  deleteLayer(name: string): Observable<void> {
    return this.http.delete<ApiResponse<void>>(`${this.apiUrl}/layers/${name}`)
      .pipe(map(() => void 0));
  }

  getLayerFeatures(layerName: string): Observable<FeatureCollection> {
    return this.http.get<FeatureCollection>(`${this.apiUrl}/layers/${layerName}/features`);
  }

  getFeature(layerName: string, featureId: string): Observable<Feature> {
    return this.http.get<Feature>(`${this.apiUrl}/layers/${layerName}/features/${featureId}`);
  }

  createFeature(layerName: string, feature: CreateFeatureRequest): Observable<Feature> {
    return this.http.post<Feature>(`${this.apiUrl}/layers/${layerName}/features`, feature);
  }

  deleteFeature(layerName: string, featureId: string): Observable<void> {
    return this.http.delete<void>(`${this.apiUrl}/layers/${layerName}/features/${featureId}`);
  }

  updateFeature(layerName: string, featureId: string, feature: CreateFeatureRequest): Observable<Feature> {
    return this.http.put<Feature>(`${this.apiUrl}/layers/${layerName}/features/${featureId}`, feature);
  }

  getPreviewUrl(layerName: string, options?: PreviewOptions): string {
    let params = new HttpParams();
    if (options?.width) {
      params = params.set('width', options.width.toString());
    }
    if (options?.height) {
      params = params.set('height', options.height.toString());
    }
    if (options?.format) {
      params = params.set('format', options.format);
    }
    const queryString = params.toString();
    return `${this.apiUrl}/layers/${layerName}/preview${queryString ? '?' + queryString : ''}`;
  }

  getWmsPreviewUrl(layer: Layer, width = 600, height = 400): string {
    const bounds: LayerBounds = (layer as any).native_bounds?.bounds || layer.bounds;
    if (!bounds) return '';
    const srs = (layer as any).native_bounds?.crs || layer.srs || 'EPSG:4326';
    // 使用 WMS 1.1.1 避免 EPSG:4326 轴序问题
    const params = new URLSearchParams({
      service: 'WMS',
      version: '1.1.1',
      request: 'GetMap',
      layers: layer.name,
      srs: srs,
      bbox: `${bounds.minx},${bounds.miny},${bounds.maxx},${bounds.maxy}`,
      width: width.toString(),
      height: height.toString(),
      format: 'application/openlayers',
    });
    return `/wms?${params}`;
  }

  getMapImageUrl(
    layer: Layer,
    options?: {
      width?: number;
      height?: number;
      crs?: string;
      bbox?: string;
      format?: 'image/png' | 'image/jpeg';
      transparent?: boolean;
      styles?: string;
    }
  ): string {
    const crs = options?.crs || (layer as any).native_bounds?.crs || layer.srs || 'EPSG:4326';

    // 优先使用显式传入的 bbox，其次从图层中读取
    let bbox = options?.bbox;
    if (!bbox) {
      const bounds: LayerBounds = (layer as any).native_bounds?.bounds || layer.bounds;
      if (!bounds) return '';
      bbox = `${bounds.minx},${bounds.miny},${bounds.maxx},${bounds.maxy}`;
    }

    const layerName = layer.name.includes(':') 
      ? layer.name 
      : `${layer.workspace}:${layer.name}`;

    const params = new URLSearchParams({
      service: 'WMS',
      version: '1.1.1',
      request: 'GetMap',
      layers: layerName,
      srs: crs,
      bbox: bbox,
      width: (options?.width || 600).toString(),
      height: (options?.height || 400).toString(),
      format: options?.format || 'image/png',
      transparent: (options?.transparent ?? true).toString(),
    });

    if (options?.styles) {
      params.set('styles', options.styles);
    }

    return `/wms?${params}`;
  }

  getDashboardStats(): Observable<DashboardStats> {
    return this.getLayers().pipe(
      map(layers => ({
        layerCount: layers.length,
        featureCount: 0,
        activeLayerCount: layers.filter(l => l.enabled).length,
        workspaceCount: new Set(layers.map(l => l.workspace)).size
      }))
    );
  }

  getWorkspaces(): Observable<string[]> {
    return this.getLayers().pipe(
      map(layers => [...new Set(layers.map(l => l.workspace))])
    );
  }

  getAllWorkspaces(): Observable<Workspace[]> {
    return this.http.get<ApiResponse<Workspace[]>>(`${this.apiUrl}/workspaces`)
      .pipe(map(response => response.data || []));
  }

  getWorkspace(name: string): Observable<Workspace> {
    return this.http.get<ApiResponse<Workspace>>(`${this.apiUrl}/workspaces/${name}`)
      .pipe(map(response => response.data as Workspace));
  }

  createWorkspace(request: CreateWorkspaceRequest): Observable<Workspace> {
    return this.http.post<ApiResponse<Workspace>>(`${this.apiUrl}/workspaces`, request)
      .pipe(map(response => response.data as Workspace));
  }

  updateWorkspace(name: string, updates: UpdateWorkspaceRequest): Observable<void> {
    return this.http.put<ApiResponse<void>>(`${this.apiUrl}/workspaces/${name}`, updates)
      .pipe(map(() => void 0));
  }

  deleteWorkspace(name: string): Observable<void> {
    return this.http.delete<ApiResponse<void>>(`${this.apiUrl}/workspaces/${name}`)
      .pipe(map(() => void 0));
  }

  // ---- 存储 (Stores) ----

  getStores(): Observable<any[]> {
    return this.http.get<ApiResponse<any[]>>(`${this.apiUrl}/stores`)
      .pipe(map(response => response.data || []));
  }

  getStore(name: string): Observable<any> {
    return this.http.get<ApiResponse<any>>(`${this.apiUrl}/stores/${name}`)
      .pipe(map(response => response.data));
  }

  getWorkspaceStores(workspace: string): Observable<any[]> {
    return this.http.get<ApiResponse<any[]>>(`${this.apiUrl}/workspaces/${workspace}/stores`)
      .pipe(map(response => response.data || []));
  }

  // ---- 瓦片缓存 (GeoWebCache) ----

  getTileCacheStats(): Observable<any> {
    return this.http.get<ApiResponse<any>>(`${this.apiUrl}/tiles/cache/stats`)
      .pipe(map(response => response.data));
  }

  clearTileCache(layerName: string): Observable<any> {
    return this.http.delete<ApiResponse<any>>(`${this.apiUrl}/tiles/cache/clear/${layerName}`)
      .pipe(map(response => response.data));
  }

  // ---- SQL 视图 ----

  getSqlViews(): Observable<SqlView[]> {
    return this.http.get<ApiResponse<SqlView[]>>(`${this.apiUrl}/sql-views`)
      .pipe(map(response => response.data || []));
  }

  getSqlView(name: string): Observable<SqlView> {
    return this.http.get<ApiResponse<SqlView>>(`${this.apiUrl}/sql-views/${name}`)
      .pipe(map(response => response.data as SqlView));
  }

  createSqlView(request: CreateSqlViewRequest): Observable<any> {
    return this.http.post<ApiResponse<any>>(`${this.apiUrl}/sql-views`, request)
      .pipe(map(response => response.data));
  }

  updateSqlView(name: string, request: UpdateSqlViewRequest): Observable<void> {
    return this.http.put<ApiResponse<void>>(`${this.apiUrl}/sql-views/${name}`, request)
      .pipe(map(() => void 0));
  }

  deleteSqlView(name: string): Observable<void> {
    return this.http.delete<ApiResponse<void>>(`${this.apiUrl}/sql-views/${name}`)
      .pipe(map(() => void 0));
  }

  previewSqlView(request: { sql: string; workspace: string; store: string; parameters?: any[] }): Observable<any> {
    return this.http.post<ApiResponse<any>>(`${this.apiUrl}/sql-views/preview`, request)
      .pipe(map(response => response.data));
  }

  // ---- 命名空间 ----

  getNamespaces(): Observable<Namespace[]> {
    return this.http.get<ApiResponse<Namespace[]>>(`${this.apiUrl}/namespaces`)
      .pipe(map(response => response.data || []));
  }

  getNamespace(prefix: string): Observable<Namespace> {
    return this.http.get<ApiResponse<Namespace>>(`${this.apiUrl}/namespaces/${prefix}`)
      .pipe(map(response => response.data as Namespace));
  }

  createNamespace(request: CreateNamespaceRequest): Observable<Namespace> {
    return this.http.post<ApiResponse<Namespace>>(`${this.apiUrl}/namespaces`, request)
      .pipe(map(response => response.data as Namespace));
  }

  updateNamespace(prefix: string, updates: UpdateNamespaceRequest): Observable<void> {
    return this.http.put<ApiResponse<void>>(`${this.apiUrl}/namespaces/${prefix}`, updates)
      .pipe(map(() => void 0));
  }

  deleteNamespace(prefix: string): Observable<void> {
    return this.http.delete<ApiResponse<void>>(`${this.apiUrl}/namespaces/${prefix}`)
      .pipe(map(() => void 0));
  }

  getServerStatus(): Observable<ServerStatus> {
    return this.http.get<ApiResponse<ServerStatus>>(`${this.apiUrl}/server/status`)
      .pipe(map(response => response.data as ServerStatus));
  }

  getDataSources(): Observable<DataSource[]> {
    return this.http.get<ApiResponse<DataSource[]>>(`${this.apiUrl}/data-sources`)
      .pipe(map(response => response.data || []));
  }

  getDataSource(name: string): Observable<DataSource> {
    return this.http.get<ApiResponse<DataSource>>(`${this.apiUrl}/data-sources/${name}`)
      .pipe(map(response => response.data as DataSource));
  }

  createDataSource(request: CreateDataSourceRequest): Observable<DataSource> {
    return this.http.post<ApiResponse<DataSource>>(`${this.apiUrl}/data-sources`, request)
      .pipe(map(response => response.data as DataSource));
  }

  updateDataSource(name: string, request: UpdateDataSourceRequest): Observable<void> {
    return this.http.put<ApiResponse<void>>(`${this.apiUrl}/data-sources/${name}`, request)
      .pipe(map(() => void 0));
  }

  deleteDataSource(name: string): Observable<void> {
    return this.http.delete<ApiResponse<void>>(`${this.apiUrl}/data-sources/${name}`)
      .pipe(map(() => void 0));
  }

  testConnection(request: CreateDataSourceRequest): Observable<ConnectionTestResult> {
    return this.http.post<ConnectionTestResult>(`${this.apiUrl}/data-sources/test`, request);
  }

  testDataSourceConnection(name: string): Observable<ConnectionTestResult> {
    return this.http.post<ConnectionTestResult>(`${this.apiUrl}/data-sources/${name}/test`, {});
  }

  getDataSourceTables(dataSourceName: string): Observable<string[]> {
    return this.http.get<ApiResponse<string[]>>(`${this.apiUrl}/data-sources/${dataSourceName}/tables`)
      .pipe(map(response => response.data || []));
  }

  getStyles(): Observable<StyleInfo[]> {
    return this.http.get<ApiResponse<StyleInfo[]>>(`${this.apiUrl}/styles`)
      .pipe(map(response => response.data || []));
  }

  getStyle(name: string): Observable<StyleInfo> {
    return this.http.get<ApiResponse<StyleInfo>>(`${this.apiUrl}/styles/${name}`)
      .pipe(map(response => response.data as StyleInfo));
  }

  createStyle(style: { name: string; title?: string; content: string }): Observable<any> {
    return this.http.post<ApiResponse<any>>(`${this.apiUrl}/styles`, style)
      .pipe(map(response => response.data));
  }

  updateStyle(name: string, updates: { title?: string; content?: string }): Observable<any> {
    return this.http.put<ApiResponse<any>>(`${this.apiUrl}/styles/${name}`, updates)
      .pipe(map(response => response.data));
  }

  deleteStyle(name: string): Observable<any> {
    return this.http.delete<ApiResponse<any>>(`${this.apiUrl}/styles/${name}`)
      .pipe(map(response => response.data));
  }

  getLayerStyle(layerName: string): Observable<string> {
    return this.http.get(`${this.apiUrl}/layers/${layerName}/style`, { responseType: 'text' });
  }

  updateLayerStyle(layerName: string, sldContent: string): Observable<any> {
    return this.http.put<ApiResponse<any>>(`${this.apiUrl}/layers/${layerName}/style`, sldContent, {
      headers: { 'Content-Type': 'application/vnd.ogc.sld+xml' }
    }).pipe(map(response => response.data));
  }

  getLayerFeatureType(layerName: string): Observable<PropertyDef[]> {
    return this.http.get<ApiResponse<PropertyDef[]>>(`${this.apiUrl}/layers/${layerName}/feature-type`)
      .pipe(map(response => response.data || []));
  }

  // ---- 文件上传 ----

  /** 上传 Shapefile (.zip) */
  uploadShapefile(file: File, name?: string): Observable<ApiResponse<UploadResult>> {
    const formData = new FormData();
    formData.append('file', file);
    const params = name ? `?name=${encodeURIComponent(name)}` : '';
    return this.http.post<ApiResponse<UploadResult>>(`${this.apiUrl}/data/upload/shapefile${params}`, formData);
  }

  /** 上传 GeoTIFF (.tif/.tiff) */
  uploadGeoTiff(file: File, name?: string): Observable<ApiResponse<UploadResult>> {
    const formData = new FormData();
    formData.append('file', file);
    const params = name ? `?name=${encodeURIComponent(name)}` : '';
    return this.http.post<ApiResponse<UploadResult>>(`${this.apiUrl}/data/upload/geotiff${params}`, formData);
  }

  /** 上传 GeoJSON (JSON body) */
  uploadGeoJson(geojson: any): Observable<ApiResponse<any>> {
    return this.http.post<ApiResponse<any>>(`${this.apiUrl}/data/upload`, geojson);
  }

  // ---- 图层组 ----

  getLayerGroups(): Observable<LayerGroup[]> {
    return this.http.get<ApiResponse<LayerGroup[]>>(`${this.apiUrl}/layer-groups`)
      .pipe(map(response => response.data || []));
  }

  getLayerGroup(name: string): Observable<LayerGroup> {
    return this.http.get<ApiResponse<LayerGroup>>(`${this.apiUrl}/layer-groups/${name}`)
      .pipe(map(response => response.data as LayerGroup));
  }

  createLayerGroup(group: { name: string; title?: string; layers: string[] }): Observable<any> {
    return this.http.post<ApiResponse<any>>(`${this.apiUrl}/layer-groups`, group)
      .pipe(map(response => response.data));
  }

  deleteLayerGroup(name: string): Observable<any> {
    return this.http.delete<ApiResponse<any>>(`${this.apiUrl}/layer-groups/${name}`)
      .pipe(map(response => response.data));
  }

  /** 导出图层要素为 GeoJSON */
  exportFeaturesGeoJson(layerName: string): Observable<Blob> {
    return this.http.get(`${this.apiUrl}/layers/${layerName}/features`, {
      params: { format: 'application/json' },
      responseType: 'blob'
    });
  }

  /** 导出图层要素为 CSV */
  exportFeaturesCsv(layerName: string): Observable<Blob> {
    return this.http.get(`${this.apiUrl}/layers/${layerName}/features`, {
      params: { format: 'text/csv' },
      responseType: 'blob'
    });
  }
}
