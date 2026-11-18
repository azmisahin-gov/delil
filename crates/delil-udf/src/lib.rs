//! UYAP UDF (Ulusal Yargı Ağı Projesi Doküman Formatı) desteği.
//!
//! UDF, Deflate ile sıkıştırılmış bir ZIP arşividir ve içinde tek bir
//! `content.xml` barındırır. `content.xml` UTF-8 kodludur ve şu yapıya
//! sahiptir:
//!
//! ```xml
//! <?xml version="1.0" encoding="UTF-8" ?>
//! <template format_id="1.8">
//!   <content><![CDATA[metin havuzu]]></content>
//!   <properties><pageFormat .../></properties>
//!   <elements resolver="hvl-default"> ... </elements>
//!   <styles> ... </styles>
//! </template>
//! ```
//!
//! Kritik kural: `<elements>` içindeki elemanlar metni `startOffset` ve
//! `length` ile referans alır. Bu değerler **karakter (rune)** cinsindendir,
//! bayt değil. Türkçe karakterler ve emoji dahil her karakter 1 sayılır.
//!
//! Bu crate üç işlevi sağlar:
//! * [`writer`] — belge modelinden gerçek UDF üretir.
//! * [`reader`] — UDF'yi çözümleyip metin ve yapıyı çıkarır.
//! * [`validator`] — paket bütünlüğü, XML yapısı, ofset tutarlılığı vb.
//!   doğrular.

pub mod model;
pub mod reader;
pub mod validator;
pub mod writer;

pub use model::*;
pub use reader::{read_udf, UdfDocument};
pub use validator::{validate_udf, ValidationReport};
pub use writer::{write_udf, UdfBuilder};

use thiserror::Error;

/// UDF işlemlerinde oluşan hatalar.
#[derive(Debug, Error)]
pub enum UdfError {
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("ZIP hatası: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("XML hatası: {0}")]
    Xml(String),
    #[error("content.xml bulunamadı")]
    MissingContent,
    #[error("Doğrulama başarısız: {0}")]
    Invalid(String),
    #[error("Base64 çözme hatası: {0}")]
    Base64(#[from] base64::DecodeError),
}

pub type Result<T> = std::result::Result<T, UdfError>;

/// UDF format sürümü.
pub const FORMAT_ID: &str = "1.8";

/// UDF renk kodlama: işaretli 32-bit ARGB tam sayı.
pub mod color {
    /// Siyah: #000000.
    pub const BLACK: i32 = -16777216;
    /// Beyaz: #FFFFFF.
    pub const WHITE: i32 = -1;
    /// Varsayılan metin rengi (koyu gri).
    pub const DEFAULT_FOREGROUND: i32 = -13421773;

    /// `#RRGGBB` değerini işaretli ARGB tam sayısına çevirir.
    pub fn rgb_to_argb_i32(r: u8, g: u8, b: u8) -> i32 {
        let v: u32 = 0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        v as i32
    }

    /// İşaretli ARGB tam sayısını `(r, g, b)` üçlüsüne çevirir.
    pub fn argb_i32_to_rgb(v: i32) -> (u8, u8, u8) {
        let u = v as u32;
        (
            ((u >> 16) & 0xFF) as u8,
            ((u >> 8) & 0xFF) as u8,
            (u & 0xFF) as u8,
        )
    }
}

/// A4 sayfa boyutu (punto).
pub const A4_WIDTH_PT: f64 = 595.28;
pub const A4_HEIGHT_PT: f64 = 841.89;

/// Resim yer tutucusu (CDATA içinde resim konumu).
pub const OBJECT_REPLACEMENT_CHAR: char = '\u{FFFC}';
/// Boş paragraf işaretçisi.
pub const ZERO_WIDTH_SPACE: char = '\u{200B}';
/// Sekme karakteri.
pub const TAB_CHAR: char = '\t';

#[cfg(test)]
mod tests {
    use super::color::*;

    #[test]
    fn black_is_known_value() {
        assert_eq!(BLACK, -16777216);
        assert_eq!(rgb_to_argb_i32(0, 0, 0), BLACK);
    }

    #[test]
    fn white_is_known_value() {
        assert_eq!(WHITE, -1);
        assert_eq!(rgb_to_argb_i32(255, 255, 255), WHITE);
    }

    #[test]
    fn rgb_roundtrip() {
        for (r, g, b) in [(0u8, 0u8, 0u8), (255, 0, 0), (0, 128, 64), (12, 34, 56)] {
            let v = rgb_to_argb_i32(r, g, b);
            assert_eq!(argb_i32_to_rgb(v), (r, g, b));
        }
    }
}
