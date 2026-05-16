use std::env;
use std::path::Path;
use std::fs;

fn main() {
    println!("cargo:rerun-if-changed=frontend/package.json");
    println!("cargo:rerun-if-changed=frontend/angular.json");
    
    let skip_frontend = env::var("SKIP_FRONTEND").is_ok();
    if skip_frontend {
        println!("cargo:info=SKIP_FRONTEND set, skipping frontend build");
        return;
    }

    let frontend_dir = Path::new("frontend");
    let static_dir = Path::new("static");
    let dist_dir = frontend_dir.join("dist").join("rust-geoserver-ui");

    if !dist_dir.exists() {
        println!("cargo:warning=Frontend dist directory not found: {:?}", dist_dir);
        println!("cargo:warning=Please build frontend manually before running cargo build:");
        println!("cargo:warning=  cd frontend && npm install && npm run build");
        println!("cargo:warning=Or set SKIP_FRONTEND=1 to skip this check");
        return;
    }

    if static_dir.exists() {
        if let Err(e) = fs::remove_dir_all(static_dir) {
            eprintln!("cargo:warning=Failed to clean static directory: {}", e);
        }
    }

    match copy_dir_all(&dist_dir, static_dir) {
        Ok(_) => {
            println!("cargo:info=Frontend files copied to static directory");
        }
        Err(e) => {
            eprintln!("cargo:warning=Failed to copy frontend files: {}", e);
        }
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            let dest_path = dst.join(entry.file_name());
            if dest_path.exists() {
                fs::remove_file(&dest_path)?;
            }
            fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}
