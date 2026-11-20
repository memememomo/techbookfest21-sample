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

    fn push(&mut self, bus: &mut impl Bus, value: u8) {
        bus.write(0x0100 | self.sp as u16, value);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn pop(&mut self, bus: &mut impl Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        bus.read(0x0100 | self.sp as u16)
    }

    fn push_u16(&mut self, bus: &mut impl Bus, value: u16) {
        self.push(bus, (value >> 8) as u8);
        self.push(bus, value as u8);
    }

    fn pop_u16(&mut self, bus: &mut impl Bus) -> u16 {
        let lo = self.pop(bus) as u16;
        let hi = self.pop(bus) as u16;
        (hi << 8) | lo
    }

    /// 命令を 1 つ実行し、かかったサイクル数を返す
    pub fn step(&mut self, bus: &mut impl Bus) -> u8 {
        let opcode = self.fetch(bus);
        let op = OPCODES[opcode as usize];
        let addr = self.operand_address(bus, op.mode);
        self.execute(bus, op.instr, op.mode, addr);
        op.cycles
    }

    /// オペランドを読み、命令が読み書きするアドレスを求める
    fn operand_address(&mut self, bus: &mut impl Bus, mode: Mode) -> u16 {
        match mode {
            Mode::Implied | Mode::Accumulator => 0,
            // 即値は、オペランドの場所そのものを読み込む
            Mode::Immediate => {
                let addr = self.pc;
                self.pc = self.pc.wrapping_add(1);
                addr
            }
            Mode::ZeroPage => self.fetch(bus) as u16,
            Mode::Absolute => self.fetch_u16(bus),
            Mode::AbsoluteX => self.fetch_u16(bus).wrapping_add(self.x as u16),
            Mode::AbsoluteY => self.fetch_u16(bus).wrapping_add(self.y as u16),
            Mode::Relative => {
                // 分岐先 = 次の命令のアドレス + 符号付きのオフセット
                let offset = self.fetch(bus) as i8;
                self.pc.wrapping_add(offset as u16)
            }
        }
    }

    /// 命令を実行する
    fn execute(&mut self, bus: &mut impl Bus, instr: Instr, mode: Mode, addr: u16) {
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
            Instr::Txs => self.sp = self.x,

            // ---- スタック ----
            Instr::Pha => self.push(bus, self.a),
            Instr::Pla => {
                self.a = self.pop(bus);
                self.set_zn(self.a);
            }

            // ---- 演算 ----
            Instr::Adc => {
                let value = bus.read(addr);
                self.add(value);
            }
            Instr::Sbc => {
                // A - M - (1 - C) = A + (M のビット反転) + C
                let value = bus.read(addr);
                self.add(!value);
            }
            Instr::And => {
                self.a &= bus.read(addr);
                self.set_zn(self.a);
            }
            Instr::Ora => {
                self.a |= bus.read(addr);
                self.set_zn(self.a);
            }
            Instr::Eor => {
                self.a ^= bus.read(addr);
                self.set_zn(self.a);
            }
            Instr::Bit => {
                let value = bus.read(addr);
                self.set_flag(FLAG_Z, self.a & value == 0);
                self.set_flag(FLAG_V, value & 0x40 != 0);
                self.set_flag(FLAG_N, value & 0x80 != 0);
            }
            Instr::Cmp => {
                let value = bus.read(addr);
                self.compare(self.a, value);
            }
            Instr::Cpx => {
                let value = bus.read(addr);
                self.compare(self.x, value);
            }
            Instr::Cpy => {
                let value = bus.read(addr);
                self.compare(self.y, value);
            }

            // ---- インクリメント・デクリメント ----
            Instr::Inc => {
                let value = bus.read(addr).wrapping_add(1);
                bus.write(addr, value);
                self.set_zn(value);
            }
            Instr::Dec => {
                let value = bus.read(addr).wrapping_sub(1);
                bus.write(addr, value);
                self.set_zn(value);
            }
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

            // ---- シフト・ローテート ----
            Instr::Lsr | Instr::Rol => {
                if mode == Mode::Accumulator {
                    self.a = self.shift(instr, self.a);
                } else {
                    let value = bus.read(addr);
                    let result = self.shift(instr, value);
                    bus.write(addr, result);
                }
            }

            // ---- ジャンプ・サブルーチン ----
            Instr::Jmp => self.pc = addr,
            Instr::Jsr => {
                // 戻り先 - 1（JSR の最後のバイトのアドレス）を積む
                self.push_u16(bus, self.pc.wrapping_sub(1));
                self.pc = addr;
            }
            Instr::Rts => self.pc = self.pop_u16(bus).wrapping_add(1),
            Instr::Rti => {
                let value = self.pop(bus);
                self.p = (value & !FLAG_B) | FLAG_U;
                self.pc = self.pop_u16(bus);
            }

            // ---- 分岐 ----
            Instr::Bcc => self.branch(!self.flag(FLAG_C), addr),
            Instr::Bcs => self.branch(self.flag(FLAG_C), addr),
            Instr::Bne => self.branch(!self.flag(FLAG_Z), addr),
            Instr::Beq => self.branch(self.flag(FLAG_Z), addr),
            Instr::Bpl => self.branch(!self.flag(FLAG_N), addr),
            Instr::Bvc => self.branch(!self.flag(FLAG_V), addr),
            Instr::Bvs => self.branch(self.flag(FLAG_V), addr),

            // ---- フラグ操作 ----
            Instr::Clc => self.set_flag(FLAG_C, false),
            Instr::Sec => self.set_flag(FLAG_C, true),
            Instr::Sei => self.set_flag(FLAG_I, true),
            Instr::Cld => self.set_flag(FLAG_D, false),
            Instr::Nop => {}
        }
    }

    /// A + value + C を計算して A に入れる（ADC と SBC の共通部分）
    fn add(&mut self, value: u8) {
        let sum = self.a as u16 + value as u16 + (self.p & FLAG_C) as u16;
        let result = sum as u8;
        self.set_flag(FLAG_C, sum > 0xFF);
        // 符号付きの数として見たときのオーバーフロー:
        // 同じ符号どうしを足したのに、結果の符号が変わった
        self.set_flag(FLAG_V, (self.a ^ result) & (value ^ result) & 0x80 != 0);
        self.a = result;
        self.set_zn(result);
    }

    fn compare(&mut self, register: u8, value: u8) {
        self.set_flag(FLAG_C, register >= value);
        self.set_zn(register.wrapping_sub(value));
    }

    /// LSR（右へずらす）と ROL（左へ回す）の計算。はみ出したビットはキャリーに入る
    fn shift(&mut self, instr: Instr, value: u8) -> u8 {
        let (result, carry_out) = if instr == Instr::Lsr {
            (value >> 1, value & 0x01 != 0)
        } else {
            ((value << 1) | (self.p & FLAG_C), value & 0x80 != 0)
        };
        self.set_flag(FLAG_C, carry_out);
        self.set_zn(result);
        result
    }

    /// 条件が成り立てば分岐する
    fn branch(&mut self, condition: bool, target: u16) {
        if condition {
            self.pc = target;
        }
    }

    /// NMI（マスクできない割り込み）。PPU が VBlank に入ったときに起きる
    pub fn nmi(&mut self, bus: &mut impl Bus) -> u8 {
        self.push_u16(bus, self.pc);
        // 割り込みで積むときは B フラグを 0 にする
        self.push(bus, (self.p & !FLAG_B) | FLAG_U);
        self.p |= FLAG_I;
        self.pc = self.read_u16(bus, 0xFFFA);
        7
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

    #[test]
    fn adc_sets_carry_and_overflow() {
        // CLC; LDA #$50; ADC #$50 → $A0（正 + 正 = 負 なのでオーバーフロー）
        let (mut cpu, mut bus) = setup(&[0x18, 0xA9, 0x50, 0x69, 0x50]);
        for _ in 0..3 {
            cpu.step(&mut bus);
        }
        assert_eq!(cpu.a, 0xA0);
        assert!(cpu.flag(FLAG_V));
        assert!(!cpu.flag(FLAG_C));
    }

    #[test]
    fn sbc_borrows() {
        // SEC; LDA #$10; SBC $00（$00 は $20）→ $F0、キャリー（借りがないこと）は 0
        let (mut cpu, mut bus) = setup(&[0x38, 0xA9, 0x10, 0xE5, 0x00]);
        bus.mem[0x0000] = 0x20;
        for _ in 0..3 {
            cpu.step(&mut bus);
        }
        assert_eq!(cpu.a, 0xF0);
        assert!(!cpu.flag(FLAG_C));
        assert!(cpu.flag(FLAG_N));
    }

    #[test]
    fn jsr_and_rts() {
        // JSR $8010; （$8010: LDX #$07; RTS）
        let mut program = vec![0x20, 0x10, 0x80, 0xEA];
        program.resize(0x10, 0xEA);
        program.extend_from_slice(&[0xA2, 0x07, 0x60]);
        let (mut cpu, mut bus) = setup(&program);
        assert_eq!(cpu.step(&mut bus), 6);
        assert_eq!(cpu.pc, 0x8010);
        assert_eq!(cpu.sp, 0xFB);
        cpu.step(&mut bus);
        assert_eq!(cpu.step(&mut bus), 6);
        assert_eq!(cpu.pc, 0x8003);
        assert_eq!(cpu.x, 7);
    }

    #[test]
    fn branch() {
        // LDX #$00（Z=1）; BEQ +2（分岐する）; NOP; NOP; BNE -4（分岐しない）
        let (mut cpu, mut bus) = setup(&[0xA2, 0x00, 0xF0, 0x02, 0xEA, 0xEA, 0xD0, 0xFC]);
        cpu.step(&mut bus);
        cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x8006);
        cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x8008);
    }

    #[test]
    fn nmi_pushes_state() {
        let (mut cpu, mut bus) = setup(&[0xEA]);
        bus.mem[0xFFFA] = 0x00;
        bus.mem[0xFFFB] = 0x90;
        assert_eq!(cpu.nmi(&mut bus), 7);
        assert_eq!(cpu.pc, 0x9000);
        assert_eq!(bus.mem[0x01FD], 0x80); // 戻り先の上位
        assert_eq!(bus.mem[0x01FC], 0x00); // 戻り先の下位
        assert_eq!(bus.mem[0x01FB] & FLAG_B, 0); // B フラグは 0
        assert!(cpu.flag(FLAG_I));
    }
}
