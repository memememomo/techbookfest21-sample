// mynes の Web 版: WebAssembly にしたエミュレーターを動かして、Canvas に描く
import init, { WebNes } from "./pkg/mynes.js";

const canvas = document.getElementById("screen");
const ctx = canvas.getContext("2d");
const image = ctx.createImageData(256, 240);
const status = document.getElementById("status");

// ファミコン（NTSC）は 1 秒間に約 60.0988 フレーム
const FRAME_MS = 1000 / 60.0988;

let nes = null;
let lastTime = 0;
let pending = 0; // まだ進めていない時間（ミリ秒）

// WebAssembly を読み込んで初期化する
await init();

document.getElementById("rom").addEventListener("change", async (event) => {
  const file = event.target.files[0];
  if (!file) return;
  const data = new Uint8Array(await file.arrayBuffer());
  try {
    nes = new WebNes(data);
    status.textContent = `${file.name} を読み込みました`;
  } catch (error) {
    nes = null;
    status.textContent = `読み込めませんでした: ${error}`;
  }
});

document.getElementById("reset").addEventListener("click", () => nes?.reset());

function loop(now) {
  if (nes) {
    // 経過した時間の分だけフレームを進める（モニターが 60Hz でなくても同じ速さになる）
    pending += Math.min(now - lastTime, 100);
    while (pending >= FRAME_MS) {
      nes.run_frame();
      pending -= FRAME_MS;
    }
    image.data.set(nes.frame());
    ctx.putImageData(image, 0, 0);
  }
  lastTime = now;
  requestAnimationFrame(loop);
}

requestAnimationFrame((now) => {
  lastTime = now;
  loop(now);
});
