//! 命令表: オペコード（命令の 1 バイト目）→ 命令の種類・アドレッシングモード・サイクル数

/// アドレッシングモード（オペランドの指定のしかた）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// オペランドなし（INX など）
    Implied,
    /// 即値（LDA #$10）
    Immediate,
    /// ゼロページ（LDA $10）
    ZeroPage,
    /// 絶対アドレス（LDA $1234）
    Absolute,
}

/// 命令の種類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instr {
    Lda,
    Ldx,
    Ldy,
    Sta,
    Stx,
    Tax,
    Tay,
    Txa,
    Inx,
    Iny,
    Dex,
    Dey,
    Jmp,
    Nop,
}

#[derive(Debug, Clone, Copy)]
pub struct Opcode {
    pub instr: Instr,
    pub mode: Mode,
    pub cycles: u8,
}

const fn op(instr: Instr, mode: Mode, cycles: u8) -> Opcode {
    Opcode {
        instr,
        mode,
        cycles,
    }
}

/// 256 個のオペコードの表
pub static OPCODES: [Opcode; 256] = build_table();

const fn build_table() -> [Opcode; 256] {
    use Instr::*;
    use Mode::*;
    // まだ実装していない命令は、とりあえず 1 バイトの NOP として扱う
    let mut t = [op(Nop, Implied, 2); 256];

    t[0xA9] = op(Lda, Immediate, 2);
    t[0xA5] = op(Lda, ZeroPage, 3);
    t[0xAD] = op(Lda, Absolute, 4);
    t[0xA2] = op(Ldx, Immediate, 2);
    t[0xA6] = op(Ldx, ZeroPage, 3);
    t[0xA0] = op(Ldy, Immediate, 2);
    t[0x85] = op(Sta, ZeroPage, 3);
    t[0x8D] = op(Sta, Absolute, 4);
    t[0x8E] = op(Stx, Absolute, 4);
    t[0xAA] = op(Tax, Implied, 2);
    t[0xA8] = op(Tay, Implied, 2);
    t[0x8A] = op(Txa, Implied, 2);
    t[0xE8] = op(Inx, Implied, 2);
    t[0xC8] = op(Iny, Implied, 2);
    t[0xCA] = op(Dex, Implied, 2);
    t[0x88] = op(Dey, Implied, 2);
    t[0x4C] = op(Jmp, Absolute, 3);
    t[0xEA] = op(Nop, Implied, 2);

    t
}
