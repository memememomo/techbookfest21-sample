//! PPU の写し: CPU が PPU のレジスタに書き込むのを見て、ネームテーブルとパレットを写し取る
//!
//! mynes の PPU（ppu.rs）の write_register / read_register と同じ動きをする。
//! OAM は mynes の PPU が公開している（ppu.oam）ので、写さずにそのまま読む。

use mynes::bus::NesBus;
use mynes::cartridge::Mirroring;
use mynes::cpu::Bus;

pub struct Shadow {
    /// $2000 PPUCTRL に最後に書いた値
    pub ctrl: u8,
    /// $2001 PPUMASK に最後に書いた値
    pub mask: u8,
    /// $2006/$2007 で読み書きする VRAM のアドレス
    v: u16,
    /// $2006 の 1 回目の書き込みか 2 回目か（$2005 と共通）
    w: bool,
    /// ネームテーブル用の VRAM（本体の 2 KiB）
    pub vram: [u8; 0x800],
    /// パレット RAM（$3F00〜$3F1F）
    pub palette: [u8; 32],
}

impl Default for Shadow {
    fn default() -> Self {
        Self::new()
    }
}

impl Shadow {
    pub fn new() -> Self {
        Shadow {
            ctrl: 0,
            mask: 0,
            v: 0,
            w: false,
            vram: [0; 0x800],
            palette: [0; 32],
        }
    }

    /// 背景のパターンテーブル（PPUCTRL のビット 4）
    pub fn bg_pattern_table(&self) -> u16 {
        if self.ctrl & 0x10 != 0 { 0x1000 } else { 0 }
    }

    /// スプライトのパターンテーブル（PPUCTRL のビット 3）
    pub fn sprite_pattern_table(&self) -> u16 {
        if self.ctrl & 0x08 != 0 { 0x1000 } else { 0 }
    }

    /// 画面に出しているネームテーブル（PPUCTRL のビット 0〜1）の先頭アドレス
    pub fn shown_nametable(&self) -> u16 {
        0x2000 + (self.ctrl as u16 & 3) * 0x400
    }

    /// CPU が $2000〜$3FFF を読んだ
    fn on_read(&mut self, addr: u16) {
        match addr & 7 {
            2 => self.w = false,
            7 => self.increment_v(),
            _ => {}
        }
    }

    /// CPU が $2000〜$3FFF に書いた
    fn on_write(&mut self, addr: u16, value: u8, mirroring: Mirroring) {
        match addr & 7 {
            0 => self.ctrl = value,
            1 => self.mask = value,
            5 => self.w = !self.w,
            6 => {
                if !self.w {
                    self.v = (self.v & 0x00FF) | ((value as u16 & 0x3F) << 8);
                } else {
                    self.v = (self.v & 0xFF00) | value as u16;
                }
                self.w = !self.w;
            }
            7 => {
                let addr = self.v & 0x3FFF;
                match addr {
                    0x0000..=0x1FFF => {} // パターンテーブル（CHR ROM）には書き込めない
                    0x2000..=0x3EFF => self.vram[nametable_index(addr, mirroring)] = value,
                    _ => self.palette[palette_index(addr)] = value & 0x3F,
                }
                self.increment_v();
            }
            _ => {}
        }
    }

    fn increment_v(&mut self) {
        let step = if self.ctrl & 0x04 != 0 { 32 } else { 1 };
        self.v = self.v.wrapping_add(step) & 0x7FFF;
    }

    /// PPU のアドレス $2000〜$2FFF の 1 バイト
    pub fn nametable_byte(&self, addr: u16, mirroring: Mirroring) -> u8 {
        self.vram[nametable_index(addr, mirroring)]
    }
}

/// $2000〜$3EFF を、VRAM（2 KiB）の位置に変換する（ppu.rs と同じ）
pub fn nametable_index(addr: u16, mirroring: Mirroring) -> usize {
    let addr = (addr - 0x2000) & 0x0FFF;
    let table = (addr / 0x400) as usize;
    let offset = (addr % 0x400) as usize;
    let physical = match mirroring {
        Mirroring::Horizontal => [0, 0, 1, 1][table],
        Mirroring::Vertical => [0, 1, 0, 1][table],
    };
    physical * 0x400 + offset
}

/// $3F00〜$3FFF をパレット RAM の位置に変換する（ppu.rs と同じ）
pub fn palette_index(addr: u16) -> usize {
    let index = (addr & 0x1F) as usize;
    if index >= 16 && index & 3 == 0 {
        index - 16
    } else {
        index
    }
}

/// NesBus に読み書きを取り次ぎながら、PPU のレジスタへの読み書きを Shadow に伝えるバス
pub struct WatchBus<'a> {
    pub bus: &'a mut NesBus,
    pub shadow: &'a mut Shadow,
}

impl Bus for WatchBus<'_> {
    fn read(&mut self, addr: u16) -> u8 {
        let value = self.bus.read(addr);
        if (0x2000..=0x3FFF).contains(&addr) {
            self.shadow.on_read(addr);
        }
        value
    }

    fn write(&mut self, addr: u16, value: u8) {
        self.bus.write(addr, value);
        if (0x2000..=0x3FFF).contains(&addr) {
            self.shadow.on_write(addr, value, self.bus.cart.mirroring);
        }
    }
}
