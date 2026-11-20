//! ブラウザ（JavaScript）から使うためのラッパー。wasm-bindgen で JavaScript に公開する

use crate::cartridge::Cartridge;
use crate::nes::Nes;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WebNes {
    nes: Nes,
}

#[wasm_bindgen]
impl WebNes {
    /// ROM のデータ（Uint8Array）から作る
    #[wasm_bindgen(constructor)]
    pub fn new(rom: &[u8]) -> Result<WebNes, JsValue> {
        let cart = Cartridge::from_ines(rom).map_err(|e| JsValue::from_str(&e))?;
        Ok(WebNes {
            nes: Nes::new(cart),
        })
    }

    /// 1 フレーム進める
    pub fn run_frame(&mut self) {
        self.nes.run_frame();
    }

    /// 画面（RGBA、256 × 240）を Uint8Array で返す
    pub fn frame(&self) -> Vec<u8> {
        self.nes.frame().to_vec()
    }

    pub fn reset(&mut self) {
        self.nes.reset();
    }

    /// コントローラーのボタン（ビット 0 から A, B, SELECT, START, 上, 下, 左, 右）
    pub fn set_buttons(&mut self, buttons: u8) {
        self.nes.set_buttons(buttons);
    }

    pub fn set_sample_rate(&mut self, rate: f64) {
        self.nes.bus.apu.set_sample_rate(rate);
    }

    /// たまった音のサンプルを Float32Array で返す
    pub fn take_samples(&mut self) -> Vec<f32> {
        self.nes.take_samples()
    }
}
