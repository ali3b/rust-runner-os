//! `cargo xtask record`: boots the demo image headless and records an MP4 from the
//! very first BIOS frame. Frames are grabbed with QMP `screendump` at a fixed rate
//! and piped to ffmpeg; start/retry/title are sent as real key presses via QMP.
use crate::{AUDIODEV, Result, qemu, target_dir};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const FPS: u32 = 30;
const CAPTION_FONT: &str = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf";
/// A caption stays up this long, or until the next one replaces it.
const CAPTION_SECONDS: f64 = 2.5;

/// Captions never show for less than this (later ones queue up behind).
const CAPTION_MIN_SECONDS: f64 = 1.2;

/// The caption for a serial log line, if it deserves one. `shown` holds the
/// captions so far, for one-time captions.
fn serial_caption(line: &str, shown: &[(f64, String)]) -> Option<String> {
    let once = |text: &str| (!shown.iter().any(|(_, t)| t == text)).then(|| text.to_string());
    let rest = |prefix: &str| line.strip_prefix(prefix).map(str::trim);
    if let Some(level) = rest("level:") {
        let level = level.split_whitespace().next()?;
        return Some(
            match level {
                "1" => "10 gold: BOOST",
                "2" => "20 gold: TURBO",
                "3" => "30 gold: RAINBOW",
                _ => "40 gold: EXTRA MODE, score x2",
            }
            .to_string(),
        );
    }
    if let Some(zone) = rest("zone:") {
        // "2 SUNSET DESERT lap=1"
        let (number, name) = zone.split_once(' ')?;
        let name = name.split(" lap=").next()?;
        return Some(format!("ZONE {number}: {name}"));
    }
    if let Some(power) = rest("powerup:") {
        return Some(format!("Power-up: {power}"));
    }
    if let Some(combo) = rest("combo:") {
        return Some(format!("{} COMBO", combo.split_whitespace().next()?));
    }
    match line.trim() {
        "shield: saved" => Some("Shield saved the run!".into()),
        "action: slide" => once("DOWN: slide under beams"),
        "action: kick wall" => once("RIGHT: kick through walls"),
        l if l.starts_with("state: GameOver") => Some("Crash! Game over".into()),
        // Later title visits are covered by the ESC caption.
        l if l.starts_with("state: Title") => once("Title screen"),
        _ => None,
    }
}

/// Caption for a scripted key press.
fn key_caption(qcode: &str) -> &'static str {
    match qcode {
        "spc" => "SPACE: run!",
        "r" => "R: retry",
        "esc" => "ESC: title, best kept",
        _ => "",
    }
}
const MAX_SECONDS: u64 = 240;

struct Qmp {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
}

impl Qmp {
    fn connect(path: &Path) -> Result<Self> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let stream = loop {
            match UnixStream::connect(path) {
                Ok(s) => break s,
                Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
                Err(e) => return Err(format!("could not connect to QMP: {e}")),
            }
        };
        let reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let mut qmp = Qmp { writer: stream, reader };
        qmp.read_reply()?; // greeting
        qmp.execute(r#"{"execute":"qmp_capabilities"}"#)?;
        Ok(qmp)
    }

    /// Reads lines until a command reply (skipping asynchronous events).
    fn read_reply(&mut self) -> Result<String> {
        loop {
            let mut line = String::new();
            if self.reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                return Err("QMP connection closed".into());
            }
            if !line.contains("\"event\"") {
                return Ok(line);
            }
        }
    }

    fn execute(&mut self, cmd: &str) -> Result<String> {
        writeln!(self.writer, "{cmd}").map_err(|e| e.to_string())?;
        let reply = self.read_reply()?;
        if reply.contains("\"error\"") {
            return Err(format!("QMP error for {cmd}: {reply}"));
        }
        Ok(reply)
    }

    fn key(&mut self, qcode: &str) -> Result<()> {
        println!("  key: {qcode}");
        self.execute(&format!(
            r#"{{"execute":"send-key","arguments":{{"keys":[{{"type":"qcode","data":"{qcode}"}}],"hold-time":120}}}}"#
        ))
        .map(|_| ())
    }
}

/// Scripted key presses, each fired `delay` after `needle` appears `count` times on serial.
struct Cue {
    needle: &'static str,
    count: usize,
    delay: Duration,
    key: Option<&'static str>,
}

const SCRIPT: &[Cue] = &[
    Cue { needle: "state: Title", count: 1, delay: Duration::from_millis(2000), key: Some("spc") },
    Cue { needle: "state: GameOver", count: 1, delay: Duration::from_millis(2000), key: Some("r") },
    Cue { needle: "state: GameOver", count: 2, delay: Duration::from_millis(1500), key: Some("esc") },
    // Linger on the title (now showing the session best), then stop recording.
    Cue { needle: "state: Title", count: 2, delay: Duration::from_millis(2500), key: None },
];

pub fn record(image: &Path) -> Result<()> {
    let dir = target_dir().join("video");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let serial = dir.join("serial.log");
    let socket = dir.join("qmp.sock");
    let frame = dir.join("frame.ppm");
    let silent = dir.join("video-only.mp4");
    let wav = dir.join("audio.wav");
    let out = dir.join("rust-runner-os.mp4");

    let mut cmd = qemu(image, "none");
    cmd.args(["-display", "none", "-S"]) // start paused so we catch the first frame
        .arg("-serial")
        .arg(format!("file:{}", serial.display()))
        .arg("-qmp")
        .arg(format!("unix:{},server,nowait", socket.display()))
        .stdout(Stdio::null());
    let mut qemu = cmd.spawn().map_err(|e| format!("failed to start qemu-system-x86_64: {e}"))?;
    let mut qmp = Qmp::connect(&socket)?;

    // Frames change size during boot (VGA text mode -> VESA), so scale each to 720p.
    let mut ffmpeg = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-f", "image2pipe", "-c:v", "ppm"])
        .args(["-framerate", &FPS.to_string(), "-i", "-"])
        .args([
            "-vf",
            "scale=1280:720:force_original_aspect_ratio=decrease:flags=neighbor,pad=1280:720:(ow-iw)/2:(oh-ih)/2,format=yuv420p",
        ])
        .args(["-c:v", "libx264", "-preset", "medium", "-crf", "18", "-movflags", "+faststart"])
        .arg(&silent)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to start ffmpeg: {e}"))?;
    let mut sink = ffmpeg.stdin.take().unwrap();

    // Record the sound card's output from power-on, in sync with the video.
    qmp.execute(&format!(
        r#"{{"execute":"human-monitor-command","arguments":{{"command-line":"wavcapture {} {AUDIODEV} 48000 16 2"}}}}"#,
        wav.display()
    ))?;
    qmp.execute(r#"{"execute":"cont"}"#)?;
    println!("record: capturing at {FPS} fps from power-on");
    let start = Instant::now();
    let frame_time = Duration::from_secs(1) / FPS;
    let screendump =
        format!(r#"{{"execute":"screendump","arguments":{{"filename":"{}","format":"ppm"}}}}"#, frame.display());

    let mut written: u64 = 0;
    let mut cue = 0;
    let mut cue_ready_at: Option<Instant> = None;
    let mut last = Vec::new();
    let mut captions: Vec<(f64, String)> = vec![(0.0, "Power on: BIOS + bootloader".into())];
    let mut serial_seen = 0;
    let result = loop {
        if start.elapsed() > Duration::from_secs(MAX_SECONDS) {
            break Err(format!("gave up after {MAX_SECONDS}s"));
        }
        // Grab the current screen. Before the VGA BIOS has drawn anything this can fail;
        // in that case repeat the previous frame.
        if qmp.execute(&screendump).is_ok()
            && let Ok(bytes) = fs::read(&frame) {
                last = bytes;
            }
        // Keep real-time pacing: duplicate the frame if a capture took longer than 1/FPS.
        let due = start.elapsed().as_nanos() as u64 / frame_time.as_nanos() as u64 + 1;
        if !last.is_empty() {
            while written < due {
                if sink.write_all(&last).is_err() {
                    break;
                }
                written += 1;
            }
        }

        let log = fs::read_to_string(&serial).unwrap_or_default();
        if log.contains("KERNEL PANIC") {
            break Err(format!("kernel panicked:\n{log}"));
        }
        // Caption new, complete serial lines at the current video time.
        let now = start.elapsed().as_secs_f64();
        if let Some(end) = log.rfind('\n') {
            for line in log.get(serial_seen..end).unwrap_or("").lines() {
                if let Some(text) = serial_caption(line, &captions) {
                    captions.push((now, text));
                }
            }
            serial_seen = end.max(serial_seen);
        }
        if let Some(c) = SCRIPT.get(cue) {
            if cue_ready_at.is_none() && log.matches(c.needle).count() >= c.count {
                cue_ready_at = Some(Instant::now() + c.delay);
            }
            if cue_ready_at.is_some_and(|t| Instant::now() >= t) {
                match c.key {
                    Some(key) => {
                        qmp.key(key)?;
                        captions.push((start.elapsed().as_secs_f64(), key_caption(key).to_string()));
                    }
                    None => break Ok(()),
                }
                cue += 1;
                cue_ready_at = None;
            }
        }

        let next = start + frame_time * (written as u32);
        if let Some(wait) = next.checked_duration_since(Instant::now()) {
            thread::sleep(wait);
        }
    };

    drop(sink);
    // Finalises the WAV header.
    let _ = qmp.execute(r#"{"execute":"human-monitor-command","arguments":{"command-line":"stopcapture 0"}}"#);
    let _ = qmp.execute(r#"{"execute":"quit"}"#);
    let _ = qemu.wait();
    let status = ffmpeg.wait().map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&frame);
    result?;
    if !status.success() {
        return Err(format!("ffmpeg failed ({status})"));
    }
    // QEMU only writes audio once the guest starts playback, while the video starts
    // at power-on; both stop together, so pad the audio's start by the difference.
    let video_secs = written as f64 / FPS as f64;
    let wav_bytes = fs::metadata(&wav).map_err(|e| format!("no audio captured: {e}"))?.len().saturating_sub(44);
    let audio_secs = wav_bytes as f64 / (48_000.0 * 4.0);
    let lead_ms = ((video_secs - audio_secs) * 1000.0).max(0.0).round() as u64;
    println!("record: audio {audio_secs:.2}s, video {video_secs:.2}s, delaying audio {lead_ms} ms");
    let filter = dir.join("captions.txt");
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-loglevel", "error", "-y", "-i"]).arg(&silent).arg("-i").arg(&wav);
    cmd.args(["-af", &format!("adelay={lead_ms}:all=1")]);
    if Path::new(CAPTION_FONT).exists() {
        fs::write(&filter, caption_filter(&captions, video_secs)).map_err(|e| e.to_string())?;
        cmd.arg("-filter_script:v").arg(&filter);
        cmd.args(["-c:v", "libx264", "-preset", "medium", "-crf", "18"]);
    } else {
        println!("record: {CAPTION_FONT} not found, skipping captions");
        cmd.args(["-c:v", "copy"]);
    }
    let status = cmd
        .args(["-c:a", "aac", "-b:a", "192k", "-shortest", "-movflags", "+faststart"])
        .arg(&out)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("muxing audio failed ({status})"));
    }
    println!(
        "record: {} frames ({:.1}s) -> {}\n--- serial log ---\n{}",
        written,
        written as f64 / FPS as f64,
        out.display(),
        fs::read_to_string(&serial).unwrap_or_default()
    );
    Ok(())
}

/// ffmpeg `drawtext` chain showing each caption until the next one (or a timeout),
/// bottom-centre in a translucent box, plus a bottom-left note that jumps are automated.
fn caption_filter(captions: &[(f64, String)], video_secs: f64) -> String {
    // Queue captions that arrive in a burst so each is readable.
    let mut timed: Vec<(f64, &str)> = Vec::new();
    for (t, text) in captions {
        let t = timed.last().map_or(*t, |&(prev, _)| t.max(prev + CAPTION_MIN_SECONDS));
        if t < video_secs {
            timed.push((t, text));
        }
    }
    let captions = &timed;
    let style = format!("fontfile={CAPTION_FONT}:fontcolor=white:box=1:boxcolor=black@0.6:boxborderw=14");
    let mut parts = vec![format!(
        "drawtext={style}:fontsize=20:x=24:y=h-th-24:text='demo\\: autopilot jumps'"
    )];
    for (i, &(t, text)) in captions.iter().enumerate() {
        let next = captions.get(i + 1).map_or(video_secs, |&(n, _)| n);
        let end = next.min(t + CAPTION_SECONDS);
        // Escape for drawtext: colons and quotes would end the option.
        let text = text.replace(':', "\\:").replace('\'', "");
        parts.push(format!(
            "drawtext={style}:fontsize=40:x=(w-tw)/2:y=h-th-48:text='{text}':enable='between(t,{t:.2},{end:.2})'"
        ));
    }
    parts.join(",\n")
}
