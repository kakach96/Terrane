@echo off
setlocal enabledelayedexpansion

echo ==================== RRGeoServer Build Script ====================
echo.

set BUILD_MODE=debug
set SKIP_FRONTEND=0

if "%1"=="-r" set BUILD_MODE=release
if "%1"=="--release" set BUILD_MODE=release
if "%1"=="-s" set SKIP_FRONTEND=1
if "%1"=="--skip-frontend" set SKIP_FRONTEND=1

if %SKIP_FRONTEND%==1 goto skip_frontend_section

echo Build mode: %BUILD_MODE%
echo.

echo [Step 1/4] Checking environment...
where node >nul 2>&1
if errorlevel 1 (
    echo ERROR: Node.js not found
    exit /b 1
)
echo Node.js OK

where npm >nul 2>&1
if errorlevel 1 (
    echo ERROR: npm not found
    exit /b 1
)
echo npm OK

where rustc >nul 2>&1
if errorlevel 1 (
    echo ERROR: Rust not found
    exit /b 1
)
echo Rust OK
echo.

echo [Step 2/4] Building frontend...
if not exist "frontend\node_modules" (
    echo Installing dependencies...
    cd frontend
    call npm install
    cd ..
    if errorlevel 1 (
        echo ERROR: npm install failed
        exit /b 1
    )
)

cd frontend
call npm run build
cd ..
if errorlevel 1 (
    echo ERROR: Frontend build failed
    exit /b 1
)
echo Frontend build OK
echo.

echo [Step 3/4] Copying frontend to static...
if exist "static" (
    rmdir /s /q static
)
xcopy /s /e /i "frontend\dist\rust-geoserver-ui" "static" >nul
if errorlevel 1 (
    echo ERROR: Copy to static failed
    exit /b 1
)
echo Copy OK
echo.

goto build_rust_section

:skip_frontend_section
echo [Step 1/3] Skipping frontend build
if not exist "static" (
    echo ERROR: static directory not found
    exit /b 1
)
echo static directory OK
echo.

:build_rust_section
echo [Step 3/4] Building Rust backend (%BUILD_MODE%)...
if %BUILD_MODE%==release (
    cargo build --release
) else (
    cargo build
)
if errorlevel 1 (
    echo ERROR: Rust build failed
    exit /b 1
)
echo Rust build OK
echo.

if %BUILD_MODE%==release (
    echo [Step 4/4] Preparing release package...
    
    set RELEASE_DIR=target\release\release-package
    if exist "!RELEASE_DIR!" (
        rmdir /s /q "!RELEASE_DIR!"
    )
    mkdir "!RELEASE_DIR!"
    
    echo Copying executable...
    copy "target\release\rust-geoserver.exe" "!RELEASE_DIR!\" >nul
    
    echo Copying static files...
    xcopy /s /e /i "static" "!RELEASE_DIR!\static" >nul
    
    echo Copying config file...
    if exist "geoserver.toml" (
        copy "geoserver.toml" "!RELEASE_DIR!\" >nul
        echo Config file copied
    ) else (
        echo No config file found, skipping
    )
    
    echo Creating README...
    (
        echo RRGeoServer v1.0.0
        echo.
        echo Usage:
        echo   rust-geoserver.exe
        echo.
        echo Configuration:
        echo   Edit geoserver.toml to configure server settings.
        echo.
        echo API: http://localhost:8080/geoserver
        echo Web: http://localhost:8080
    ) > "!RELEASE_DIR!\README.txt"
    
    echo Release package: !RELEASE_DIR!
    echo.
)

echo ==================== Build Complete ====================
echo.
if %BUILD_MODE%==release (
    echo Release package: target\release\release-package\
    echo Executable: target\release\release-package\rust-geoserver.exe
) else (
    echo Executable: target\debug\rust-geoserver.exe
)
echo Frontend: static\
echo.

endlocal