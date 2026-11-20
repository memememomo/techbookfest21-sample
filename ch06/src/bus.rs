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
use crate::cpu::Bus;

pub struct NesBus {
    ram: [u8; 0x800],
    pub cart: Cartridge,
}

impl NesBus {
    pub fn new(cart: Cartridge) -> Self {
        NesBus {
            ram: [0; 0x800],
            cart,
        }
    }
}

impl Bus for NesBus {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            0x2000..=0x401F => 0, // PPU や APU（まだ作っていない）
            0x4020..=0xFFFF => self.cart.cpu_read(addr),
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize] = value,
            0x2000..=0x401F => {} // PPU や APU（まだ作っていない）
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
}
