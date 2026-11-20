//! 命令表: オペコード（命令の 1 バイト目）→ 命令の種類・アドレッシングモード・サイクル数

/// アドレッシングモード（オペランドの指定のしかた）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// オペランドなし（INX など）
    Implied,
    /// A レジスタが対象（LSR A）
    Accumulator,
    /// 即値（LDA #$10）
    Immediate,
    /// ゼロページ（LDA $10）
    ZeroPage,
    /// 絶対アドレス（LDA $1234）
    Absolute,
    /// 絶対アドレス + X（LDA $1234,X）
    AbsoluteX,
    /// 絶対アドレス + Y（LDA $1234,Y）
    AbsoluteY,
    /// 相対（分岐命令）
    Relative,
}

/// 命令の種類
#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instr {
    Adc, And, Bcc, Bcs, Beq, Bit, Bne, Bpl, Bvc, Bvs, Clc, Cld, Cmp, Cpx, Cpy,
    Dec, Dex, Dey, Eor, Inc, Inx, Iny, Jmp, Jsr, Lda, Ldx, Ldy, Lsr, Nop, Ora,
    Pha, Pla, Rol, Rti, Rts, Sbc, Sec, Sei, Sta, Stx, Tax, Tay, Txa, Txs,
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
    // この本のデモ ROM で使わない命令は、1 バイトの NOP として扱う
    let mut t = [op(Nop, Implied, 2); 256];

    // ロード・ストア
    t[0xA9] = op(Lda, Immediate, 2);
    t[0xA5] = op(Lda, ZeroPage, 3);
    t[0xAD] = op(Lda, Absolute, 4);
    t[0xBD] = op(Lda, AbsoluteX, 4);
    t[0xB9] = op(Lda, AbsoluteY, 4);
    t[0xA2] = op(Ldx, Immediate, 2);
    t[0xA6] = op(Ldx, ZeroPage, 3);
    t[0xA0] = op(Ldy, Immediate, 2);
    t[0x85] = op(Sta, ZeroPage, 3);
    t[0x8D] = op(Sta, Absolute, 4);
    t[0x9D] = op(Sta, AbsoluteX, 5);
    t[0x8E] = op(Stx, Absolute, 4);
    // レジスタ間の転送
    t[0xAA] = op(Tax, Implied, 2);
    t[0xA8] = op(Tay, Implied, 2);
    t[0x8A] = op(Txa, Implied, 2);
    t[0x9A] = op(Txs, Implied, 2);
    // スタック
    t[0x48] = op(Pha, Implied, 3);
    t[0x68] = op(Pla, Implied, 4);
    // 演算
    t[0x69] = op(Adc, Immediate, 2);
    t[0x65] = op(Adc, ZeroPage, 3);
    t[0x79] = op(Adc, AbsoluteY, 4);
    t[0xE5] = op(Sbc, ZeroPage, 3);
    t[0x29] = op(And, Immediate, 2);
    t[0x25] = op(And, ZeroPage, 3);
    t[0x09] = op(Ora, Immediate, 2);
    t[0x49] = op(Eor, Immediate, 2);
    t[0x2C] = op(Bit, Absolute, 4);
    t[0xC9] = op(Cmp, Immediate, 2);
    t[0xC5] = op(Cmp, ZeroPage, 3);
    t[0xE0] = op(Cpx, Immediate, 2);
    t[0xC0] = op(Cpy, Immediate, 2);
    // インクリメント・デクリメント
    t[0xE6] = op(Inc, ZeroPage, 5);
    t[0xC6] = op(Dec, ZeroPage, 5);
    t[0xE8] = op(Inx, Implied, 2);
    t[0xC8] = op(Iny, Implied, 2);
    t[0xCA] = op(Dex, Implied, 2);
    t[0x88] = op(Dey, Implied, 2);
    // シフト・ローテート
    t[0x4A] = op(Lsr, Accumulator, 2);
    t[0x26] = op(Rol, ZeroPage, 5);
    // ジャンプ・サブルーチン
    t[0x4C] = op(Jmp, Absolute, 3);
    t[0x20] = op(Jsr, Absolute, 6);
    t[0x60] = op(Rts, Implied, 6);
    t[0x40] = op(Rti, Implied, 6);
    // 分岐
    t[0x90] = op(Bcc, Relative, 2);
    t[0xB0] = op(Bcs, Relative, 2);
    t[0xD0] = op(Bne, Relative, 2);
    t[0xF0] = op(Beq, Relative, 2);
    t[0x10] = op(Bpl, Relative, 2);
    t[0x50] = op(Bvc, Relative, 2);
    t[0x70] = op(Bvs, Relative, 2);
    // フラグ操作
    t[0x18] = op(Clc, Implied, 2);
    t[0x38] = op(Sec, Implied, 2);
    t[0x78] = op(Sei, Implied, 2);
    t[0xD8] = op(Cld, Implied, 2);
    t[0xEA] = op(Nop, Implied, 2);

    t
}
