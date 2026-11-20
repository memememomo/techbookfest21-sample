//! CPU（ファミコンの 2A03 のうち、6502 と同じ部分）

use crate::opcodes::{Instr, Mode, OPCODES};

/// CPU から見たメモリ（バス）。
/// 読み込みにも副作用がある（PPU のレジスタなど）ので、read も &mut self を取る
pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, value: u8);
}

// ステータスレジスタ P の各ビット
pub const FLAG_C: u8 = 1 << 0; // キャリー
pub const FLAG_Z: u8 = 1 << 1; // ゼロ
pub const FLAG_I: u8 = 1 << 2; // IRQ 禁止
pub const FLAG_D: u8 = 1 << 3; // 10 進モード（ファミコンでは効果がない）
pub const FLAG_B: u8 = 1 << 4; // BRK（スタックに積んだ値の中にだけ現れる）
pub const FLAG_U: u8 = 1 << 5; // 未使用（常に 1）
pub const FLAG_V: u8 = 1 << 6; // オーバーフロー
pub const FLAG_N: u8 = 1 << 7; // ネガティブ（負）

pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    /// スタックポインタ（スタックは $0100〜$01FF）
    pub sp: u8,
    pub pc: u16,
    /// ステータスレジスタ
    pub p: u8,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            a: 0,
            x: 0,
            y: 0,
            sp: 0,
            pc: 0,
            p: FLAG_U,
        }
    }

    /// リセット: $FFFC〜$FFFD に書かれたアドレスから実行を始める
    pub fn reset(&mut self, bus: &mut impl Bus) {
        // リセット中の CPU は、書き込みをしないまま 3 回プッシュしたように SP を減らす
        self.sp = self.sp.wrapping_sub(3);
        self.p |= FLAG_I;
        self.pc = self.read_u16(bus, 0xFFFC);
    }

    pub fn flag(&self, flag: u8) -> bool {
        self.p & flag != 0
    }

    fn set_flag(&mut self, flag: u8, on: bool) {
        if on {
            self.p |= flag;
        } else {
            self.p &= !flag;
        }
    }

    /// 結果の値に応じて Z と N を設定する
    fn set_zn(&mut self, value: u8) {
        self.set_flag(FLAG_Z, value == 0);
        self.set_flag(FLAG_N, value & 0x80 != 0);
    }

    fn read_u16(&mut self, bus: &mut impl Bus, addr: u16) -> u16 {
        let lo = bus.read(addr) as u16;
        let hi = bus.read(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    /// PC の位置から 1 バイト読んで、PC を進める
    fn fetch(&mut self, bus: &mut impl Bus) -> u8 {
        let value = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        value
    }

    fn fetch_u16(&mut self, bus: &mut impl Bus) -> u16 {
        let lo = self.fetch(bus) as u16;
        let hi = self.fetch(bus) as u16;
        (hi << 8) | lo
    }

    /// 命令を 1 つ実行し、かかったサイクル数を返す
    pub fn step(&mut self, bus: &mut impl Bus) -> u8 {
        let opcode = self.fetch(bus);
        let op = OPCODES[opcode as usize];
        let addr = self.operand_address(bus, op.mode);
        self.execute(bus, op.instr, addr);
        op.cycles
    }

    /// オペランドを読み、命令が読み書きするアドレスを求める
    fn operand_address(&mut self, bus: &mut impl Bus, mode: Mode) -> u16 {
        match mode {
            Mode::Implied => 0,
            // 即値は、オペランドの場所そのものを読み込む
            Mode::Immediate => {
                let addr = self.pc;
                self.pc = self.pc.wrapping_add(1);
                addr
            }
            Mode::ZeroPage => self.fetch(bus) as u16,
            Mode::Absolute => self.fetch_u16(bus),
        }
    }

    /// 命令を実行する
    fn execute(&mut self, bus: &mut impl Bus, instr: Instr, addr: u16) {
        match instr {
            // ---- ロード・ストア ----
            Instr::Lda => {
                self.a = bus.read(addr);
                self.set_zn(self.a);
            }
            Instr::Ldx => {
                self.x = bus.read(addr);
                self.set_zn(self.x);
            }
            Instr::Ldy => {
                self.y = bus.read(addr);
                self.set_zn(self.y);
            }
            Instr::Sta => bus.write(addr, self.a),
            Instr::Stx => bus.write(addr, self.x),

            // ---- レジスタ間の転送 ----
            Instr::Tax => {
                self.x = self.a;
                self.set_zn(self.x);
            }
            Instr::Tay => {
                self.y = self.a;
                self.set_zn(self.y);
            }
            Instr::Txa => {
                self.a = self.x;
                self.set_zn(self.a);
            }

            // ---- インクリメント・デクリメント ----
            Instr::Inx => {
                self.x = self.x.wrapping_add(1);
                self.set_zn(self.x);
            }
            Instr::Iny => {
                self.y = self.y.wrapping_add(1);
                self.set_zn(self.y);
            }
            Instr::Dex => {
                self.x = self.x.wrapping_sub(1);
                self.set_zn(self.x);
            }
            Instr::Dey => {
                self.y = self.y.wrapping_sub(1);
                self.set_zn(self.y);
            }

            // ---- ジャンプ・サブルーチン ----
            Instr::Jmp => self.pc = addr,
            Instr::Nop => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// テスト用のバス: 64 KiB がすべて RAM
    struct TestBus {
        mem: Vec<u8>,
    }

    impl Bus for TestBus {
        fn read(&mut self, addr: u16) -> u8 {
            self.mem[addr as usize]
        }
        fn write(&mut self, addr: u16, value: u8) {
            self.mem[addr as usize] = value;
        }
    }

    /// $8000 にプログラムを置き、リセットした CPU を作る
    fn setup(program: &[u8]) -> (Cpu, TestBus) {
        let mut bus = TestBus {
            mem: vec![0; 0x10000],
        };
        bus.mem[0x8000..0x8000 + program.len()].copy_from_slice(program);
        bus.mem[0xFFFC] = 0x00;
        bus.mem[0xFFFD] = 0x80;
        let mut cpu = Cpu::new();
        cpu.reset(&mut bus);
        (cpu, bus)
    }

    #[test]
    fn reset_reads_vector() {
        let (cpu, _) = setup(&[]);
        assert_eq!(cpu.pc, 0x8000);
        assert_eq!(cpu.sp, 0xFD);
        assert_eq!(cpu.p, 0x24);
    }

    #[test]
    fn lda_immediate_sets_flags() {
        let (mut cpu, mut bus) = setup(&[0xA9, 0x00, 0xA9, 0x80]); // LDA #$00; LDA #$80
        assert_eq!(cpu.step(&mut bus), 2);
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.flag(FLAG_Z));
        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x80);
        assert!(!cpu.flag(FLAG_Z));
        assert!(cpu.flag(FLAG_N));
    }

    #[test]
    fn sta_and_transfer() {
        // LDA #$42; TAX; INX; STX $0200
        let (mut cpu, mut bus) = setup(&[0xA9, 0x42, 0xAA, 0xE8, 0x8E, 0x00, 0x02]);
        for _ in 0..4 {
            cpu.step(&mut bus);
        }
        assert_eq!(cpu.x, 0x43);
        assert_eq!(bus.mem[0x0200], 0x43);
    }
}
