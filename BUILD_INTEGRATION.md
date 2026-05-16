# 集成构建说明

本项目使用 `build.rs` 实现 Cargo 自动构建前端并复制到 `static/` 目录。

## 🚀 快速开始

### 完整构建（含前端）

```bash
# 第一次构建会自动安装 npm 依赖
cargo build

# 或者直接运行，会先自动构建前端
cargo run
```

### 开发模式

#### 后端 + 前端（构建时自动）

```bash
# 这会自动执行 npm install 和 npm run build
cargo run
```

访问：http://localhost:8080

#### 前端开发服务器（推荐）

```bash
# 终端 1 - 启动后端
cd rust-geoserver
cargo run

# 终端 2 - 启动 Angular 开发服务器（支持热重载）
cd frontend
npm install  # 第一次运行需要
npm start
```

访问：http://localhost:4200

## 🔧 构建选项

### 跳过前端构建

如果没有安装 Node.js 或只想修改后端，可以跳过前端构建：

```bash
# PowerShell
$env:SKIP_FRONTEND=1
cargo build

# 或者直接运行
$env:SKIP_FRONTEND=1
cargo run

# Linux/macOS
SKIP_FRONTEND=1 cargo run
```

### 手动构建前端

如果想自己控制前端构建：

```bash
cd frontend
npm install
npm run build
```

构建产物会在 `frontend/dist/rust-geoserver-ui/` 目录。

## 📁 构建流程

1. **检测变更** - `build.rs` 检测 `frontend/src/`、`package.json` 等变更
2. **检查依赖** - 检查 `node_modules` 是否存在，不存在则运行 `npm install`
3. **构建前端** - 运行 `npm run build`（生产模式）
4. **复制文件** - 将 `frontend/dist/rust-geoserver-ui/` 复制到 `static/` 目录
5. **启动服务** - Actix-web 从 `static/` 目录提供静态文件

## 📊 项目结构

```
rust-geoserver/
├── src/                    # Rust 源代码
├── frontend/              # Angular 前端
│   ├── src/
│   ├── node_modules/      # npm 依赖（自动生成）
│   └── dist/             # 构建产物（自动生成）
├── static/               # 静态文件（由 build.rs 自动生成）
│   ├── index.html
│   ├── main.*.js
│   └── ...
├── build.rs              # 构建脚本
└── Cargo.toml
```

## 🔨 开发工作流

### 后端开发

```bash
# 修改 Rust 代码后
cargo run

# 跳过前端构建（更快）
$env:SKIP_FRONTEND=1
cargo run
```

### 前端开发

```bash
cd frontend

# 安装依赖（第一次）
npm install

# 开发服务器（热重载）
npm start

# 生产构建
npm run build
```

### 全栈开发

使用两个终端：

```bash
# 终端 1 - 后端
cargo run

# 终端 2 - 前端
cd frontend
npm start
```

## 📝 注意事项

1. **Node.js 必需** - 需要安装 Node.js 18+
2. **首次构建慢** - 第一次 `npm install` 可能需要几分钟
3. **Git 忽略** - `static/` 和 `node_modules/` 已加入 .gitignore
4. **静态文件** - `build.rs` 会自动覆盖 `static/` 目录内容

## 🎯 生产部署

```bash
# 构建生产版本
cargo build --release

# 运行
./target/release/rust-geoserver
```

## ❓ 常见问题

### npm 命令找不到

- 确认已安装 Node.js：https://nodejs.org/
- 重启终端或重新加载环境变量

### 前端构建失败

```bash
cd frontend
npm install
npm run build
```

手动查看错误信息。

### 找不到 static/index.html

- 运行 `cargo build` 确保前端已构建
- 检查 `static/` 目录是否有文件

## 📚 更多文档

- [前端文档](frontend/README.md)
- [主项目文档](README.md)
