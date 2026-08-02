#!/bin/bash

# ==================== GeoFerris Build Script (Linux) ====================

set -e

BUILD_MODE="debug"
SKIP_FRONTEND=0

# Parse arguments
for arg in "$@"; do
    case "$arg" in
        -r|--release)
            BUILD_MODE="release"
            ;;
        -s|--skip-frontend)
            SKIP_FRONTEND=1
            ;;
    esac
done

echo "==================== GeoFerris Build Script ===================="
echo ""

if [ $SKIP_FRONTEND -eq 0 ]; then
    echo "Build mode: $BUILD_MODE"
    echo ""
    
    echo "[Step 1/4] Checking environment..."
    
    if ! command -v node &> /dev/null; then
        echo "ERROR: Node.js not found"
        exit 1
    fi
    echo "Node.js OK"
    
    if ! command -v npm &> /dev/null; then
        echo "ERROR: npm not found"
        exit 1
    fi
    echo "npm OK"
fi

if ! command -v rustc &> /dev/null; then
    echo "ERROR: Rust not found"
    exit 1
fi
echo "Rust OK"
echo ""

if [ $SKIP_FRONTEND -eq 0 ]; then
    echo "[Step 2/4] Building frontend..."
    
    if [ ! -d "frontend/node_modules" ]; then
        echo "Installing dependencies..."
        cd frontend && npm install && cd ..
    fi
    
    cd frontend && npm run build && cd ..
    echo "Frontend build OK"
    echo ""
    
    echo "[Step 3/4] Copying frontend to static..."
    rm -rf static
    cp -r frontend/dist/geoferris-ui static
    echo "Copy OK"
    echo ""
else
    echo "[Step 1/3] Skipping frontend build"
    if [ ! -d "static" ]; then
        echo "ERROR: static directory not found"
        exit 1
    fi
    echo "static directory OK"
    echo ""
fi

echo "[Step 3/4] Building Rust backend ($BUILD_MODE)..."
if [ "$BUILD_MODE" = "release" ]; then
    cargo build --release --quiet
else
    cargo build --quiet
fi
echo "Rust build OK"
echo ""

echo "[Step 4/4] Copying config file to artifact directory..."
if [ "$BUILD_MODE" = "release" ]; then
    ARTIFACT_DIR="target/release"
else
    ARTIFACT_DIR="target/debug"
fi
mkdir -p "$ARTIFACT_DIR"
if [ -f "geoferris.toml" ]; then
    cp -f "geoferris.toml" "$ARTIFACT_DIR/geoferris.toml"
    echo "Config copied: $ARTIFACT_DIR/geoferris.toml"
else
    cp -f "geoferris.toml.example" "$ARTIFACT_DIR/geoferris.toml"
    echo "Config template copied as: $ARTIFACT_DIR/geoferris.toml"
fi
echo ""

if [ "$BUILD_MODE" = "release" ]; then
    echo "[Step 5/5] Preparing release package..."
    
    RELEASE_DIR="target/release/release-package"
    rm -rf "$RELEASE_DIR"
    mkdir -p "$RELEASE_DIR"
    
    echo "Copying executable..."
    cp "target/release/geoferris" "$RELEASE_DIR/"
    
    echo "Copying static files..."
    cp -r static "$RELEASE_DIR/"
    
    echo "Copying config file..."
    if [ -f "geoferris.toml" ]; then
        cp "geoferris.toml" "$RELEASE_DIR/"
        echo "Config file copied"
    else
        echo "No config file found, skipping"
    fi

    echo "Copying config template..."
    if [ -f "geoferris.toml.example" ]; then
        cp "geoferris.toml.example" "$RELEASE_DIR/"
        echo "Config template copied"
    fi

    echo "Creating README..."
    cat > "$RELEASE_DIR/README.txt" << EOF
GeoFerris v0.1.0

Usage:
  ./geoferris

Configuration:
  Edit geoferris.toml to configure server settings.

API: http://localhost:8080/geoserver
Web: http://localhost:8080
EOF
    
    echo "Release package: $RELEASE_DIR"
    echo ""
fi

echo "==================== Build Complete ===================="
echo ""
if [ "$BUILD_MODE" = "release" ]; then
    echo "Release package: target/release/release-package/"
    echo "Executable: target/release/release-package/geoferris"
else
    echo "Executable: target/debug/geoferris"
fi
echo "Frontend: static/"
echo ""