use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

fn main() {
    warn_if_firmware_stale();
    tauri_build::build();
}

/// `FIRMWARE` in lib.rs embeds build/merged-binary.bin as it is on disk. When a
/// firmware source is newer, `idf.py build merge-bin` wasn't rerun and the app
/// would flash an image without the latest changes (such as a new face, which
/// the old firmware reads as `classic`).
fn warn_if_firmware_stale() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let image = root.join("build/merged-binary.bin");
    let main_dir = root.join("main");
    let defaults = root.join("sdkconfig.defaults");
    for path in [&image, &main_dir, &defaults] {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let Some(built) = modified(&image) else {
        return;
    };
    let mut sources = vec![defaults];
    collect_files(&main_dir, &mut sources);
    let newer: Vec<_> = sources.iter().filter(|p| modified(p).is_some_and(|t| t > built)).collect();
    if let Some(first) = newer.first() {
        let name = first.strip_prefix(&root).unwrap_or(first).display();
        println!(
            "cargo:warning=build/merged-binary.bin is older than {name} ({} firmware file(s) changed since): \
             run `idf.py build merge-bin` and rebuild the app, or it will flash a stale firmware",
            newer.len()
        );
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}
