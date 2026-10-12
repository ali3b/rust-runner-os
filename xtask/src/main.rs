//! Host-side helper: `cargo xtask <image|run|smoke>`.
//!
//! - `image`: build the kernel and wrap it in a BIOS-bootable disk image.
//! - `run`:   build the image and launch it in QEMU with a window (extra args go to QEMU).
//! - `smoke`: headless boot that drives the game through the QEMU monitor and checks
//!   the serial log: title -> playing -> game over -> playing.
//! - `record`: boot the autoplay demo build headless and record an MP4 from power-on.
//! - `web`:   build the game as WebAssembly plus a static page in `target/web/`.
//! - `v86`:   build the 32-bit kernel + v86 emulator site in `target/v86/`
//!   (`--smoke` boots it headless in Node, `--demo` builds the autoplay kernel).
//! - `pages`: both browser builds assembled as the GitHub Pages site in `target/pages/`.
mod pages;
mod record;
mod v86;
mod web;

use bootloader::{BiosBoot, BootConfig};
use std::{
    env, fs,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::{Child, Command, ExitCode, Stdio},
    thread,
    time::{Duration, Instant},
};

const KERNEL_TARGET: &str = "x86_64-unknown-none";

pub(crate) type Result<T> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("image") => build_image().map(|p| println!("image: {}", p.display())),
        Some("run") => build_image().and_then(|p| run(&p, &args[1..])),
        Some("smoke") => build_image().and_then(|p| smoke(&p)),
        Some("web") => web::build().map(|dir| {
            println!("web: {} (serve it with: python3 -m http.server -d {})", dir.display(), dir.display())
        }),
        Some("v86") => v86::build(&args[1..]).map(|dir| {
            println!("v86: {} (serve it with: python3 -m http.server -d {})", dir.display(), dir.display())
        }),
        Some("pages") => pages::build().map(|dir| println!("pages: {}", dir.display())),
        Some("record") => build_image_with(&["autoplay"], "rust-runner-os-demo.img").and_then(|p| record::record(&p)),
        _ => Err("usage: cargo xtask <image|run [qemu args...]|smoke|record|web|v86 [--smoke] [--demo]|pages>".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

pub(crate) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

pub(crate) fn target_dir() -> PathBuf {
    env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| workspace_root().join("target"))
}

fn build_kernel(features: &[&str]) -> Result<PathBuf> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(workspace_root())
        .args(["build", "--package", "kernel", "--release", "--target", KERNEL_TARGET])
        .args(["--features", &features.join(",")])
        .status()
        .map_err(|e| format!("failed to run cargo: {e}"))?;
    if !status.success() {
        return Err("kernel build failed".into());
    }
    Ok(target_dir().join(KERNEL_TARGET).join("release").join("kernel"))
}

fn build_image() -> Result<PathBuf> {
    build_image_with(&[], "rust-runner-os-bios.img")
}

fn build_image_with(features: &[&str], name: &str) -> Result<PathBuf> {
    let kernel = build_kernel(features)?;
    let image = target_dir().join(name);
    let mut config = BootConfig::default();
    config.frame_buffer.minimum_framebuffer_width = Some(640);
    config.frame_buffer.minimum_framebuffer_height = Some(480);
    // Keep the bootloader's own log on serial only, so the screen goes straight to the game.
    config.frame_buffer_logging = false;
    BiosBoot::new(&kernel)
        .set_boot_config(&config)
        .create_disk_image(&image)
        .map_err(|e| format!("creating disk image: {e:#}"))?;
    Ok(image)
}

fn kvm_usable() -> bool {
    fs::OpenOptions::new().read(true).write(true).open("/dev/kvm").is_ok()
}

/// QEMU audio device id used by `-audiodev` and the monitor's `wavcapture`.
pub(crate) const AUDIODEV: &str = "snd0";

/// `audio` is a QEMU audio backend: `pa`, `pipewire`, `alsa`, `sdl`, `none`, ...
pub(crate) fn qemu(image: &Path, audio: &str) -> Command {
    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.args(["-machine", "pc", "-m", "256M", "-vga", "std", "-no-reboot"]);
    cmd.arg("-audiodev").arg(format!("{audio},id={AUDIODEV}"));
    cmd.arg("-device").arg(format!("AC97,audiodev={AUDIODEV}"));
    cmd.arg("-drive").arg(format!("format=raw,file={}", image.display()));
    cmd.args(["-accel", if kvm_usable() { "kvm" } else { "tcg" }]);
    cmd
}

fn run(image: &Path, extra: &[String]) -> Result<()> {
    // PulseAudio protocol works on both PulseAudio and PipeWire desktops.
    let audio = env::var("RUNNER_AUDIO").unwrap_or_else(|_| "pa".into());
    let mut cmd = qemu(image, &audio);
    cmd.args(["-serial", "stdio"]).args(extra);
    let status = cmd.status().map_err(|e| format!("failed to start qemu-system-x86_64 (is QEMU installed?): {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("qemu exited with {status}")) }
}

struct Smoke {
    child: Child,
    monitor: UnixStream,
    serial: PathBuf,
    dir: PathBuf,
}

impl Smoke {
    fn log(&self) -> String {
        fs::read_to_string(&self.serial).unwrap_or_default()
    }

    /// Waits until `needle` appears at least `count` times in the serial log.
    fn wait_for(&mut self, needle: &str, count: usize, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        loop {
            let log = self.log();
            if log.contains("KERNEL PANIC") {
                return Err(format!("kernel panicked:\n{log}"));
            }
            if log.matches(needle).count() >= count {
                println!("  ok: saw {needle:?} x{count}");
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(format!("timed out waiting for {needle:?} x{count}; serial log:\n{log}"));
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                return Err(format!("qemu exited early ({status}); serial log:\n{log}"));
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn command(&mut self, cmd: &str) -> Result<()> {
        writeln!(self.monitor, "{cmd}").map_err(|e| format!("monitor write: {e}"))?;
        // Give QEMU a moment and drain whatever it echoed back.
        thread::sleep(Duration::from_millis(200));
        let mut sink = [0u8; 4096];
        while let Ok(n) = self.monitor.read(&mut sink) {
            if n < sink.len() {
                break;
            }
        }
        Ok(())
    }

    fn screenshot(&mut self, name: &str) -> Result<()> {
        let path = self.dir.join(format!("{name}.png"));
        self.command(&format!("screendump {} -f png", path.display()))?;
        println!("  screenshot: {}", path.display());
        Ok(())
    }
}

impl Drop for Smoke {
    fn drop(&mut self) {
        let _ = writeln!(self.monitor, "quit");
        thread::sleep(Duration::from_millis(300));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn smoke(image: &Path) -> Result<()> {
    let dir = target_dir().join("smoke");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let serial = dir.join("serial.log");
    let socket = dir.join("monitor.sock");

    let mut cmd = qemu(image, "none");
    cmd.args(["-display", "none"])
        .arg("-serial")
        .arg(format!("file:{}", serial.display()))
        .arg("-monitor")
        .arg(format!("unix:{},server,nowait", socket.display()))
        .stdout(Stdio::null());
    let child = cmd.spawn().map_err(|e| format!("failed to start qemu-system-x86_64 (is QEMU installed?): {e}"))?;

    let deadline = Instant::now() + Duration::from_secs(10);
    let monitor = loop {
        match UnixStream::connect(&socket) {
            Ok(s) => break s,
            Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(format!("could not connect to QEMU monitor: {e}")),
        }
    };
    monitor.set_read_timeout(Some(Duration::from_millis(200))).map_err(|e| e.to_string())?;
    let mut s = Smoke { child, monitor, serial, dir };

    println!("smoke: booting");
    s.wait_for("Rust Runner OS started", 1, Duration::from_secs(30))?;
    s.wait_for("audio: AC97", 1, Duration::from_secs(10))?;
    s.wait_for("state: Title", 1, Duration::from_secs(10))?;
    thread::sleep(Duration::from_secs(1));
    s.screenshot("1-title")?;

    s.command("sendkey spc")?;
    s.wait_for("state: Playing", 1, Duration::from_secs(5))?;
    // The first hazards arrive after two empty bars (3.2 s): try every move first.
    s.command("sendkey down")?;
    s.wait_for("action: slide", 1, Duration::from_secs(3))?;
    s.screenshot("2a-sliding")?;
    thread::sleep(Duration::from_millis(400));
    s.command("sendkey right")?;
    s.wait_for("action: kick", 1, Duration::from_secs(3))?;
    s.screenshot("2b-kicking")?;
    thread::sleep(Duration::from_millis(300));
    s.command("sendkey spc")?;
    s.screenshot("2c-jumping")?;
    s.command("sendkey c")?;
    s.screenshot("2d-crt-off")?;
    s.command("sendkey c")?;
    thread::sleep(Duration::from_millis(600));
    s.screenshot("2e-first-hazards")?;

    // Nobody jumps, so the first obstacle ends the run.
    s.wait_for("state: GameOver", 1, Duration::from_secs(30))?;
    thread::sleep(Duration::from_millis(800));
    s.screenshot("3-game-over")?;

    s.command("sendkey spc")?;
    s.wait_for("state: Playing", 2, Duration::from_secs(5))?;
    s.wait_for("state: GameOver", 2, Duration::from_secs(30))?;
    thread::sleep(Duration::from_millis(800));
    s.command("sendkey r")?;
    s.wait_for("state: Playing", 3, Duration::from_secs(5))?;
    s.command("sendkey esc")?;
    s.wait_for("state: Title", 2, Duration::from_secs(5))?;
    s.screenshot("4-title-with-best")?;

    // Let the fps counter report at least once.
    s.wait_for("fps:", 1, Duration::from_secs(10))?;
    println!("smoke: PASS\n--- serial log ---\n{}", s.log());
    Ok(())
}

/// Standard base64 (RFC 4648, with padding), for embedding files in pages.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_matches_rfc4648() {
        for (input, expected) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foobar", "Zm9vYmFy")] {
            assert_eq!(super::base64(input.as_bytes()), expected);
        }
    }
}
