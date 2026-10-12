// Headless check of the v86 build: boot the kernel in v86 under Node, press
// Space, and wait for the game to start and end. Usage:
//   node smoke.mjs <v86 package dir> <site dir>
import fs from "node:fs";
const [pkg, site] = process.argv.slice(2);
const { V86 } = await import(pkg + "/build/libv86.mjs");
const file = (name) => {
  const b = fs.readFileSync(`${site}/${name}`);
  return { buffer: b.buffer.slice(b.byteOffset, b.byteOffset + b.length) };
};
const emu = new V86({
  wasm_path: `${pkg}/build/v86.wasm`,
  bios: file("seabios.bin"),
  vga_bios: file("vgabios.bin"),
  multiboot: file("rust-runner-os.elf"),
  memory_size: 64 << 20,
  vga_memory_size: 8 << 20,
  autostart: true,
  disable_speaker: true,
});
let serial = "";
emu.add_listener("serial0-output-byte", (b) => { serial += String.fromCharCode(b); });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function expect(text, seconds, count = 1) {
  for (let i = 0; i < seconds * 10; i++) {
    if (serial.split(text).length - 1 >= count) return console.log(`  ok: ${text}`);
    if (serial.includes("KERNEL PANIC")) break;
    await sleep(100);
  }
  console.log(serial);
  console.log(`FAIL: no "${text}"`);
  process.exit(1);
}
await expect("Rust Runner OS started", 30);
await expect("display: Bochs VBE", 5);
await expect("audio: SB16", 5);
await expect("state: Title", 5);
await sleep(500);
emu.keyboard_send_scancodes([0x39, 0xb9]); // Space
await expect("state: Playing", 5);
emu.keyboard_send_scancodes([0xe0, 0x50, 0xe0, 0xd0]); // Down
await expect("action: slide", 5);
await expect("state: GameOver", 30);
await expect("fps:", 10);
const fps = [...serial.matchAll(/fps: (\d+)/g)].map((m) => Number(m[1]));
console.log(`  fps samples: ${fps.join(", ")}`);
emu.stop();
console.log("v86 smoke: PASS");
process.exit(0);
