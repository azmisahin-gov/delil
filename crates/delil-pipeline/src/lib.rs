//! Uçtan uca otomatik iş akışı orkestrasyonu (şartname 4.2).
//!
//! Bu crate, ürünün üç temel işlemini tek bir otomatik akışta birleştirir:
//! 1. **DELİLİ AL** — görevlendirmeyi oku, materyali algıla, envantere al,
//!    kopyala ve SHA-256 ile doğrula.
//! 2. **SESİ ÇÖZ** — ses dosyalarını bul, transkripsiyon kuyruğuna al,
//!    kalite kontrolünü çalıştır.
//! 3. **UDF RAPORUNU OLUŞTUR** — rapor taslağını üret, doğrudan UDF oluştur,
//!    yapısal doğrulamayı çalıştır.
//!
//! İş akışı adımları kullanıcıya [`PipelineStep`] durumları olarak bildirilir;
//! her adım için ayrı bir düğmeye basılması gerekmez.

pub mod steps;

pub use steps::{
    run_full_pipeline, PipelineConfig, PipelineEvent, PipelineResult, PipelineRunner,
    TranscriptionOutcome,
};

use thiserror::Error;

/// İş akışı hataları.
#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("Edinim hatası: {0}")]
    Evidence(#[from] delil_evidence::EvidenceError),
    #[error("Görevlendirme okuma hatası: {0}")]
    Assignment(#[from] delil_assignment::AssignmentError),
    #[error("Transkripsiyon hatası: {0}")]
    Transcription(#[from] delil_transcription::TranscriptionError),
    #[error("Rapor hatası: {0}")]
    Report(#[from] delil_report::ReportError),
    #[error("UDF hatası: {0}")]
    Udf(#[from] delil_udf::UdfError),
    #[error("Depo hatası: {0}")]
    Store(#[from] delil_case_store::StoreError),
    #[error("Dışa aktarma hatası: {0}")]
    Export(#[from] delil_export::ExportError),
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON hatası: {0}")]
    Json(#[from] serde_json::Error),
    #[error("İşlem iptal edildi")]
    Cancelled,
    #[error("Kullanıcı kararı gerekli: {0}")]
    NeedsDecision(String),
}

pub type Result<T> = std::result::Result<T, PipelineError>;
