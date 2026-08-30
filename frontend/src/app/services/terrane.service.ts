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
  FileEntry,
  S3BrowseRequest,
  UploadResult,
  LayerGroup,
  MonitorStats,
  RequestRecord,
  AuditLogEntry,
  SqlViewParameter,
  TileCacheStats,
  TileCacheResult,
  SeedJob,
  SeedRequest,
  SeedJobResult,
  TruncateResult,
  User,
} from '../models/terrane.models';
import { transformBounds } from '../utils/coords';

@Injectable({
  providedIn: 'root',
})
export class TerraneService {
  private readonly apiUrl = '/terrane';

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
    // WMS convention: BBOX is in the request SRS; transform bbox when the target SRS differs from the layer's native SRS
    const bbox = transformBounds(bounds, nativeCrs, srs);
    // Use WMS 1.1.1 to avoid EPSG:4326 axis-order issues
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
      format?: string;
      transparent?: boolean;
      styles?: string;
    },
  ): string {
    const nativeCrs = layer.native_bounds?.crs || layer.srs || 'EPSG:4326';
    const crs = options?.crs || nativeCrs;

    // Prefer an explicitly passed bbox; otherwise read from the layer's native
    // bounds and convert when the request SRS differs from the native SRS
    // (WMS convention: BBOX is in the request SRS)
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

  /**
   * Build a Mapbox Vector Tile (MVT) URL for a layer at a given tile coordinate.
   * Uses the shared tile endpoint (`/tiles/{layer}/{z}/{x}/{y}.pbf`), which is
   * registered before the generic PNG tile route so the `.pbf` suffix is not
   * shadowed.
   */
  getMvtTileUrl(layer: Layer, z: number, x: number, y: number): string {
    const layerName = layer.name.includes(':') ? layer.name : `${layer.workspace}:${layer.name}`;
    return `${this.apiUrl}/tiles/${layerName}/${z}/${x}/${y}.pbf`;
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

  // ---- Tile cache (GeoWebCache) ----

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

  // ---- Tile seed jobs (GWC-style seed / truncate) ----

  /** Create and start a seed job (seed / reseed via POST /tiles/seed) */
  createSeedJob(request: SeedRequest): Observable<SeedJobResult> {
    return this.http
      .post<ApiResponse<SeedJobResult>>(`${this.apiUrl}/tiles/seed`, request)
      .pipe(map((response) => response.data as SeedJobResult));
  }

  /** List all seed jobs */
  getSeedJobs(): Observable<SeedJob[]> {
    return this.http
      .get<ApiResponse<SeedJob[]>>(`${this.apiUrl}/tiles/seed`)
      .pipe(map((response) => response.data || []));
  }

  /** Cancel a running seed job (cooperative) */
  cancelSeedJob(id: string): Observable<{ job: string; message: string }> {
    return this.http
      .delete<ApiResponse<{ job: string; message: string }>>(`${this.apiUrl}/tiles/seed/${id}`)
      .pipe(map((response) => response.data as { job: string; message: string }));
  }

  /** Truncate (clear) a layer's cached tiles, optionally per gridset */
  truncateTileCache(layer: string, gridset?: string): Observable<TruncateResult> {
    return this.http
      .post<ApiResponse<TruncateResult>>(`${this.apiUrl}/tiles/seed/truncate`, {
        layer,
        gridset,
      })
      .pipe(map((response) => response.data as TruncateResult));
  }

  // ---- SQL views ----

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

  // ---- Permissions ----

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

  /** Browse the server's local directory (directories + files) */
  browseLocalDirectory(path?: string): Observable<FileEntry[]> {
    const params = new HttpParams().set('path', path || '');
    return this.http
      .get<ApiResponse<{ path: string; entries: FileEntry[] }>>(
        `${this.apiUrl}/data-sources/browse`,
        { params },
      )
      .pipe(map((response) => response.data?.entries || []));
  }

  /** Browse an S3 bucket directory (carrying connection config) */
  browseS3Directory(request: S3BrowseRequest): Observable<FileEntry[]> {
    return this.http
      .post<ApiResponse<{ path: string; entries: FileEntry[] }>>(
        `${this.apiUrl}/data-sources/s3/browse`,
        request,
      )
      .pipe(map((response) => response.data?.entries || []));
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

  // ---- File uploads ----

  /** Upload a Shapefile (.zip) */
  uploadShapefile(file: File, name?: string): Observable<ApiResponse<UploadResult>> {
    const formData = new FormData();
    formData.append('file', file);
    const params = name ? `?name=${encodeURIComponent(name)}` : '';
    return this.http.post<ApiResponse<UploadResult>>(
      `${this.apiUrl}/data/upload/shapefile${params}`,
      formData,
    );
  }

  /** Upload a GeoTIFF (.tif/.tiff) */
  uploadGeoTiff(file: File, name?: string): Observable<ApiResponse<UploadResult>> {
    const formData = new FormData();
    formData.append('file', file);
    const params = name ? `?name=${encodeURIComponent(name)}` : '';
    return this.http.post<ApiResponse<UploadResult>>(
      `${this.apiUrl}/data/upload/geotiff${params}`,
      formData,
    );
  }

  /** Upload GeoJSON (JSON body) */
  uploadGeoJson(geojson: unknown): Observable<ApiResponse<UploadResult>> {
    return this.http.post<ApiResponse<UploadResult>>(`${this.apiUrl}/data/upload`, geojson);
  }

  // ---- Layer groups ----

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

  /** Export layer features as GeoJSON */
  exportFeaturesGeoJson(layerName: string): Observable<Blob> {
    return this.http.get(`${this.apiUrl}/layers/${layerName}/features`, {
      params: { format: 'application/json' },
      responseType: 'blob',
    });
  }

  /** Export layer features as CSV */
  exportFeaturesCsv(layerName: string): Observable<Blob> {
    return this.http.get(`${this.apiUrl}/layers/${layerName}/features`, {
      params: { format: 'text/csv' },
      responseType: 'blob',
    });
  }

  // ===== Monitoring =====

  /** Get monitoring stats */
  getMonitorStats(): Observable<MonitorStats> {
    return this.http.get<MonitorStats>(`${this.apiUrl}/monitor/stats`);
  }

  /** Get recent request records */
  getRecentRequests(limit: number = 100): Observable<RequestRecord[]> {
    return this.http.get<RequestRecord[]>(`${this.apiUrl}/monitor/requests`, {
      params: { limit: limit.toString() },
    });
  }

  /** Get audit logs */
  getAuditLogs(limit: number = 100, offset: number = 0): Observable<AuditLogEntry[]> {
    return this.http.get<AuditLogEntry[]>(`${this.apiUrl}/monitor/logs`, {
      params: { limit: limit.toString(), offset: offset.toString() },
    });
  }

  /** Reset monitoring stats */
  resetMonitorStats(): Observable<void> {
    return this.http.delete<void>(`${this.apiUrl}/monitor/reset`);
  }

  // ===== User management =====

  /** List all users */
  listUsers(): Observable<User[]> {
    return this.http
      .get<ApiResponse<User[]>>(`${this.apiUrl}/auth/users`)
      .pipe(map((r) => r.data || []));
  }

  /** Create a user */
  createUser(username: string, password: string, role: string): Observable<void> {
    return this.http
      .post<ApiResponse<void>>(`${this.apiUrl}/auth/users`, { username, password, role })
      .pipe(map(() => void 0));
  }

  /** Delete a user */
  deleteUser(username: string): Observable<void> {
    return this.http
      .delete<ApiResponse<void>>(`${this.apiUrl}/auth/users/${username}`)
      .pipe(map(() => void 0));
  }

  /** Change the current user's password */
  changePassword(oldPassword: string, newPassword: string): Observable<void> {
    return this.http
      .post<ApiResponse<void>>(`${this.apiUrl}/auth/change-password`, {
        old_password: oldPassword,
        new_password: newPassword,
      })
      .pipe(map(() => void 0));
  }
}
