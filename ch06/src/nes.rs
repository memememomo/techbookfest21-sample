//! ファミコン本体: CPU とバス（その先のカートリッジなど）をまとめる

use crate::bus::NesBus;
use crate::cartridge::Cartridge;
use crate::cpu::Cpu;

pub struct Nes {
    pub cpu: Cpu,
    pub bus: NesBus,
}

impl Nes {
    /// カートリッジを挿して電源を入れる
    pub fn new(cart: Cartridge) -> Self {
        let mut nes = Nes {
            cpu: Cpu::new(),
            bus: NesBus::new(cart),
        };
        nes.cpu.reset(&mut nes.bus);
        nes
    }

    /// リセットボタン
    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
    }

    /// 命令を 1 つ実行する
    pub fn step(&mut self) -> u32 {
        self.cpu.step(&mut self.bus) as u32
    }
}
