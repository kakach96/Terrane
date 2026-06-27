export interface Layer {
  name: string;
  title: string;
  workspace: string;
  store: string;
  native_name?: string;
  srs: string;
  bounds: LayerBounds;
  native_bounds?: NativeBounds;
  enabled: boolean;
  abstract?: string;
  styles?: StyleRef[];
}

export interface StyleRef {
  name: string;
  href?: string;
}

export interface StyleInfo {
  name: string;
  title: string;
  is_builtin: boolean;
  content?: string;
}

export interface LayerBounds {
  minx: number;
  miny: number;
  maxx: number;
  maxy: number;
}

export interface NativeBounds {
  crs: string;
  bounds: LayerBounds;
}

export interface Feature {
  id: string;
  type: 'Feature';
  geometry: GeoJsonGeometry;
  properties: Record<string, any>;
}

export interface GeoJsonGeometry {
  type: 'Point' | 'LineString' | 'Polygon' | 'MultiPoint' | 'MultiLineString' | 'MultiPolygon';
  coordinates: number[] | number[][] | number[][][];
}

export interface FeatureCollection {
  type: 'FeatureCollection';
  features: Feature[];
  totalFeatures?: number;
}

export interface PropertyDef {
  name: string;
  type: string;
  length?: number | null;
  nullable: boolean;
}

export interface CreateLayerRequest {
  name: string;
  title: string;
  workspace: string;
  store: string;
  native_name?: string;
  srs?: string;
  minx?: number;
  miny?: number;
  maxx?: number;
  maxy?: number;
  abstract?: string;
}

export interface UpdateLayerRequest {
  title?: string;
  abstract?: string;
  native_name?: string;
  enabled?: boolean;
}

export interface CreateFeatureRequest {
  geometry: GeoJsonGeometry;
  properties: Record<string, any>;
}

export interface UpdateFeatureRequest {
  geometry?: GeoJsonGeometry;
  properties?: Record<string, any>;
}

export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  message?: string;
}

export interface DashboardStats {
  layerCount: number;
  featureCount: number;
  activeLayerCount: number;
  workspaceCount: number;
}

export interface PreviewOptions {
  width?: number;
  height?: number;
  format?: 'png' | 'jpeg' | 'gif';
}

export interface Workspace {
  name: string;
  title?: string;
  enabled: boolean;
  layerCount: number;
  description?: string;
  created?: string;
  modified?: string;
}

export interface CreateWorkspaceRequest {
  name: string;
  title?: string;
  description?: string;
}

export interface UpdateWorkspaceRequest {
  title?: string;
  description?: string;
  enabled?: boolean;
}

export interface ServerStatus {
  uptime: string;
  memory: {
    used: number;
    total: number;
    percent: number;
  };
  cpu: number;
  requests: number;
  errors: number;
  layerCount: number;
  enabledLayers: number;
  workspaceCount: number;
}

export interface DataSource {
  name: string;
  type: 'postgis' | 'shapefile' | 'geotiff' | 'geopackage';
  workspace?: string;
  enabled: boolean;
  connection?: DataSourceConnection;
  created?: string;
  modified?: string;
}

export interface DataSourceConnection {
  // PostGIS 字段
  host?: string;
  port?: number;
  database?: string;
  schema?: string;
  username?: string;
  password?: string;
  // 文件型字段
  file_path?: string;
  file_storage_type?: string;
}

export interface CreateDataSourceRequest {
  name: string;
  type: 'postgis' | 'shapefile' | 'geotiff' | 'geopackage';
  workspace?: string;
  enabled?: boolean;
  connection?: DataSourceConnection;
}

export interface UpdateDataSourceRequest {
  type?: 'postgis' | 'shapefile' | 'geotiff' | 'geopackage' | 'worldimage' | 'cascaded_wms' | 'arcgrid';
  workspace?: string;
  enabled?: boolean;
  connection?: DataSourceConnection;
}

export interface ConnectionTestResult {
  success: boolean;
  message?: string;
}

/** 图层组 */
export interface LayerGroup {
  name: string;
  title: string;
  layers: string[];
  styles?: (string | null)[];
}

/** 命名空间 */
export interface Namespace {
  prefix: string;
  uri: string;
  isolated: boolean;
  workspace?: string;
  created?: string;
  modified?: string;
}

export interface CreateNamespaceRequest {
  prefix: string;
  uri: string;
  workspace?: string;
  isolated?: boolean;
}

export interface UpdateNamespaceRequest {
  uri?: string;
  isolated?: boolean;
  workspace?: string;
}

/** SQL 视图 */
export interface SqlView {
  name: string;
  sql: string;
  workspace: string;
  store: string;
  geometryColumn: string;
  geometryType: string;
  crs: string;
  parameters: SqlViewParameter[];
  description?: string;
  created?: string;
  modified?: string;
}

export interface SqlViewParameter {
  name: string;
  defaultValue: string;
  regexValidator?: string;
}

export interface CreateSqlViewRequest {
  name: string;
  sql: string;
  workspace: string;
  store: string;
  geometryColumn?: string;
  geometryType?: string;
  crs?: string;
  parameters?: SqlViewParameter[];
  description?: string;
}

export interface UpdateSqlViewRequest {
  sql?: string;
  geometryColumn?: string;
  geometryType?: string;
  crs?: string;
  parameters?: SqlViewParameter[];
  description?: string;
}

/** 上传结果 */
export interface UploadResult {
  name: string;
  type: string;
  file_path?: string;
  message: string;
}
