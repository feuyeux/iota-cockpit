fn main() {
    // Tauri validates every externalBin entry even for `cargo test`. The real
    // runner is staged by the tauri npm scripts; keep plain workspace builds
    // self-contained with a disposable placeholder when it is absent.
    let target = std::env::var("TARGET").expect("Cargo always provides TARGET");
    let sidecar = std::path::Path::new("binaries").join(format!("cockpit-runner-{target}"));
    if !sidecar.exists() {
        std::fs::create_dir_all(sidecar.parent().expect("sidecar has a parent"))
            .expect("create sidecar directory");
        std::fs::write(&sidecar, []).expect("create sidecar placeholder");
    }
    tauri_build::build();
}
