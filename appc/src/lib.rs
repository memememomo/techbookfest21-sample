//! 付録 C「ビット演算のしくみ」の課題の解答例。
//! 各課題の関数の後ろに、その課題のテストを置いている。

// ---- 課題 1: 2 進数・16 進数・10 進数で表示する ----

/// 1 バイトを `%1010_0101 = $A5 = 165` の形の文字列にする
pub fn show(value: u8) -> String {
    let bits = format!("{value:08b}");
    format!("%{}_{} = ${value:02X} = {value}", &bits[..4], &bits[4..])
}

#[cfg(test)]
mod test_show {
    use super::*;

    #[test]
    fn show_formats_three_ways() {
        assert_eq!(show(0xA5), "%1010_0101 = $A5 = 165");
        assert_eq!(show(0x07), "%0000_0111 = $07 = 7");
        assert_eq!(show(0x80), "%1000_0000 = $80 = 128");
    }
}

// ---- 課題 2: 1 ビットを調べる・立てる・消す・反転する ----

/// ビット n が 1 か
pub fn is_set(value: u8, n: u32) -> bool {
    value & (1 << n) != 0
}

/// ビット n を 1 にした値
pub fn set_bit(value: u8, n: u32) -> u8 {
    value | (1 << n)
}

/// ビット n を 0 にした値
pub fn clear_bit(value: u8, n: u32) -> u8 {
    value & !(1 << n)
}

/// ビット n を反転した値
pub fn toggle_bit(value: u8, n: u32) -> u8 {
    value ^ (1 << n)
}

#[cfg(test)]
mod test_bit_ops {
    use super::*;

    #[test]
    fn single_bit_operations() {
        assert!(is_set(0xA5, 0));
        assert!(!is_set(0xA5, 1));
        assert!(is_set(0xA5, 7));
        assert_eq!(set_bit(0xA5, 4), 0xB5);
        assert_eq!(set_bit(0xA5, 0), 0xA5); // もう 1 なら変わらない
        assert_eq!(clear_bit(0xA5, 0), 0xA4);
        assert_eq!(clear_bit(0xA5, 1), 0xA5); // もう 0 なら変わらない
        assert_eq!(toggle_bit(0xA5, 7), 0x25);
        assert_eq!(toggle_bit(toggle_bit(0xA5, 3), 3), 0xA5); // 2 回反転すると元に戻る
    }
}

// ---- 課題 3: シフトでビットを数える・並べ替える ----

/// 1 になっているビットの数
pub fn count_ones(mut value: u8) -> u32 {
    let mut count = 0;
    while value != 0 {
        count += (value & 1) as u32;
        value >>= 1;
    }
    count
}

/// ビットの並びを左右逆にする（ビット 0 とビット 7、ビット 1 とビット 6、… を入れ替える）
pub fn reverse_bits(mut value: u8) -> u8 {
    let mut result = 0;
    for _ in 0..8 {
        result = (result << 1) | (value & 1);
        value >>= 1;
    }
    result
}

#[cfg(test)]
mod test_shift {
    use super::*;

    #[test]
    fn count_and_reverse() {
        assert_eq!(count_ones(0x00), 0);
        assert_eq!(count_ones(0xA5), 4);
        assert_eq!(count_ones(0xFF), 8);
        assert_eq!(reverse_bits(0b1000_0000), 0b0000_0001);
        assert_eq!(reverse_bits(0b1100_1010), 0b0101_0011);
        // 標準ライブラリにも同じものがある
        for v in 0..=255u8 {
            assert_eq!(count_ones(v), v.count_ones());
            assert_eq!(reverse_bits(v), v.reverse_bits());
        }
    }
}

// ---- 課題 4: ステータスレジスタを文字列にする ----

/// P を `NV-BDIZC` の形にする。立っているフラグは大文字、立っていないフラグは小文字
pub fn format_flags(p: u8) -> String {
    "NV-BDIZC"
        .chars()
        .enumerate()
        .map(|(i, name)| {
            let bit = 7 - i as u32;
            if name == '-' {
                '-'
            } else if p & (1 << bit) != 0 {
                name
            } else {
                name.to_ascii_lowercase()
            }
        })
        .collect()
}

#[cfg(test)]
mod test_format_flags {
    use super::*;

    #[test]
    fn flags_as_letters() {
        assert_eq!(format_flags(0x24), "nv-bdIzc"); // リセット直後
        assert_eq!(format_flags(0x83), "Nv-bdiZC");
        assert_eq!(format_flags(0xFF), "NV-BDIZC");
        assert_eq!(format_flags(0x00), "nv-bdizc");
    }
}

// ---- 課題 5: 下位のビットだけ残してミラーを表す ----

/// $0000〜$1FFF を、2 KiB の RAM の位置にする
pub fn ram_index(addr: u16) -> usize {
    (addr & 0x07FF) as usize
}

/// $2000〜$3FFF を、PPU レジスタ（$2000〜$2007）のアドレスにする
pub fn ppu_register(addr: u16) -> u16 {
    0x2000 | (addr & 0x0007)
}

/// $3F00〜$3FFF を、パレット RAM（32 バイト）の位置にする。$3F10/$3F14/$3F18/$3F1C は $3F00/$3F04/$3F08/$3F0C と同じ
pub fn palette_index(addr: u16) -> usize {
    let index = (addr & 0x1F) as usize;
    if index >= 16 && index & 3 == 0 {
        index - 16
    } else {
        index
    }
}

#[cfg(test)]
mod test_mirror {
    use super::*;

    #[test]
    fn mirrors() {
        assert_eq!(ram_index(0x0237), 0x0237);
        assert_eq!(ram_index(0x0A37), 0x0237);
        assert_eq!(ram_index(0x1FFF), 0x07FF);
        assert_eq!(ppu_register(0x2002), 0x2002);
        assert_eq!(ppu_register(0x3456), 0x2006);
        assert_eq!(palette_index(0x3F01), 0x01);
        assert_eq!(palette_index(0x3F11), 0x11);
        assert_eq!(palette_index(0x3F10), 0x00);
        assert_eq!(palette_index(0x3F3C), 0x0C);
    }
}

// ---- 課題 6: フィールドを取り出す ----

/// value のビット shift から width ビット分を取り出す
pub fn field(value: u16, shift: u32, width: u32) -> u16 {
    (value >> shift) & ((1 << width) - 1)
}

#[cfg(test)]
mod test_field {
    use super::*;

    #[test]
    fn extract_fields() {
        // $4001 = EPPP NSSS
        let sweep = 0b1011_1010;
        assert_eq!(field(sweep, 7, 1), 1); // E
        assert_eq!(field(sweep, 4, 3), 3); // PPP
        assert_eq!(field(sweep, 3, 1), 1); // N
        assert_eq!(field(sweep, 0, 3), 2); // SSS
        // $4003 の上位 5 ビット
        assert_eq!(field(0b0000_1010, 3, 5), 1);
    }
}

// ---- 課題 7: フィールドを書き換える ----

/// old のビット shift から width ビット分を、value の下位 width ビットに書き換える
pub fn replace_field(old: u16, shift: u32, width: u32, value: u16) -> u16 {
    let mask = ((1 << width) - 1) << shift;
    (old & !mask) | ((value << shift) & mask)
}

#[cfg(test)]
mod test_replace_field {
    use super::*;

    #[test]
    fn replace_fields() {
        // スクロールを作るときの $2000 の書き込み: t のビット 10〜11
        let t = 0b0110_0101_1010_1111;
        assert_eq!(replace_field(t, 10, 2, 0b10), 0b0110_1001_1010_1111);
        // はみ出した value のビットは無視する
        assert_eq!(replace_field(0x0000, 10, 2, 0xFF), 0x0C00);
        // 取り出すと、書いた値が戻ってくる
        assert_eq!(field(replace_field(0x1234, 4, 5, 21), 4, 5), 21);
    }
}

// ---- 課題 8: 2 バイトをつなぐ・分ける ----

/// 下位バイトと上位バイトをつなぐ
pub fn join(lo: u8, hi: u8) -> u16 {
    ((hi as u16) << 8) | lo as u16
}

/// (下位バイト, 上位バイト) に分ける
pub fn split(value: u16) -> (u8, u8) {
    (value as u8, (value >> 8) as u8)
}

/// mem[addr] を下位、mem[addr + 1] を上位として 16 ビットの値を読む（リトルエンディアン）
pub fn read_u16(mem: &[u8], addr: usize) -> u16 {
    join(mem[addr], mem[addr + 1])
}

#[cfg(test)]
mod test_join {
    use super::*;

    #[test]
    fn join_and_split() {
        assert_eq!(join(0x34, 0x12), 0x1234);
        assert_eq!(split(0x1234), (0x34, 0x12));
        assert_eq!(split(join(0xCD, 0xAB)), (0xCD, 0xAB));
        // リセットベクター $FFFC〜$FFFD に $00 $C0 と書いてあれば、$C000 から始まる
        let mut mem = vec![0; 0x10000];
        mem[0xFFFC] = 0x00;
        mem[0xFFFD] = 0xC0;
        assert_eq!(read_u16(&mem, 0xFFFC), 0xC000);
    }
}

// ---- 課題 9: 2 の補数と相対分岐 ----

/// 2 の補数で符号を反転する（!x + 1）
pub fn negate(x: u8) -> u8 {
    (!x).wrapping_add(1)
}

/// 分岐先。pc は次の命令のアドレス、offset は符号付きの 1 バイト
pub fn branch_target(pc: u16, offset: u8) -> u16 {
    pc.wrapping_add(offset as i8 as u16)
}

#[cfg(test)]
mod test_signed {
    use super::*;

    #[test]
    fn twos_complement() {
        assert_eq!(negate(5), 0xFB);
        assert_eq!(negate(0xFB), 5);
        assert_eq!(negate(0), 0);
        assert_eq!(negate(0x80), 0x80); // -128 だけは反転できない
        for x in 0..=127u8 {
            assert_eq!(negate(x) as i8, -(x as i8));
        }
        assert_eq!(branch_target(0x8010, 0x04), 0x8014); // +4
        assert_eq!(branch_target(0x8010, 0xFC), 0x800C); // -4
        assert_eq!(branch_target(0x8010, 0x80), 0x7F90); // -128
        assert_eq!(branch_target(0x0002, 0xFC), 0xFFFE); // 0 より前は $FFFF へ回る
    }
}

// ---- 課題 10: ADC と SBC ----

/// A + value + C を計算する。戻り値は (結果, C, V)
pub fn adc(a: u8, value: u8, carry: bool) -> (u8, bool, bool) {
    let sum = a as u16 + value as u16 + carry as u16;
    let result = sum as u8;
    let overflow = (a ^ result) & (value ^ result) & 0x80 != 0;
    (result, sum > 0xFF, overflow)
}

/// A - value - (1 - C) を計算する。戻り値は (結果, C, V)
pub fn sbc(a: u8, value: u8, carry: bool) -> (u8, bool, bool) {
    adc(a, !value, carry)
}

#[cfg(test)]
mod test_adc {
    use super::*;

    #[test]
    fn add_with_carry() {
        assert_eq!(adc(0x50, 0x10, false), (0x60, false, false));
        assert_eq!(adc(0x50, 0x50, false), (0xA0, false, true));
        assert_eq!(adc(0x50, 0xD0, false), (0x20, true, false));
        assert_eq!(adc(0xD0, 0x90, false), (0x60, true, true));
        assert_eq!(adc(0xF0, 0x10, false), (0x00, true, false));
        assert_eq!(adc(0xFF, 0x00, true), (0x00, true, false)); // C も足す
    }

    #[test]
    fn subtract_with_carry() {
        assert_eq!(sbc(5, 3, true), (2, true, false)); // 借りなし
        assert_eq!(sbc(3, 5, true), (0xFE, false, false)); // 借りあり（-2）
        assert_eq!(sbc(5, 3, false), (1, true, false)); // C = 0 ならさらに 1 引く
        assert_eq!(sbc(0x80, 1, true), (0x7F, true, true)); // -128 - 1 は範囲外
    }
}

// ---- 課題 11: LSR と ROL ----

/// 右へ 1 ずらす。戻り値は (結果, はみ出したビット 0)
pub fn lsr(value: u8) -> (u8, bool) {
    (value >> 1, value & 0x01 != 0)
}

/// 左へ 1 回す。carry はビット 0 に入る。戻り値は (結果, はみ出したビット 7)
pub fn rol(value: u8, carry: bool) -> (u8, bool) {
    ((value << 1) | carry as u8, value & 0x80 != 0)
}

#[cfg(test)]
mod test_lsr_rol {
    use super::*;

    #[test]
    fn shift_through_carry() {
        assert_eq!(lsr(0b1001_0011), (0b0100_1001, true));
        assert_eq!(lsr(0b1001_0010), (0b0100_1001, false));
        assert_eq!(rol(0b1001_0011, true), (0b0010_0111, true));
        assert_eq!(rol(0b0001_0011, false), (0b0010_0110, false));
        // 9 回 ROL すると元に戻る（8 ビット + C の 9 ビットで回っているので）
        let (mut v, mut c) = (0xA5, false);
        for _ in 0..9 {
            (v, c) = rol(v, c);
        }
        assert_eq!((v, c), (0xA5, false));
    }
}

// ---- 課題 12: デモ ROM のコントローラーの読み方をまねる ----

/// read を 8 回呼んで、返ってきたビット 0 を LSR と ROL で 1 バイトに集める。
/// 最初に読んだビットがビット 7 に、最後に読んだビットがビット 0 に入る
pub fn read_joypad(mut read: impl FnMut() -> u8) -> u8 {
    // 番兵: この 1 が 8 回の ROL でキャリーに出てきたら終わり
    let mut buttons = 1;
    loop {
        let (_, carry) = lsr(read());
        let (result, done) = rol(buttons, carry);
        buttons = result;
        if done {
            return buttons;
        }
    }
}

#[cfg(test)]
mod test_read_joypad {
    use super::*;

    #[test]
    fn collects_eight_reads() {
        // A、B、Select、Start、上、下、左、右 の順に返る。A・Start・右を押している
        let mut bits = [1, 0, 0, 1, 0, 0, 0, 1].into_iter();
        let mut count = 0;
        let buttons = read_joypad(|| {
            count += 1;
            bits.next().unwrap()
        });
        assert_eq!(buttons, 0b1001_0001);
        assert_eq!(count, 8);
    }
}

// ---- 課題 13: iNES ヘッダー ----

/// マッパー番号（フラグ 7 の上位 4 ビットとフラグ 6 の上位 4 ビット）
pub fn mapper_number(header: &[u8; 16]) -> u8 {
    (header[7] & 0xF0) | (header[6] >> 4)
}

/// 垂直ミラーリングか（フラグ 6 のビット 0）
pub fn vertical_mirroring(header: &[u8; 16]) -> bool {
    header[6] & 0x01 != 0
}

#[cfg(test)]
mod test_ines {
    use super::*;

    #[test]
    fn reads_header_flags() {
        let mut header = [0; 16];
        header[..4].copy_from_slice(b"NES\x1A");
        assert_eq!(mapper_number(&header), 0);
        assert!(!vertical_mirroring(&header));
        header[6] = 0x41;
        assert_eq!(mapper_number(&header), 4);
        assert!(vertical_mirroring(&header));
        header[6] = 0x10;
        header[7] = 0x40;
        assert_eq!(mapper_number(&header), 0x41);
    }
}

// ---- 課題 14: タイルの 1 行を色番号にする ----

/// lo と hi から 8 ドット分の色番号（0〜3）を左から並べる。flip_h なら左右反転する
pub fn tile_row(lo: u8, hi: u8, flip_h: bool) -> [u8; 8] {
    let mut row = [0; 8];
    for (px, color) in row.iter_mut().enumerate() {
        let bit = if flip_h { px } else { 7 - px };
        *color = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);
    }
    row
}

#[cfg(test)]
mod test_tile_row {
    use super::*;

    #[test]
    fn decodes_two_planes() {
        assert_eq!(tile_row(0x3C, 0x66, false), [0, 2, 3, 1, 1, 3, 2, 0]);
        assert_eq!(tile_row(0xF0, 0x00, false), [1, 1, 1, 1, 0, 0, 0, 0]);
        assert_eq!(tile_row(0xF0, 0x00, true), [0, 0, 0, 0, 1, 1, 1, 1]);
        assert_eq!(tile_row(0xFF, 0xFF, false), [3; 8]);
    }
}

// ---- 課題 15: パレット RAM の位置を組み立てる ----

/// 背景のドットのパレット RAM の位置（%000P_PCC のうち、ビット 4 は 0）
pub fn bg_palette_addr(palette: u8, color: u8) -> u8 {
    (palette << 2) | color
}

/// スプライトのドットのパレット RAM の位置。パレット番号は属性のビット 1〜0
pub fn sprite_palette_addr(attr: u8, color: u8) -> u8 {
    0x10 | ((attr & 3) << 2) | color
}

#[cfg(test)]
mod test_palette_addr {
    use super::*;

    #[test]
    fn builds_palette_addresses() {
        assert_eq!(bg_palette_addr(0, 1), 0x01);
        assert_eq!(bg_palette_addr(2, 3), 0x0B);
        assert_eq!(sprite_palette_addr(0x00, 1), 0x11);
        assert_eq!(sprite_palette_addr(0x63, 2), 0x1E); // 属性の上位ビット（反転など）は関係ない
    }
}

// ---- 課題 16: 属性テーブルからパレット番号を取り出す ----

/// タイルの行 row、列 col のパレット番号（0〜3）。attr はそのタイルを含む属性バイト
pub fn attr_palette(attr: u8, row: u16, col: u16) -> u8 {
    let shift = (row % 4 / 2) * 4 + (col % 4 / 2) * 2;
    (attr >> shift) & 3
}

#[cfg(test)]
mod test_attr_palette {
    use super::*;

    #[test]
    fn picks_quadrant() {
        let attr = 0b11_10_01_00; // 右下 3、左下 2、右上 1、左上 0
        assert_eq!(attr_palette(attr, 0, 0), 0);
        assert_eq!(attr_palette(attr, 1, 3), 1);
        assert_eq!(attr_palette(attr, 2, 1), 2);
        assert_eq!(attr_palette(attr, 3, 2), 3);
        assert_eq!(attr_palette(attr, 6, 7), 3); // 次の 4×4 でも同じ位置関係
    }
}

// ---- 課題 17: コントローラー ----

pub const BUTTON_A: u8 = 1 << 0;
pub const BUTTON_B: u8 = 1 << 1;
pub const BUTTON_SELECT: u8 = 1 << 2;
pub const BUTTON_START: u8 = 1 << 3;
pub const BUTTON_UP: u8 = 1 << 4;
pub const BUTTON_DOWN: u8 = 1 << 5;
pub const BUTTON_LEFT: u8 = 1 << 6;
pub const BUTTON_RIGHT: u8 = 1 << 7;

#[derive(Default)]
pub struct Controller {
    pub buttons: u8,
    shift: u8,
}

impl Controller {
    /// $4016 への書き込み。ビット 0 が 1 なら、いまのボタンを取り込む
    pub fn write(&mut self, value: u8) {
        if value & 1 != 0 {
            self.shift = self.buttons;
        }
    }

    /// $4016 の読み出し。ビット 0 を返し、右へずらして上から 1 を入れる
    pub fn read(&mut self) -> u8 {
        let bit = self.shift & 1;
        self.shift = (self.shift >> 1) | 0x80;
        bit
    }
}

#[cfg(test)]
mod test_controller {
    use super::*;

    #[test]
    fn reads_buttons_in_order() {
        let mut c = Controller {
            buttons: BUTTON_A | BUTTON_START | BUTTON_RIGHT,
            ..Default::default()
        };
        c.write(1);
        c.write(0);
        let bits: Vec<u8> = (0..10).map(|_| c.read()).collect();
        assert_eq!(bits, [1, 0, 0, 1, 0, 0, 0, 1, 1, 1]);
        // 課題 12 の読み方で読むと、順番が逆の 1 バイトになる
        c.write(1);
        c.write(0);
        assert_eq!(read_joypad(|| c.read()), 0b1001_0001);
        assert_eq!(reverse_bits(0b1001_0001), c.buttons);
    }
}

// ---- 課題 18: PPU の v ----

/// v を (fine Y, NT, coarse Y, coarse X) に分ける
pub fn decode_v(v: u16) -> (u16, u16, u16, u16) {
    ((v >> 12) & 7, (v >> 10) & 3, (v >> 5) & 31, v & 31)
}

/// (fine Y, NT, coarse Y, coarse X) から v を作る
pub fn encode_v(fine_y: u16, nt: u16, coarse_y: u16, coarse_x: u16) -> u16 {
    ((fine_y & 7) << 12) | ((nt & 3) << 10) | ((coarse_y & 31) << 5) | (coarse_x & 31)
}

/// 右のタイルへ進める。31 列目の次は、隣のネームテーブルの 0 列目
pub fn increment_x(v: u16) -> u16 {
    if v & 0x001F == 31 {
        (v & !0x001F) ^ 0x0400
    } else {
        v + 1
    }
}

/// v が指しているタイルの属性バイトのアドレス
pub fn attr_addr(v: u16) -> u16 {
    0x23C0 | (v & 0x0C00) | ((v >> 4) & 0x38) | ((v >> 2) & 0x07)
}

#[cfg(test)]
mod test_ppu_v {
    use super::*;

    #[test]
    fn encode_and_decode() {
        assert_eq!(decode_v(0x2C5F), (2, 3, 2, 31));
        assert_eq!(encode_v(2, 3, 2, 31), 0x2C5F);
        assert_eq!(encode_v(5, 1, 13, 22), 0x55B6);
    }

    #[test]
    fn moves_right() {
        assert_eq!(increment_x(encode_v(0, 0, 3, 5)), encode_v(0, 0, 3, 6));
        assert_eq!(increment_x(encode_v(0, 0, 3, 31)), encode_v(0, 1, 3, 0));
        assert_eq!(increment_x(encode_v(4, 3, 3, 31)), encode_v(4, 2, 3, 0));
    }

    #[test]
    fn finds_attribute_byte() {
        assert_eq!(attr_addr(encode_v(0, 0, 0, 0)), 0x23C0);
        assert_eq!(attr_addr(encode_v(5, 1, 13, 22)), 0x27DD);
        assert_eq!(attr_addr(encode_v(0, 3, 29, 31)), 0x2FFF);
    }
}

// ---- 課題 19: APU のレジスタとノイズ ----

/// $4002 の書き込み: 周期の下位 8 ビットを入れ替える
pub fn write_period_lo(period: u16, value: u8) -> u16 {
    (period & 0x0700) | value as u16
}

/// $4003 の書き込み: 周期の上位 3 ビットを入れ替える。戻り値は (周期, 長さカウンタの番号)
pub fn write_period_hi(period: u16, value: u8) -> (u16, u8) {
    ((period & 0x00FF) | ((value as u16 & 0x07) << 8), value >> 3)
}

/// ノイズのシフトレジスタを 1 回進める
pub fn lfsr_step(shift: u16) -> u16 {
    let feedback = (shift ^ (shift >> 1)) & 1;
    (shift >> 1) | (feedback << 14)
}

#[cfg(test)]
mod test_apu {
    use super::*;

    #[test]
    fn splits_period() {
        let period = write_period_lo(0, 0xFD);
        assert_eq!(period, 0x0FD);
        assert_eq!(write_period_hi(period, 0b0000_1010), (0x2FD, 1)); // 長さ 1、上位 3 ビット 2
        assert_eq!(write_period_lo(0x2FD, 0x10), 0x210); // 上位 3 ビットは残る
    }

    #[test]
    fn noise_repeats_after_32767_steps() {
        let mut shift = 1;
        let mut steps = 0;
        loop {
            shift = lfsr_step(shift);
            steps += 1;
            assert!(shift < 0x8000); // 15 ビットに収まっている
            if shift == 1 {
                break;
            }
        }
        assert_eq!(steps, 32767);
    }
}
