//! バス: CPU から見たメモリ空間（メモリマップ）
//!
//! | アドレス        | 中身                                        |
//! |-----------------|---------------------------------------------|
//! | $0000〜$07FF    | RAM（2 KiB）                                |
//! | $0800〜$1FFF    | RAM のミラー（同じ 2 KiB が 3 回くり返す）  |
//! | $2000〜$2007    | PPU のレジスタ                              |
//! | $2008〜$3FFF    | PPU のレジスタのミラー（8 バイトごと）      |
//! | $4000〜$4017    | APU・コントローラー・OAM DMA                |
//! | $4020〜$FFFF    | カートリッジ（$6000〜 ワーク RAM、$8000〜 PRG ROM） |

use crate::cartridge::Cartridge;
use crate::controller::Controller;
use crate::cpu::Bus;
use crate::ppu::Ppu;

pub struct NesBus {
    ram: [u8; 0x800],
    pub cart: Cartridge,
    pub ppu: Ppu,
    pub controller: Controller,
    /// OAM DMA で CPU が止まるサイクル数
    dma_cycles: u32,
}

impl NesBus {
    pub fn new(cart: Cartridge) -> Self {
        NesBus {
            ram: [0; 0x800],
            cart,
            ppu: Ppu::new(),
            controller: Controller::default(),
            dma_cycles: 0,
        }
    }

    /// CPU が cycles サイクル進んだぶん、PPU を進める（PPU は CPU の 3 倍の速さ）
    pub fn tick(&mut self, cycles: u32) {
        for _ in 0..cycles * 3 {
            self.ppu.step(&mut self.cart);
        }
    }

    /// OAM DMA のために止まるサイクル数を受け取る
    pub fn take_dma_cycles(&mut self) -> u32 {
        std::mem::take(&mut self.dma_cycles)
    }

    /// OAM DMA: $XX00〜$XXFF の 256 バイトを OAM に転送する
    fn oam_dma(&mut self, page: u8) {
        let base = (page as u16) << 8;
        for i in 0..256 {
            let value = self.read(base + i);
            // $2004 に書くのと同じ
            self.ppu.write_register(0x2004, value, &mut self.cart);
        }
        // 転送の間、CPU は 513 サイクル止まる
        self.dma_cycles += 513;
    }
}

impl Bus for NesBus {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            0x2000..=0x3FFF => self.ppu.read_register(addr, &mut self.cart),
            0x4016 => self.controller.read(),
            0x4000..=0x401F => 0, // 読めないレジスタ
            0x4020..=0xFFFF => self.cart.cpu_read(addr),
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize] = value,
            0x2000..=0x3FFF => self.ppu.write_register(addr, value, &mut self.cart),
            0x4014 => self.oam_dma(value),
            0x4016 => self.controller.write(value),
            0x4000..=0x401F => {}
            0x4020..=0xFFFF => {} // ROM には書き込めない
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_bus() -> NesBus {
        let mut data = vec![b'N', b'E', b'S', 0x1A, 1, 1, 0, 0];
        data.resize(16 + 16 * 1024 + 8 * 1024, 0);
        data[16] = 0xAB; // PRG ROM の先頭
        NesBus::new(Cartridge::from_ines(&data).unwrap())
    }

    #[test]
    fn ram_is_mirrored() {
        let mut bus = test_bus();
        bus.write(0x0001, 0x42);
        assert_eq!(bus.read(0x0801), 0x42);
        assert_eq!(bus.read(0x1801), 0x42);
    }

    #[test]
    fn reads_prg_rom() {
        let mut bus = test_bus();
        assert_eq!(bus.read(0x8000), 0xAB);
        assert_eq!(bus.read(0xC000), 0xAB);
    }

    #[test]
    fn oam_dma_copies_page() {
        let mut bus = test_bus();
        for i in 0..256u16 {
            bus.write(0x0200 + i, i as u8);
        }
        bus.write(0x4014, 0x02);
        assert_eq!(bus.ppu.oam[0], 0);
        assert_eq!(bus.ppu.oam[255], 255);
        assert_eq!(bus.take_dma_cycles(), 513);
    }
}
