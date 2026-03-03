use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures_dir = manifest_dir.join("tests/fixtures");
    let repo_root = manifest_dir
        .parent()
        .expect("integration-test must be inside repo root");

    if std::env::var("SKIP_WASH_BUILD").is_ok() {
        println!("cargo:warning=SKIP_WASH_BUILD set – skipping fixture builds");
        return;
    }

    std::fs::create_dir_all(&fixtures_dir).expect("Failed to create tests/fixtures directory");

    let components: Vec<(&str, PathBuf, PathBuf)> = vec![
        (
            "http-mcp",
            repo_root.join("components/http-mcp"),
            fixtures_dir.join("betty_mcp_component.wasm"),
        ),
        (
            "mock-actions",
            repo_root.join("components/http-mcp/tests/mock-actions"),
            fixtures_dir.join("mock_actions.wasm"),
        ),
        (
            "auth",
            repo_root.join("components/utilities/auth"),
            fixtures_dir.join("jwt_auth_component.wasm"),
        ),
    ];

    for (name, work_dir, dest) in &components {
        println!("cargo:warning=Building {} component...", name);

        // Clean stale build artifacts so find_signed_wasm doesn't find
        // multiple .wasm files (e.g. after a component rename).
        let build_dir = work_dir.join("build");
        if build_dir.exists() {
            std::fs::remove_dir_all(&build_dir).unwrap_or_else(|e| {
                panic!("Failed to clean build dir {}: {}", build_dir.display(), e)
            });
        }

        let status = Command::new("wash")
            .args(["build", "--skip-fetch"])
            .current_dir(work_dir)
            .status()
            .unwrap_or_else(|e| panic!("Failed to run wash build for {}: {}", name, e));

        if !status.success() {
            panic!(
                "wash build failed for {} (exit code: {:?})",
                name,
                status.code()
            );
        }

        let build_dir = work_dir.join("build");
        let wasm_file = find_signed_wasm(&build_dir, name);

        std::fs::copy(&wasm_file, dest).unwrap_or_else(|e| {
            panic!(
                "Failed to copy {} -> {}: {}",
                wasm_file.display(),
                dest.display(),
                e
            )
        });

        println!(
            "cargo:warning=Built and copied {} -> {}",
            name,
            dest.display()
        );
    }

    println!("cargo:rerun-if-changed=../components/http-mcp/src/");
    println!("cargo:rerun-if-changed=../components/http-mcp/wasmcloud.toml");
    println!("cargo:rerun-if-changed=../components/http-mcp/tests/mock-actions/src/");
    println!("cargo:rerun-if-changed=../components/http-mcp/tests/mock-actions/wasmcloud.toml");
    println!("cargo:rerun-if-changed=../components/utilities/auth/src/");
    println!("cargo:rerun-if-changed=../components/utilities/auth/wasmcloud.toml");
    println!("cargo:rerun-if-env-changed=SKIP_WASH_BUILD");
}

fn find_signed_wasm(build_dir: &PathBuf, component_name: &str) -> PathBuf {
    if !build_dir.exists() {
        panic!(
            "Build directory {} does not exist after wash build for {}",
            build_dir.display(),
            component_name
        );
    }

    let entries: Vec<PathBuf> = std::fs::read_dir(build_dir)
        .unwrap_or_else(|e| panic!("Failed to read build dir {}: {}", build_dir.display(), e))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|ext| ext == "wasm")
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with("_s.wasm"))
        })
        .collect();

    match entries.len() {
        0 => panic!(
            "No signed wasm file (*_s.wasm) found in {} for {}",
            build_dir.display(),
            component_name
        ),
        1 => entries.into_iter().next().unwrap(),
        _ => panic!(
            "Multiple signed wasm files found in {} for {}: {:?}",
            build_dir.display(),
            component_name,
            entries
        ),
    }
}
