//! viewer: mynes でデモ ROM を動かしながら、PPU の中身（パターンテーブル、
//! ネームテーブル、属性テーブル、パレット、OAM）を絵にして見せる
//!
//! mynes の PPU は、ネームテーブルやパレットを外から読めるようにしていない。
//! そこで、CPU が PPU のレジスタ（$2000〜$2007）に書き込むのを横から見て、
//! 同じ内容を手元（[`Shadow`]）に写し取る。mynes のコードには手を入れない。

pub mod shadow;
#[cfg(target_arch = "wasm32")]
pub mod web;

use mynes::cartridge::{Cartridge, Mirroring};
use mynes::nes::Nes;
use mynes::palette::PALETTE;
use shadow::{Shadow, WatchBus};

/// パターンテーブルを描くときの色の選び方
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Colors {
    /// 色番号 0〜3 を黒・暗い灰色・明るい灰色・白で描く
    Gray,
    /// パレット 0〜7（0〜3 が背景用、4〜7 がスプライト用）の色で描く
    Palette(u8),
}

pub struct Viewer {
    pub nes: Nes,
    pub shadow: Shadow,
    /// 電源を入れてから進めたフレーム数
    pub frames: u64,
}

impl Viewer {
    pub fn new(rom: &[u8]) -> Result<Viewer, String> {
        let cart = Cartridge::from_ines(rom)?;
        Ok(Viewer {
            nes: Nes::new(cart),
            shadow: Shadow::new(),
            frames: 0,
        })
    }

    pub fn reset(&mut self) {
        self.nes.reset();
    }

    /// Nes::step と同じことを、PPU への書き込みを見張りながら行う
    fn step(&mut self) {
        let nes = &mut self.nes;
        let nmi = nes.bus.ppu.take_nmi();
        let mut bus = WatchBus {
            bus: &mut nes.bus,
            shadow: &mut self.shadow,
        };
        let mut cycles = if nmi {
            nes.cpu.nmi(&mut bus)
        } else {
            nes.cpu.step(&mut bus)
        } as u32;
        cycles += nes.bus.take_dma_cycles();
        nes.bus.tick(cycles);
    }

    /// 1 フレーム進める（Nes::run_frame と同じ）
    pub fn run_frame(&mut self) {
        loop {
            self.step();
            if self.nes.bus.ppu.take_frame() {
                break;
            }
        }
        self.frames += 1;
        // 音は鳴らさないので、たまったサンプルは捨てる
        self.nes.take_samples();
    }

    pub fn set_buttons(&mut self, buttons: u8) {
        self.nes.set_buttons(buttons);
    }

    /// CPU から見た $8000〜$FFFF の 2 バイト（リトルエンディアン）
    pub fn read_u16(&mut self, addr: u16) -> u16 {
        let lo = self.nes.bus.cart.cpu_read(addr) as u16;
        let hi = self.nes.bus.cart.cpu_read(addr + 1) as u16;
        (hi << 8) | lo
    }

    /// パターンテーブルの 1 バイト（$0000〜$1FFF）
    pub fn chr(&mut self, addr: u16) -> u8 {
        self.nes.bus.cart.ppu_read(addr)
    }

    pub fn mirroring(&self) -> Mirroring {
        self.nes.bus.cart.mirroring
    }

    /// PPU のアドレス $2000〜$2FFF の 1 バイト（ミラーリングを反映する）
    pub fn nametable_byte(&self, addr: u16) -> u8 {
        self.shadow.nametable_byte(addr, self.mirroring())
    }

    /// 色番号（$00〜$3F）を RGBA に
    fn rgba(index: u8) -> [u8; 4] {
        let [r, g, b] = PALETTE[(index & 0x3F) as usize];
        [r, g, b, 255]
    }

    /// パレット palette（0〜7）の色 color（0〜3）を RGBA に。色 0 は背景色（$3F00）
    pub fn palette_rgba(&self, palette: u8, color: u8) -> [u8; 4] {
        let index = if color == 0 {
            self.shadow.palette[0]
        } else {
            self.shadow.palette[(palette as usize & 7) * 4 + color as usize]
        };
        Self::rgba(index)
    }

    /// タイルの 1 行（8 ドット）の色番号（0〜3）
    pub fn tile_row(&mut self, table: u16, tile: u8, row: u16) -> [u8; 8] {
        // 1 タイル 16 バイト。前半 8 バイトが下位ビット、後半 8 バイトが上位ビット
        let addr = table + tile as u16 * 16 + row;
        let lo = self.chr(addr);
        let hi = self.chr(addr + 8);
        let mut pixels = [0; 8];
        for (bit, pixel) in pixels.iter_mut().enumerate() {
            *pixel = ((lo >> (7 - bit)) & 1) | (((hi >> (7 - bit)) & 1) << 1);
        }
        pixels
    }

    fn color(&self, colors: Colors, color: u8) -> [u8; 4] {
        match colors {
            Colors::Gray => {
                let v = color * 85;
                [v, v, v, 255]
            }
            Colors::Palette(p) => self.palette_rgba(p, color),
        }
    }

    /// 1 タイル（8 × 8 ドット）を RGBA で。transparent なら色 0 を透明にする
    pub fn tile_image(
        &mut self,
        table: u16,
        tile: u8,
        colors: Colors,
        transparent: bool,
    ) -> Vec<u8> {
        let mut image = vec![0; 8 * 8 * 4];
        for row in 0..8 {
            for (x, &c) in self.tile_row(table, tile, row).iter().enumerate() {
                if transparent && c == 0 {
                    continue;
                }
                let i = (row as usize * 8 + x) * 4;
                image[i..i + 4].copy_from_slice(&self.color(colors, c));
            }
        }
        image
    }

    /// パターンテーブル 1 枚（$0000 か $1000）を、16 × 16 タイル = 128 × 128 ドットの RGBA で
    pub fn pattern_table_image(&mut self, table: u16, colors: Colors) -> Vec<u8> {
        const W: usize = 128;
        let mut image = vec![0; W * W * 4];
        for tile in 0..=255u8 {
            let (tx, ty) = (tile as usize % 16, tile as usize / 16);
            for row in 0..8 {
                for (x, &c) in self.tile_row(table, tile, row).iter().enumerate() {
                    let i = ((ty * 8 + row as usize) * W + tx * 8 + x) * 4;
                    image[i..i + 4].copy_from_slice(&self.color(colors, c));
                }
            }
        }
        image
    }

    /// 4 枚のネームテーブル（$2000 左上、$2400 右上、$2800 左下、$2C00 右下）を
    /// 512 × 480 ドットの RGBA で。属性テーブルのパレットで色を付ける
    pub fn nametables_image(&mut self) -> Vec<u8> {
        const W: usize = 512;
        let mut image = vec![0; W * 480 * 4];
        let table = self.shadow.bg_pattern_table();
        for nt in 0..4u16 {
            let base = 0x2000 + nt * 0x400;
            let (ox, oy) = ((nt as usize % 2) * 256, (nt as usize / 2) * 240);
            for row in 0..30u16 {
                for col in 0..32u16 {
                    let tile = self.nametable_byte(base + row * 32 + col);
                    let palette = self.attribute_palette(base, col, row);
                    for y in 0..8 {
                        for (x, &c) in self.tile_row(table, tile, y).iter().enumerate() {
                            let px = ox + col as usize * 8 + x;
                            let py = oy + row as usize * 8 + y as usize;
                            let i = (py * W + px) * 4;
                            image[i..i + 4].copy_from_slice(&self.palette_rgba(palette, c));
                        }
                    }
                }
            }
        }
        image
    }

    /// 属性テーブルから、タイル (col, row) のパレット番号（0〜3）を求める
    pub fn attribute_palette(&self, nametable: u16, col: u16, row: u16) -> u8 {
        // 4 × 4 タイルごとに 1 バイト。その中の 2 × 2 タイルごとに 2 ビット
        let attr = self.nametable_byte(nametable + 0x3C0 + (row / 4) * 8 + col / 4);
        let shift = (row % 4 / 2) * 4 + (col % 4 / 2) * 2;
        (attr >> shift) & 3
    }
}
