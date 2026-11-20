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

    /// 命令を 1 つ（NMI が来ていれば NMI を）実行し、そのぶん PPU を進める
    pub fn step(&mut self) -> u32 {
        let cycles = if self.bus.ppu.take_nmi() {
            self.cpu.nmi(&mut self.bus)
        } else {
            self.cpu.step(&mut self.bus)
        } as u32;

        self.bus.tick(cycles);
        cycles
    }

    /// 1 フレーム（PPU が VBlank に入るまで）実行する
    pub fn run_frame(&mut self) {
        loop {
            self.step();
            if self.bus.ppu.take_frame() {
                break;
            }
        }
    }
}
