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
  /** 瓦片缓存后端数据源名称 (type = "redis"); 缺省 = 默认内存/本地缓存 */
  cache_store?: string | null;
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
  format?: string;
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
  properties: Record<string, unknown>;
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
  /** 瓦片缓存后端数据源名称 (type = "redis"); 缺省 = 默认内存/本地缓存 */
  cache_store?: string | null;
}

export interface UpdateLayerRequest {
  title?: string;
  abstract?: string;
  native_name?: string;
  enabled?: boolean;
  /** 瓦片缓存后端: 数据源名称, null = 默认内存/本地缓存, 缺省 = 不修改 */
  cache_store?: string | null;
}

export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  message?: string;
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

export interface DataSource {
  name: string;
  type: 'postgis' | 'shapefile' | 'geotiff' | 'geopackage' | 'metadata' | string;
  workspace?: string;
  enabled: boolean;
  /** 是否为内置数据源 (如复用元数据存储的 metadata), 不可编辑/删除 */
  builtin?: boolean;
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
  // S3 对象存储字段 (file_storage_type = 's3' 时生效)
  s3_endpoint?: string;
  s3_region?: string;
  s3_bucket?: string;
  s3_access_key?: string;
  s3_secret_key?: string;
}

/** 目录浏览返回的条目 (本地目录 / S3 对象) */
export interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
}

/** 目录浏览响应 (本地与 S3 共用) */
export interface BrowseResponse {
  path: string;
  entries: FileEntry[];
}

/** S3 目录浏览请求体 (携带连接配置与要列出的前缀) */
export interface S3BrowseRequest {
  s3_endpoint?: string;
  s3_region?: string;
  s3_bucket?: string;
  s3_access_key?: string;
  s3_secret_key?: string;
  prefix?: string;
}

export interface CreateDataSourceRequest {
  name: string;
  type:
    | 'postgis'
    | 'shapefile'
    | 'geotiff'
    | 'geopackage'
    | 'worldimage'
    | 'cascaded_wms'
    | 'arcgrid'
    | 'geojson'
    | 'redis'
    | 'image_mosaic';
  workspace?: string;
  enabled?: boolean;
  connection?: DataSourceConnection;
}

export interface UpdateDataSourceRequest {
  type?:
    | 'postgis'
    | 'shapefile'
    | 'geotiff'
    | 'geopackage'
    | 'worldimage'
    | 'cascaded_wms'
    | 'arcgrid'
    | 'geojson'
    | 'redis'
    | 'image_mosaic';
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

/** 权限 */
export interface Permission {
  id?: number;
  username: string;
  role: string;
  resourceType: string;
  resourceName: string;
  accessMode: 'read' | 'write' | 'admin';
  effect: 'allow' | 'deny';
  priority: number;
}

export interface CreatePermissionRequest {
  username?: string;
  role?: string;
  resourceType: string;
  resourceName?: string;
  accessMode?: string;
  effect?: string;
  priority?: number;
}

/** 上传结果 */
export interface UploadResult {
  name: string;
  type: string;
  file_path?: string;
  message: string;
}

// ===== 监控 =====

export interface MonitorStats {
  uptime_seconds: number;
  total_requests: number;
  total_errors: number;
  error_rate: number;
  requests_per_second: number;
  endpoints: Record<string, EndpointStats>;
  methods: Record<string, number>;
  status_codes: Record<string, number>;
  system: SystemInfo;
}

export interface EndpointStats {
  count: number;
  error_count: number;
  avg_duration_ms: number;
  max_duration_ms: number;
}

export interface SystemInfo {
  version: string;
  rust_version: string;
  os: string;
  hostname: string;
  cpu_cores: number;
  memory_mb: number;
}

export interface RequestRecord {
  id: number;
  timestamp: string;
  method: string;
  path: string;
  status: number;
  duration_ms: number;
  user_agent: string;
  remote_addr: string;
}

export interface AuditLogEntry {
  id: number;
  timestamp: string;
  action: string;
  username: string;
  resource?: string;
  detail?: string;
}

// ===== 用户 / 存储 / 瓦片缓存 =====

/** 用户 */
export interface User {
  username: string;
  role: string;
  enabled: boolean;
  created?: string;
  modified?: string;
}

/** 瓦片缓存统计 */
export interface TileCacheStats {
  enabled: boolean;
  hits: number;
  misses: number;
  hitRate: number;
  totalTiles: number;
  cacheSizeBytes: number;
  cacheSizeMb?: string;
}

/** 清除瓦片缓存结果 */
export interface TileCacheResult {
  cleared?: number;
  message?: string;
}
