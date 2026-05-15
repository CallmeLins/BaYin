fn main() {
    // cpal uses oboe (C++) on Android; link the C++ standard library
    // On iOS, cpal uses AudioUnit (no extra linking needed)
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "android" {
        println!("cargo:rustc-link-lib=c++_shared");
    }

    tauri_build::build()
}
