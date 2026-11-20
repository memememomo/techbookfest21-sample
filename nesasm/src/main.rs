//! `.asm` をアセンブルし、NROM（Mapper 0）の iNES（`.nes`）を書き出す。
//!
//! 使用例:
//!   cargo run -- ../roms/src/hello.asm hello.nes
//!   cargo run -- prog.asm game.nes --chr tiles.chr --pad 00

mod assembler;

use assembler::assemble;
use std::env;
use std::fs;
use std::path::Path;
use std::process::exit;

const PRG_LEN: usize = 16 * 1024;
const CHR_LEN: usize = 8 * 1024;

fn main() {
    if let Err(e) = run() {
        eprintln!("nesasm: {e}");
        exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1).peekable();
    if args.peek().map(|a| a == "--help" || a == "-h") == Some(true) {
        print_usage();
        return Ok(());
    }

    let input = args
        .next()
        .ok_or_else(|| "入力 .asm を指定してください (--help)".to_string())?;
    let output = args
        .next()
        .ok_or_else(|| "出力 .nes を指定してください (--help)".to_string())?;

    let mut chr_path: Option<String> = None;
    let mut pad_byte: u8 = 0xEA;
    let mut vertical_mirror = false;

    while let Some(a) = args.next() {
        match a.as_str() {
            "--chr" => {
                chr_path = Some(
                    args
                        .next()
                        .ok_or_else(|| "--chr にはファイルパスが必要です".to_string())?,
                );
            }
            "--pad" => {
                let hex = args
                    .next()
                    .ok_or_else(|| "--pad には 1..2 桁の16進が必要です".to_string())?;
                pad_byte = parse_hex_byte(&hex)?;
            }
            "--vertical" => vertical_mirror = true,
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => return Err(format!("不明な引数: {other}（--help）")),
        }
    }

    let src =
        fs::read_to_string(&input).map_err(|e| format!("{input}: {e}"))?;

    let assembled = assemble(&src).map_err(|e| {
        format!("{}:{}: {}", input, e.line, e.message)
    })?;

    let prg = extract_prg(&assembled, pad_byte)?;
    let chr = load_chr(chr_path.as_deref())?;

    let header = nrom_header(vertical_mirror, prg.len() / PRG_LEN);
    let mut out = Vec::with_capacity(16 + prg.len() + CHR_LEN);
    out.extend_from_slice(&header);
    out.extend_from_slice(&prg);
    out.extend_from_slice(&chr);

    fs::write(&output, &out).map_err(|e| format!("{output}: {e}"))?;

    eprintln!(
        "Wrote {} (NROM, PRG {} B, CHR {CHR_LEN} B)",
        Path::new(&output).display(),
        prg.len()
    );
    Ok(())
}

fn print_usage() {
    eprintln!(
        "\
nesasm — 6502 アセンブラ → iNES（Mapper 0）

使用法:
  nesasm <input.asm> <output.nes> [オプション]

オプション:
  --chr PATH       8 KiB の CHR バイナリ（無指定時は 0 詰め）
  --pad HEX        PRG が 16 KiB 未満のときの埋め草（既定: EA）
  --vertical      iNES flags6 の垂直ミラー（無指定は水平）
  -h, --help       このヘルプ

PRG の取り方:
  アセンブル結果が 0x8000 バイト超のとき、CPU アドレス $8000 以降を PRG として扱います。
  - $8000〜 の内容が 16 KiB 以内 → 16 KiB PRG（`.org $8000` で置いたデモ）
  - $8000〜$BFFF が空（すべて 0）→ $C000〜$FFFF を 16 KiB PRG とする（`.org $C000`）
  - それ以外 → $8000〜$FFFF の 32 KiB PRG
  0x8000 バイト以下のときは先頭から最大 16 KiB を PRG とします。
"
    );
}

fn parse_hex_byte(s: &str) -> Result<u8, String> {
    let t = s.trim();
    let t = t.strip_prefix("0x").unwrap_or(t).strip_prefix("0X").unwrap_or(t);
    if t.len() > 2 {
        return Err(format!("--pad は 1 バイト分の16進で指定してください: {s}"));
    }
    u8::from_str_radix(t, 16).map_err(|_| format!("無効な16進: {s}"))
}

fn extract_prg(assembled: &[u8], pad: u8) -> Result<Vec<u8>, String> {
    let body = if assembled.len() > 0x8000 {
        let upper = &assembled[0x8000..];
        if upper.len() <= PRG_LEN {
            upper
        } else if upper[..PRG_LEN].iter().all(|&b| b == 0) {
            // `.org $C000` から始まる 16 KiB の ROM（$8000〜$BFFF にはミラーが見える）
            &upper[PRG_LEN..]
        } else {
            let mut prg = upper.to_vec();
            prg.resize(2 * PRG_LEN, pad);
            return Ok(prg);
        }
    } else {
        assembled
    };

    if body.len() > PRG_LEN {
        return Err(format!(
            "PRG が {PRG_LEN} バイトを超えています（{} バイト）",
            body.len()
        ));
    }

    let mut prg = body.to_vec();
    prg.resize(PRG_LEN, pad);
    Ok(prg)
}

fn load_chr(path: Option<&str>) -> Result<Vec<u8>, String> {
    match path {
        None => Ok(vec![0u8; CHR_LEN]),
        Some(p) => {
            let data = fs::read(p).map_err(|e| format!("{p}: {e}"))?;
            if data.len() != CHR_LEN {
                return Err(format!(
                    "CHR は正確に {CHR_LEN} バイトである必要があります（{} バイト）",
                    data.len()
                ));
            }
            Ok(data)
        }
    }
}

fn nrom_header(vertical: bool, prg_banks: usize) -> [u8; 16] {
    let mut h = [0u8; 16];
    h[0..4].copy_from_slice(b"NES\x1a");
    h[4] = prg_banks as u8;
    h[5] = 1;
    h[6] = if vertical { 1 } else { 0 };
    h[7] = 0;
    h
}
