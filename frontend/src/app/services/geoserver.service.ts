import { Injectable } from '@angular/core';
import { HttpClient, HttpParams } from '@angular/common/http';
import { Observable } from 'rxjs';
import { map } from 'rxjs/operators';
import {
  Layer,
  Feature,
  FeatureCollection,
  CreateLayerRequest,
  UpdateLayerRequest,
  CreateFeatureRequest,
  PropertyDef,
  ApiResponse,
  DashboardStats,
  PreviewOptions,
  Workspace,
  CreateWorkspaceRequest,
  UpdateWorkspaceRequest,
  ServerStatus,
  DataSource,
  CreateDataSourceRequest,
  UpdateDataSourceRequest,
  ConnectionTestResult
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

  getLayerFeatureType(layerName: string): Observable<PropertyDef[]> {
    return this.http.get<ApiResponse<PropertyDef[]>>(`${this.apiUrl}/layers/${layerName}/feature-type`)
      .pipe(map(response => response.data || []));
  }
}
