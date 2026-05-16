# Rust GeoServer 前端项目

基于 Angular 17 和 Angular Material 的现代化 GeoServer 管理界面。

## 🚀 快速开始

### 1. 安装依赖

```bash
cd frontend
npm install
```

### 2. 启动开发服务器

```bash
ng serve
```

访问 **http://localhost:4200** （开发服务器会自动代理 `/api` 请求到后端）

### 3. 启动后端服务器

在另一个终端：

```bash
cd ..  # 返回项目根目录
cargo run
```

后端运行在 **http://localhost:8080**

## 📁 项目结构

```
frontend/
├── src/
│   ├── app/
│   │   ├── components/           # 页面组件
│   │   │   ├── dashboard/        # 📊 仪表盘
│   │   │   ├── layers/           # 📚 图层列表
│   │   │   ├── layer-detail/     # 🔍 图层详情
│   │   │   ├── layer-create/     # ➕ 创建图层
│   │   │   └── preview/          # 🖼️ 预览组件
│   │   ├── services/             # 🔧 业务服务
│   │   │   ├── geoserver.service.ts      # GeoServer API
│   │   │   └── notification.service.ts   # 通知服务
│   │   ├── models/               # 📦 数据模型
│   │   │   └── geoserver.models.ts
│   │   ├── shared/               # 🔄 共享组件
│   │   │   └── components/
│   │   │       └── confirm-dialog.component.ts
│   │   ├── app.component.ts      # 根组件
│   │   ├── app.module.ts          # 根模块
│   │   └── app-routing.module.ts  # 路由配置
│   ├── styles.scss                # 全局样式
│   ├── index.html                 # HTML 入口
│   └── main.ts                    # 应用入口
├── angular.json                   # Angular 配置
├── package.json                   # 依赖配置
├── tsconfig.json                  # TypeScript 配置
├── proxy.conf.json                # 开发代理配置
└── README.md                      # 项目文档
```

## 🎯 功能模块

### 1. 仪表盘 (`/dashboard`)
- 系统统计概览
- 最近图层列表
- 快捷操作入口

### 2. 图层管理 (`/layers`)
- 图层列表展示
- 搜索和筛选
- 图层卡片视图
- 删除图层

### 3. 创建图层 (`/layers/create`)
- 表单验证
- 工作空间选择
- 坐标系统配置
- 边界范围设置

### 4. 图层详情 (`/layers/:name`)
- 图层信息展示
- 实时预览
- 要素管理（列表、删除）
- 预览尺寸调整

## 🎨 设计特点

### UI/UX
- **Material Design 3** - 遵循 Material Design 规范
- **响应式布局** - 支持桌面和移动设备
- **动画效果** - 流畅的过渡动画
- **深色侧边栏** - 专业的数据管理界面风格

### 技术亮点
- **模块化架构** - 清晰的项目结构
- **RxJS** - 响应式编程
- **TypeScript** - 类型安全
- **SCSS** - 现代化样式管理

## 🔌 API 集成

前端通过 Angular HttpClient 与后端通信，支持以下 API：

| 方法 | 端点 | 描述 |
|------|------|------|
| GET | `/api/layers` | 获取所有图层 |
| POST | `/api/layers` | 创建新图层 |
| GET | `/api/layers/:name` | 获取图层详情 |
| PUT | `/api/layers/:name` | 更新图层 |
| DELETE | `/api/layers/:name` | 删除图层 |
| GET | `/api/layers/:name/preview` | 获取图层预览图片 |
| GET | `/api/layers/:name/features` | 获取图层要素 |
| POST | `/api/layers/:name/features` | 添加要素 |
| DELETE | `/api/layers/:name/features/:id` | 删除要素 |

## 🛠️ 开发命令

```bash
# 开发服务器
ng serve

# 构建生产版本
ng build

# 运行测试
ng test

# 懒人构建（监听模式）
ng build --watch --configuration development
```

## 📦 扩展指南

### 添加新页面

1. 在 `src/app/components/` 创建组件目录
2. 创建 `.ts`, `.html`, `.scss` 文件
3. 在 `app.module.ts` 中声明组件
4. 在路由配置中添加路由

### 添加新服务

1. 在 `src/app/services/` 创建服务文件
2. 使用 `@Injectable({ providedIn: 'root' })` 装饰器
3. 在组件中通过构造函数注入使用

### 添加新模型

1. 在 `src/app/models/` 创建模型文件
2. 导出 TypeScript 接口或类
3. 在需要的地方导入使用

## 🎨 自定义主题

编辑 `src/styles.scss` 修改主题配置：

```scss
@use '@angular/material' as mat;

$geoserver-primary: mat.m2-define-palette(mat.$m2-indigo-palette, 700, 500, 900);
$geoserver-accent: mat.m2-define-palette(mat.$m2-teal-palette, A400, A200, A700);

$geoserver-theme: mat.m2-define-light-theme((
  color: (
    primary: $geoserver-primary,
    accent: $geoserver-accent,
  ),
));
```

## 📝 注意事项

1. **Node.js 版本** - 需要 Node.js 16.x 或更高版本
2. **Angular CLI** - 全局安装：`npm install -g @angular/cli`
3. **代理配置** - 开发时自动代理 API 请求到后端
4. **CORS** - 确保后端允许跨域请求

## 🚀 部署

### 开发环境
```bash
ng serve
```

### 生产构建
```bash
ng build --configuration production
```

构建产物在 `dist/rust-geoserver-ui/`

### 集成到后端

将 `dist/rust-geoserver-ui/` 目录复制到后端项目的静态文件目录，并配置服务器提供这些文件。

## 📚 学习资源

- [Angular 官方文档](https://angular.io/docs)
- [Angular Material 组件库](https://material.angular.io/)
- [TypeScript 手册](https://www.typescriptlang.org/docs/)
- [RxJS 文档](https://rxjs.dev/guide/overview)
