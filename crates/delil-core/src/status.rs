//! Durum ve numaralandırma türleri.
//!
//! Bu türler, iş akışı adımlarını, doğrulama durumlarını, transkript
//! statülerini ve kalite durumlarını tanımlar. Kullanıcıya gösterilen
//! etiketler Türkçedir; serileştirilmiş değerler kararlıdır (snake_case).

use serde::{Deserialize, Serialize};

/// Otomatik işlem ekranındaki adımlar (bkz. şartname 4.2 ve 14.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStep {
    ReadingAssignment,
    AcquiringMaterial,
    VerifyingFiles,
    SearchingAudio,
    Transcribing,
    QualityControl,
    PreparingReport,
    ValidatingUdf,
}

impl PipelineStep {
    /// Tüm adımlar, işlem sırasına göre.
    pub const ALL: [PipelineStep; 8] = [
        PipelineStep::ReadingAssignment,
        PipelineStep::AcquiringMaterial,
        PipelineStep::VerifyingFiles,
        PipelineStep::SearchingAudio,
        PipelineStep::Transcribing,
        PipelineStep::QualityControl,
        PipelineStep::PreparingReport,
        PipelineStep::ValidatingUdf,
    ];

    /// Kullanıcıya gösterilen Türkçe adım metni.
    pub fn label_tr(&self) -> &'static str {
        match self {
            PipelineStep::ReadingAssignment => "Görevlendirme okunuyor",
            PipelineStep::AcquiringMaterial => "Materyal alınıyor",
            PipelineStep::VerifyingFiles => "Dosyalar doğrulanıyor",
            PipelineStep::SearchingAudio => "Sesler aranıyor",
            PipelineStep::Transcribing => "Transkripsiyon yapılıyor",
            PipelineStep::QualityControl => "Kalite kontrolü yapılıyor",
            PipelineStep::PreparingReport => "Rapor hazırlanıyor",
            PipelineStep::ValidatingUdf => "UDF doğrulanıyor",
        }
    }
}

/// İş akışı adımının durumu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "detail")]
pub enum StepState {
    Pending,
    Running {
        progress: f32,
    },
    Done,
    Failed {
        message: String,
    },
    /// Kullanıcı kararı gerektiren istisna.
    NeedsDecision {
        message: String,
    },
}

/// Edinim/doğrulama durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// Kaynak okunamadı veya kopyalanamadı.
    Failed,
    /// Kopyalandı, hash henüz karşılaştırılmadı.
    Copied,
    /// Kaynak ve hedef hash eşleşti.
    Verified,
    /// Hash eşleşmedi.
    Mismatch,
}

impl VerificationStatus {
    pub fn label_tr(&self) -> &'static str {
        match self {
            VerificationStatus::Failed => "Başarısız",
            VerificationStatus::Copied => "Kopyalandı",
            VerificationStatus::Verified => "Doğrulandı",
            VerificationStatus::Mismatch => "Hash uyuşmadı",
        }
    }

    /// Yalnızca gerçek doğrulama başarılıysa `true`.
    pub fn is_verified(&self) -> bool {
        matches!(self, VerificationStatus::Verified)
    }
}

/// Transkript statüleri (bkz. şartname 10.3). Beş durum ayrı tutulur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptStatus {
    /// 1. Makine tarafından oluşturulmuş.
    MachineGenerated,
    /// 2. Otomatik kalite kontrolü tamamlanmış.
    QualityChecked,
    /// 3. Bilirkişi incelemesi bekliyor.
    AwaitingReview,
    /// 4. Bilirkişi tarafından düzeltilmiş.
    ExpertEdited,
    /// 5. Bilirkişi tarafından onaylanmış.
    ExpertApproved,
}

impl TranscriptStatus {
    pub fn label_tr(&self) -> &'static str {
        match self {
            TranscriptStatus::MachineGenerated => "Makine tarafından oluşturulmuş",
            TranscriptStatus::QualityChecked => "Otomatik kalite kontrolü tamamlanmış",
            TranscriptStatus::AwaitingReview => "Bilirkişi incelemesi bekliyor",
            TranscriptStatus::ExpertEdited => "Bilirkişi tarafından düzeltilmiş",
            TranscriptStatus::ExpertApproved => "Bilirkişi tarafından onaylanmış",
        }
    }

    /// Sıralı ilerleme numarası (1..=5).
    pub fn ordinal(&self) -> u8 {
        match self {
            TranscriptStatus::MachineGenerated => 1,
            TranscriptStatus::QualityChecked => 2,
            TranscriptStatus::AwaitingReview => 3,
            TranscriptStatus::ExpertEdited => 4,
            TranscriptStatus::ExpertApproved => 5,
        }
    }
}

/// Kalite kontrol durumları (bkz. şartname 9). Model olasılıkları gerçek
/// doğruluk yüzdesi olarak sunulmaz; bunun yerine bu anlaşılır durumlar
/// kullanılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityFlag {
    /// Normal.
    Normal,
    /// Kontrol önerilir.
    ReviewSuggested,
    /// Öncelikli inceleme.
    PriorityReview,
    /// Anlaşılamayan bölüm.
    Unintelligible,
}

impl QualityFlag {
    pub fn label_tr(&self) -> &'static str {
        match self {
            QualityFlag::Normal => "Normal",
            QualityFlag::ReviewSuggested => "Kontrol önerilir",
            QualityFlag::PriorityReview => "Öncelikli inceleme",
            QualityFlag::Unintelligible => "Anlaşılamayan bölüm",
        }
    }

    /// Risk sırası (yüksek = daha riskli).
    pub fn severity(&self) -> u8 {
        match self {
            QualityFlag::Normal => 0,
            QualityFlag::ReviewSuggested => 1,
            QualityFlag::PriorityReview => 2,
            QualityFlag::Unintelligible => 3,
        }
    }
}

/// Kalite kontrolünde tespit edilen durum türleri (şartname 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityIssueKind {
    LongSilence,
    EmptyTranscriptWithSpeech,
    TextInSilence,
    SuspiciousRepetition,
    NonSpeechToken,
    VeryLowVolume,
    CorruptSegment,
    OverlappingSpeech,
    UnintelligibleSegment,
    LanguageInconsistency,
    TimestampInconsistency,
    ChunkBoundaryLoss,
    ProperNounOrNumberRisk,
    NegationRisk,
}

impl QualityIssueKind {
    pub fn label_tr(&self) -> &'static str {
        match self {
            QualityIssueKind::LongSilence => "Uzun sessizlik",
            QualityIssueKind::EmptyTranscriptWithSpeech => "Konuşma varken boş transkript",
            QualityIssueKind::TextInSilence => "Sessizlikte metin üretilmesi",
            QualityIssueKind::SuspiciousRepetition => "Şüpheli tekrarlar",
            QualityIssueKind::NonSpeechToken => "Konuşma dışı kelime üretimi",
            QualityIssueKind::VeryLowVolume => "Çok düşük ses",
            QualityIssueKind::CorruptSegment => "Bozuk kayıt bölümü",
            QualityIssueKind::OverlappingSpeech => "Örtüşen konuşma",
            QualityIssueKind::UnintelligibleSegment => "Anlaşılmayan bölüm",
            QualityIssueKind::LanguageInconsistency => "Dil algılama tutarsızlığı",
            QualityIssueKind::TimestampInconsistency => "Zaman kodu tutarsızlığı",
            QualityIssueKind::ChunkBoundaryLoss => "Parça sınırında metin kaybı",
            QualityIssueKind::ProperNounOrNumberRisk => "Özel isim ve sayı hatası olasılığı",
            QualityIssueKind::NegationRisk => "Olumsuzluk sözcüğü hatası olasılığı",
        }
    }

    /// Bu türün varsayılan kalite bayrağı.
    pub fn default_flag(&self) -> QualityFlag {
        match self {
            QualityIssueKind::LongSilence
            | QualityIssueKind::TextInSilence
            | QualityIssueKind::SuspiciousRepetition
            | QualityIssueKind::NonSpeechToken
            | QualityIssueKind::TimestampInconsistency
            | QualityIssueKind::ChunkBoundaryLoss => QualityFlag::ReviewSuggested,
            QualityIssueKind::VeryLowVolume
            | QualityIssueKind::CorruptSegment
            | QualityIssueKind::OverlappingSpeech
            | QualityIssueKind::LanguageInconsistency
            | QualityIssueKind::ProperNounOrNumberRisk
            | QualityIssueKind::NegationRisk
            | QualityIssueKind::EmptyTranscriptWithSpeech => QualityFlag::PriorityReview,
            QualityIssueKind::UnintelligibleSegment => QualityFlag::Unintelligible,
        }
    }
}

/// Aygıt türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Usb,
    SdCard,
    ExternalHdd,
    ExternalSsd,
    /// Bilgisayar içindeki normal klasör/dosya.
    LocalFolder,
    Archive,
    Unknown,
}

impl DeviceKind {
    pub fn label_tr(&self) -> &'static str {
        match self {
            DeviceKind::Usb => "USB bellek",
            DeviceKind::SdCard => "SD kart",
            DeviceKind::ExternalHdd => "Harici HDD",
            DeviceKind::ExternalSsd => "Harici SSD",
            DeviceKind::LocalFolder => "Bilgisayar klasörü",
            DeviceKind::Archive => "Arşiv",
            DeviceKind::Unknown => "Bilinmeyen aygıt",
        }
    }
}

/// Yazılımsal yazma koruması durumu. Donanımsal yazma engelleyici ile
/// eşdeğer gösterilmez (şartname 6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteProtection {
    /// Yazılımsal salt okunur koruma uygulandı ve doğrulandı.
    SoftwareReadOnly,
    /// Uygulanamadı veya doğrulanamadı.
    NotApplied,
    /// Kullanıcı donanımsal yazma engelleyici kullandığını bildirdi.
    HardwareWriteBlockerReported,
}

impl WriteProtection {
    pub fn label_tr(&self) -> &'static str {
        match self {
            WriteProtection::SoftwareReadOnly => "Yazılımsal salt okunur koruma uygulandı",
            WriteProtection::NotApplied => "Yazma koruması uygulanmadı",
            WriteProtection::HardwareWriteBlockerReported => {
                "Donanımsal yazma engelleyici kullanıldığı bildirildi"
            }
        }
    }

    /// Donanımsal koruma iddiası mı? (rapor uyarısı için)
    pub fn claims_hardware(&self) -> bool {
        matches!(self, WriteProtection::HardwareWriteBlockerReported)
    }
}

/// Kuyruk işi durumu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "detail")]
pub enum JobState {
    Queued,
    Running { progress: f32 },
    Paused { progress: f32 },
    Done,
    Failed { message: String, attempts: u32 },
    Cancelled,
}

impl JobState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobState::Done | JobState::Failed { .. } | JobState::Cancelled
        )
    }
}

/// İstisna (kullanıcı kararı) türleri (şartname 4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExceptionKind {
    MultipleDevicesAmbiguous,
    CaseNumberUnreadable,
    CorruptAudio,
    UnsupportedCodec,
    VeryLowVolume,
    UnintelligibleSegment,
    OverlappingSpeech,
    SuspiciousModelOutput,
    QuestionEvidenceMissing,
    ExpertOpinionMissing,
}

impl ExceptionKind {
    pub fn label_tr(&self) -> &'static str {
        match self {
            ExceptionKind::MultipleDevicesAmbiguous => {
                "Birden fazla harici aygıt bağlı; kaynak belirsiz"
            }
            ExceptionKind::CaseNumberUnreadable => "Dosya numarası okunamıyor",
            ExceptionKind::CorruptAudio => "Ses dosyası bozuk",
            ExceptionKind::UnsupportedCodec => "Desteklenmeyen codec",
            ExceptionKind::VeryLowVolume => "Ses çok düşük",
            ExceptionKind::UnintelligibleSegment => "Bir bölüm anlaşılmıyor",
            ExceptionKind::OverlappingSpeech => "Örtüşen konuşma var",
            ExceptionKind::SuspiciousModelOutput => "Modelin ürettiği metin şüpheli",
            ExceptionKind::QuestionEvidenceMissing => "Görevlendirme sorusunun dayanağı bulunamadı",
            ExceptionKind::ExpertOpinionMissing => "Mesleki kanaat alanı henüz doldurulmamış",
        }
    }
}

/// Güncelleme durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStatus {
    UpToDate,
    Available,
    Downloaded,
    SignatureInvalid,
    HashMismatch,
    Installing,
    Installed,
    RolledBack,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_steps_have_labels() {
        for step in PipelineStep::ALL {
            assert!(!step.label_tr().is_empty());
        }
        assert_eq!(PipelineStep::ALL.len(), 8);
    }

    #[test]
    fn transcript_status_ordinals_are_ordered() {
        assert!(
            TranscriptStatus::MachineGenerated.ordinal()
                < TranscriptStatus::ExpertApproved.ordinal()
        );
        assert_eq!(TranscriptStatus::ExpertApproved.ordinal(), 5);
    }

    #[test]
    fn verification_only_true_for_verified() {
        assert!(VerificationStatus::Verified.is_verified());
        assert!(!VerificationStatus::Mismatch.is_verified());
        assert!(!VerificationStatus::Copied.is_verified());
        assert!(!VerificationStatus::Failed.is_verified());
    }

    #[test]
    fn quality_flags_are_severity_ordered() {
        assert!(QualityFlag::Normal.severity() < QualityFlag::ReviewSuggested.severity());
        assert!(QualityFlag::ReviewSuggested.severity() < QualityFlag::PriorityReview.severity());
        assert!(QualityFlag::PriorityReview.severity() < QualityFlag::Unintelligible.severity());
    }

    #[test]
    fn write_protection_does_not_equate_software_and_hardware() {
        assert!(!WriteProtection::SoftwareReadOnly.claims_hardware());
        assert!(WriteProtection::HardwareWriteBlockerReported.claims_hardware());
    }

    #[test]
    fn job_state_terminal_detection() {
        assert!(JobState::Done.is_terminal());
        assert!(JobState::Cancelled.is_terminal());
        assert!(!JobState::Queued.is_terminal());
        assert!(!JobState::Running { progress: 0.5 }.is_terminal());
    }
}
