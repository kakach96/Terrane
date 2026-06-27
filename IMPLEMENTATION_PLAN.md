# Rust GeoServer — 功能差距分析与实现计划

> 基于 GeoServer 官方文档 (https://docs.geoserver.org/latest/en/user/) 进行对比分析

---

## 📊 实现总览

| 功能领域 | 已实现 | 部分实现 | 未实现 | 总进度 |
|---------|:-----:|:--------:|:-----:|:-----:|
| OGC 核心服务 | 5/7 | 0 | 2 | **71%** |
| REST API | 11/16 | 0 | 5 | **69%** |
| 数据源类型 | 7/15 | 0 | 8 | **47%** |
| 样式系统 | 1/5 | 1 | 3 | **30%** |
| 瓦片缓存 | 2/6 | 0 | 4 | **33%** |
| 安全性 | 3/7 | 0 | 4 | **43%** |
| 扩展功能 | 1/14 | 0 | 13 | **7%** |
| **总进度** | | | | **~50%** |

---

## 一、✅ 已实现功能

### 1.1 OGC 标准服务

| 服务 | 操作 | 状态 |
|------|------|:----:|
| **WMS 1.1.1/1.3.0** | GetCapabilities | ✅ |
| | GetMap | ✅ |
| | GetFeatureInfo | ✅ |
| | DescribeLayer | ✅ |
| | GetLegendGraphic | ✅ |
| | GetStyles / PutStyles | ✅ |
| **WFS 1.0/1.1/2.0** | GetCapabilities | ✅ |
| | DescribeFeatureType | ✅ |
| | GetFeature | ✅ |
| | GetFeatureWithLock | ✅ |
| | Transaction (Insert/Update/Delete) | ✅ |
| | LockFeature（定义） | ✅ |
| **WCS 1.0/1.1/2.0** | GetCapabilities | ✅ |
| | DescribeCoverage | ✅ |
| | GetCoverage | ✅ |

### 1.2 REST API

| 端点 | 功能 | 状态 |
|------|------|:----:|
| `/layers` | CRUD | ✅ |
| `/layers/{name}/preview` | 预览 | ✅ |
| `/layers/{name}/features` | 要素 CRUD | ✅ |
| `/layers/{name}/feature-type` | 属性架构 | ✅ |
| `/layers/{name}/style` | 图层样式绑定 | ✅ |
| `/workspaces` | CRUD | ✅ |
| `/data-sources` | CRUD + 连接测试 | ✅ |
| `/styles` | CRUD + SLD | ✅ |
| `/layer-groups` | CRUD | ✅ |
| `/data/upload` | GeoJSON 上传 | ✅ |
| `/data/upload/shapefile` | Shapefile 上传 | ✅ |
| `/data/upload/geotiff` | GeoTIFF 上传 | ✅ |
| `/server/status` | 服务器状态 | ✅ |
| `/health` | 健康检查 | ✅ |
| `/tiles/{layer}/{z}/{x}/{y}` | 瓦片服务 | ✅ |

### 1.3 数据源类型

| 类型 | 说明 | 状态 |
|------|------|:----:|
| **PostGIS** | PostgreSQL/PostGIS 数据库 | ✅ |
| **Shapefile** | ESRI Shapefile 矢量格式 | ✅ |
| **GeoTIFF** | GeoTIFF 栅格格式 | ✅ |
| **GeoPackage** | OGC GeoPackage 矢量格式 (WKB) | ✅ **P2** |
| **WorldImage** | 影像+世界文件 (.pgw/.jgw/.tfw) | ✅ **P2** |
| **CascadedWms** | 级联外部 WMS 服务 | ✅ **P2** |
| **ArcGrid** | ESRI ASCII Grid 栅格格式 | ✅ **P2** |

### 1.4 扩展 REST API

| 端点 | 功能 | 状态 |
|------|------|:----:|
| `/namespaces` | 命名空间 CRUD | ✅ **新** |
| `/stores` | 存储管理 (DataStore/CoverageStore) | ✅ **新** |
| `/workspaces/{ws}/stores` | 按工作空间列存储 | ✅ **新** |
| `/sql-views` | SQL 视图 CRUD | ✅ **新** |
| `/sql-views/preview` | SQL 预览执行 | ✅ **新** |
| `/tiles/cache/clear/{layer}` | 清除瓦片缓存 | ✅ **新** |
| `/tiles/cache/stats` | 缓存统计 | ✅ **新** |

### 1.5 新增 OGC 服务

| 服务 | 操作 | 状态 |
|------|------|:----:|
| **WMTS 1.0.0** | GetCapabilities / GetTile / GetFeatureInfo | ✅ **新** |
| **CQL/ECQL Filter** | 比较/逻辑/空间/IN/BETWEEN/LIKE | ✅ **新** |

### 1.6 新增输出格式

- ✅ **WMS 多格式**: SVG (矢量) / KML / GeoJSON
- ✅ **WFS 多格式**: CSV / GML 2.1.2 / GML 3.2.1
- ✅ **WMS Vendor 参数**: cql_filter / env / angle / featureId
- ✅ **WMS TIME/ELEVATION**: ISO 8601 时间过滤 + 数值高程过滤

### 1.7 基础设施

- ✅ **GeoWebCache 瓦片缓存**: 磁盘缓存 + 过期 + Gridset
- ✅ **SQL 视图**: 参数化 SQL → 虚拟图层
- ✅ **WCS 子集增强**: 空间裁剪 + 分辨率重采样

### 1.8 前端功能

- ✅ 仪表盘 (Dashboard)
- ✅ 图层列表/创建/详情/预览
- ✅ 要素 CRUD
- ✅ 工作区管理
- ✅ 命名空间管理 (NamespacesComponent) — **新**
- ✅ 存储管理 (StoresComponent) — **新**
- ✅ 数据源管理（PostGIS/Shapefile/GeoTIFF）
- ✅ SLD 样式管理（含模板）
- ✅ 图层组管理
- ✅ 瓦片图层 + GeoWebCache 统计 (TileLayersComponent 改版) — **新**
- ✅ 服务器状态页面
- ✅ 文件上传支持（GeoJSON/Shapefile/GeoTIFF）
- ✅ 登录页面 (LoginComponent) + JWT Token 管理 — **新**

---

## 二、⚠️ 部分实现功能

### 2.1 SLD 样式

- 基本 CRUD 和 SLD 渲染支持
- **缺少**：CSS Styling、YSLD、MBStyle、渲染变换、几何变换、标注障碍、z-order、复合/混合模式

---

## 三、❌ 未实现功能（按优先级排序）

### P0 — 核心基础 ✅ 已完成

- ✅ WMTS 标准服务
- ✅ GeoWebCache 缓存引擎
- ✅ 命名空间管理
- ✅ Store 独立管理
- ✅ SQL 视图

### P1 — OGC 服务增强 ✅ 已完成

- ✅ WMS 时间 & 高程支持
- ✅ WMS 多格式输出 (SVG/KML/GeoJSON)
- ✅ WMS Vendor 参数 (cql_filter/env/angle/featureId)
- ✅ WFS 多格式输出 (CSV/GML2/GML3.2)
- ✅ WCS 范围子集
- ✅ ECQL/CQL 过滤器 |

### P2 — 数据源扩展 ✅ 部分完成

- ✅ **GeoPackage 支持** — 矢量 + WKB 几何解析
- ✅ **WorldImage** — 世界影像格式 (.pgw/.jgw/.tfw)
- ✅ **ArcGrid** — ESRI ASCII Grid 栅格格式
- ✅ **级联 WMS 服务** — HTTP 代理 WMS 上游服务
- ❌ ImageMosaic — 栅格时间序列/镶嵌数据集
- ❌ ImagePyramid — 金字塔影像
- ❌ Oracle / MySQL / SQL Server — 更多数据库支持
- ❌ MongoDB — MongoDB GeoJSON 数据源

### P3 — 安全性 ✅ 已完成

- ✅ **CORS/CSRF 保护** — `actix-cors` 中间件 + 可配置白名单
- ✅ **用户/组/角色系统** — SHA-256+salt 密码哈希 + JWT Token + 审计日志
- ✅ **REST API 认证** — Bearer Token + `require_auth()` 中间件
- ✅ **图层级权限** — Permission 模型 + CRUD + 匹配规则引擎
- ✅ 前端登录页面 (LoginComponent) + AuthInterceptor
- ✅ 默认管理员: `admin / geoserver`
- 🔐 新增端点: `/auth/login`, `/auth/verify`, `/auth/users`, `/permissions`

### P4 — 扩展功能

| # | 功能 | 说明 | 预估工作量 |
|---|------|------|:---------:|
| 25 | **WPS (Web Processing Service)** | 地理处理服务：缓冲区、交并差、坐标转换等 | 4-6 周 |
| 26 | **CSW (Catalog Service)** | 目录服务：数据发现与元数据管理 | 3-4 周 |
| 27 | **OGC API 系列** | Features / Tiles / Maps / Coverages / Processes / Styles | 各 2-3 周 |
| 28 | **矢量瓦片 (Vector Tiles)** | MVT (Mapbox Vector Tile) 格式输出 | 2-3 周 |
| 29 | **KML 输出** | KML/KMZ 格式的地图/要素导出 | 1-2 周 |
| 30 | **打印模块 (Printing)** | PDF 地图打印服务 | 3-4 周 |
| 31 | **监控 (Monitoring)** | 请求统计、性能监控、审计日志 | 2-3 周 |
| 32 | **导入器 (Importer)** | 批量数据导入工作流 | 3-4 周 |
| 33 | **CSS/YSLD/MBStyle 样式** | 替代 SLD 的样式语言支持 | 各 1-2 周 |
| 34 | **备份/恢复** | 数据目录备份与恢复 | ✅ **已完成** |
| 35 | **GeoFence** | 细粒度访问控制 | 3-4 周 |

---

## 四、📋 分阶段实施路线图

### 阶段一：核心增强 (1-2 个月)
**目标**：完善 OGC 核心服务，补齐必要的数据管理功能

```
📅 Week 1-2:  命名空间管理 + Store 独立管理
📅 Week 3-4:  完整的 WMTS + GeoWebCache 引擎
📅 Week 5-6:  SQL 视图 + WMS 时间/高程支持
📅 Week 7-8:  WFS 2.0 增强 + 多格式输出 + ECQL 过滤器
```

### 阶段二：数据源扩展 ✅ 部分完成 (4/8)

```
📅 GeoPackage    ✅ 已完成
📅 WorldImage    ✅ 已完成
📅 ArcGrid       ✅ 已完成
📅 级联 WMS      ✅ 已完成
📅 ImageMosaic   ⏳
📅 ImagePyramid  ⏳
📅 更多数据库     ⏳
📅 MongoDB       ⏳
```

### 阶段三：安全与权限 ✅ 已完成
**目标**：构建完整的安全体系

```
📅 CORS/CSRF 保护         ✅ 已完成
📅 用户/组/角色系统       ✅ 已完成
📅 图层级权限             ✅ 已完成
📅 REST API 认证          ✅ 已完成
```

### 阶段四：高级扩展 (3-6 个月)
**目标**：实现企业级高级功能

```
📅 Week 1-4:   WPS 处理服务
📅 Week 5-8:   CSW 目录服务 + OGC API 系列
📅 Week 9-12:  矢量瓦片 + KML + CSS/YSLD/MBStyle 样式
📅 Week 13-16: 打印模块 + 监控 + 导入器
📅 Week 17-20: GeoFence + 备份恢复
```

---

## 五、📝 技术建议

### 5.1 架构改进建议

1. **插件化架构**：参考 GeoServer 的扩展机制，设计 trait-based 插件系统，方便动态加载数据源和处理器
2. **图层与数据源分离**：当前图层和数据源耦合较紧，应抽象出 `DataStore` / `CoverageStore` 接口
3. **缓存层抽象**：设计统一的瓦片缓存接口，支持内存/磁盘/S3/Redis 等多后端
4. **异步流处理**：对大数据集使用异步流式响应，减少内存压力

### 5.2 依赖库建议

| 需求 | 推荐库 |
|------|--------|
| 矢量瓦片 (MVT) | `tilejson` / `mvt` crate |
| GeoPackage | `geopackage` crate 或直接使用 SQLite |
| 投影增强 | `proj` crate (绑定 PROJ) |
| WPS 处理 | `geo` crate + `geos` crate (GEOS 绑定) |
| Excel 输出 | `calamine` / `rust_xlsxwriter` |
| PDF 打印 | `printpdf` / `genpdf` |
| JWT 认证 | `jsonwebtoken` crate |
| LDAP 认证 | `ldap3` crate |

### 5.3 测试策略

- 目前无测试套件，建议引入：
  - **单元测试**：Rust 原生 `#[cfg(test)]`
  - **集成测试**：使用 `actix-rt` 测试 HTTP 端点
  - **OGC CITE 测试**：参考 GeoServer CITE 测试套件验证标准符合性

---

## 六、📊 当前功能清单汇总

```
OGC 服务     █████████████░░░░  71%
REST API     █████████████░░░░  69%
数据源        █████████░░░░░░░  47%
样式系统      ██████░░░░░░░░░░  30%
瓦片缓存      ██████░░░░░░░░░░  33%
安全性        ████████░░░░░░░░  43%
扩展功能      ██░░░░░░░░░░░░░░   7%
──────────────────────────────
总进度        ██████████░░░░░░  50%
```
