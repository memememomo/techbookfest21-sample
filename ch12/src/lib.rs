//! mynes: 入門書のために作る、シンプルなファミコンエミュレーター

pub mod apu;
pub mod bus;
pub mod cartridge;
pub mod controller;
pub mod cpu;
pub mod nes;
pub mod opcodes;
pub mod palette;
pub mod ppu;
#[cfg(target_arch = "wasm32")]
pub mod web;
