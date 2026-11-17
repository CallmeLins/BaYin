use std::path::{Path, PathBuf};

fn main() {
    // cpal uses oboe (C++) on Android; link the C++ standard library
    // On iOS, cpal uses AudioUnit (no extra linking needed)
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "android" {
        println!("cargo:rustc-link-lib=c++_shared");
    }

    // 车机版：前端产物是"生成物"，必须在它变化时重新嵌入二进制。
    // tauri-build 自己不会递归监听 dist 目录里的每个文件，所以这里显式声明。
    rerun_on_dir_changes(Path::new("../src-ui-gen"));
    rerun_on_dir_changes(Path::new("../src-ui/car"));

    tauri_build::build()
}

/// 递归列出目录下所有文件的修改时间，让 cargo 在任一文件变化时重跑 build.rs。
fn rerun_on_dir_changes(dir: &Path) {
    println!("cargo:rerun-if-changed={}", dir.display());
    if !dir.is_dir() {
        return;
    }
    let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = match std::fs::read_dir(&current) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            println!("cargo:rerun-if-changed={}", path.display());
            if path.is_dir() {
                stack.push(path);
            }
        }
    }
}