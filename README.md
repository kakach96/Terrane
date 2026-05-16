# 🌍 Rust GeoServer

基于 Rust + Actix-web 的轻量级地理空间数据服务器，配备现代化 Angular + Material 管理界面。

## ✨ 功能特性

### 后端服务 (Rust)
- 🌐 **WMS** - Web Map Service (地图服务)
- 📍 **WFS** - Web Feature Service (要素服务)
- 🛰️ **WCS** - Web Coverage Service (栅格服务)
- 🔌 **REST API** - 完整的数据管理接口
- 🗺️ **地图渲染** - 支持点、线、多边形渲染

### 前端界面 (Angular)
- 📊 **仪表盘** - 系统概览和统计
- 🗺️ **图层管理** - 可视化图层管理
- ➕ **创建图层** - 表单向导
- 🔍 **图层详情** - 信息和预览
- 📍 **要素管理** - CRUD 操作
- 🎨 **Material Design** - 现代化 UI

## 🚀 快速开始

### 环境要求

- Rust 1.95+
- Node.js 18+
- npm 9+

### 方式一：一键启动（推荐）

```bash
# 自动构建前端 + 启动服务
cargo run

# 访问 http://127.0.0.1:8080
```

### 方式二：开发模式（前后端分离）

```bash
# 终端 1 - 启动后端（跳过前端构建，更快）
$env:SKIP_FRONTEND=1
cargo run

# 终端 2 - 启动前端开发服务器（支持热重载）
cd frontend
npm install
npm start

# 访问 http://localhost:4200
```

### 方式三：完整构建

```bash
# 构建前端会自动构建前端
cargo build

# 运行
cargo run
```

### 或使用启动脚本

```bash
cd frontend
START.bat
```

## 📁 项目结构

```
rust-geoserver/
├── src/                    # Rust 后端源代码
│   ├── handlers/          # HTTP 请求处理器
│   ├── services/          # OGC 服务实现
│   ├── models/            # 数据模型
│   ├── utils/             # 工具函数
│   └── main.rs            # 应用入口
├── frontend/              # Angular 前端
│   ├── src/
│   │   └── app/
│   │       ├── components/ # 页面组件
│   │       ├── services/  # API 服务
│   │       └── models/    # 数据模型
│   └── ...
├── static/                 # 后端静态文件（备用）
└── Cargo.toml             # Rust 依赖
```

## 🌐 API 端点

### REST API

| 方法 | 端点 | 描述 |
|------|------|------|
| GET | `/api/layers` | 获取所有图层 |
| POST | `/api/layers` | 创建图层 |
| GET | `/api/layers/:name` | 获取图层详情 |
| PUT | `/api/layers/:name` | 更新图层 |
| DELETE | `/api/layers/:name` | 删除图层 |
| GET | `/api/layers/:name/preview` | 获取预览图 |
| GET | `/api/layers/:name/features` | 获取要素 |
| POST | `/api/layers/:name/features` | 添加要素 |

### OGC 服务

| 服务 | 端点 | 操作 |
|------|------|------|
| WMS | `/wms` | GetCapabilities, GetMap, GetFeatureInfo |
| WFS | `/wfs` | GetCapabilities, DescribeFeatureType, GetFeature |
| WCS | `/wcs` | GetCapabilities, DescribeCoverage, GetCoverage |

## 🎨 界面预览

### 仪表盘
- 系统统计卡片
- 最近图层列表
- 快捷操作

### 图层管理
- 卡片式图层展示
- 搜索和筛选
- 一键删除

### 图层详情
- 详细信息展示
- 实时地图预览
- 要素表格管理

## 🛠️ 技术栈

### 后端
- **Rust** - 系统编程语言
- **Actix-web** - Web 框架
- **Tokio** - 异步运行时
- **Geo** - 几何计算
- **Image** - 图像处理
- **Serde** - 序列化

### 前端
- **Angular 17** - 前端框架
- **Angular Material** - UI 组件库
- **TypeScript** - 类型安全
- **RxJS** - 响应式编程
- **SCSS** - 样式预处理

## 📦 数据格式

### 图层

```json
{
  "name": "world_cities",
  "title": "World Cities",
  "workspace": "default",
  "store": "shapes",
  "srs": "EPSG:4326",
  "bounds": {
    "minx": -180,
    "miny": -90,
    "maxx": 180,
    "maxy": 90
  }
}
```

### 要素

```json
{
  "geometry": {
    "type": "Point",
    "coordinates": [116.4, 39.9]
  },
  "properties": {
    "name": "Beijing",
    "population": 21540000
  }
}
```

## 🔧 配置

配置文件：`geoserver.toml`

```toml
[server]
host = "127.0.0.1"
port = 8080
workers = 12

[workspaces]
[[workspaces.stores.layers]]
name = "world"
title = "World"
srs = "EPSG:4326"
```

## 📚 文档

- [集成构建说明](BUILD_INTEGRATION.md)
- [前端文档](frontend/README.md)
- [前端项目总结](frontend/PROJECT_SUMMARY.md)

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 许可证

MIT License

## 🙏 致谢

- GeoServer 社区
- OGC 标准组织
- Rust 社区
- Angular 团队

---

**Made with ❤️ using Rust + Angular**
