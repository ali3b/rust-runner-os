//! `cargo xtask pages`: the static site published on GitHub Pages, in
//! `target/pages/`: the single-file game as `index.html`, and the emulated-PC
//! build under `os/`. Third-party files keep their licence notices alongside.
use crate::{Result, target_dir, v86, web};
use std::fs;

const THIRD_PARTY: &str = "\
Third-party software in this folder
===================================

libv86.js, v86.wasm
  v86 0.5.462, https://github.com/copy/v86, BSD-2-Clause (see LICENSE-v86.txt).

images.js (the `bios` and `vga_bios` entries)
  SeaBIOS and the SeaBIOS VGA BIOS, as built by the v86 project,
  https://github.com/copy/v86/tree/b8a39b11dd2076870699e6cac053556271b9bfab/bios
  Licensed under the GNU LGPL v3. Source: https://www.seabios.org/
  (the exact build configuration is in the v86 repository linked above).

Everything else (the `multiboot` entry in images.js and the page) is Rust Runner OS,
MIT OR Apache-2.0: https://github.com/ali3b/rust-runner-os
";

pub fn build() -> Result<std::path::PathBuf> {
    let web_dir = web::build()?;
    let v86_dir = v86::build(&[])?;
    let out = target_dir().join("pages");
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(out.join("os")).map_err(|e| e.to_string())?;
    let copy = |from: std::path::PathBuf, to: std::path::PathBuf| -> Result<()> {
        fs::copy(&from, &to).map(|_| ()).map_err(|e| format!("copying {}: {e}", from.display()))
    };
    copy(web_dir.join("rust-runner.html"), out.join("index.html"))?;
    for f in ["index.html", "libv86.js", "v86.wasm", "images.js", "LICENSE-v86.txt"] {
        copy(v86_dir.join(f), out.join("os").join(f))?;
    }
    fs::write(out.join("os/THIRD-PARTY.txt"), THIRD_PARTY).map_err(|e| e.to_string())?;
    // Serve files as-is (no Jekyll processing).
    fs::write(out.join(".nojekyll"), "").map_err(|e| e.to_string())?;
    Ok(out)
}
