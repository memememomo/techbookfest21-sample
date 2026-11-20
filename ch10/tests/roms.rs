//! 教材のデモ ROM を動かして確かめる
//!
//! roms/ に ROM が無いときは何もしない（サンプルリポジトリの roms/ からコピーしてください）

use mynes::cartridge::Cartridge;
use mynes::cpu::Bus;
use mynes::nes::Nes;
use mynes::palette::PALETTE;
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

/// 画面の (x, y) の色
fn pixel(nes: &Nes, x: usize, y: usize) -> [u8; 3] {
    let i = (y * 256 + x) * 4;
    let frame = nes.frame();
    [frame[i], frame[i + 1], frame[i + 2]]
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

#[test]
fn hello_shows_text() {
    let Some(mut nes) = load("hello.nes") else {
        return;
    };
    run_frames(&mut nes, 10);
    // 「H」の左の縦棒は白、その左は背景の黒
    assert_eq!(pixel(&nes, 81, 88), PALETTE[0x30]);
    assert_eq!(pixel(&nes, 80, 88), PALETTE[0x0F]);
    // 13 行目は属性テーブルでパレット 1（水色）
    assert_eq!(pixel(&nes, 73, 104), PALETTE[0x21]);
}

#[test]
fn game_shows_player_and_stars() {
    let Some(mut nes) = load("game.nes") else {
        return;
    };
    run_frames(&mut nes, 10);
    // キャラクター（OAM の Y = 136 なので 137 ライン目から）
    assert_eq!(pixel(&nes, 126, 137), PALETTE[0x28]);
    // 星は 8 個目まで見える
    assert_eq!(pixel(&nes, 184 + 3, 176), PALETTE[0x38]);
    // 9 個目・10 個目は 1 ラインに 9 個以上並ぶので見えない
    assert_eq!(pixel(&nes, 208 + 3, 176), PALETTE[0x0F]);
    assert_eq!(pixel(&nes, 232 + 3, 176), PALETTE[0x0F]);
}
