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
}
