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
  PropertyDef,
  StyleInfo,
  ApiResponse,
  PreviewOptions,
  Workspace,
  CreateWorkspaceRequest,
  UpdateWorkspaceRequest,
  SqlView,
  CreateSqlViewRequest,
  UpdateSqlViewRequest,
  Permission,
  CreatePermissionRequest,
  DataSource,
  CreateDataSourceRequest,
  UpdateDataSourceRequest,
  ConnectionTestResult,
  UploadResult,
  LayerGroup,
  MonitorStats,
  RequestRecord,
  AuditLogEntry,
  SqlViewParameter,
  TileCacheStats,
  TileCacheResult,
  User,
} from '../models/geoserver.models';
import { transformBounds } from '../utils/coords';

@Injectable({
  providedIn: 'root',
})
export class GeoserverService {
  private readonly apiUrl = '/geoserver';

  constructor(private http: HttpClient) {}

  getLayers(): Observable<Layer[]> {
    return this.http
      .get<ApiResponse<Layer[]>>(`${this.apiUrl}/layers`)
      .pipe(map((response) => response.data || []));
  }

  getLayer(name: string): Observable<Layer> {
    return this.http
      .get<ApiResponse<Layer>>(`${this.apiUrl}/layers/${name}`)
      .pipe(map((response) => response.data as Layer));
  }

  createLayer(layer: CreateLayerRequest): Observable<Layer> {
    return this.http
      .post<ApiResponse<Layer>>(`${this.apiUrl}/layers`, layer)
      .pipe(map((response) => response.data as Layer));
  }

  updateLayer(name: string, updates: UpdateLayerRequest): Observable<void> {
    return this.http
      .put<ApiResponse<void>>(`${this.apiUrl}/layers/${name}`, updates)
      .pipe(map(() => void 0));
  }

  deleteLayer(name: string): Observable<void> {
    return this.http
      .delete<ApiResponse<void>>(`${this.apiUrl}/layers/${name}`)
      .pipe(map(() => void 0));
  }

  getLayerFeatures(layerName: string): Observable<FeatureCollection> {
    return this.http.get<FeatureCollection>(`${this.apiUrl}/layers/${layerName}/features`);
  }

  getFeature(layerName: string, featureId: string): Observable<Feature> {
    return this.http.get<Feature>(`${this.apiUrl}/layers/${layerName}/features/${featureId}`);
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

  getWmsPreviewUrl(
    layer: Layer,
    options?: {
      width?: number;
      height?: number;
      crs?: string;
      format?: string;
      transparent?: boolean;
    },
  ): string {
    const bounds: LayerBounds = layer.native_bounds?.bounds || layer.bounds;
    if (!bounds) return '';
    const nativeCrs = layer.native_bounds?.crs || layer.srs || 'EPSG:4326';
    const srs = options?.crs || nativeCrs;
    // WMS 约定 BBOX 处于请求 SRS 下: 目标 SRS 与图层原生 SRS 不一致时转换 bbox
    const bbox = transformBounds(bounds, nativeCrs, srs);
    // 使用 WMS 1.1.1 避免 EPSG:4326 轴序问题
    const params = new URLSearchParams({
      service: 'WMS',
      version: '1.1.1',
      request: 'GetMap',
      layers: layer.name,
      srs: srs,
      bbox: `${bbox.minx},${bbox.miny},${bbox.maxx},${bbox.maxy}`,
      width: (options?.width || 600).toString(),
      height: (options?.height || 400).toString(),
      format: options?.format || 'application/openlayers',
      transparent: (options?.transparent ?? true).toString(),
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
    },
  ): string {
    const nativeCrs = layer.native_bounds?.crs || layer.srs || 'EPSG:4326';
    const crs = options?.crs || nativeCrs;

    // 优先使用显式传入的 bbox; 否则从图层原生边界读取, 并在请求 SRS 与
    // 图层原生 SRS 不一致时转换 (WMS 约定 BBOX 处于请求 SRS 下)
    let bbox = options?.bbox;
    if (!bbox) {
      const bounds: LayerBounds = layer.native_bounds?.bounds || layer.bounds;
      if (!bounds) return '';
      const converted = transformBounds(bounds, nativeCrs, crs);
      bbox = `${converted.minx},${converted.miny},${converted.maxx},${converted.maxy}`;
    }

    const layerName = layer.name.includes(':') ? layer.name : `${layer.workspace}:${layer.name}`;

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

  getWorkspaces(): Observable<string[]> {
    return this.getLayers().pipe(map((layers) => [...new Set(layers.map((l) => l.workspace))]));
  }

  getAllWorkspaces(): Observable<Workspace[]> {
    return this.http
      .get<ApiResponse<Workspace[]>>(`${this.apiUrl}/workspaces`)
      .pipe(map((response) => response.data || []));
  }

  getWorkspace(name: string): Observable<Workspace> {
    return this.http
      .get<ApiResponse<Workspace>>(`${this.apiUrl}/workspaces/${name}`)
      .pipe(map((response) => response.data as Workspace));
  }

  createWorkspace(request: CreateWorkspaceRequest): Observable<Workspace> {
    return this.http
      .post<ApiResponse<Workspace>>(`${this.apiUrl}/workspaces`, request)
      .pipe(map((response) => response.data as Workspace));
  }

  updateWorkspace(name: string, updates: UpdateWorkspaceRequest): Observable<void> {
    return this.http
      .put<ApiResponse<void>>(`${this.apiUrl}/workspaces/${name}`, updates)
      .pipe(map(() => void 0));
  }

  deleteWorkspace(name: string): Observable<void> {
    return this.http
      .delete<ApiResponse<void>>(`${this.apiUrl}/workspaces/${name}`)
      .pipe(map(() => void 0));
  }

  // ---- 瓦片缓存 (GeoWebCache) ----

  getTileCacheStats(): Observable<TileCacheStats> {
    return this.http
      .get<ApiResponse<TileCacheStats>>(`${this.apiUrl}/tiles/cache/stats`)
      .pipe(map((response) => response.data as TileCacheStats));
  }

  clearTileCache(layerName: string): Observable<TileCacheResult> {
    return this.http
      .delete<ApiResponse<TileCacheResult>>(`${this.apiUrl}/tiles/cache/clear/${layerName}`)
      .pipe(map((response) => response.data as TileCacheResult));
  }

  // ---- SQL 视图 ----

  getSqlViews(): Observable<SqlView[]> {
    return this.http
      .get<ApiResponse<SqlView[]>>(`${this.apiUrl}/sql-views`)
      .pipe(map((response) => response.data || []));
  }

  getSqlView(name: string): Observable<SqlView> {
    return this.http
      .get<ApiResponse<SqlView>>(`${this.apiUrl}/sql-views/${name}`)
      .pipe(map((response) => response.data as SqlView));
  }

  createSqlView(request: CreateSqlViewRequest): Observable<SqlView> {
    return this.http
      .post<ApiResponse<SqlView>>(`${this.apiUrl}/sql-views`, request)
      .pipe(map((response) => response.data as SqlView));
  }

  updateSqlView(name: string, request: UpdateSqlViewRequest): Observable<void> {
    return this.http
      .put<ApiResponse<void>>(`${this.apiUrl}/sql-views/${name}`, request)
      .pipe(map(() => void 0));
  }

  deleteSqlView(name: string): Observable<void> {
    return this.http
      .delete<ApiResponse<void>>(`${this.apiUrl}/sql-views/${name}`)
      .pipe(map(() => void 0));
  }

  previewSqlView(request: {
    sql: string;
    workspace: string;
    store: string;
    parameters?: SqlViewParameter[];
  }): Observable<Record<string, unknown>[]> {
    return this.http
      .post<ApiResponse<Record<string, unknown>[]>>(`${this.apiUrl}/sql-views/preview`, request)
      .pipe(map((response) => response.data || []));
  }

  // ---- 权限 ----

  getPermissions(): Observable<Permission[]> {
    return this.http
      .get<ApiResponse<Permission[]>>(`${this.apiUrl}/permissions`)
      .pipe(map((response) => response.data || []));
  }

  createPermission(request: CreatePermissionRequest): Observable<void> {
    return this.http
      .post<ApiResponse<void>>(`${this.apiUrl}/permissions`, request)
      .pipe(map(() => void 0));
  }

  deletePermission(id: number): Observable<void> {
    return this.http
      .delete<ApiResponse<void>>(`${this.apiUrl}/permissions/${id}`)
      .pipe(map(() => void 0));
  }

  checkPermission(type: string, name: string, mode?: string): Observable<Record<string, unknown>> {
    const modeParam = mode ? `?mode=${mode}` : '';
    return this.http
      .get<ApiResponse<Record<string, unknown>>>(
        `${this.apiUrl}/permissions/check/${type}/${name}${modeParam}`,
      )
      .pipe(map((response) => response.data as Record<string, unknown>));
  }

  getDataSources(): Observable<DataSource[]> {
    return this.http
      .get<ApiResponse<DataSource[]>>(`${this.apiUrl}/data-sources`)
      .pipe(map((response) => response.data || []));
  }

  getDataSource(name: string): Observable<DataSource> {
    return this.http
      .get<ApiResponse<DataSource>>(`${this.apiUrl}/data-sources/${name}`)
      .pipe(map((response) => response.data as DataSource));
  }

  createDataSource(request: CreateDataSourceRequest): Observable<DataSource> {
    return this.http
      .post<ApiResponse<DataSource>>(`${this.apiUrl}/data-sources`, request)
      .pipe(map((response) => response.data as DataSource));
  }

  updateDataSource(name: string, request: UpdateDataSourceRequest): Observable<void> {
    return this.http
      .put<ApiResponse<void>>(`${this.apiUrl}/data-sources/${name}`, request)
      .pipe(map(() => void 0));
  }

  deleteDataSource(name: string): Observable<void> {
    return this.http
      .delete<ApiResponse<void>>(`${this.apiUrl}/data-sources/${name}`)
      .pipe(map(() => void 0));
  }

  testConnection(request: CreateDataSourceRequest): Observable<ConnectionTestResult> {
    return this.http.post<ConnectionTestResult>(`${this.apiUrl}/data-sources/test`, request);
  }

  testDataSourceConnection(name: string): Observable<ConnectionTestResult> {
    return this.http.post<ConnectionTestResult>(`${this.apiUrl}/data-sources/${name}/test`, {});
  }

  getDataSourceTables(dataSourceName: string): Observable<string[]> {
    return this.http
      .get<ApiResponse<string[]>>(`${this.apiUrl}/data-sources/${dataSourceName}/tables`)
      .pipe(map((response) => response.data || []));
  }

  getStyles(): Observable<StyleInfo[]> {
    return this.http
      .get<ApiResponse<StyleInfo[]>>(`${this.apiUrl}/styles`)
      .pipe(map((response) => response.data || []));
  }

  getStyle(name: string): Observable<StyleInfo> {
    return this.http
      .get<ApiResponse<StyleInfo>>(`${this.apiUrl}/styles/${name}`)
      .pipe(map((response) => response.data as StyleInfo));
  }

  createStyle(style: {
    name: string;
    title?: string;
    content: string;
    format?: string;
  }): Observable<void> {
    return this.http
      .post<ApiResponse<void>>(`${this.apiUrl}/styles`, style)
      .pipe(map(() => void 0));
  }

  updateStyle(
    name: string,
    updates: { title?: string; content?: string; format?: string },
  ): Observable<void> {
    return this.http
      .put<ApiResponse<void>>(`${this.apiUrl}/styles/${name}`, updates)
      .pipe(map(() => void 0));
  }

  deleteStyle(name: string): Observable<void> {
    return this.http
      .delete<ApiResponse<void>>(`${this.apiUrl}/styles/${name}`)
      .pipe(map(() => void 0));
  }

  getLayerStyle(layerName: string): Observable<string> {
    return this.http.get(`${this.apiUrl}/layers/${layerName}/style`, { responseType: 'text' });
  }

  updateLayerStyle(layerName: string, sldContent: string): Observable<void> {
    return this.http
      .put<ApiResponse<void>>(`${this.apiUrl}/layers/${layerName}/style`, sldContent, {
        headers: { 'Content-Type': 'application/vnd.ogc.sld+xml' },
      })
      .pipe(map(() => void 0));
  }

  getLayerFeatureType(layerName: string): Observable<PropertyDef[]> {
    return this.http
      .get<ApiResponse<PropertyDef[]>>(`${this.apiUrl}/layers/${layerName}/feature-type`)
      .pipe(map((response) => response.data || []));
  }

  // ---- 文件上传 ----

  /** 上传 Shapefile (.zip) */
  uploadShapefile(file: File, name?: string): Observable<ApiResponse<UploadResult>> {
    const formData = new FormData();
    formData.append('file', file);
    const params = name ? `?name=${encodeURIComponent(name)}` : '';
    return this.http.post<ApiResponse<UploadResult>>(
      `${this.apiUrl}/data/upload/shapefile${params}`,
      formData,
    );
  }

  /** 上传 GeoTIFF (.tif/.tiff) */
  uploadGeoTiff(file: File, name?: string): Observable<ApiResponse<UploadResult>> {
    const formData = new FormData();
    formData.append('file', file);
    const params = name ? `?name=${encodeURIComponent(name)}` : '';
    return this.http.post<ApiResponse<UploadResult>>(
      `${this.apiUrl}/data/upload/geotiff${params}`,
      formData,
    );
  }

  /** 上传 GeoJSON (JSON body) */
  uploadGeoJson(geojson: unknown): Observable<ApiResponse<UploadResult>> {
    return this.http.post<ApiResponse<UploadResult>>(`${this.apiUrl}/data/upload`, geojson);
  }

  // ---- 图层组 ----

  getLayerGroups(): Observable<LayerGroup[]> {
    return this.http
      .get<ApiResponse<LayerGroup[]>>(`${this.apiUrl}/layer-groups`)
      .pipe(map((response) => response.data || []));
  }

  getLayerGroup(name: string): Observable<LayerGroup> {
    return this.http
      .get<ApiResponse<LayerGroup>>(`${this.apiUrl}/layer-groups/${name}`)
      .pipe(map((response) => response.data as LayerGroup));
  }

  createLayerGroup(group: { name: string; title?: string; layers: string[] }): Observable<void> {
    return this.http
      .post<ApiResponse<void>>(`${this.apiUrl}/layer-groups`, group)
      .pipe(map(() => void 0));
  }

  deleteLayerGroup(name: string): Observable<void> {
    return this.http
      .delete<ApiResponse<void>>(`${this.apiUrl}/layer-groups/${name}`)
      .pipe(map(() => void 0));
  }

  /** 导出图层要素为 GeoJSON */
  exportFeaturesGeoJson(layerName: string): Observable<Blob> {
    return this.http.get(`${this.apiUrl}/layers/${layerName}/features`, {
      params: { format: 'application/json' },
      responseType: 'blob',
    });
  }

  /** 导出图层要素为 CSV */
  exportFeaturesCsv(layerName: string): Observable<Blob> {
    return this.http.get(`${this.apiUrl}/layers/${layerName}/features`, {
      params: { format: 'text/csv' },
      responseType: 'blob',
    });
  }

  // ===== 监控 =====

  /** 获取监控统计 */
  getMonitorStats(): Observable<MonitorStats> {
    return this.http.get<MonitorStats>(`${this.apiUrl}/monitor/stats`);
  }

  /** 获取最近请求记录 */
  getRecentRequests(limit: number = 100): Observable<RequestRecord[]> {
    return this.http.get<RequestRecord[]>(`${this.apiUrl}/monitor/requests`, {
      params: { limit: limit.toString() },
    });
  }

  /** 获取审计日志 */
  getAuditLogs(limit: number = 100, offset: number = 0): Observable<AuditLogEntry[]> {
    return this.http.get<AuditLogEntry[]>(`${this.apiUrl}/monitor/logs`, {
      params: { limit: limit.toString(), offset: offset.toString() },
    });
  }

  /** 重置监控统计 */
  resetMonitorStats(): Observable<void> {
    return this.http.delete<void>(`${this.apiUrl}/monitor/reset`);
  }

  // ===== 用户管理 =====

  /** 列出所有用户 */
  listUsers(): Observable<User[]> {
    return this.http
      .get<ApiResponse<User[]>>(`${this.apiUrl}/auth/users`)
      .pipe(map((r) => r.data || []));
  }

  /** 创建用户 */
  createUser(username: string, password: string, role: string): Observable<void> {
    return this.http
      .post<ApiResponse<void>>(`${this.apiUrl}/auth/users`, { username, password, role })
      .pipe(map(() => void 0));
  }

  /** 删除用户 */
  deleteUser(username: string): Observable<void> {
    return this.http
      .delete<ApiResponse<void>>(`${this.apiUrl}/auth/users/${username}`)
      .pipe(map(() => void 0));
  }

  /** 修改密码 */
  changePassword(oldPassword: string, newPassword: string): Observable<void> {
    return this.http
      .post<ApiResponse<void>>(`${this.apiUrl}/auth/change-password`, {
        old_password: oldPassword,
        new_password: newPassword,
      })
      .pipe(map(() => void 0));
  }
}
