//! デモ ROM を動かして、写し取った PPU の中身が画面と合っているかを確かめる

use std::path::Path;
use viewer::Viewer;

fn load(name: &str) -> Option<Viewer> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("roms")
        .join(name);
    let Ok(data) = std::fs::read(&path) else {
        eprintln!("{} が無いので飛ばします", path.display());
        return None;
    };
    Some(Viewer::new(&data).unwrap())
}

/// 画面に出しているネームテーブルを描いた絵（512 × 480 の一部）が、
/// mynes が描いた画面と同じになる（hello.nes はスプライトを使わない）
#[test]
fn hello_nametable_matches_screen() {
    let Some(mut viewer) = load("hello.nes") else {
        return;
    };
    for _ in 0..10 {
        viewer.run_frame();
    }
    let shown = viewer.shadow.shown_nametable();
    let (ox, oy) = (
        ((shown - 0x2000) / 0x400 % 2) as usize * 256,
        ((shown - 0x2000) / 0x400 / 2) as usize * 240,
    );
    let image = viewer.nametables_image();
    let screen = viewer.nes.frame().to_vec();
    for y in 0..240 {
        for x in 0..256 {
            let i = (y * 256 + x) * 4;
            let j = ((oy + y) * 512 + ox + x) * 4;
            assert_eq!(screen[i..i + 4], image[j..j + 4], "({x}, {y}) の色が違う");
        }
    }
    // パレットが書き込まれている
    assert!(viewer.shadow.palette.iter().any(|&c| c != 0));
}

#[test]
fn game_has_sprites_in_oam() {
    let Some(mut viewer) = load("game.nes") else {
        return;
    };
    for _ in 0..10 {
        viewer.run_frame();
    }
    // 画面の中（Y < $EF）にいるスプライトがある
    let oam = &viewer.nes.bus.ppu.oam;
    let visible = (0..64).filter(|i| oam[i * 4] < 0xEF).count();
    assert!(visible >= 4, "見えているスプライトは {visible} 個");
    // パターンテーブルの絵は 128 × 128 ドット
    let image = viewer.pattern_table_image(0x0000, viewer::Colors::Gray);
    assert_eq!(image.len(), 128 * 128 * 4);
}
