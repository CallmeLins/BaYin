fn main() {
    // Tauri defines a `mobile` cfg when building for Android/iOS.
    println!("cargo:rustc-check-cfg=cfg(mobile)");
}

