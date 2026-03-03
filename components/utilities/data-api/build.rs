use std::{env, process::Command};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = env::var("OUT_DIR").unwrap();

    let is_test_build = out_dir.contains("debug");

    let tonic_builder = tonic_build::configure()
        .build_client(true)
        .build_transport(false) // Don't generate transport code for WASI
        .build_server(is_test_build);

    if is_test_build {
        println!("cargo:warning=Building WASM component for tests…");
        println!("cargo::rerun-if-changed=test-component/src/lib.rs");
        println!("cargo::rerun-if-changed=src/lib.rs");

        let status = Command::new("cargo")
            .args(["build", "--target", "wasm32-wasip2", "--release"])
            .status()
            .expect("Failed to compile WASM component");

        assert!(status.success(), "WASM component build failed");

        let status = Command::new("cargo")
            .args([
                "build",
                "--manifest-path",
                "test-component/Cargo.toml",
                "--target",
                "wasm32-wasip2",
                "--release",
            ])
            .status()
            .expect("Failed to compile WASM component");

        assert!(status.success(), "WASM component build failed");

        // Copy built WASM artifacts to tests/fixtures/ so include_bytes! can
        // find them via CARGO_MANIFEST_DIR (workspace builds output to the
        // root target/ directory, not the crate-local one).
        let manifest_dir = std::path::PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let workspace_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .expect("Could not find workspace root");
        let wasm_target_dir = workspace_root.join("target/wasm32-wasip2/release/deps");
        let fixtures_dir = manifest_dir.join("tests/fixtures");
        std::fs::create_dir_all(&fixtures_dir).expect("Failed to create tests/fixtures");

        for (src_name, dest_name) in [
            ("data_api_component.wasm", "data_api_component.wasm"),
            ("test_component.wasm", "test_component.wasm"),
        ] {
            let src = wasm_target_dir.join(src_name);
            let dest = fixtures_dir.join(dest_name);
            std::fs::copy(&src, &dest).unwrap_or_else(|e| {
                panic!(
                    "Failed to copy {} -> {}: {}",
                    src.display(),
                    dest.display(),
                    e
                )
            });
            println!(
                "cargo:warning=Copied {} -> {}",
                src.display(),
                dest.display()
            );
        }
    } else {
        println!("cargo:warning=Skipping WASM build (not a test build)");
    }

    tonic_builder.compile_protos(&["proto/data-api.proto"], &["proto"])?;

    Ok(())
}
