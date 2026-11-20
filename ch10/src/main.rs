//! mynes のコマンドライン版
//!
//!   cargo run -- info roms/hello.nes

use mynes::cartridge::Cartridge;
use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["info", rom] => info(rom),
        _ => {
            eprintln!("使い方:");
            eprintln!("  mynes info <ROM>");
            process::exit(2);
        }
    };
    if let Err(message) = result {
        eprintln!("エラー: {message}");
        process::exit(1);
    }
}

fn load(path: &str) -> Result<Cartridge, String> {
    let data = fs::read(path).map_err(|e| format!("{path} を読めません: {e}"))?;
    Cartridge::from_ines(&data)
}

/// ROM の情報を表示する
fn info(path: &str) -> Result<(), String> {
    let cart = load(path)?;
    println!("PRG ROM: {} KiB", cart.prg_rom.len() / 1024);
    println!("CHR ROM: {} KiB", cart.chr_rom.len() / 1024);
    println!("ミラーリング: {:?}", cart.mirroring);
    Ok(())
}
