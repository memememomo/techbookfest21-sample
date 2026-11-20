//! PPU（画像を作るチップ）
//!
//! 1 フレームは 262 ライン × 341 ドット。PPU は CPU の 3 倍の速さで 1 ドットずつ進む。
//!   0〜239 ライン: 画面に映るライン
//!   240 ライン   : 何もしない
//!   241〜260 ライン: VBlank（241 ラインの 1 ドット目で VBlank フラグが立ち、NMI が起きる）
//!   261 ライン   : 次のフレームの準備（プリレンダーライン）

use crate::cartridge::{Cartridge, Mirroring};
use crate::palette::PALETTE;

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 240;

// $2000 PPUCTRL
const CTRL_INCREMENT_32: u8 = 1 << 2;
const CTRL_BG_TABLE: u8 = 1 << 4;
const CTRL_NMI: u8 = 1 << 7;
// $2001 PPUMASK
const MASK_BG: u8 = 1 << 3;
// $2002 PPUSTATUS
const STATUS_VBLANK: u8 = 1 << 7;

pub struct Ppu {
    ctrl: u8,
    mask: u8,
    status: u8,
    oam_addr: u8,
    /// $2006/$2007 で読み書きする VRAM のアドレス
    v: u16,
    /// $2006 の 1 回目の書き込みか 2 回目か（$2005 と共通）
    w: bool,
    /// $2007 の読み込みバッファ
    read_buffer: u8,

    /// ネームテーブル用の VRAM（本体の 2 KiB）
    vram: [u8; 0x800],
    palette: [u8; 32],
    pub oam: [u8; 256],

    pub scanline: u16,
    pub dot: u16,
    nmi_pending: bool,
    frame_ready: bool,

    /// 画面（RGBA、256 × 240）
    pub frame: Vec<u8>,
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ppu {
    pub fn new() -> Self {
        Ppu {
            ctrl: 0,
            mask: 0,
            status: 0,
            oam_addr: 0,
            v: 0,
            w: false,
            read_buffer: 0,
            vram: [0; 0x800],
            palette: [0; 32],
            oam: [0; 256],
            scanline: 0,
            dot: 0,
            nmi_pending: false,
            frame_ready: false,
            frame: vec![0; WIDTH * HEIGHT * 4],
        }
    }

    /// NMI を起こすべきか（起こしたら false に戻る）
    pub fn take_nmi(&mut self) -> bool {
        std::mem::take(&mut self.nmi_pending)
    }

    /// 1 フレーム描き終わったか（確認したら false に戻る）
    pub fn take_frame(&mut self) -> bool {
        std::mem::take(&mut self.frame_ready)
    }

    // ---- CPU から見えるレジスタ（$2000〜$2007）----

    pub fn read_register(&mut self, addr: u16, cart: &mut Cartridge) -> u8 {
        match addr & 7 {
            2 => {
                let value = self.status;
                // 読むと VBlank フラグと書き込みの順番（w）がリセットされる
                self.status &= !STATUS_VBLANK;
                self.w = false;
                value
            }
            4 => self.oam[self.oam_addr as usize],
            7 => {
                let addr = self.v & 0x3FFF;
                let value = if addr >= 0x3F00 {
                    // パレットはすぐに読める（バッファには「下にある」ネームテーブルが入る）
                    self.read_buffer = self.read_vram(addr - 0x1000, cart);
                    self.read_vram(addr, cart)
                } else {
                    // それ以外は 1 回前に読んだ値が返る
                    let buffered = self.read_buffer;
                    self.read_buffer = self.read_vram(addr, cart);
                    buffered
                };
                self.increment_v();
                value
            }
            _ => 0, // 書き込み専用のレジスタ
        }
    }

    pub fn write_register(&mut self, addr: u16, value: u8, cart: &mut Cartridge) {
        match addr & 7 {
            0 => {
                let nmi_was_enabled = self.ctrl & CTRL_NMI != 0;
                self.ctrl = value;
                // VBlank 中に NMI を有効にすると、その場で NMI が起きる
                if !nmi_was_enabled && value & CTRL_NMI != 0 && self.status & STATUS_VBLANK != 0 {
                    self.nmi_pending = true;
                }
            }
            1 => self.mask = value,
            3 => self.oam_addr = value,
            4 => {
                self.oam[self.oam_addr as usize] = value;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            // スクロール位置（mynes では作らない）。書き込みの順番（w）は $2006 と共通
            5 => self.w = !self.w,
            6 => {
                if !self.w {
                    // 1 回目: アドレスの上位 6 ビット
                    self.v = (self.v & 0x00FF) | ((value as u16 & 0x3F) << 8);
                } else {
                    // 2 回目: アドレスの下位 8 ビット
                    self.v = (self.v & 0xFF00) | value as u16;
                }
                self.w = !self.w;
            }
            7 => {
                self.write_vram(self.v & 0x3FFF, value, cart);
                self.increment_v();
            }
            _ => {} // $2002 は読み込み専用
        }
    }

    /// $2007 を読み書きした後、アドレスを 1 か 32 進める
    fn increment_v(&mut self) {
        let step = if self.ctrl & CTRL_INCREMENT_32 != 0 {
            32
        } else {
            1
        };
        self.v = self.v.wrapping_add(step) & 0x7FFF;
    }

    // ---- PPU のメモリ空間（$0000〜$3FFF）----

    fn read_vram(&mut self, addr: u16, cart: &mut Cartridge) -> u8 {
        match addr {
            0x0000..=0x1FFF => cart.ppu_read(addr),
            0x2000..=0x3EFF => self.vram[nametable_index(addr, cart.mirroring)],
            _ => self.palette[palette_index(addr)],
        }
    }

    fn write_vram(&mut self, addr: u16, value: u8, cart: &mut Cartridge) {
        match addr {
            0x0000..=0x1FFF => {} // パターンテーブル（CHR ROM）には書き込めない
            0x2000..=0x3EFF => self.vram[nametable_index(addr, cart.mirroring)] = value,
            _ => self.palette[palette_index(addr)] = value & 0x3F,
        }
    }

    // ---- タイミング ----

    /// 1 ドット進める
    pub fn step(&mut self, cart: &mut Cartridge) {
        if self.scanline < 240 && self.dot == 1 {
            self.render_scanline(cart);
        }

        if self.scanline == 241 && self.dot == 1 {
            self.status |= STATUS_VBLANK;
            if self.ctrl & CTRL_NMI != 0 {
                self.nmi_pending = true;
            }
            self.frame_ready = true;
        }
        if self.scanline == 261 && self.dot == 1 {
            self.status &= !STATUS_VBLANK;
        }

        self.dot += 1;
        if self.dot > 340 {
            self.dot = 0;
            self.scanline += 1;
            if self.scanline > 261 {
                self.scanline = 0;
            }
        }
    }

    // ---- 描画 ----

    /// いまのライン（scanline）を描く
    fn render_scanline(&mut self, cart: &mut Cartridge) {
        let y = self.scanline as usize;
        let background = self.background_line(cart);

        for (x, &color) in background.iter().enumerate() {
            // 透明なら背景色（$3F00）
            let index = if color & 3 == 0 {
                self.palette[0]
            } else {
                self.palette[color as usize]
            };
            let [r, g, b] = PALETTE[(index & 0x3F) as usize];
            let i = (y * WIDTH + x) * 4;
            self.frame[i..i + 4].copy_from_slice(&[r, g, b, 255]);
        }
    }

    /// 1 ライン分の背景。各ドットのパレットの番号（$00〜$0F。下位 2 ビットが 0 なら透明）
    fn background_line(&mut self, cart: &mut Cartridge) -> [u8; WIDTH] {
        let mut line = [0; WIDTH];
        if self.mask & MASK_BG == 0 {
            return line;
        }
        let pattern_base = if self.ctrl & CTRL_BG_TABLE != 0 {
            0x1000
        } else {
            0
        };
        // PPUCTRL で選んだネームテーブルを、スクロールせずに描く（スクロールは作らない）
        let nametable = 0x2000 + (self.ctrl as u16 & 0x03) * 0x400;
        let row = self.scanline / 8;
        let fine_y = self.scanline % 8;
        for col in 0..32 {
            let tile = self.read_vram(nametable + row * 32 + col, cart) as u16;
            // 属性テーブルは 4×4 タイルごとに 1 バイト。その中の 2×2 タイルごとに 2 ビット
            let attr = self.read_vram(nametable + 0x3C0 + (row / 4) * 8 + col / 4, cart);
            let shift = (row % 4 / 2) * 4 + (col % 4 / 2) * 2;
            let palette = (attr >> shift) & 3;

            // パターンテーブル: 1 タイル 16 バイト。前半 8 バイトが下位ビット、後半 8 バイトが上位ビット
            let addr = pattern_base + tile * 16 + fine_y;
            let lo = cart.ppu_read(addr);
            let hi = cart.ppu_read(addr + 8);
            for bit in 0..8 {
                let color = ((lo >> (7 - bit)) & 1) | (((hi >> (7 - bit)) & 1) << 1);
                let x = (col * 8 + bit) as usize;
                if color != 0 {
                    line[x] = (palette << 2) | color;
                }
            }
        }
        line
    }
}

/// $2000〜$3EFF を、PPU 内部の VRAM の位置に変換する
fn nametable_index(addr: u16, mirroring: Mirroring) -> usize {
    let addr = (addr - 0x2000) & 0x0FFF; // $3000〜$3EFF は $2000〜$2EFF と同じ
    let table = (addr / 0x400) as usize; // 0: 左上、1: 右上、2: 左下、3: 右下
    let offset = (addr % 0x400) as usize;
    let physical = match mirroring {
        Mirroring::Horizontal => [0, 0, 1, 1][table],
        Mirroring::Vertical => [0, 1, 0, 1][table],
    };
    physical * 0x400 + offset
}

/// $3F00〜$3FFF をパレット RAM（32 バイト）の位置に変換する
fn palette_index(addr: u16) -> usize {
    let index = (addr & 0x1F) as usize;
    // $3F10/$3F14/$3F18/$3F1C は $3F00/$3F04/$3F08/$3F0C と同じ場所
    if index >= 16 && index & 3 == 0 {
        index - 16
    } else {
        index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cart() -> Cartridge {
        let mut data = vec![b'N', b'E', b'S', 0x1A, 1, 1, 0, 0];
        data.resize(16 + 16 * 1024 + 8 * 1024, 0);
        Cartridge::from_ines(&data).unwrap()
    }

    /// PPU を指定したライン・ドットまで進める
    fn run_until(ppu: &mut Ppu, cart: &mut Cartridge, scanline: u16, dot: u16) {
        while !(ppu.scanline == scanline && ppu.dot == dot) {
            ppu.step(cart);
        }
    }

    #[test]
    fn vblank_flag_and_nmi() {
        let mut ppu = Ppu::new();
        let mut cart = test_cart();
        ppu.write_register(0x2000, 0x80, &mut cart); // NMI を有効に
        run_until(&mut ppu, &mut cart, 241, 1);
        assert!(!ppu.take_nmi());
        ppu.step(&mut cart); // 241 ラインの 1 ドット目を処理する
        assert!(ppu.take_nmi());
        assert_eq!(ppu.read_register(0x2002, &mut cart) & 0x80, 0x80);
        // 読むと VBlank フラグは消える
        assert_eq!(ppu.read_register(0x2002, &mut cart) & 0x80, 0);
    }

    #[test]
    fn ppudata_read_is_buffered() {
        let mut ppu = Ppu::new();
        let mut cart = test_cart();
        // $2400 に $11, $22 を書く
        ppu.write_register(0x2006, 0x24, &mut cart);
        ppu.write_register(0x2006, 0x00, &mut cart);
        ppu.write_register(0x2007, 0x11, &mut cart);
        ppu.write_register(0x2007, 0x22, &mut cart);
        // 読み出すと、最初の 1 回は古いバッファの値
        ppu.write_register(0x2006, 0x24, &mut cart);
        ppu.write_register(0x2006, 0x00, &mut cart);
        ppu.read_register(0x2007, &mut cart);
        assert_eq!(ppu.read_register(0x2007, &mut cart), 0x11);
        assert_eq!(ppu.read_register(0x2007, &mut cart), 0x22);
    }

    #[test]
    fn nametable_mirroring() {
        assert_eq!(nametable_index(0x2400, Mirroring::Horizontal), 0);
        assert_eq!(nametable_index(0x2800, Mirroring::Horizontal), 0x400);
        assert_eq!(nametable_index(0x2400, Mirroring::Vertical), 0x400);
        assert_eq!(nametable_index(0x2C05, Mirroring::Vertical), 0x405);
        assert_eq!(nametable_index(0x3005, Mirroring::Vertical), 0x005);
    }

    #[test]
    fn palette_mirroring() {
        assert_eq!(palette_index(0x3F10), 0);
        assert_eq!(palette_index(0x3F14), 4);
        assert_eq!(palette_index(0x3F11), 0x11);
        assert_eq!(palette_index(0x3F20), 0);
    }
}
