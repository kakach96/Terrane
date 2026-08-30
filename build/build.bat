@echo off
setlocal enabledelayedexpansion

rem Change to the repository root so relative paths work from any directory
cd /d "%~dp0.."

echo ==================== Terrane Build Script ====================
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
if exist "service\static" (
    rmdir /s /q service\static
)
xcopy /s /e /i "frontend\dist\terrane-ui\browser" "service\static" >nul
if errorlevel 1 (
    echo ERROR: Copy to static failed
    exit /b 1
)
echo Copy OK
echo.

goto build_rust_section

:skip_frontend_section
echo [Step 1/3] Skipping frontend build
if not exist "service\static" (
    echo ERROR: static directory not found
    exit /b 1
)
echo static directory OK
echo.

:build_rust_section
echo [Step 3/4] Building Rust backend (%BUILD_MODE%)...
if %BUILD_MODE%==release (
    cargo build --release --manifest-path service\Cargo.toml
) else (
    cargo build --manifest-path service\Cargo.toml
)
if errorlevel 1 (
    echo ERROR: Rust build failed
    exit /b 1
)
echo Rust build OK
echo.

echo [Step 4/4] Copying config file to artifact directory...
if %BUILD_MODE%==release (
    set ARTIFACT_DIR=service\target\release
) else (
    set ARTIFACT_DIR=service\target\debug
)
if not exist "!ARTIFACT_DIR!" (
    mkdir "!ARTIFACT_DIR!"
)
rem Keep an existing config next to the artifact (users maintain service\target\<mode>\terrane.toml);
rem only seed it from the service dir or the example template when it does not exist yet.
if exist "!ARTIFACT_DIR!\terrane.toml" (
    echo Config kept: !ARTIFACT_DIR!\terrane.toml
) else (
    if exist "service\terrane.toml" (
        copy /y "service\terrane.toml" "!ARTIFACT_DIR!\terrane.toml" >nul
        echo Config copied: !ARTIFACT_DIR!\terrane.toml
    ) else (
        copy /y "service\terrane.toml.example" "!ARTIFACT_DIR!\terrane.toml" >nul
        echo Config template copied as: !ARTIFACT_DIR!\terrane.toml
    )
)
echo.

if %BUILD_MODE%==release (
    echo [Step 5/5] Preparing release package...
    
    set RELEASE_DIR=service\target\release\release-package
    if exist "!RELEASE_DIR!" (
        rmdir /s /q "!RELEASE_DIR!"
    )
    mkdir "!RELEASE_DIR!"
    
    echo Copying executable...
    copy "service\target\release\terrane.exe" "!RELEASE_DIR!\" >nul
    
    echo Copying static files...
    xcopy /s /e /i "service\static" "!RELEASE_DIR!\static" >nul
    
    echo Copying config file...
    if exist "service\terrane.toml" (
        copy "service\terrane.toml" "!RELEASE_DIR!\" >nul
        echo Config file copied
    ) else (
        echo No config file found, skipping
    )

    echo Copying config template...
    if exist "service\terrane.toml.example" (
        copy "service\terrane.toml.example" "!RELEASE_DIR!\" >nul
        echo Config template copied
    )

    echo Creating README...
    (
        echo Terrane v0.1.0
        echo.
        echo Usage:
        echo   terrane.exe
        echo.
        echo Configuration:
        echo   Edit terrane.toml to configure server settings.
        echo.
        echo API: http://localhost:8080/terrane
        echo Web: http://localhost:8080
    ) > "!RELEASE_DIR!\README.txt"
    
    echo Release package: !RELEASE_DIR!
    echo.
)

echo ==================== Build Complete ====================
echo.
if %BUILD_MODE%==release (
    echo Release package: service\target\release\release-package\
    echo Executable: service\target\release\release-package\terrane.exe
    echo Frontend: service\target\release\release-package\static\
) else (
    echo Executable: service\target\debug\terrane.exe
    echo Frontend: service\static\
)
echo.

endlocal