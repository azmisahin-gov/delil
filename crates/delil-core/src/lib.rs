//! DELİL çekirdek alan modeli.
//!
//! Bu crate, ürünün tamamında paylaşılan veri türlerini barındırır:
//! vaka, görevlendirme, materyal, ses envanteri, transkript, soru ve rapor.
//!
//! Tasarım ilkeleri:
//! * Her tür `serde` ile serileştirilebilir; vaka deposu ve manifestolar bu
//!   türleri doğrudan JSON olarak saklar.
//! * Zamanlar UTC `chrono::DateTime` olarak tutulur.
//! * Kimlikler `uuid::Uuid` tabanlıdır ve `new_v4` ile üretilir.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub mod ids;
pub mod model;
pub mod status;
pub mod transcript_edit;

pub use ids::*;
pub use model::*;
pub use status::*;
pub use transcript_edit::*;

/// Yeni bir sürüm kimliği üretir.
pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

/// Şu anki UTC zaman damgası.
pub fn now() -> DateTime<Utc> {
    Utc::now()
}

/// Uygulama sürümü; rapor ve manifestolarda kullanılır.
pub const APP_NAME: &str = "DELİL";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const UDF_FORMAT_ID: &str = "1.8";
pub const WHISPER_MODEL_NAME: &str = "whisper-large-v3-turbo";

/// DELİL'in üç temel işlemi. İş akışı adımları bu evrelere eşlenir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreOperation {
    /// Delili al: algıla, envantere al, kopyala, doğrula.
    AcquireEvidence,
    /// Sesi çöz: ses dosyalarını belirle, metne dönüştür, eşleştir.
    TranscribeAudio,
    /// UDF raporunu oluştur: birleştir, doğrula, üret.
    GenerateUdfReport,
}

impl CoreOperation {
    /// Kullanıcıya gösterilecek Türkçe başlık.
    pub fn label_tr(&self) -> &'static str {
        match self {
            CoreOperation::AcquireEvidence => "Delili al",
            CoreOperation::TranscribeAudio => "Sesi çöz",
            CoreOperation::GenerateUdfReport => "UDF raporunu oluştur",
        }
    }
}

/// `PathBuf` alanlarını taşıyan yapılar için ortak yardımcı.
pub fn path_to_string(p: &std::path::Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Dosya yolu yardımcı türü: `PathBuf` doğrudan serde ile taşınır.
pub type FilePath = PathBuf;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
    }

    #[test]
    fn core_operations_have_turkish_labels() {
        assert_eq!(CoreOperation::AcquireEvidence.label_tr(), "Delili al");
        assert_eq!(CoreOperation::TranscribeAudio.label_tr(), "Sesi çöz");
        assert_eq!(
            CoreOperation::GenerateUdfReport.label_tr(),
            "UDF raporunu oluştur"
        );
    }
}
