export interface Layer {
  name: string;
  title: string;
  workspace: string;
  store: string;
  srs: string;
  bounds: LayerBounds;
  enabled: boolean;
  abstract?: string;
}

export interface LayerBounds {
  minx: number;
  miny: number;
  maxx: number;
  maxy: number;
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

export interface CreateLayerRequest {
  name: string;
  title: string;
  workspace: string;
  store: string;
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
  enabled?: boolean;
}

export interface CreateFeatureRequest {
  geometry: GeoJsonGeometry;
  properties: Record<string, any>;
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
