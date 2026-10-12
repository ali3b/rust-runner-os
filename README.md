# Rust Runner OS

A tiny bootable game machine: power on (in QEMU) and you land straight in a
Bit.Trip Runner–style side-scroller. No desktop, shell or guest OS: just a
`no_std` Rust kernel, a minimal hardware layer and the game.

**Play it now:** https://ali3b.github.io/rust-runner-os/ (keyboard, mouse or
touch). **Boot the whole OS in an emulated PC:** https://ali3b.github.io/rust-runner-os/os/

## Prerequisites

- Linux x86-64 host
- [rustup](https://rustup.rs). The pinned nightly toolchain, `rust-src`,
  `llvm-tools-preview` and the `x86_64-unknown-none` target install
  automatically from `rust-toolchain.toml` on first build.
- QEMU: `sudo apt install qemu-system-x86`
- Optional: KVM access (`sudo usermod -aG kvm $USER`, then log in again).
  Without it QEMU uses TCG, which still runs the game at 60 fps.

## Build and play

```sh
cargo xtask run      # build kernel + BIOS disk image, launch QEMU with a window
cargo xtask image    # just build target/rust-runner-os-bios.img
cargo xtask smoke    # headless boot test driven via the QEMU monitor
cargo xtask record   # record target/video/rust-runner-os.mp4 from power-on (needs ffmpeg)
cargo test           # host unit tests for the engine and game logic
```

Arguments after `run` go to QEMU, e.g. `cargo xtask run -display sdl`.
Sound goes through an emulated AC'97 card using QEMU's PulseAudio backend (works
on PipeWire desktops too). Pick another with `RUNNER_AUDIO`, e.g.
`RUNNER_AUDIO=alsa cargo xtask run`, or `RUNNER_AUDIO=none` for silence.
To launch an existing image by hand:

```sh
qemu-system-x86_64 -machine pc -m 256M -vga std -accel tcg \
  -audiodev pa,id=snd0 -device AC97,audiodev=snd0 \
  -drive format=raw,file=target/rust-runner-os-bios.img -serial stdio
```

`record` boots a demo build (`--features autoplay`) in which a bot plays
every move on the beat. It uses 6-bar zones, so one ~1-minute video shows all
four zones, then stops pressing keys so the video shows a crash. Start, retry and Esc
are sent as real key presses through QEMU's QMP interface. Short captions
("SPACE: run!", "10 gold: BOOST", "Crash! Game over", ...) are timed from the
serial log and burned in with ffmpeg `drawtext` (DejaVu Sans Bold).

Debug and panic messages go to the serial port (your terminal with
`-serial stdio`).

## Play it in a browser

There are two browser builds, both static sites you can put on any web host:

```sh
cargo xtask web          # target/web/rust-runner.html: the game as one HTML file
cargo xtask v86 --smoke  # target/v86: the whole OS, booted by the v86 PC emulator
cargo xtask pages        # target/pages: both, as the GitHub Pages site
python3 -m http.server -d target/v86   # then open http://localhost:8000
```

- **`web`** compiles `runner-core` to `wasm32` with a small JavaScript shim
  (canvas, Web Audio, keyboard and touch buttons). There's no emulator and no
  kernel, so it runs fast everywhere. `rust-runner.html` (~230 KiB) has the
  module embedded as base64, so that one file is the whole game: open it
  straight from disk or put it anywhere. It fills the window; on phones and
  tablets it adds on-screen JUMP / SLIDE / KICK buttons and goes fullscreen on
  the first tap. A click or tap on the game counts as jump, and F toggles
  fullscreen on desktop. The same folder also has
  `index.html` + `runner.wasm` for hosts that prefer separate files.
- **`v86`** boots the real OS. v86 only emulates 32-bit CPUs, so this uses a
  second kernel, `kernel-v86`: a 32-bit multiboot kernel built from a custom
  target spec. It drives the Bochs VGA card in 640×480 palette mode (each
  frame is a 300 KiB copy) and plays sound through a Sound Blaster 16 with
  auto-init ISA DMA, polled like the AC'97. The page shows SeaBIOS booting it
  and streams the kernel's COM1 serial log next to the screen. The v86
  emulator (0.5.462) and BIOS images are downloaded, pinned and checked by
  SHA-256 at build time. `--smoke` boots the site headless in Node and checks
  the game runs. The same kernel also boots in
  `qemu-system-i386 -kernel target/i686-runner/release/kernel-v86 -device sb16`.

## Controls

| Key                  | Action                                               |
|----------------------|------------------------------------------------------|
| Space / Up / W       | Start from title, jump, retry after game over        |
| Down / S             | Slide under beams (in the air: slam down)            |
| Right / D / K        | Kick through walls                                   |
| R                    | Retry after game over                                |
| C                    | Toggle CRT scanlines                                 |
| Esc                  | Back to title                                        |

## How to play

It's a rhythm runner at 150 BPM. Every hazard is placed so that pressing its
move **on the beat** clears it, and the music is driven by the game's own
clock, so the two never drift apart.

- **Blocks**: jump. **Beams** hanging from above: slide under them. **Walls**:
  too tall to jump, so kick through them.
- **Gold** comes in rows, in arcs over blocks (take the ideal jump to collect
  them all) and under beams. Every 10 gold in a run powers up the rainbow
  dust you shed (BOOST, TURBO, RAINBOW) and adds a layer to the music. The dust
  stays where you dropped it, so your jumps leave rainbow arcs hanging in the
  air. At 40 gold you enter
  **EXTRA** mode: a psychedelic sky, thicker dust with sparkles, an
  extra synth layer, and all points ×2.
- **Combo**: every gold and every hazard cleared extends your streak; at 10, 25
  and 50 your multiplier goes to ×2, ×4 and ×8. Missing a gold bar breaks it.
  A move within ±2 frames of the beat scores **PERFECT** for bonus points.
- **Power-ups**, one per zone: **Shield** (survive one hit), **Magnet** (pulls
  in gold for 8 s) and **Slow-mo** (5 s at 0.6× speed; the music slows down
  with it).
- **Zones**: Neon City → Sunset Desert → Deep Space → Hyper Grid, about 25 s
  each and faster each time (200 → 320 px/s). Each zone has its own scenery,
  colours and song. After Hyper Grid you start lap 2, 10 % faster.

Points come from distance, gold (5), clears (10) and PERFECTs (25), each
multiplied by your combo. One press gives one move: holding a key never
repeats it. Retry keys are ignored for 0.4 s after a crash. The best score
lasts for the boot session.

## Layout

```
kernel/        64-bit kernel for QEMU: bootloader entry, RGB framebuffer
               scaler, AC'97 setup
kernel-v86/    32-bit multiboot kernel for v86 (custom target + link.ld)
pc/            hardware shared by both kernels: serial, PS/2 keyboard,
               TSC timer calibrated via PIT, PCI, AC'97, Sound Blaster 16,
               Bochs VGA, plus the game loop itself
web/           the game as WebAssembly (web/src) and the two browser pages
               (web/www, web/v86)
runner-core/   no_std library, unit-tested on the host:
  engine/      canvas, font, input latch, AABB, fixed timestep, PRNG,
               particles
  game/        state (world clock, spawning, scoring), player moves,
               hazards, pickups, beat patterns, zones, combo, autopilot,
               tuning
  game/render/ palette (per-zone, fades, EXTRA cycling), scenery for each
               zone, actors (hero, hazards, gold, items), HUD
  audio/       integer chiptune synth: per-zone songs, voices, effects
xtask/         host tool: disk image, QEMU run/smoke/record, web and v86
               builds
```

The game logic lives in its own crate (`runner-core`) instead of inside
`kernel/` so that `cargo test` runs its tests on the host with `std`.

## Technical notes

- **Bootloader:** `bootloader` / `bootloader_api` 0.11.17, BIOS boot. Its
  BIOS stage caps VESA modes at 1280×720, so the 640×480 playfield is scaled
  nearest-neighbour to 960×720 (1.5×) and centred. Other framebuffer sizes,
  strides and pixel formats (RGB/BGR/grey/bitmask) are handled; anything
  smaller than 640×480 panics with a serial message.
- **No interrupts:** the keyboard is polled through the i8042, and time comes
  from the TSC, calibrated once against PIT channel 2. Simulation runs at a
  fixed 60 Hz with a 250 ms frame clamp and a 5-step catch-up cap.
- **Audio:** an AC'97 driver finds the card on PCI bus 0, then streams 48 kHz
  stereo through a ring of 32 DMA buffers (5 ms each). The main loop keeps 10
  buffers queued, about 50 ms of latency, with no interrupts. The DMA memory
  comes from the boot memory map through the bootloader's physical-memory
  mapping. Music and sound effects come from an integer-only synth (the kernel
  target has no hardware floats): pulse, triangle and noise voices playing a
  120 BPM loop in A minor. Drums and bass play at level 0; arpeggios, lead and
  harmony join as the trail levels up. If no card is found, the game runs
  silently and logs why.
- **Beat-exact spawning:** world time is kept in 1/256 frames, and each bar's
  pattern is spawned 2 bars ahead. Each object is placed exactly as far away
  as the world will scroll before its beat, following the per-zone speed
  schedule (`zones::travel_between`), so hazards arrive on the beat even across
  speed changes and slow-mo.
- **Fairness tests:** pattern rules (moves never overlap, gold rows are
  reachable) are checked statically. A bot then plays every zone for two laps
  on many seeds: perfectly, and with presses ±2 frames off the beat. Every run
  must survive.
- **CRT:** scanlines darken the output rows that repeat a source row, using a
  second, dimmed palette lookup table, so the effect costs nothing per pixel.
- **Perf line:** every 5 s the serial log prints fps plus µs per frame for
  audio, update, render and present. Under TCG, present (the scaled blit) takes
  about 5–8 ms and render 1–2.5 ms.
- Tuning values live in `runner-core/src/game/tuning.rs`.

## Troubleshooting

- *Build fails inside the `bootloader` build script:* make sure rustup
  installed the pinned nightly with `rust-src` and `llvm-tools-preview` (run
  `rustup show` in the repo).
- *Black screen, nothing on serial:* check QEMU is using `-machine pc` and a
  raw drive (`format=raw`).
- *Game feels slow:* check the `fps:` lines on serial; enable KVM if they're
  well under 60.

## License

Rust Runner OS is licensed under either of [Apache License 2.0](LICENSE-APACHE)
or [MIT](LICENSE-MIT), at your option. The browser build of the OS also ships
the v86 emulator (BSD-2-Clause) and SeaBIOS (LGPLv3); they are downloaded at
build time, not stored in this repository (see `target/pages/os/THIRD-PARTY.txt`).
