//! カートリッジ: iNES 形式の ROM ファイルを読み込む（マッパー 0 の NROM だけを扱う）

/// ネームテーブルの並べ方（ミラーリング）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirroring {
    /// 左右に同じ内容（縦スクロールのゲーム向け）… $2000=$2400, $2800=$2C00
    Horizontal,
    /// 上下に同じ内容（横スクロールのゲーム向け）… $2000=$2800, $2400=$2C00
    Vertical,
}

pub const PRG_BANK_SIZE: usize = 16 * 1024;
pub const CHR_BANK_SIZE: usize = 8 * 1024;

pub struct Cartridge {
    /// プログラム（CPU の $8000〜$FFFF に見える）
    pub prg_rom: Vec<u8>,
    /// パターンテーブル（PPU の $0000〜$1FFF に見える）
    pub chr_rom: Vec<u8>,
    pub mirroring: Mirroring,
}

impl Cartridge {
    /// iNES 形式のデータからカートリッジを作る
    pub fn from_ines(data: &[u8]) -> Result<Cartridge, String> {
        if data.len() < 16 || &data[0..4] != b"NES\x1A" {
            return Err("iNES 形式の ROM ではありません".to_string());
        }
        let prg_banks = data[4] as usize;
        let chr_banks = data[5] as usize;
        let flags6 = data[6];
        let flags7 = data[7];
        let mapper = (flags7 & 0xF0) | (flags6 >> 4);
        if mapper != 0 {
            return Err(format!("マッパー {mapper} には対応していません"));
        }
        if prg_banks == 0 || chr_banks == 0 {
            return Err("PRG ROM か CHR ROM がありません".to_string());
        }
        let mirroring = if flags6 & 0x01 != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };

        // ヘッダ（16 バイト）の後ろに PRG、CHR の順に並ぶ
        let prg_end = 16 + prg_banks * PRG_BANK_SIZE;
        let chr_end = prg_end + chr_banks * CHR_BANK_SIZE;
        if data.len() < chr_end {
            return Err(format!(
                "ファイルが短すぎます（{chr_end} バイト必要ですが {} バイトしかありません）",
                data.len()
            ));
        }

        Ok(Cartridge {
            prg_rom: data[16..prg_end].to_vec(),
            chr_rom: data[prg_end..chr_end].to_vec(),
            mirroring,
        })
    }

    /// CPU からの読み込み（$4020〜$FFFF）
    pub fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            // PRG ROM が 16 KiB のときは、$8000〜$BFFF と $C000〜$FFFF に同じ内容が見える
            0x8000..=0xFFFF => self.prg_rom[(addr - 0x8000) as usize % self.prg_rom.len()],
            _ => 0,
        }
    }

    /// PPU からの読み込み（$0000〜$1FFF のパターンテーブル）
    pub fn ppu_read(&mut self, addr: u16) -> u8 {
        self.chr_rom[addr as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// テスト用の iNES データを作る
    fn make_ines(prg_banks: u8, chr_banks: u8, flags6: u8) -> Vec<u8> {
        let mut data = vec![b'N', b'E', b'S', 0x1A, prg_banks, chr_banks, flags6, 0];
        data.resize(16, 0);
        for bank in 0..prg_banks {
            data.resize(data.len() + PRG_BANK_SIZE, bank);
        }
        data.resize(data.len() + chr_banks as usize * CHR_BANK_SIZE, 0xCC);
        data
    }

    #[test]
    fn parses_header() {
        let cart = Cartridge::from_ines(&make_ines(2, 1, 0x01)).unwrap();
        assert_eq!(cart.prg_rom.len(), 32 * 1024);
        assert_eq!(cart.chr_rom.len(), 8 * 1024);
        assert_eq!(cart.mirroring, Mirroring::Vertical);
    }

    #[test]
    fn rejects_bad_data() {
        assert!(Cartridge::from_ines(b"hello").is_err());
        let mut data = make_ines(1, 1, 0);
        data.truncate(100);
        assert!(Cartridge::from_ines(&data).is_err());
        // マッパー 2（flags6 の上位 4 ビットが 2）には対応していない
        assert!(Cartridge::from_ines(&make_ines(1, 1, 0x20)).is_err());
    }

    #[test]
    fn nrom_128_is_mirrored() {
        // 16 KiB の PRG は $8000 と $C000 の両方に見える
        let mut data = make_ines(1, 1, 0);
        data[16] = 0x42;
        let mut cart = Cartridge::from_ines(&data).unwrap();
        assert_eq!(cart.cpu_read(0x8000), 0x42);
        assert_eq!(cart.cpu_read(0xC000), 0x42);
    }
}
