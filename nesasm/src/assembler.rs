//! Minimal line-oriented 6502 assembler aligned with `crate::cpu` opcode decoding.
//!
//! 構文の概要（詳しくは `README.md`）:
//!
//! - 命令: `lda #$10` / `sta $2000` / `lda table,x` / `lda (ptr),y` / `jmp (vector)`
//! - ラベル: `name:`（大文字・小文字は区別しない。前方参照できる）
//! - 定数: `NAME = 式`
//! - 式: 数値（`$FF`, `%1010`, `0x1F`, `10`, `'A'`）、ラベル・定数、`+ - * /`、括弧、
//!   単項演算子 `<`（下位バイト）・`>`（上位バイト）・`-`
//! - 疑似命令: `.org 式`、`.byte 式|"文字列", ...`（`.db`）、`.word 式, ...`（`.dw`）
//! - コメント: `;` から行末まで
//!
//! 出力はアドレス 0 から始まる連続したバイト列で、`.org` で飛ばした部分は 0 で埋める。

use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssembleError {
    pub line: usize,
    pub message: String,
}

impl AssembleError {
    fn new(line: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AddrKind {
    Impl,
    Acc,
    Imm,
    Zp,
    ZpX,
    ZpY,
    Abs,
    AbsX,
    AbsY,
    Ind,
    IndX,
    IndY,
    Rel,
}

/// 1 行ぶんの文（ラベルは別に持つ）。
#[derive(Clone, Copy, Debug)]
enum Stmt<'a> {
    Empty,
    Org(&'a str),
    Const(&'a str, &'a str),
    Bytes(&'a str),
    Words(&'a str),
    Instr(&'a str, &'a str),
}

struct Line<'a> {
    no: usize,
    label: Option<&'a str>,
    stmt: Stmt<'a>,
}

type Symbols = HashMap<String, i32>;

/// Assemble 6502 source; output is contiguous bytes with `.org` inserting zero padding.
pub fn assemble(source: &str) -> Result<Vec<u8>, AssembleError> {
    let raw_lines: Vec<&str> = source.split('\n').collect();
    let mut lines = Vec::with_capacity(raw_lines.len());
    for (i, raw) in raw_lines.iter().enumerate() {
        lines.push(parse_line(i + 1, raw.trim_end_matches('\r'))?);
    }

    // ---- パス 1: ラベルのアドレスと各命令のアドレッシングモード（=長さ）を決める ----
    let mut symbols: Symbols = HashMap::new();
    let mut pending_consts: Vec<(usize, &str, &str)> = Vec::new();
    let mut kinds: Vec<Option<AddrKind>> = vec![None; lines.len()];
    let mut pc: u32 = 0;

    for (idx, line) in lines.iter().enumerate() {
        let no = line.no;
        if let Stmt::Org(expr) = line.stmt {
            let target = match eval_expr(expr, &symbols) {
                Ok(v) => v,
                Err(EvalError::Undefined(name)) => {
                    return Err(AssembleError::new(
                        no,
                        format!(".org cannot use a symbol defined later (`{name}`)"),
                    ))
                }
                Err(EvalError::Syntax(msg)) => return Err(AssembleError::new(no, msg)),
            };
            if !(0..=0xFFFF).contains(&target) {
                return Err(AssembleError::new(no, format!(".org address out of range ({target})")));
            }
            if (target as u32) < pc {
                return Err(AssembleError::new(
                    no,
                    format!(".org ${target:04X} moves backward (current ${pc:04X})"),
                ));
            }
            pc = target as u32;
            continue;
        }
        if let Some(name) = line.label {
            define_symbol(&mut symbols, name, pc as i32, no)?;
        }
        match line.stmt {
            Stmt::Empty | Stmt::Org(_) => {}
            Stmt::Const(name, expr) => match eval_expr(expr, &symbols) {
                Ok(v) => define_symbol(&mut symbols, name, v, no)?,
                Err(EvalError::Undefined(_)) => pending_consts.push((no, name, expr)),
                Err(EvalError::Syntax(msg)) => return Err(AssembleError::new(no, msg)),
            },
            Stmt::Bytes(args) => pc += byte_list_len(args, no)?,
            Stmt::Words(args) => pc += 2 * split_args(args, no)?.len() as u32,
            Stmt::Instr(mnemonic, operand) => {
                let kind = classify(mnemonic, operand, &symbols, no)?;
                kinds[idx] = Some(kind);
                pc += instr_len(kind);
            }
        }
        if pc > 0x1_0000 {
            return Err(AssembleError::new(no, "program runs past $FFFF"));
        }
    }

    // 後ろで定義されたラベルを参照する定数を解決する
    while !pending_consts.is_empty() {
        let before = pending_consts.len();
        let mut rest = Vec::new();
        for (no, name, expr) in pending_consts {
            match eval_expr(expr, &symbols) {
                Ok(v) => define_symbol(&mut symbols, name, v, no)?,
                Err(EvalError::Undefined(_)) => rest.push((no, name, expr)),
                Err(EvalError::Syntax(msg)) => return Err(AssembleError::new(no, msg)),
            }
        }
        if rest.len() == before {
            let (no, _, expr) = rest[0];
            let err = eval_expr(expr, &symbols).unwrap_err();
            return Err(AssembleError::new(no, err.message()));
        }
        pending_consts = rest;
    }

    // ---- パス 2: 機械語を出力する ----
    let mut out: Vec<u8> = Vec::new();
    pc = 0;
    for (idx, line) in lines.iter().enumerate() {
        let no = line.no;
        let eval = |expr: &str| -> Result<i32, AssembleError> {
            eval_expr(expr, &symbols).map_err(|e| AssembleError::new(no, e.message()))
        };
        match line.stmt {
            Stmt::Empty | Stmt::Const(..) => {}
            Stmt::Org(expr) => pc = eval(expr)? as u32,
            Stmt::Bytes(args) => {
                pad_to(&mut out, pc);
                for arg in split_args(args, no)? {
                    if let Some(text) = string_literal(arg) {
                        out.extend_from_slice(text.as_bytes());
                    } else {
                        out.push(to_byte(eval(arg)?, no)?);
                    }
                }
                pc = out.len() as u32;
            }
            Stmt::Words(args) => {
                pad_to(&mut out, pc);
                for arg in split_args(args, no)? {
                    let v = to_word(eval(arg)?, no)?;
                    out.extend_from_slice(&v.to_le_bytes());
                }
                pc = out.len() as u32;
            }
            Stmt::Instr(mnemonic, operand) => {
                pad_to(&mut out, pc);
                let kind = kinds[idx].expect("classified in pass 1");
                emit(&mut out, mnemonic, kind, operand, pc, &eval, no)?;
                pc = out.len() as u32;
            }
        }
    }
    Ok(out)
}

fn pad_to(out: &mut Vec<u8>, pc: u32) {
    if out.len() < pc as usize {
        out.resize(pc as usize, 0);
    }
}

fn define_symbol(symbols: &mut Symbols, name: &str, value: i32, line: usize) -> Result<(), AssembleError> {
    let key = name.to_ascii_uppercase();
    if symbols.contains_key(&key) {
        return Err(AssembleError::new(line, format!("`{name}` is defined twice")));
    }
    symbols.insert(key, value);
    Ok(())
}

// ---- 行の解析 ------------------------------------------------------------------

fn parse_line(no: usize, raw: &str) -> Result<Line<'_>, AssembleError> {
    let text = strip_comment(raw).trim();
    let (label, rest) = split_label(text);
    let stmt = parse_stmt(rest, no)?;
    if label.is_some() && matches!(stmt, Stmt::Org(_) | Stmt::Const(..)) {
        return Err(AssembleError::new(no, "label cannot be on the same line as .org or `=`"));
    }
    Ok(Line { no, label, stmt })
}

/// `;` 以降を取り除く（文字列・文字リテラルの中の `;` は残す）。
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (i, c) in line.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == ';' => return &line[..i],
            None => {}
        }
    }
    line
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '.' || c == '@'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '@'
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if is_ident_start(c)) && chars.all(is_ident_char)
}

fn split_label(text: &str) -> (Option<&str>, &str) {
    if let Some(colon) = text.find(':') {
        let name = text[..colon].trim();
        if is_ident(name) {
            return (Some(name), text[colon + 1..].trim());
        }
    }
    (None, text)
}

fn parse_stmt(text: &str, no: usize) -> Result<Stmt<'_>, AssembleError> {
    if text.is_empty() {
        return Ok(Stmt::Empty);
    }
    let (word, rest) = match text.find(char::is_whitespace) {
        Some(i) => (&text[..i], text[i..].trim()),
        None => (text, ""),
    };
    // NAME = 式
    if let Some(eq) = text.find('=') {
        let name = text[..eq].trim();
        if is_ident(name) {
            let expr = text[eq + 1..].trim();
            if expr.is_empty() {
                return Err(AssembleError::new(no, format!("`{name} =` needs a value")));
            }
            return Ok(Stmt::Const(name, expr));
        }
    }
    let upper = word.to_ascii_uppercase();
    let need_args = |what: &str| -> Result<(), AssembleError> {
        if rest.is_empty() {
            Err(AssembleError::new(no, format!("{what} needs a value")))
        } else {
            Ok(())
        }
    };
    match upper.as_str() {
        ".ORG" | "ORG" => {
            need_args(".org")?;
            Ok(Stmt::Org(rest))
        }
        ".BYTE" | ".DB" => {
            need_args(".byte")?;
            Ok(Stmt::Bytes(rest))
        }
        ".WORD" | ".DW" => {
            need_args(".word")?;
            Ok(Stmt::Words(rest))
        }
        _ if upper.starts_with('.') => Err(AssembleError::new(no, format!("unknown directive `{word}`"))),
        _ => Ok(Stmt::Instr(word, rest)),
    }
}

/// カンマで区切る（文字列・文字リテラル・括弧の中のカンマでは区切らない）。
fn split_args(args: &str, no: usize) -> Result<Vec<&str>, AssembleError> {
    let mut parts = Vec::new();
    let mut quote: Option<char> = None;
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in args.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                '"' | '\'' => quote = Some(c),
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    parts.push(args[start..i].trim());
                    start = i + 1;
                }
                _ => {}
            },
        }
    }
    if quote.is_some() {
        return Err(AssembleError::new(no, "unterminated string"));
    }
    parts.push(args[start..].trim());
    if parts.iter().any(|p| p.is_empty()) {
        return Err(AssembleError::new(no, "empty value in .byte/.word list"));
    }
    Ok(parts)
}

fn string_literal(arg: &str) -> Option<&str> {
    let inner = arg.strip_prefix('"')?.strip_suffix('"')?;
    Some(inner)
}

fn byte_list_len(args: &str, no: usize) -> Result<u32, AssembleError> {
    let mut n = 0;
    for arg in split_args(args, no)? {
        n += match string_literal(arg) {
            Some(text) => text.len() as u32,
            None => 1,
        };
    }
    Ok(n)
}

fn to_byte(v: i32, line: usize) -> Result<u8, AssembleError> {
    if (-128..=255).contains(&v) {
        Ok(v as u8)
    } else {
        Err(AssembleError::new(line, format!("value {v} does not fit in a byte")))
    }
}

fn to_word(v: i32, line: usize) -> Result<u16, AssembleError> {
    if (-32768..=0xFFFF).contains(&v) {
        Ok(v as u16)
    } else {
        Err(AssembleError::new(line, format!("value {v} does not fit in a word")))
    }
}

// ---- 式の評価 ------------------------------------------------------------------

#[derive(Debug)]
enum EvalError {
    /// まだ定義されていないシンボル（パス 1 では「値不明」として扱う）
    Undefined(String),
    Syntax(String),
}

impl EvalError {
    fn message(&self) -> String {
        match self {
            EvalError::Undefined(name) => format!("unknown symbol `{name}`"),
            EvalError::Syntax(msg) => msg.clone(),
        }
    }
}

fn eval_expr(text: &str, symbols: &Symbols) -> Result<i32, EvalError> {
    let mut p = ExprParser {
        s: text.as_bytes(),
        i: 0,
        symbols,
    };
    let v = p.expr()?;
    p.skip_ws();
    if p.i != p.s.len() {
        return Err(EvalError::Syntax(format!("unexpected `{}` in expression `{text}`", &text[p.i..])));
    }
    Ok(v)
}

struct ExprParser<'a> {
    s: &'a [u8],
    i: usize,
    symbols: &'a Symbols,
}

impl ExprParser<'_> {
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.s.get(self.i).copied()
    }

    fn expr(&mut self) -> Result<i32, EvalError> {
        let mut v = self.term()?;
        loop {
            match self.peek() {
                Some(b'+') => {
                    self.i += 1;
                    v = v.wrapping_add(self.term()?);
                }
                Some(b'-') => {
                    self.i += 1;
                    v = v.wrapping_sub(self.term()?);
                }
                _ => return Ok(v),
            }
        }
    }

    fn term(&mut self) -> Result<i32, EvalError> {
        let mut v = self.unary()?;
        loop {
            match self.peek() {
                Some(b'*') => {
                    self.i += 1;
                    v = v.wrapping_mul(self.unary()?);
                }
                Some(b'/') => {
                    self.i += 1;
                    let d = self.unary()?;
                    if d == 0 {
                        return Err(EvalError::Syntax("division by zero".into()));
                    }
                    v /= d;
                }
                _ => return Ok(v),
            }
        }
    }

    fn unary(&mut self) -> Result<i32, EvalError> {
        match self.peek() {
            Some(b'<') => {
                self.i += 1;
                Ok(self.unary()? & 0xFF)
            }
            Some(b'>') => {
                self.i += 1;
                Ok((self.unary()? >> 8) & 0xFF)
            }
            Some(b'-') => {
                self.i += 1;
                Ok(self.unary()?.wrapping_neg())
            }
            Some(b'(') => {
                self.i += 1;
                let v = self.expr()?;
                if self.peek() != Some(b')') {
                    return Err(EvalError::Syntax("missing `)`".into()));
                }
                self.i += 1;
                Ok(v)
            }
            Some(b'$') => {
                self.i += 1;
                self.digits(16)
            }
            Some(b'%') => {
                self.i += 1;
                self.digits(2)
            }
            Some(b'\'') => {
                let c = *self.s.get(self.i + 1).ok_or_else(|| EvalError::Syntax("bad character literal".into()))?;
                if self.s.get(self.i + 2) != Some(&b'\'') {
                    return Err(EvalError::Syntax("bad character literal".into()));
                }
                self.i += 3;
                Ok(c as i32)
            }
            Some(c) if c.is_ascii_digit() => {
                if c == b'0' && matches!(self.s.get(self.i + 1), Some(b'x' | b'X')) {
                    self.i += 2;
                    self.digits(16)
                } else {
                    self.digits(10)
                }
            }
            Some(c) if is_ident_start(c as char) => {
                let start = self.i;
                while self.i < self.s.len() && is_ident_char(self.s[self.i] as char) {
                    self.i += 1;
                }
                let name = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("");
                self.symbols
                    .get(&name.to_ascii_uppercase())
                    .copied()
                    .ok_or_else(|| EvalError::Undefined(name.to_string()))
            }
            Some(c) => Err(EvalError::Syntax(format!("unexpected `{}` in expression", c as char))),
            None => Err(EvalError::Syntax("missing value".into())),
        }
    }

    fn digits(&mut self, radix: u32) -> Result<i32, EvalError> {
        let mut v: i64 = 0;
        let mut n = 0;
        while let Some(&c) = self.s.get(self.i) {
            if c == b'_' {
                self.i += 1;
                continue;
            }
            match (c as char).to_digit(radix) {
                Some(d) => {
                    v = v * radix as i64 + d as i64;
                    if v > 0xFF_FFFF {
                        return Err(EvalError::Syntax("number too large".into()));
                    }
                    self.i += 1;
                    n += 1;
                }
                None => break,
            }
        }
        if n == 0 {
            return Err(EvalError::Syntax("missing digits".into()));
        }
        Ok(v as i32)
    }
}

// ---- 命令 ----------------------------------------------------------------------

fn instr_len(kind: AddrKind) -> u32 {
    match kind {
        AddrKind::Impl | AddrKind::Acc => 1,
        AddrKind::Imm
        | AddrKind::Zp
        | AddrKind::ZpX
        | AddrKind::ZpY
        | AddrKind::IndX
        | AddrKind::IndY
        | AddrKind::Rel => 2,
        AddrKind::Abs | AddrKind::AbsX | AddrKind::AbsY | AddrKind::Ind => 3,
    }
}

fn branch_opcode(mnemonic: &str) -> Option<u8> {
    Some(match mnemonic {
        "BPL" => 0x10,
        "BMI" => 0x30,
        "BVC" => 0x50,
        "BVS" => 0x70,
        "BCC" => 0x90,
        "BCS" => 0xB0,
        "BNE" => 0xD0,
        "BEQ" => 0xF0,
        _ => return None,
    })
}

/// `(式,X)` の中身
fn parse_ind_x(op: &str) -> Option<&str> {
    let t = op.trim();
    if t.len() < 4 || !t.starts_with('(') || !t[t.len() - 3..].eq_ignore_ascii_case(",x)") {
        return None;
    }
    Some(t[1..t.len() - 3].trim())
}

/// `(式),Y` の中身
fn parse_ind_y(op: &str) -> Option<&str> {
    let t = op.trim();
    if t.len() < 4 || !t.starts_with('(') || !t[t.len() - 3..].eq_ignore_ascii_case("),y") {
        return None;
    }
    Some(t[1..t.len() - 3].trim())
}

/// `式,X` / `式,Y` を分ける
fn split_index(op: &str) -> (&str, Option<char>) {
    let t = op.trim();
    if t.len() > 2 {
        let (base, suffix) = t.split_at(t.len() - 2);
        if suffix.eq_ignore_ascii_case(",x") {
            return (base.trim(), Some('X'));
        }
        if suffix.eq_ignore_ascii_case(",y") {
            return (base.trim(), Some('Y'));
        }
    }
    (t, None)
}

/// `$0010` のように 3 桁以上の 16 進で書かれていたら、値が小さくても絶対アドレスとして扱う。
fn forced_absolute(expr: &str) -> bool {
    match expr.trim().strip_prefix('$') {
        Some(hex) => hex.len() > 2 && hex.chars().all(|c| c.is_ascii_hexdigit() || c == '_'),
        None => false,
    }
}

/// パス 1: アドレッシングモードを決める。
/// 値がまだ分からない（後ろで定義される）ときはゼロページでなく絶対アドレスにする。
fn classify(mnemonic: &str, operand: &str, symbols: &Symbols, no: usize) -> Result<AddrKind, AssembleError> {
    let m = mnemonic.to_ascii_uppercase();
    let op = operand.trim();
    const ALL_KINDS: [AddrKind; 12] = [
        AddrKind::Impl,
        AddrKind::Acc,
        AddrKind::Imm,
        AddrKind::Zp,
        AddrKind::ZpX,
        AddrKind::ZpY,
        AddrKind::Abs,
        AddrKind::AbsX,
        AddrKind::AbsY,
        AddrKind::Ind,
        AddrKind::IndX,
        AddrKind::IndY,
    ];
    if branch_opcode(&m).is_none() && !ALL_KINDS.iter().any(|&k| lookup_opcode(&m, k).is_some()) {
        return Err(AssembleError::new(no, format!("unknown instruction `{mnemonic}`")));
    }
    if branch_opcode(&m).is_some() {
        if op.is_empty() {
            return Err(AssembleError::new(no, format!("{m} needs a target")));
        }
        return Ok(AddrKind::Rel);
    }
    let candidates: &[AddrKind] = if op.is_empty() {
        &[AddrKind::Impl, AddrKind::Acc]
    } else if op.eq_ignore_ascii_case("a") && lookup_opcode(&m, AddrKind::Acc).is_some() {
        &[AddrKind::Acc]
    } else if op.starts_with('#') {
        &[AddrKind::Imm]
    } else if parse_ind_x(op).is_some() {
        &[AddrKind::IndX]
    } else if parse_ind_y(op).is_some() {
        &[AddrKind::IndY]
    } else if m == "JMP" && op.starts_with('(') && op.ends_with(')') {
        &[AddrKind::Ind]
    } else {
        let (base, index) = split_index(op);
        let (zp, abs) = match index {
            Some('X') => (AddrKind::ZpX, AddrKind::AbsX),
            Some(_) => (AddrKind::ZpY, AddrKind::AbsY),
            None => (AddrKind::Zp, AddrKind::Abs),
        };
        let small = match eval_expr(base, symbols) {
            Ok(v) => (0..=0xFF).contains(&v) && !forced_absolute(base),
            Err(EvalError::Undefined(_)) => false,
            Err(EvalError::Syntax(msg)) => return Err(AssembleError::new(no, msg)),
        };
        if small && lookup_opcode(&m, zp).is_some() {
            return Ok(zp);
        }
        if lookup_opcode(&m, abs).is_some() {
            return Ok(abs);
        }
        return Err(AssembleError::new(
            no,
            format!("`{mnemonic}` does not support this addressing mode (`{operand}`)"),
        ));
    };
    candidates
        .iter()
        .copied()
        .find(|&k| lookup_opcode(&m, k).is_some())
        .ok_or_else(|| {
            AssembleError::new(
                no,
                format!("`{mnemonic}` does not support this addressing mode (`{operand}`)"),
            )
        })
}

/// パス 2: 決めたアドレッシングモードで機械語を出力する。
fn emit(
    out: &mut Vec<u8>,
    mnemonic: &str,
    kind: AddrKind,
    operand: &str,
    pc: u32,
    eval: &dyn Fn(&str) -> Result<i32, AssembleError>,
    no: usize,
) -> Result<(), AssembleError> {
    let m = mnemonic.to_ascii_uppercase();
    let op = operand.trim();
    if kind == AddrKind::Rel {
        let target = eval(op)?;
        let offset = target - (pc as i32 + 2);
        if !(-128..=127).contains(&offset) {
            return Err(AssembleError::new(no, format!("branch out of range (offset {offset})")));
        }
        out.push(branch_opcode(&m).expect("branch"));
        out.push(offset as i8 as u8);
        return Ok(());
    }
    out.push(lookup_opcode(&m, kind).expect("checked in pass 1"));
    let zero_page = |expr: &str| -> Result<u8, AssembleError> {
        let v = eval(expr)?;
        if (0..=0xFF).contains(&v) {
            Ok(v as u8)
        } else {
            Err(AssembleError::new(no, format!("`{expr}` is not a zero-page address")))
        }
    };
    match kind {
        AddrKind::Impl | AddrKind::Acc | AddrKind::Rel => {}
        AddrKind::Imm => out.push(to_byte(eval(&op[1..])?, no)?),
        AddrKind::Zp | AddrKind::ZpX | AddrKind::ZpY => out.push(zero_page(split_index(op).0)?),
        AddrKind::IndX => out.push(zero_page(parse_ind_x(op).expect("(zp,X)"))?),
        AddrKind::IndY => out.push(zero_page(parse_ind_y(op).expect("(zp),Y"))?),
        AddrKind::Abs | AddrKind::AbsX | AddrKind::AbsY => {
            let v = to_word(eval(split_index(op).0)?, no)?;
            out.extend_from_slice(&v.to_le_bytes());
        }
        AddrKind::Ind => {
            let v = to_word(eval(&op[1..op.len() - 1])?, no)?;
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    Ok(())
}

fn lookup_opcode(mnemonic: &str, kind: AddrKind) -> Option<u8> {
    use AddrKind::*;
    Some(match (mnemonic, kind) {
        ("LDA", Imm) => 0xA9,
        ("LDA", Zp) => 0xA5,
        ("LDA", ZpX) => 0xB5,
        ("LDA", Abs) => 0xAD,
        ("LDA", AbsX) => 0xBD,
        ("LDA", AbsY) => 0xB9,
        ("LDA", IndX) => 0xA1,
        ("LDA", IndY) => 0xB1,

        ("LDX", Imm) => 0xA2,
        ("LDX", Zp) => 0xA6,
        ("LDX", ZpY) => 0xB6,
        ("LDX", Abs) => 0xAE,
        ("LDX", AbsY) => 0xBE,

        ("LDY", Imm) => 0xA0,
        ("LDY", Zp) => 0xA4,
        ("LDY", ZpX) => 0xB4,
        ("LDY", Abs) => 0xAC,
        ("LDY", AbsX) => 0xBC,

        ("STA", Zp) => 0x85,
        ("STA", ZpX) => 0x95,
        ("STA", Abs) => 0x8D,
        ("STA", AbsX) => 0x9D,
        ("STA", AbsY) => 0x99,
        ("STA", IndX) => 0x81,
        ("STA", IndY) => 0x91,

        ("STX", Zp) => 0x86,
        ("STX", ZpY) => 0x96,
        ("STX", Abs) => 0x8E,

        ("STY", Zp) => 0x84,
        ("STY", ZpX) => 0x94,
        ("STY", Abs) => 0x8C,

        ("TAX", Impl) => 0xAA,
        ("TAY", Impl) => 0xA8,
        ("TXA", Impl) => 0x8A,
        ("TYA", Impl) => 0x98,
        ("TSX", Impl) => 0xBA,
        ("TXS", Impl) => 0x9A,

        ("PHA", Impl) => 0x48,
        ("PLA", Impl) => 0x68,
        ("PHP", Impl) => 0x08,
        ("PLP", Impl) => 0x28,

        ("AND", Imm) => 0x29,
        ("AND", Zp) => 0x25,
        ("AND", ZpX) => 0x35,
        ("AND", Abs) => 0x2D,
        ("AND", AbsX) => 0x3D,
        ("AND", AbsY) => 0x39,
        ("AND", IndX) => 0x21,
        ("AND", IndY) => 0x31,

        ("ORA", Imm) => 0x09,
        ("ORA", Zp) => 0x05,
        ("ORA", ZpX) => 0x15,
        ("ORA", Abs) => 0x0D,
        ("ORA", AbsX) => 0x1D,
        ("ORA", AbsY) => 0x19,
        ("ORA", IndX) => 0x01,
        ("ORA", IndY) => 0x11,

        ("EOR", Imm) => 0x49,
        ("EOR", Zp) => 0x45,
        ("EOR", ZpX) => 0x55,
        ("EOR", Abs) => 0x4D,
        ("EOR", AbsX) => 0x5D,
        ("EOR", AbsY) => 0x59,
        ("EOR", IndX) => 0x41,
        ("EOR", IndY) => 0x51,

        ("BIT", Zp) => 0x24,
        ("BIT", Abs) => 0x2C,

        ("ADC", Imm) => 0x69,
        ("ADC", Zp) => 0x65,
        ("ADC", ZpX) => 0x75,
        ("ADC", Abs) => 0x6D,
        ("ADC", AbsX) => 0x7D,
        ("ADC", AbsY) => 0x79,
        ("ADC", IndX) => 0x61,
        ("ADC", IndY) => 0x71,

        ("SBC", Imm) => 0xE9,
        ("SBC", Zp) => 0xE5,
        ("SBC", ZpX) => 0xF5,
        ("SBC", Abs) => 0xED,
        ("SBC", AbsX) => 0xFD,
        ("SBC", AbsY) => 0xF9,
        ("SBC", IndX) => 0xE1,
        ("SBC", IndY) => 0xF1,

        ("CMP", Imm) => 0xC9,
        ("CMP", Zp) => 0xC5,
        ("CMP", ZpX) => 0xD5,
        ("CMP", Abs) => 0xCD,
        ("CMP", AbsX) => 0xDD,
        ("CMP", AbsY) => 0xD9,
        ("CMP", IndX) => 0xC1,
        ("CMP", IndY) => 0xD1,

        ("CPX", Imm) => 0xE0,
        ("CPX", Zp) => 0xE4,
        ("CPX", Abs) => 0xEC,

        ("CPY", Imm) => 0xC0,
        ("CPY", Zp) => 0xC4,
        ("CPY", Abs) => 0xCC,

        ("INC", Zp) => 0xE6,
        ("INC", ZpX) => 0xF6,
        ("INC", Abs) => 0xEE,
        ("INC", AbsX) => 0xFE,

        ("DEC", Zp) => 0xC6,
        ("DEC", ZpX) => 0xD6,
        ("DEC", Abs) => 0xCE,
        ("DEC", AbsX) => 0xDE,

        ("INX", Impl) => 0xE8,
        ("INY", Impl) => 0xC8,
        ("DEX", Impl) => 0xCA,
        ("DEY", Impl) => 0x88,

        ("ASL", Acc) => 0x0A,
        ("ASL", Zp) => 0x06,
        ("ASL", ZpX) => 0x16,
        ("ASL", Abs) => 0x0E,
        ("ASL", AbsX) => 0x1E,

        ("LSR", Acc) => 0x4A,
        ("LSR", Zp) => 0x46,
        ("LSR", ZpX) => 0x56,
        ("LSR", Abs) => 0x4E,
        ("LSR", AbsX) => 0x5E,

        ("ROL", Acc) => 0x2A,
        ("ROL", Zp) => 0x26,
        ("ROL", ZpX) => 0x36,
        ("ROL", Abs) => 0x2E,
        ("ROL", AbsX) => 0x3E,

        ("ROR", Acc) => 0x6A,
        ("ROR", Zp) => 0x66,
        ("ROR", ZpX) => 0x76,
        ("ROR", Abs) => 0x6E,
        ("ROR", AbsX) => 0x7E,

        ("JMP", Abs) => 0x4C,
        ("JMP", Ind) => 0x6C,

        ("JSR", Abs) => 0x20,
        ("RTS", Impl) => 0x60,
        ("RTI", Impl) => 0x40,
        ("BRK", Impl) => 0x00,

        ("CLC", Impl) => 0x18,
        ("SEC", Impl) => 0x38,
        ("CLI", Impl) => 0x58,
        ("SEI", Impl) => 0x78,
        ("CLV", Impl) => 0xB8,
        ("CLD", Impl) => 0xD8,
        ("SED", Impl) => 0xF8,

        ("NOP", Impl) => 0xEA,

        _ => return None,
    })
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assemble_lda_sta_abs() {
        let bin = assemble(
            "
            LDA #$00
            STA $2000
        ",
        )
        .unwrap();
        assert_eq!(bin, vec![0xA9, 0x00, 0x8D, 0x00, 0x20]);
    }

    #[test]
    fn assemble_jmp_label() {
        let bin = assemble(
            "
            jmp there
            nop
        there:
            lda #$42
        ",
        )
        .unwrap();
        assert_eq!(bin, vec![0x4C, 0x04, 0x00, 0xEA, 0xA9, 0x42]);
    }

    #[test]
    fn assemble_branch_forward() {
        let bin = assemble(
            "
        beq skip
        lda #1
    skip:
        lda #2
        ",
        )
        .unwrap();
        assert_eq!(bin, vec![0xF0, 0x02, 0xA9, 0x01, 0xA9, 0x02]);
    }

    #[test]
    fn assemble_indirect_y() {
        let bin = assemble("lda ($40),y\n").unwrap();
        assert_eq!(bin, vec![0xB1, 0x40]);
    }

    #[test]
    fn org_and_data_directives() {
        let bin = assemble(
            "
            .org $10
            .byte $AA
            .word $1234
        ",
        )
        .unwrap();
        assert_eq!(bin.len(), 0x10 + 1 + 2);
        assert_eq!(bin[0x10], 0xAA);
        assert_eq!(bin[0x11], 0x34);
        assert_eq!(bin[0x12], 0x12);
    }

    #[test]
    fn constants_and_expressions() {
        let bin = assemble(
            "
PPUCTRL = $2000
ptr     = $10
        lda #%1000_0000
        sta PPUCTRL
        lda ptr+1,x
        lda (ptr),y
        lda #'A'+1
        lda #-1
        lda #(2+3)*4
        ",
        )
        .unwrap();
        assert_eq!(
            bin,
            vec![0xA9, 0x80, 0x8D, 0x00, 0x20, 0xB5, 0x11, 0xB1, 0x10, 0xA9, 0x42, 0xA9, 0xFF, 0xA9, 20]
        );
    }

    #[test]
    fn forward_label_uses_absolute_addressing() {
        let bin = assemble(
            "
        .org $8000
        lda table,x
        lda #<table
        ldx #>table
        rts
table:  .byte 1, 2
        ",
        )
        .unwrap();
        assert_eq!(&bin[0x8000..], &[0xBD, 0x08, 0x80, 0xA9, 0x08, 0xA2, 0x80, 0x60, 1, 2]);
    }

    #[test]
    fn constant_defined_after_use() {
        // 値が後ろで決まる定数は絶対アドレスで出力する（長さがパス 1 と 2 で変わらないように）
        let bin = assemble("lda later\nlater = $12\n").unwrap();
        assert_eq!(bin, vec![0xAD, 0x12, 0x00]);
    }

    #[test]
    fn strings_and_words() {
        let bin = assemble(".byte \"HI; X\", 0, ';'\n.word end, $1234\nend:\n").unwrap();
        assert_eq!(bin, vec![b'H', b'I', b';', b' ', b'X', 0, b';', 0x0B, 0x00, 0x34, 0x12]);
    }

    #[test]
    fn zero_page_modes() {
        let bin = assemble("lda $0010\nlda $10,y\nldx $10,y\nasl a\nlsr\n").unwrap();
        assert_eq!(bin, vec![0xAD, 0x10, 0x00, 0xB9, 0x10, 0x00, 0xB6, 0x10, 0x0A, 0x4A]);
    }

    #[test]
    fn reports_errors_with_line_numbers() {
        let err = assemble("nop\nlda missing\n").unwrap_err();
        assert_eq!(err.line, 2);
        assert!(err.message.contains("missing"), "{}", err.message);
        let err = assemble("x:\nx:\n").unwrap_err();
        assert!(err.message.contains("twice"));
        let err = assemble("lda #256\n").unwrap_err();
        assert!(err.message.contains("byte"));
        let err = assemble("foo $10\n").unwrap_err();
        assert!(err.message.contains("unknown instruction"));
        let err = assemble("sty $1234,y\n").unwrap_err();
        assert!(err.message.contains("addressing"));
    }
}
