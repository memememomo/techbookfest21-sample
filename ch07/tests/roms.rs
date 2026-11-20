//! 教材のデモ ROM を動かして確かめる
//!
//! roms/ に ROM が無いときは何もしない（サンプルリポジトリの roms/ からコピーしてください）

use mynes::cartridge::Cartridge;
use mynes::cpu::Bus;
use mynes::nes::Nes;
use std::path::Path;

fn load(name: &str) -> Option<Nes> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("roms")
        .join(name);
    let Ok(data) = std::fs::read(&path) else {
        eprintln!("{} が無いので飛ばします", path.display());
        return None;
    };
    Some(Nes::new(Cartridge::from_ines(&data).unwrap()))
}

fn run_frames(nes: &mut Nes, frames: usize) {
    for _ in 0..frames {
        nes.run_frame();
    }
}

#[test]
fn hello_counts_nmi() {
    let Some(mut nes) = load("hello.nes") else {
        return;
    };
    run_frames(&mut nes, 30);
    let before = nes.bus.read(0x0000);
    run_frames(&mut nes, 10);
    // NMI のたびに $0000 が 1 増える
    assert_eq!(nes.bus.read(0x0000), before.wrapping_add(10));
}

#[test]
fn hello_writes_nametable() {
    let Some(mut nes) = load("hello.nes") else {
        return;
    };
    run_frames(&mut nes, 10);
    // $216A（11 行 10 列）に 'H' が書かれている。$2007 の 1 回目の読み込みは古い値なので 2 回読む
    nes.bus.write(0x2006, 0x21);
    nes.bus.write(0x2006, 0x6A);
    nes.bus.read(0x2007);
    assert_eq!(nes.bus.read(0x2007), b'H');
}
