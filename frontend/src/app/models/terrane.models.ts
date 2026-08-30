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
  /** Tile cache backend data source name (type = "redis"); default = in-memory/local cache */
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
  /** Tile cache backend data source name (type = "redis"); default = in-memory/local cache */
  cache_store?: string | null;
}

export interface UpdateLayerRequest {
  title?: string;
  abstract?: string;
  native_name?: string;
  enabled?: boolean;
  /** Tile cache backend: data source name, null = default in-memory/local cache, omitted = no change */
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
  /** Whether this is a built-in data source (e.g. metadata reusing the metadata store); not editable/deletable */
  builtin?: boolean;
  connection?: DataSourceConnection;
  created?: string;
  modified?: string;
}

export interface DataSourceConnection {
  // PostGIS fields
  /** Single host, comma-separated cluster list ("pg1,pg2" / "pg1:5433,pg2") or a full connection URL */
  host?: string;
  port?: number;
  database?: string;
  schema?: string;
  username?: string;
  password?: string;
  /** MongoDB replica-set name (appended as ?replicaSet=… for cluster connections) */
  replica_set?: string;
  // File-based fields
  file_path?: string;
  file_storage_type?: string;
  // S3 object storage fields (active when file_storage_type = 's3')
  s3_endpoint?: string;
  s3_region?: string;
  s3_bucket?: string;
  s3_access_key?: string;
  s3_secret_key?: string;
}

/** Directory browse entry (local directory / S3 object) */
export interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
}

/** Directory browse response (shared by local and S3) */
export interface BrowseResponse {
  path: string;
  entries: FileEntry[];
}

/** S3 directory browse request (connection config + prefix to list) */
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
    | 'mysql'
    | 'mongo'
    | 'shapefile'
    | 'geotiff'
    | 'geopackage'
    | 'worldimage'
    | 'cascaded_wms'
    | 'arcgrid'
    | 'geojson'
    | 'redis'
    | 'image_mosaic'
    | 'image_pyramid';
  workspace?: string;
  enabled?: boolean;
  connection?: DataSourceConnection;
}

export interface UpdateDataSourceRequest {
  type?:
    | 'postgis'
    | 'mysql'
    | 'mongo'
    | 'shapefile'
    | 'geotiff'
    | 'geopackage'
    | 'worldimage'
    | 'cascaded_wms'
    | 'arcgrid'
    | 'geojson'
    | 'redis'
    | 'image_mosaic'
    | 'image_pyramid';
  workspace?: string;
  enabled?: boolean;
  connection?: DataSourceConnection;
}

export interface ConnectionTestResult {
  success: boolean;
  message?: string;
}

/** Layer group */
export interface LayerGroup {
  name: string;
  title: string;
  layers: string[];
  styles?: (string | null)[];
}

/** SQL view */
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

/** Permission */
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

/** Upload result */
export interface UploadResult {
  name: string;
  type: string;
  file_path?: string;
  message: string;
}

// ===== Monitoring =====

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

// ===== Users / storage / tile cache =====

/** User */
export interface User {
  username: string;
  role: string;
  enabled: boolean;
  created?: string;
  modified?: string;
}

/** Tile cache stats */
export interface TileCacheStats {
  enabled: boolean;
  hits: number;
  misses: number;
  hitRate: number;
  totalTiles: number;
  cacheSizeBytes: number;
  cacheSizeMb?: string;
}

/** Clear tile cache result */
export interface TileCacheResult {
  cleared?: number;
  message?: string;
}

/** Tile seed job status (GWC style) */
export type SeedStatus = 'Pending' | 'Running' | 'Completed' | 'Failed' | 'Cancelled';

/** Tile seed job (GWC-style seed / truncate) */
export interface SeedJob {
  id: string;
  layer: string;
  gridset: string;
  z_min: number;
  z_max: number;
  format: string;
  status: SeedStatus;
  total: number;
  done: number;
  error?: string | null;
  created_at: string;
  updated_at: string;
}

/** Create seed job request (POST /tiles/seed) */
export interface SeedRequest {
  layer: string;
  gridset?: string;
  z_min: number;
  z_max: number;
  format?: string;
}

/** Truncate tile cache request (POST /tiles/seed/truncate) */
export interface TruncateRequest {
  layer: string;
  gridset?: string;
}

/** Seed job creation result */
export interface SeedJobResult {
  job: SeedJob;
  message: string;
}

/** Truncate tile cache result */
export interface TruncateResult {
  layer: string;
  gridset?: string;
  removed: number;
  message: string;
}
