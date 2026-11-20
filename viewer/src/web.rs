//! ブラウザ（JavaScript）から使うためのラッパー

use crate::{Colors, Viewer};
use mynes::cartridge::Mirroring;
use mynes::palette::PALETTE;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WebViewer {
    viewer: Viewer,
}

/// JavaScript から渡すパレットの番号: -1 なら灰色、0〜7 ならそのパレット
fn colors(palette: i32) -> Colors {
    if palette < 0 {
        Colors::Gray
    } else {
        Colors::Palette(palette as u8)
    }
}

#[wasm_bindgen]
impl WebViewer {
    #[wasm_bindgen(constructor)]
    pub fn new(rom: &[u8]) -> Result<WebViewer, JsValue> {
        let viewer = Viewer::new(rom).map_err(|e| JsValue::from_str(&e))?;
        Ok(WebViewer { viewer })
    }

    pub fn run_frame(&mut self) {
        self.viewer.run_frame();
    }

    pub fn reset(&mut self) {
        self.viewer.reset();
    }

    pub fn set_buttons(&mut self, buttons: u8) {
        self.viewer.set_buttons(buttons);
    }

    /// 電源を入れてから進めたフレーム数
    pub fn frames(&self) -> f64 {
        self.viewer.frames as f64
    }

    /// 画面（RGBA、256 × 240）
    pub fn screen(&self) -> Vec<u8> {
        self.viewer.nes.frame().to_vec()
    }

    // ---- ROM の情報 ----

    pub fn prg_size(&self) -> usize {
        self.viewer.nes.bus.cart.prg_rom.len()
    }

    pub fn chr_size(&self) -> usize {
        self.viewer.nes.bus.cart.chr_rom.len()
    }

    /// "horizontal" か "vertical"
    pub fn mirroring(&self) -> String {
        match self.viewer.mirroring() {
            Mirroring::Horizontal => "horizontal",
            Mirroring::Vertical => "vertical",
        }
        .to_string()
    }

    /// CPU のアドレス空間の 2 バイト（割り込みベクタを読むのに使う）
    pub fn read_u16(&mut self, addr: u16) -> u16 {
        self.viewer.read_u16(addr)
    }

    // ---- PPU の中身 ----

    /// PPUCTRL（$2000）に最後に書いた値
    pub fn ctrl(&self) -> u8 {
        self.viewer.shadow.ctrl
    }

    /// PPUMASK（$2001）に最後に書いた値
    pub fn mask(&self) -> u8 {
        self.viewer.shadow.mask
    }

    /// パターンテーブル（$0000〜$1FFF、8 KiB）
    pub fn chr(&mut self) -> Vec<u8> {
        (0..0x2000).map(|addr| self.viewer.chr(addr)).collect()
    }

    /// ネームテーブル（$2000〜$2FFF、4 KiB。ミラーリングを反映する）
    pub fn nametables(&self) -> Vec<u8> {
        (0x2000..0x3000)
            .map(|addr| self.viewer.nametable_byte(addr))
            .collect()
    }

    /// パレット RAM（$3F00〜$3F1F、32 バイト）
    pub fn palette(&self) -> Vec<u8> {
        self.viewer.shadow.palette.to_vec()
    }

    /// OAM（256 バイト）
    pub fn oam(&self) -> Vec<u8> {
        self.viewer.nes.bus.ppu.oam.to_vec()
    }

    /// ファミコンの 64 色（RGB × 64）
    pub fn system_palette(&self) -> Vec<u8> {
        PALETTE.iter().flatten().copied().collect()
    }

    // ---- 絵（RGBA）----

    /// パターンテーブル 1 枚（128 × 128）
    pub fn pattern_table_image(&mut self, table: u16, palette: i32) -> Vec<u8> {
        self.viewer.pattern_table_image(table, colors(palette))
    }

    /// 4 枚のネームテーブル（512 × 480）
    pub fn nametables_image(&mut self) -> Vec<u8> {
        self.viewer.nametables_image()
    }

    /// 1 タイル（8 × 8）。色 0 は透明
    pub fn tile_image(&mut self, table: u16, tile: u8, palette: i32) -> Vec<u8> {
        self.viewer.tile_image(table, tile, colors(palette), true)
    }
}
