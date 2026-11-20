// ROM ビューアー: mynes でデモ ROM を動かしながら、PPU の中身を絵と表で見せる
import init, { WebViewer } from "./pkg/viewer.js";

const $ = (id) => document.getElementById(id);
const hex = (n, digits = 2) => "$" + n.toString(16).toUpperCase().padStart(digits, "0");
const bin = (n) => "%" + n.toString(2).padStart(8, "0");

// ファミコン（NTSC）は 1 秒間に約 60.0988 フレーム
const FRAME_MS = 1000 / 60.0988;

let viewer = null;
let paused = false;
let lastTime = 0;
let pending = 0;
let dirty = false;

// いま見ているもの（マウスを乗せた場所）
let hoverNt = null; // { nt, col, row }：ネームテーブルのタイル
let hoverSprite = null; // OAM の番号
let tileFocus = null; // { table, tile, palette }：詳しく表示するタイル
let pinned = false; // tileFocus をクリックで固定したか

// フレームごとに読み直す PPU の中身
const state = {
  chr: null,
  nametables: null,
  palette: null,
  oam: null,
  ctrl: 0,
  mask: 0,
  system: null,
};

await init();

// ---------------------------------------------------------------- ROM の読み込み

async function loadRom(name, data) {
  try {
    viewer = new WebViewer(data);
  } catch (error) {
    viewer = null;
    $("status").textContent = `読み込めませんでした: ${error}`;
    return;
  }
  $("status").textContent = `${name} を動かしています`;
  for (const id of ["pause", "step", "reset"]) $(id).disabled = false;
  paused = false;
  $("pause").textContent = "一時停止";
  state.chr = viewer.chr();
  state.system = viewer.system_palette();
  pinned = false;
  tileFocus = null;
  showRomInfo();
  dirty = true;
}

for (const button of document.querySelectorAll("button[data-rom]")) {
  button.addEventListener("click", async () => {
    const name = button.dataset.rom;
    const response = await fetch(`../roms/${name}`).catch(() => null);
    if (!response || !response.ok) {
      $("status").textContent =
        `../roms/${name} を読めません。viewer ディレクトリでサーバーを起動したか確かめてください`;
      return;
    }
    loadRom(name, new Uint8Array(await response.arrayBuffer()));
  });
}

$("rom").addEventListener("change", async (event) => {
  const file = event.target.files[0];
  if (file) loadRom(file.name, new Uint8Array(await file.arrayBuffer()));
});

$("pause").addEventListener("click", () => {
  paused = !paused;
  $("pause").textContent = paused ? "再開" : "一時停止";
});

$("step").addEventListener("click", () => {
  paused = true;
  $("pause").textContent = "再開";
  viewer.set_buttons(buttons);
  viewer.run_frame();
  dirty = true;
});

$("reset").addEventListener("click", () => {
  viewer.reset();
  dirty = true;
});

for (const el of document.querySelectorAll("input[type=checkbox], select")) {
  el.addEventListener("change", () => (dirty = true));
}

// ---------------------------------------------------------------- コントローラー

// キーボードのキー → コントローラーのボタン（ビット 0 から A, B, SELECT, START, 上, 下, 左, 右）
const KEYS = {
  KeyX: 0x01,
  KeyZ: 0x02,
  ShiftRight: 0x04,
  Enter: 0x08,
  ArrowUp: 0x10,
  ArrowDown: 0x20,
  ArrowLeft: 0x40,
  ArrowRight: 0x80,
};
let buttons = 0;

window.addEventListener("keydown", (event) => {
  const bit = KEYS[event.code];
  if (bit && event.target.tagName !== "SELECT") {
    buttons |= bit;
    event.preventDefault();
  }
});

window.addEventListener("keyup", (event) => {
  const bit = KEYS[event.code];
  if (bit) {
    buttons &= ~bit;
    event.preventDefault();
  }
});

// ---------------------------------------------------------------- 色

/** 色番号（$00〜$3F）を CSS の色に */
function css(index) {
  const i = (index & 0x3f) * 3;
  const s = state.system;
  return `rgb(${s[i]}, ${s[i + 1]}, ${s[i + 2]})`;
}

/** パレット RAM の位置 0〜31 の色番号（$3F10 などは $3F00 などの写し） */
function paletteEntry(i) {
  return i >= 16 && i % 4 === 0 ? state.palette[i - 16] : state.palette[i];
}

/** パレット p（0〜7）の色 c（0〜3）の色番号。色 0 は背景色 */
function paletteColor(p, c) {
  return c === 0 ? state.palette[0] : paletteEntry(p * 4 + c);
}

/** 灰色で描くときの色 */
function gray(c) {
  const v = c * 85;
  return `rgb(${v}, ${v}, ${v})`;
}

function colorCss(palette, c) {
  return palette < 0 ? gray(c) : css(paletteColor(palette, c));
}

/** RGBA の配列を、拡大してキャンバスに描く */
const scratch = document.createElement("canvas");
function blit(ctx, rgba, w, h, dw, dh) {
  scratch.width = w;
  scratch.height = h;
  scratch.getContext("2d").putImageData(new ImageData(new Uint8ClampedArray(rgba), w, h), 0, 0);
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(scratch, 0, 0, dw, dh);
}

function strokeRect(ctx, x, y, w, h, color, width = 2) {
  ctx.strokeStyle = color;
  ctx.lineWidth = width;
  ctx.strokeRect(x + width / 2, y + width / 2, w - width, h - width);
}

function grid(ctx, w, h, step, color, width = 1, dash = []) {
  ctx.strokeStyle = color;
  ctx.lineWidth = width;
  ctx.setLineDash(dash);
  ctx.beginPath();
  for (let x = step; x < w; x += step) {
    ctx.moveTo(x + 0.5, 0);
    ctx.lineTo(x + 0.5, h);
  }
  for (let y = step; y < h; y += step) {
    ctx.moveTo(0, y + 0.5);
    ctx.lineTo(w, y + 0.5);
  }
  ctx.stroke();
  ctx.setLineDash([]);
}

function label(ctx, text, x, y, color = "#fff") {
  ctx.font = "bold 13px ui-monospace, Menlo, Consolas, monospace";
  const w = ctx.measureText(text).width + 8;
  ctx.fillStyle = "#000b";
  ctx.fillRect(x, y, w, 18);
  ctx.fillStyle = color;
  ctx.fillText(text, x + 4, y + 13);
}

// ---------------------------------------------------------------- マウスの横に出すアドレス

const tip = $("tip");

/** rows: [[アドレス, 説明], ...] をマウスの右下に出す */
function showTip(event, rows) {
  tip.innerHTML = rows.map(([addr, text]) => `<div class="tip-row"><b>${addr}</b><span>${text}</span></div>`).join("");
  tip.hidden = false;
  // 画面の端ではみ出さないように、左や上に回り込ませる
  const pad = 16;
  const w = tip.offsetWidth;
  const h = tip.offsetHeight;
  let x = event.clientX + pad;
  let y = event.clientY + pad;
  if (x + w > window.innerWidth - 4) x = event.clientX - pad - w;
  if (y + h > window.innerHeight - 4) y = event.clientY - pad - h;
  tip.style.left = `${Math.max(4, x)}px`;
  tip.style.top = `${Math.max(4, y)}px`;
}

function hideTip() {
  tip.hidden = true;
}

/** ネームテーブルのタイルについて、ネームテーブルと属性テーブルのアドレス */
function ntTipRows(pos) {
  const base = 0x2000 + pos.nt * 0x400;
  const addr = base + pos.row * 32 + pos.col;
  const attr = attributeOf(pos);
  return [
    [hex(addr, 4), `ネームテーブル（${hex(base, 4)} の ${pos.col} 列 ${pos.row} 行）→ タイル ${hex(ntTile(pos))}`],
    [hex(attr.addr, 4), `属性テーブル（ビット ${attr.shift + 1}-${attr.shift}）→ 背景パレット ${attr.palette}`],
  ];
}

const ACCENT = "#ffcc33";
const HOVER = "#33ddff";

// ---------------------------------------------------------------- ROM の情報・レジスタ

function rows(table, items) {
  table.innerHTML = items.map(([k, v]) => `<tr><th>${k}</th><td>${v}</td></tr>`).join("");
}

function showRomInfo() {
  const mirroring = viewer.mirroring() === "vertical" ? "垂直（左右に並べる）" : "水平（上下に並べる）";
  rows($("rom-info"), [
    ["PRG ROM", `${viewer.prg_size() / 1024} KiB`],
    ["CHR ROM", `${viewer.chr_size() / 1024} KiB`],
    ["ミラーリング", mirroring],
    ["NMI ベクタ", `$FFFA → ${hex(viewer.read_u16(0xfffa), 4)}`],
    ["RESET ベクタ", `$FFFC → ${hex(viewer.read_u16(0xfffc), 4)}`],
    ["IRQ ベクタ", `$FFFE → ${hex(viewer.read_u16(0xfffe), 4)}`],
  ]);
}

function showRegisters() {
  const c = state.ctrl;
  const m = state.mask;
  const onOff = (bit) => (bit ? "オン" : "オフ");
  rows($("regs"), [
    ["フレーム", viewer.frames()],
    ["PPUCTRL ($2000)", bin(c)],
    ["　NMI", onOff(c & 0x80)],
    ["　背景のパターン", c & 0x10 ? "$1000" : "$0000"],
    ["　スプライトのパターン", c & 0x08 ? "$1000" : "$0000"],
    ["　$2007 の増え方", c & 0x04 ? "32（下へ）" : "1（右へ）"],
    ["　ネームテーブル", hex(0x2000 + (c & 3) * 0x400, 4)],
    ["PPUMASK ($2001)", bin(m)],
    ["　背景を表示", onOff(m & 0x08)],
    ["　スプライトを表示", onOff(m & 0x10)],
  ]);
}

// ---------------------------------------------------------------- パレット

function showPalettes() {
  let html = "";
  for (const [title, start] of [
    ["背景", 0],
    ["スプライト", 4],
  ]) {
    html += `<div class="palette-group">`;
    for (let p = start; p < start + 4; p++) {
      html += `<div class="palette"><div class="label">${title} ${p - start}</div><div class="swatches">`;
      for (let c = 0; c < 4; c++) {
        const i = p * 4 + c;
        const value = paletteEntry(i);
        const transparent = c === 0 && i !== 0;
        html +=
          `<div class="swatch" data-addr="${0x3f00 + i}" data-value="${value}">` +
          `<div class="chip${transparent ? " transparent" : ""}" style="background-color:${css(value)}"></div>` +
          `<div class="addr">${hex(0x3f00 + i, 4).slice(3)}</div><div>${hex(value)}</div></div>`;
      }
      html += `</div></div>`;
    }
    html += `</div>`;
  }
  $("palettes").innerHTML = html;
}

$("palettes").addEventListener("mousemove", (event) => {
  const swatch = event.target.closest(".swatch");
  if (!swatch) return hideTip();
  const addr = Number(swatch.dataset.addr);
  const i = addr - 0x3f00;
  const kind = i < 16 ? `背景パレット ${i >> 2}` : `スプライトパレット ${(i - 16) >> 2}`;
  const note = i >= 16 && i % 4 === 0 ? `（${hex(addr - 0x10, 4)} と同じ場所）` : "";
  showTip(event, [[hex(addr, 4), `${kind} の色 ${i & 3}${note} = 色番号 ${hex(Number(swatch.dataset.value))}`]]);
});
$("palettes").addEventListener("mouseleave", hideTip);

// ---------------------------------------------------------------- パターンテーブル

const PT_SCALE = 3; // 128 × 128 → 384 × 384

function drawPatternTables() {
  const selected = Number($("pattern-palette").value);
  const bgTable = state.ctrl & 0x10 ? 0x1000 : 0;
  const spTable = state.ctrl & 0x08 ? 0x1000 : 0;
  // 強調するタイル: マウスを乗せたネームテーブルのタイル、スプライトのタイル
  const marks = [];
  if (hoverNt) marks.push([bgTable, ntTile(hoverNt), HOVER]);
  if (hoverSprite !== null) marks.push([spTable, state.oam[hoverSprite * 4 + 1], HOVER]);
  if (tileFocus) marks.push([tileFocus.table, tileFocus.tile, pinned ? ACCENT : HOVER]);

  document.querySelectorAll("canvas.pattern").forEach((canvas, n) => {
    const table = Number(canvas.dataset.table);
    const ctx = canvas.getContext("2d");
    blit(ctx, viewer.pattern_table_image(table, selected), 128, 128, 384, 384);
    if ($("pattern-grid").checked) grid(ctx, 384, 384, 8 * PT_SCALE, "#ffffff30");
    for (const [t, tile, color] of marks) {
      if (t !== table) continue;
      const s = 8 * PT_SCALE;
      strokeRect(ctx, (tile % 16) * s - 2, Math.floor(tile / 16) * s - 2, s + 4, s + 4, color, 2);
    }
    const uses = [];
    if (bgTable === table) uses.push("背景");
    if (spTable === table) uses.push("スプライト");
    $(`pattern-caption-${n}`).textContent =
      `${hex(table, 4)}〜${hex(table + 0xfff, 4)}` + (uses.length ? `（${uses.join("・")}が使用中）` : "（未使用）");
  });
}

function patternTileAt(canvas, event) {
  const rect = canvas.getBoundingClientRect();
  const x = Math.floor(((event.clientX - rect.left) / rect.width) * 16);
  const y = Math.floor(((event.clientY - rect.top) / rect.height) * 16);
  if (x < 0 || x > 15 || y < 0 || y > 15) return null;
  return { table: Number(canvas.dataset.table), tile: y * 16 + x, palette: Number($("pattern-palette").value) };
}

for (const canvas of document.querySelectorAll("canvas.pattern")) {
  canvas.addEventListener("mousemove", (event) => {
    if (!viewer) return;
    const hit = patternTileAt(canvas, event);
    if (!hit) return hideTip();
    const base = hit.table + hit.tile * 16;
    showTip(event, [
      [`${hex(base, 4)}〜${hex(base + 15, 4)}`, `タイル ${hex(hit.tile)}（${hit.tile} 番目）`],
      [`${hex(base, 4)}〜${hex(base + 7, 4)}`, "下位ビット"],
      [`${hex(base + 8, 4)}〜${hex(base + 15, 4)}`, "上位ビット"],
    ]);
    if (pinned) return;
    tileFocus = hit;
    dirty = true;
  });
  canvas.addEventListener("mouseleave", hideTip);
  canvas.addEventListener("click", (event) => {
    if (!viewer) return;
    const hit = patternTileAt(canvas, event);
    if (pinned && tileFocus && hit && hit.table === tileFocus.table && hit.tile === tileFocus.tile) {
      pinned = false;
    } else {
      tileFocus = hit;
      pinned = true;
    }
    dirty = true;
  });
}

/** 1 バイトを、1 ビットずつ色を変えて表示する */
function bitSpans(byte) {
  let html = "";
  for (let b = 7; b >= 0; b--) {
    const v = (byte >> b) & 1;
    html += `<span class="b${v}">${v}</span>`;
  }
  return html;
}

function showTileDetail() {
  const el = $("tile-detail");
  if (!tileFocus) return;
  const { table, tile, palette } = tileFocus;
  const base = table + tile * 16;
  const chr = state.chr;
  let rowsHtml = "";
  let pix = "";
  for (let y = 0; y < 8; y++) {
    const lo = chr[base + y];
    const hi = chr[base + y + 8];
    let colors = "";
    for (let b = 7; b >= 0; b--) {
      const c = ((lo >> b) & 1) | (((hi >> b) & 1) << 1);
      colors += c;
      pix += `<span style="background:${colorCss(palette, c)}">${c}</span>`;
    }
    rowsHtml +=
      `<tr><td>${y}</td><td>${hex(base + y, 4)}</td><td>${bitSpans(lo)}</td>` +
      `<td>${hex(base + y + 8, 4)}</td><td>${bitSpans(hi)}</td><td class="hi">${colors}</td></tr>`;
  }
  const paletteName =
    palette < 0 ? "灰色" : palette < 4 ? `背景パレット ${palette}` : `スプライトパレット ${palette - 4}`;
  el.innerHTML =
    `<p><b>タイル ${hex(tile)}</b>（${hex(table, 4)} のパターンテーブルの ${tile} 番目）` +
    `${pinned ? " <small>固定中（もう一度クリックで解除）</small>" : ""}<br>` +
    `アドレス ${hex(base, 4)}〜${hex(base + 15, 4)}（1 タイル = 16 バイト）</p>` +
    `<table class="bits"><tr><th>行</th><th colspan="2">下位ビット（前半 8 バイト）</th>` +
    `<th colspan="2">上位ビット（後半 8 バイト）</th><th>色番号</th></tr>${rowsHtml}</table>` +
    `<p class="note">色番号 = 上位ビット × 2 + 下位ビット。同じ位置のビットを 2 枚重ねて、1 ドット 2 ビット（4 色）にします。</p>` +
    `<div class="tile-big"><div class="pix">${pix}</div>` +
    `<p class="note">${paletteName}で塗った場合。<br>色番号 0 は透明（背景色）です。</p></div>`;
}

// ---------------------------------------------------------------- ネームテーブル

const NT_SCALE = 2; // 512 × 480 → 1024 × 960

function ntTile({ nt, col, row }) {
  return state.nametables[nt * 0x400 + row * 32 + col];
}

/** 属性テーブルのアドレス・値・ビットの位置・パレット番号 */
function attributeOf({ nt, col, row }) {
  const base = 0x2000 + nt * 0x400;
  const addr = base + 0x3c0 + (row >> 2) * 8 + (col >> 2);
  const value = state.nametables[addr - 0x2000];
  const shift = ((row % 4) >> 1) * 4 + ((col % 4) >> 1) * 2;
  return { addr, value, shift, palette: (value >> shift) & 3 };
}

/** ミラーリングで、ネームテーブル nt（0〜3）が実際に使う VRAM（0 か 1） */
function physical(nt) {
  return viewer.mirroring() === "vertical" ? [0, 1, 0, 1][nt] : [0, 0, 1, 1][nt];
}

function drawNametables() {
  const canvas = $("nametables");
  const ctx = canvas.getContext("2d");
  blit(ctx, viewer.nametables_image(), 512, 480, 1024, 960);
  const tile = 8 * NT_SCALE;

  if ($("nt-grid").checked) grid(ctx, 1024, 960, tile, "#ffffff28");
  if ($("nt-attr").checked) {
    grid(ctx, 1024, 960, tile * 2, "#ffffff50", 1, [3, 3]); // 2 × 2 タイル（2 ビットずつ）
    grid(ctx, 1024, 960, tile * 4, "#ffffffa0", 1); // 4 × 4 タイル（属性 1 バイト）
  }
  if ($("nt-palno").checked) {
    ctx.font = "bold 14px ui-monospace, Menlo, Consolas, monospace";
    ctx.textAlign = "center";
    for (let nt = 0; nt < 4; nt++) {
      for (let row = 0; row < 30; row += 2) {
        for (let col = 0; col < 32; col += 2) {
          const p = attributeOf({ nt, col, row }).palette;
          const x = ((nt % 2) * 256 + col * 8 + 8) * NT_SCALE;
          const y = (Math.floor(nt / 2) * 240 + row * 8 + 8) * NT_SCALE;
          ctx.fillStyle = "#000a";
          ctx.fillRect(x - 9, y - 9, 18, 18);
          ctx.fillStyle = "#fff";
          ctx.fillText(p, x, y + 5);
        }
      }
    }
    ctx.textAlign = "left";
  }

  // 4 枚の区切りと名前。画面に出しているものを強調する
  ctx.fillStyle = "#fff";
  ctx.fillRect(511 * NT_SCALE, 0, NT_SCALE, 960);
  ctx.fillRect(0, 239 * NT_SCALE + 1, 1024, NT_SCALE);
  const shown = state.ctrl & 3;
  for (let nt = 0; nt < 4; nt++) {
    const x = (nt % 2) * 512;
    const y = Math.floor(nt / 2) * 480;
    const first = [0, 1, 2, 3].find((n) => physical(n) === physical(nt));
    const name = hex(0x2000 + nt * 0x400, 4) + (first !== nt ? `（= ${hex(0x2000 + first * 0x400, 4)} の写し）` : "");
    label(ctx, name + (nt === shown ? " 画面に表示中" : ""), x + 4, y + 4, nt === shown ? ACCENT : "#fff");
    if (nt === shown) strokeRect(ctx, x, y, 512, 480, ACCENT, 3);
  }

  // マウスを乗せたタイルと、同じ属性を使う 2 × 2 タイル、同じ属性バイトの 4 × 4 タイル
  if (hoverNt) {
    const { nt, col, row } = hoverNt;
    const ox = (nt % 2) * 512;
    const oy = Math.floor(nt / 2) * 480;
    strokeRect(ctx, ox + (col & ~3) * tile, oy + (row & ~3) * tile, tile * 4, tile * 4, ACCENT, 2);
    strokeRect(ctx, ox + (col & ~1) * tile, oy + (row & ~1) * tile, tile * 2, tile * 2, ACCENT, 2);
    strokeRect(ctx, ox + col * tile - 1, oy + row * tile - 1, tile + 2, tile + 2, HOVER, 2);
  }

  const v = viewer.mirroring() === "vertical";
  $("mirroring-note").textContent =
    `ネームテーブルは 4 枚分のアドレスがありますが、本体の VRAM は 2 枚分（2 KiB）です。` +
    (v
      ? "この ROM は垂直ミラーリングなので、$2800 は $2000 の、$2C00 は $2400 の写しになります。"
      : "この ROM は水平ミラーリングなので、$2400 は $2000 の、$2C00 は $2800 の写しになります。") +
    " mynes はスクロールを作っていないので、PPUCTRL で選んだ 1 枚がそのまま画面に出ます。";
}

$("nametables").addEventListener("mousemove", (event) => {
  if (!viewer) return;
  const rect = event.target.getBoundingClientRect();
  const x = Math.floor(((event.clientX - rect.left) / rect.width) * 512);
  const y = Math.floor(((event.clientY - rect.top) / rect.height) * 480);
  const nt = (x >= 256 ? 1 : 0) + (y >= 240 ? 2 : 0);
  hoverNt = { nt, col: (x % 256) >> 3, row: (y % 240) >> 3 };
  showTip(event, ntTipRows(hoverNt));
  if (!pinned) tileFocus = { table: state.ctrl & 0x10 ? 0x1000 : 0, tile: ntTile(hoverNt), palette: attributeOf(hoverNt).palette };
  dirty = true;
});
$("nametables").addEventListener("mouseleave", () => {
  hideTip();
  hoverNt = null;
  dirty = true;
});

function showNametableDetail() {
  const el = $("nt-detail");
  if (!hoverNt) return;
  const { nt, col, row } = hoverNt;
  const base = 0x2000 + nt * 0x400;
  const addr = base + row * 32 + col;
  const tile = ntTile(hoverNt);
  const attr = attributeOf(hoverNt);
  const where = ["左上", "右上", "左下", "右下"][attr.shift / 2];
  let bits = "";
  for (let b = 7; b >= 0; b--) {
    const sel = b === attr.shift || b === attr.shift + 1;
    bits += `<span class="${sel ? "sel" : ""}">${(attr.value >> b) & 1}</span>`;
    if (b % 2 === 0 && b > 0) bits += " ";
  }
  let swatches = "";
  for (let c = 0; c < 4; c++) {
    const i = attr.palette * 4 + c;
    swatches +=
      `<div class="swatch"><div class="chip" style="background-color:${css(paletteColor(attr.palette, c))}"></div>` +
      `<div class="addr">${c === 0 ? "3F00" : hex(0x3f00 + i, 4).slice(1)}</div></div>`;
  }
  const bgTable = state.ctrl & 0x10 ? 0x1000 : 0;
  el.innerHTML =
    `<p><b>${hex(base, 4)} のネームテーブルの ${col} 列 ${row} 行</b>（1 枚 = 32 列 × 30 行のタイル）</p>` +
    `<h3>① ネームテーブル: どのタイルを置くか</h3>` +
    `<p class="mono">${hex(base, 4)} + ${row} × 32 + ${col} = <b>${hex(addr, 4)}</b> → タイル <b>${hex(tile)}</b></p>` +
    `<p class="note">背景のパターンテーブル（${hex(bgTable, 4)}）の ${tile} 番目の絵を使います。</p>` +
    `<h3>② 属性テーブル: どのパレットで塗るか</h3>` +
    `<p class="mono">${hex(base + 0x3c0, 4)} + ${row >> 2} × 8 + ${col >> 2} = <b>${hex(attr.addr, 4)}</b> → <span class="attr-bits">${bits}</span></p>` +
    `<p class="note">属性 1 バイトが 4 × 4 タイル（外側の枠）を受け持ち、その中の 2 × 2 タイル（内側の枠）ごとに 2 ビットずつ使います。` +
    `ビット 1-0 が左上、3-2 が右上、5-4 が左下、7-6 が右下です。このタイルは<b>${where}</b>なので、ビット ${attr.shift + 1}-${attr.shift} = <b>${attr.palette}</b>。</p>` +
    `<h3>③ パレット: 色番号を色に</h3>` +
    `<p>背景パレット ${attr.palette}</p><div class="swatches">${swatches}</div>`;
}

// ---------------------------------------------------------------- 画面

const SCREEN_SCALE = 2;

function visibleSprites() {
  const list = [];
  for (let i = 0; i < 64; i++) if (state.oam[i * 4] < 0xef) list.push(i);
  return list;
}

/** 1 ラインに 9 個以上並んで、どこかのラインで表示されないスプライト（mynes の sprite_line と同じ数え方） */
function droppedSprites() {
  const dropped = new Set();
  for (let line = 0; line < 240; line++) {
    let count = 0;
    for (let i = 0; i < 64; i++) {
      const row = line - (state.oam[i * 4] + 1);
      if (row < 0 || row >= 8) continue;
      count += 1;
      if (count > 8) dropped.add(i);
    }
  }
  return dropped;
}

function drawScreen() {
  const ctx = $("screen").getContext("2d");
  blit(ctx, viewer.screen(), 256, 240, 512, 480);
  if ($("screen-grid").checked) grid(ctx, 512, 480, 8 * SCREEN_SCALE, "#ffffff30");
  // スプライトの表示がオフなら、OAM の中身は画面に出ない
  if ($("screen-sprites").checked && state.mask & 0x10) {
    ctx.font = "10px ui-monospace, Menlo, Consolas, monospace";
    const dropped = droppedSprites();
    for (const i of visibleSprites()) {
      const y = (state.oam[i * 4] + 1) * SCREEN_SCALE;
      const x = state.oam[i * 4 + 3] * SCREEN_SCALE;
      const color = i === hoverSprite ? ACCENT : "#ff55aa";
      ctx.setLineDash(dropped.has(i) ? [3, 2] : []); // 8 個の制限で消えているものは点線
      strokeRect(ctx, x, y, 16, 16, color, i === hoverSprite ? 2 : 1);
      ctx.setLineDash([]);
      ctx.fillStyle = color;
      ctx.fillText(i, x + 1, y - 2);
    }
  }
  if (hoverNt && hoverNt.nt === (state.ctrl & 3)) {
    strokeRect(ctx, hoverNt.col * 16 - 1, hoverNt.row * 16 - 1, 18, 18, HOVER, 2);
  }
}

$("screen").addEventListener("mousemove", (event) => {
  if (!viewer) return;
  const rect = event.target.getBoundingClientRect();
  const x = Math.floor(((event.clientX - rect.left) / rect.width) * 256);
  const y = Math.floor(((event.clientY - rect.top) / rect.height) * 240);
  hoverNt = { nt: state.ctrl & 3, col: x >> 3, row: y >> 3 };
  // そのドットに重なっているスプライト（番号の小さいほうが手前）
  const sprites = (state.mask & 0x10 ? visibleSprites() : []).filter((i) => {
    const sy = state.oam[i * 4] + 1;
    const sx = state.oam[i * 4 + 3];
    return x >= sx && x < sx + 8 && y >= sy && y < sy + 8;
  });
  hoverSprite = sprites.length ? sprites[0] : null;
  if (!pinned) {
    if (hoverSprite !== null) {
      tileFocus = spriteTileFocus(hoverSprite);
    } else {
      tileFocus = { table: state.ctrl & 0x10 ? 0x1000 : 0, tile: ntTile(hoverNt), palette: attributeOf(hoverNt).palette };
    }
  }
  const rows = [[`(${x}, ${y})`, "画面の座標"], ...ntTipRows(hoverNt)];
  if (hoverSprite !== null) {
    const o = hoverSprite * 4;
    rows.push([`OAM ${hex(o)}〜${hex(o + 3)}`, `スプライト ${hoverSprite}（Y・タイル ${hex(state.oam[o + 1])}・属性・X）`]);
  }
  showTip(event, rows);
  $("screen-info").innerHTML =
    `<p class="mono">画面の (${x}, ${y}) → タイル ${x >> 3} 列 ${y >> 3} 行` +
    (sprites.length ? `／スプライト ${sprites.join(", ")} 番` : "") +
    `</p>`;
  dirty = true;
});
$("screen").addEventListener("mouseleave", () => {
  hideTip();
  hoverNt = null;
  hoverSprite = null;
  $("screen-info").innerHTML = "";
  dirty = true;
});

// ---------------------------------------------------------------- OAM

function spriteTileFocus(i) {
  return { table: state.ctrl & 0x08 ? 0x1000 : 0, tile: state.oam[i * 4 + 1], palette: 4 + (state.oam[i * 4 + 2] & 3) };
}

const oamRows = [];
(function buildOamTable() {
  const tbody = document.querySelector("#oam tbody");
  for (let i = 0; i < 64; i++) {
    const tr = document.createElement("tr");
    tr.innerHTML =
      `<td>${i}</td><td>${hex(i * 4)}</td><td></td><td></td><td></td><td></td>` +
      `<td><canvas width="8" height="8"></canvas></td><td></td>`;
    tr.addEventListener("mousemove", (event) => {
      const o = i * 4;
      const tile = state.oam ? state.oam[o + 1] : 0;
      const table = state.ctrl & 0x08 ? 0x1000 : 0;
      showTip(event, [
        [`OAM ${hex(o)}〜${hex(o + 3)}`, `スプライト ${i}（+0 Y・+1 タイル・+2 属性・+3 X）`],
        [`${hex(table + tile * 16, 4)}〜${hex(table + tile * 16 + 15, 4)}`, `タイル ${hex(tile)} の絵（パターンテーブル）`],
      ]);
    });
    tr.addEventListener("mouseenter", () => {
      hoverSprite = i;
      if (!pinned && viewer) tileFocus = spriteTileFocus(i);
      dirty = true;
    });
    tr.addEventListener("mouseleave", () => {
      hideTip();
      hoverSprite = null;
      dirty = true;
    });
    tbody.appendChild(tr);
    oamRows.push(tr);
  }
})();

let lastOamKey = "";

function showOam() {
  const showHidden = $("oam-hidden").checked;
  const key = [state.oam.join(), state.palette.join(), state.ctrl, state.mask, showHidden, hoverSprite].join("|");
  if (key === lastOamKey) return;
  lastOamKey = key;
  $("oam-note").textContent =
    state.mask & 0x10 ? "" : "PPUMASK でスプライトの表示がオフなので、OAM の中身は画面に出ません。";
  const table = state.ctrl & 0x08 ? 0x1000 : 0;
  const dropped = droppedSprites();
  for (let i = 0; i < 64; i++) {
    const [y, tile, attr, x] = state.oam.slice(i * 4, i * 4 + 4);
    const tr = oamRows[i];
    const hidden = y >= 0xef;
    tr.style.display = hidden && !showHidden ? "none" : "";
    tr.classList.toggle("hidden", hidden);
    tr.classList.toggle("active", i === hoverSprite);
    const cells = tr.children;
    cells[2].textContent = `${hex(y)} (${y})`;
    cells[3].textContent = hex(tile);
    cells[4].textContent = bin(attr);
    cells[5].textContent = `${hex(x)} (${x})`;
    const canvas = cells[6].firstChild;
    const ctx = canvas.getContext("2d");
    ctx.clearRect(0, 0, 8, 8);
    ctx.putImageData(new ImageData(new Uint8ClampedArray(viewer.tile_image(table, tile, 4 + (attr & 3))), 8, 8), 0, 0);
    canvas.style.transform = `scale(${attr & 0x40 ? -1 : 1}, ${attr & 0x80 ? -1 : 1})`;
    const meaning = [`パレット ${attr & 3}（${hex(0x3f10 + (attr & 3) * 4, 4)}〜）`];
    meaning.push(attr & 0x20 ? "背景の後ろ" : "背景の手前");
    if (attr & 0x40) meaning.push("左右反転");
    if (attr & 0x80) meaning.push("上下反転");
    if (dropped.has(i)) meaning.push("1 ラインに 9 個目以降なので表示されない");
    cells[7].textContent = hidden ? "画面の外" : meaning.join("・");
  }
}

// ---------------------------------------------------------------- 描き直し

function redraw() {
  state.nametables = viewer.nametables();
  state.palette = viewer.palette();
  state.oam = viewer.oam();
  state.ctrl = viewer.ctrl();
  state.mask = viewer.mask();
  drawScreen();
  showRegisters();
  showPalettes();
  drawPatternTables();
  showTileDetail();
  drawNametables();
  showNametableDetail();
  showOam();
}

function loop(now) {
  if (viewer) {
    if (!paused) {
      pending += Math.min(now - lastTime, 100);
      while (pending >= FRAME_MS) {
        viewer.set_buttons(buttons);
        viewer.run_frame();
        pending -= FRAME_MS;
        dirty = true;
      }
    }
    if (dirty) {
      redraw();
      dirty = false;
    }
  }
  lastTime = now;
  requestAnimationFrame(loop);
}

requestAnimationFrame((now) => {
  lastTime = now;
  loop(now);
});
