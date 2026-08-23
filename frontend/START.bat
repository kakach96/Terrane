@echo off
chcp 65001 > nul
echo ========================================
echo   Terrane + Angular UI 启动器
echo ========================================
echo.

echo [1/3] 检查 Node.js 和 npm...
where node > nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ 未找到 Node.js，请先安装
    echo    下载地址: https://nodejs.org/
    pause
    exit /b 1
)

node --version
echo ✅ Node.js 已安装
echo.

echo [2/3] 检查 Angular CLI...
where ng > nul 2>&1
if %errorlevel% neq 0 (
    echo 📦 正在全局安装 Angular CLI...
    npm install -g @angular/cli
)

ng version --minimal
echo ✅ Angular CLI 已安装
echo.

echo [3/3] 检查前端依赖...
cd /d "%~dp0frontend"
if not exist "node_modules" (
    echo 📦 正在安装前端依赖...
    npm install
    echo ✅ 依赖安装完成
) else (
    echo ✅ 依赖已存在
)
echo.

echo ========================================
echo   启动完成！
echo ========================================
echo.
echo 📋 启动说明：
echo.
echo 1. 在此终端窗口启动后端（Rust）:
echo    cd ..\service
echo    cargo run
echo.
echo 2. 在新的终端窗口启动前端（Angular）:
echo    cd frontend
echo    ng serve
echo.
echo 🌐 访问地址：
echo    前端: http://localhost:4200
echo    后端: http://localhost:8080
echo.
echo ========================================
echo.

cd /d "%~dp0"
pause
