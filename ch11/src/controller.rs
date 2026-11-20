//! コントローラー（$4016）
//!
//! $4016 に 1 → 0 を書くと、その瞬間のボタンの状態がシフトレジスタに取り込まれる（ストローブ）。
//! その後 $4016 を読むたびに、A, B, SELECT, START, 上, 下, 左, 右 の順に 1 ビットずつ返る。

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
    /// いま押されているボタン（BUTTON_* の組み合わせ）
    pub buttons: u8,
    shift: u8,
}

impl Controller {
    pub fn write(&mut self, value: u8) {
        // ストローブが 1 のあいだ、ボタンの状態を取り込む
        if value & 1 != 0 {
            self.shift = self.buttons;
        }
    }

    pub fn read(&mut self) -> u8 {
        let bit = self.shift & 1;
        // 8 個読み終わった後は 1 が返る
        self.shift = (self.shift >> 1) | 0x80;
        bit
    }
}

#[cfg(test)]
mod tests {
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
    }
}
