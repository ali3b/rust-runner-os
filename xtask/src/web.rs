//! `cargo xtask web`: the game compiled straight to WebAssembly (no emulator).
//! Produces, in `target/web/`:
//! - `rust-runner.html`: everything in one file (the module inlined as base64);
//! - `index.html` + `runner.wasm`: the same page loading the module separately;
//! - `rust-runner-fragment.html`: the single file without the document wrapper,
//!   for hosts that add their own (used to publish the page).
use crate::{Result, base64, target_dir, workspace_root};
use std::{env, fs, path::PathBuf, process::Command};

pub fn build() -> Result<PathBuf> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(workspace_root())
        .args(["build", "--package", "web", "--release", "--target", "wasm32-unknown-unknown"])
        // Ship without debug info: it is most of the file.
        .args(["--config", "profile.release.debug=false", "--config", "profile.release.strip=true"])
        .status()
        .map_err(|e| format!("failed to run cargo: {e}"))?;
    if !status.success() {
        return Err("wasm build failed".into());
    }
    let out = target_dir().join("web");
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let wasm = target_dir().join("wasm32-unknown-unknown/release/web.wasm");
    fs::copy(&wasm, out.join("runner.wasm")).map_err(|e| format!("copying {}: {e}", wasm.display()))?;
    // The page is a fragment (so it can also be published as-is); wrap it into a
    // complete document for a plain static host.
    let page = fs::read_to_string(workspace_root().join("web/www/game.html")).map_err(|e| e.to_string())?;
    let document = |body: &str| {
        format!(
            "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\">\n\
             </head>\n<body>\n{body}\n</body>\n</html>\n"
        )
    };
    fs::write(out.join("index.html"), document(&page)).map_err(|e| e.to_string())?;

    const MARKER: &str = "<!-- runner-wasm: `cargo xtask web` inlines the game here as base64 -->";
    if !page.contains(MARKER) {
        return Err("web/www/game.html lost its runner-wasm marker".into());
    }
    let wasm_bytes = fs::read(out.join("runner.wasm")).map_err(|e| e.to_string())?;
    let inline = page.replacen(MARKER, &format!("<script>window.RUNNER_WASM = \"{}\";</script>", base64(&wasm_bytes)), 1);
    fs::write(out.join("rust-runner-fragment.html"), &inline).map_err(|e| e.to_string())?;
    fs::write(out.join("rust-runner.html"), document(&inline)).map_err(|e| e.to_string())?;
    let single = fs::metadata(out.join("rust-runner.html")).map(|m| m.len()).unwrap_or(0);
    println!("web: rust-runner.html {} KiB (single file)", single / 1024);
    let size = fs::metadata(out.join("runner.wasm")).map(|m| m.len()).unwrap_or(0);
    println!("web: runner.wasm {} KiB", size / 1024);
    Ok(out)
}
