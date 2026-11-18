//! Alan modeli: vaka, görevlendirme, materyal, ses, transkript, soru, rapor.

use crate::ids::*;
use crate::status::*;
use crate::{APP_VERSION, WHISPER_MODEL_NAME};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Bir görevlendirme belgesinden çıkarılan bilgiler (şartname 5).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Assignment {
    /// Görevlendiren merci.
    pub authority: Option<String>,
    /// Mahkeme/birim adı.
    pub court: Option<String>,
    /// Dosya numarası.
    pub case_number: Option<String>,
    /// Görevlendirme belgesinden çıkarılan bilirkişi bilgileri (varsa).
    pub expert: ExpertInfo,
    /// Taraf bilgileri.
    pub parties: Vec<Party>,
    /// Görevlendirme tarihi (serbest metin + ayrıştırılmış tarih).
    pub assignment_date_text: Option<String>,
    pub assignment_date: Option<DateTime<Utc>>,
    /// Rapor teslim süresi.
    pub due_date_text: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
    /// İnceleme konusu.
    pub subject: Option<String>,
    /// Bilirkişiden istenen işlemler.
    pub requested_actions: Vec<String>,
    /// Bilirkişiye yöneltilen sorular (orijinal sırayla).
    pub questions: Vec<Question>,
    /// İncelenecek materyaller.
    pub materials_declared: Vec<String>,
    /// Özel talimatlar.
    pub special_instructions: Vec<String>,
    /// Kaynak belge yolu.
    pub source_document: Option<String>,
    /// Belge okuma yöntemi (metin katmanı / OCR).
    pub read_method: DocumentReadMethod,
    /// Alan-bazlı kaynak referansları (şartname 5.2).
    pub field_sources: Vec<FieldSource>,
}

impl Assignment {
    /// Rapor için zorunlu eksik alanların listesi.
    pub fn missing_required_fields(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.court.is_none() && self.authority.is_none() {
            v.push("Görevlendiren merci / mahkeme");
        }
        if self.case_number.is_none() {
            v.push("Dosya numarası");
        }
        if self.questions.is_empty() {
            v.push("Bilirkişiye yöneltilen sorular");
        }
        v
    }
}

/// Taraf bilgisi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Party {
    pub role: String,
    pub name: String,
    /// Kaynak belgedeki orijinal metin.
    pub raw_text: Option<String>,
}

/// Görevlendirme sorusu (şartname 5.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Question {
    pub id: QuestionId,
    /// Orijinal belge sıra numarası.
    pub ordinal: u32,
    /// Soru metninin orijinal hâli; anlam değiştirecek şekilde yazılmaz.
    pub original_text: String,
    /// İlgili materyal kimlikleri.
    pub material_ids: Vec<MaterialId>,
    /// İlgili ses dosyası kimlikleri.
    pub audio_asset_ids: Vec<AudioAssetId>,
    /// İlgili transkript bölümü referansları.
    pub evidence_references: Vec<EvidenceReference>,
    /// Bilirkişi yanıtı (mesleki kanaat; bilirkişi yazar/onaylar).
    pub expert_answer: Option<String>,
    /// Yanıtın bilirkişi tarafından onaylanıp onaylanmadığı.
    pub answer_approved: bool,
}

/// Bir soruya bağlanan dayanak (şartname 11.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceReference {
    pub audio_asset_id: AudioAssetId,
    pub transcript_id: TranscriptId,
    /// Transkript segmenti sırası.
    pub segment_index: usize,
    /// Zaman kodu aralığı (saniye).
    pub start_seconds: f64,
    pub end_seconds: f64,
    /// Dayanak metni (transkript parçası).
    pub quote: String,
    /// Bağlantı bilirkişi tarafından mı kuruldu, yoksa öneri mi?
    pub confirmed_by_expert: bool,
}

/// Belge okuma yöntemi.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentReadMethod {
    /// PDF metin katmanı.
    PdfTextLayer,
    /// Yerel OCR.
    Ocr,
    /// UDF içeriği.
    Udf,
    /// DOCX/DOC çözümlemesi.
    Office,
    /// Düz metin.
    PlainText,
    #[default]
    Unknown,
}

impl DocumentReadMethod {
    pub fn label_tr(&self) -> &'static str {
        match self {
            DocumentReadMethod::PdfTextLayer => "PDF metin katmanı",
            DocumentReadMethod::Ocr => "Yerel OCR",
            DocumentReadMethod::Udf => "UDF içeriği",
            DocumentReadMethod::Office => "Office belgesi çözümlemesi",
            DocumentReadMethod::PlainText => "Düz metin",
            DocumentReadMethod::Unknown => "Bilinmiyor",
        }
    }
}

/// Çıkarılan bir alanın kaynak belgeyle ilişkisi (şartname 5.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldSource {
    /// Alan adı (ör. "case_number").
    pub field: String,
    /// Kaynak belgedeki sayfa numarası (1 tabanlı; bilinmiyorsa 0).
    pub page: u32,
    /// Kaynak metin alıntısı.
    pub excerpt: String,
}

/// Bir delil kaynağı (aygıt veya klasör) tanımı (şartname 6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceSource {
    pub id: DeviceId,
    pub kind: DeviceKind,
    /// Sürücü harfi veya kök yol.
    pub root: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub capacity_bytes: Option<u64>,
    pub serial_number: Option<String>,
    pub filesystem: Option<String>,
    pub disk_info: Option<String>,
    pub removable: bool,
    /// Yazma koruması durumu.
    pub write_protection: WriteProtection,
    /// Aygıtın sistem diski olup olmadığı (asla otomatik seçilmez).
    pub is_system_disk: bool,
    pub detected_at: DateTime<Utc>,
}

/// İçeri alınan tek bir dosyanın kaydı (şartname 6.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Material {
    pub id: MaterialId,
    pub source_device_id: DeviceId,
    /// Kaynak yol (kaynak aygıt üzerinde).
    pub source_path: String,
    pub file_name: String,
    pub file_type: String,
    pub size_bytes: u64,
    /// Kaynak SHA-256.
    pub source_sha256: Option<String>,
    /// Hedef (kopya) SHA-256.
    pub target_sha256: Option<String>,
    /// Yerel vaka deposundaki göreli yol.
    pub stored_relative_path: String,
    pub acquired_at: DateTime<Utc>,
    pub verification: VerificationStatus,
    /// Aynı hash'e sahip yinelenen dosya ise orijinal materyal kimliği.
    pub duplicate_of: Option<MaterialId>,
    /// Kopyalama/doğrulama sırasında oluşan uyarılar.
    pub warnings: Vec<String>,
}

/// Ses teknik envanteri (şartname 7.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioTechnicalInfo {
    pub container: String,
    pub codec: String,
    pub duration_seconds: f64,
    pub sample_rate_hz: Option<u32>,
    pub bit_depth: Option<u16>,
    pub bit_rate_bps: Option<u64>,
    pub channels: Option<u16>,
    pub channel_layout: Option<String>,
    pub metadata_tags: Vec<(String, String)>,
    pub readable: bool,
    pub warnings: Vec<String>,
    /// Kanal işleme parametreleri (karışım/ayırma yapıldıysa).
    pub channel_processing: Option<String>,
}

impl Default for AudioTechnicalInfo {
    fn default() -> Self {
        Self {
            container: String::new(),
            codec: String::new(),
            duration_seconds: 0.0,
            sample_rate_hz: None,
            bit_depth: None,
            bit_rate_bps: None,
            channels: None,
            channel_layout: None,
            metadata_tags: Vec::new(),
            readable: false,
            warnings: Vec::new(),
            channel_processing: None,
        }
    }
}

/// Bir ses varlığı: materyal + teknik envanter (şartname 7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioAsset {
    pub id: AudioAssetId,
    pub material_id: MaterialId,
    pub file_name: String,
    pub sha256: Option<String>,
    /// Ses akışı video kapsayıcıdan mı çıkarıldı?
    pub extracted_from_video: bool,
    pub technical: AudioTechnicalInfo,
    /// Bu ses için transkripsiyon kuyruğuna alındı mı?
    pub queued: bool,
    /// Bu ses desteklenmiyorsa nedeni.
    pub unsupported_reason: Option<String>,
}

/// Bir transkript segmenti (cümle/segment düzeyi zaman kodu).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptSegment {
    /// Segment sırası.
    pub index: usize,
    pub start_seconds: f64,
    pub end_seconds: f64,
    /// Birebir transkript metni (düzenlenmiş sürümde düzeltilmiş metin).
    pub text: String,
    /// Makine çıktısı orijinal metni; hiçbir zaman kaybedilmez.
    pub machine_text: String,
    /// Segment için kalite bayrağı.
    pub quality: QualityFlag,
    /// Segment için tespit edilen kalite sorunları.
    pub issues: Vec<QualityIssueKind>,
    /// Anlaşılamayan bölüm işareti.
    pub marked_unintelligible: bool,
    /// Konuşmacı etiketi (ör. "Konuşmacı 1"); gerçek kimlik değildir.
    pub speaker: Option<String>,
    /// Bilirkişi notu.
    pub note: Option<String>,
    /// Kelime düzeyinde zaman kodu var mı? (yoksa gösterilmez)
    pub word_level_timestamps: bool,
}

/// Transkript sürümü: her işlem yeni bir sürüm üretir (şartname 8.4, 10.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptVersion {
    pub version: u32,
    pub created_at: DateTime<Utc>,
    /// Bu sürümü oluşturan kullanıcı ("sistem" veya kullanıcı adı).
    pub created_by: String,
    pub status: TranscriptStatus,
    pub segments: Vec<TranscriptSegment>,
    /// Bu sürümün dayandığı önceki sürüm numarası.
    pub based_on: Option<u32>,
    /// Değişiklik açıklaması.
    pub change_note: Option<String>,
}

/// Bir ses varlığına karşılık gelen transkript ve tüm sürümleri.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    pub id: TranscriptId,
    pub audio_asset_id: AudioAssetId,
    /// Algılanan dil (ISO 639-1) ve kullanıcı düzeltmesi.
    pub detected_language: Option<String>,
    pub language_user_override: Option<String>,
    /// Kullanılan model ve sürümü.
    pub model_name: String,
    pub model_version: String,
    /// Kullanım durumu: birebir (verbatim) veya düzenlenmiş.
    pub verbatim: bool,
    /// Sürümler; ilk eleman makine çıktısıdır.
    pub versions: Vec<TranscriptVersion>,
    /// Geçerli (görüntülenen) sürüm numarası.
    pub current_version: u32,
    /// Cümle/segment düzeyi zaman kodu kullanılıyor mu?
    pub segment_level_timestamps: bool,
}

impl Transcript {
    /// Yeni bir makine transkripti oluşturur.
    pub fn new_machine(
        audio_asset_id: AudioAssetId,
        model_version: impl Into<String>,
        segments: Vec<TranscriptSegment>,
        detected_language: Option<String>,
    ) -> Self {
        let v = TranscriptVersion {
            version: 1,
            created_at: Utc::now(),
            created_by: "sistem".to_string(),
            status: TranscriptStatus::MachineGenerated,
            segments,
            based_on: None,
            change_note: Some("Yerel Whisper motoru ilk çıktısı".to_string()),
        };
        Self {
            id: TranscriptId::new(),
            audio_asset_id,
            detected_language,
            language_user_override: None,
            model_name: WHISPER_MODEL_NAME.to_string(),
            model_version: model_version.into(),
            verbatim: true,
            versions: vec![v],
            current_version: 1,
            segment_level_timestamps: true,
        }
    }

    /// Geçerli sürümü döndürür.
    pub fn current(&self) -> &TranscriptVersion {
        self.versions
            .iter()
            .find(|v| v.version == self.current_version)
            .unwrap_or_else(|| self.versions.last().expect("en az bir sürüm"))
    }

    /// Yeni bir düzenlenmiş sürüm ekler; ilk makine çıktısı korunur.
    pub fn add_version(
        &mut self,
        segments: Vec<TranscriptSegment>,
        created_by: impl Into<String>,
        status: TranscriptStatus,
        change_note: Option<String>,
    ) -> u32 {
        let next = self.versions.iter().map(|v| v.version).max().unwrap_or(0) + 1;
        self.versions.push(TranscriptVersion {
            version: next,
            created_at: Utc::now(),
            created_by: created_by.into(),
            status,
            segments,
            based_on: Some(self.current_version),
            change_note,
        });
        self.current_version = next;
        next
    }

    /// Makine çıktısı (ilk sürüm) her zaman korunur mu?
    pub fn preserves_machine_output(&self) -> bool {
        self.versions
            .first()
            .map(|v| v.status == TranscriptStatus::MachineGenerated)
            .unwrap_or(false)
    }
}

/// Vaka işlem kaydı girdisi (şartname 15.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub at: DateTime<Utc>,
    pub actor: String,
    pub action: String,
    pub detail: String,
}

impl AuditEntry {
    pub fn new(
        actor: impl Into<String>,
        action: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            at: Utc::now(),
            actor: actor.into(),
            action: action.into(),
            detail: detail.into(),
        }
    }
}

/// Rapor için kullanılan yazılım/model bilgisi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoftwareInfo {
    pub app_name: String,
    pub app_version: String,
    pub model_name: String,
    pub model_version: String,
    pub os: String,
}

impl Default for SoftwareInfo {
    fn default() -> Self {
        Self {
            app_name: crate::APP_NAME.to_string(),
            app_version: APP_VERSION.to_string(),
            model_name: WHISPER_MODEL_NAME.to_string(),
            model_version: "sabitlenmiş paket".to_string(),
            os: std::env::consts::OS.to_string(),
        }
    }
}

/// Kullanıcı kararı gerektiren istisna kaydı (şartname 4.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingException {
    pub kind: ExceptionKind,
    /// Sade bir dille açıklama (teknik hata kodu değil).
    pub description: String,
    /// İlgili materyal/ses (varsa).
    pub related: Option<String>,
    /// Kullanıcı kararı (varsa).
    pub decision: Option<String>,
    pub resolved: bool,
}

/// Vaka: tüm inceleme verisini bir arada tutar (şartname 15.1, 16).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Case {
    pub id: CaseId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub assignment: Assignment,
    pub sources: Vec<EvidenceSource>,
    pub materials: Vec<Material>,
    pub audio_assets: Vec<AudioAsset>,
    pub transcripts: Vec<Transcript>,
    pub report: Option<crate::model::Report>,
    pub audit: Vec<AuditEntry>,
    pub exceptions: Vec<PendingException>,
    pub software: SoftwareInfo,
    /// Vaka deposunun kök yolu (mutlak).
    pub case_root: String,
}

impl Case {
    pub fn new(case_root: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: CaseId::new(),
            created_at: now,
            updated_at: now,
            assignment: Assignment::default(),
            sources: Vec::new(),
            materials: Vec::new(),
            audio_assets: Vec::new(),
            transcripts: Vec::new(),
            report: None,
            audit: Vec::new(),
            exceptions: Vec::new(),
            software: SoftwareInfo::default(),
            case_root: case_root.into(),
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    pub fn add_audit(
        &mut self,
        actor: impl Into<String>,
        action: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.audit.push(AuditEntry::new(actor, action, detail));
        self.touch();
    }

    /// Doğrulanmış materyal sayısı.
    pub fn verified_material_count(&self) -> usize {
        self.materials
            .iter()
            .filter(|m| m.verification.is_verified())
            .count()
    }

    /// Transkripti olan ses varlıkları.
    pub fn transcribed_audio_ids(&self) -> Vec<AudioAssetId> {
        self.transcripts
            .iter()
            .map(|t| t.audio_asset_id.clone())
            .collect()
    }
}

/// Rapor modeli (şartname 11.2 — 24 ana bölüm).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub id: ReportId,
    pub created_at: DateTime<Utc>,
    pub report_date: DateTime<Utc>,
    // 1. Kapak
    pub cover_title: String,
    pub cover_subtitle: Option<String>,
    // 2. Görevlendiren merci
    pub authority: Option<String>,
    // 3. Dosya numarası
    pub case_number: Option<String>,
    // 4. Taraf bilgileri
    pub parties: Vec<Party>,
    // 5. Bilirkişi bilgileri
    pub expert: ExpertInfo,
    // 6. Görevlendirme tarihi ve süresi
    pub assignment_date_text: Option<String>,
    pub due_date_text: Option<String>,
    // 7. İnceleme konusu
    pub subject: Option<String>,
    // 8. Bilirkişiye yöneltilen sorular
    pub questions: Vec<Question>,
    // 9. İncelenen materyaller
    pub materials: Vec<Material>,
    // 10. Materyal teslim/edinim açıklaması
    pub acquisition_note: String,
    // 11. İnceleme yöntemi
    pub method: String,
    // 12. Kullanılan yazılım ve model bilgisi
    pub software: SoftwareInfo,
    // 13. Transkripsiyon yöntemi
    pub transcription_method: String,
    // 14. Teknik ses envanteri
    pub audio_inventory: Vec<AudioAsset>,
    // 15. Transkript içeriği (onaylanmış)
    pub transcripts: Vec<Transcript>,
    // 16. Zaman kodları
    pub include_timestamps: bool,
    // 17. Belirsiz/anlaşılamayan bölümler
    pub unintelligible_summary: Vec<String>,
    // 18-19. Soru yanıtları ve dayanakları `questions` içinde
    // 20. Yöntem sınırlılıkları
    pub limitations: Vec<String>,
    // 21. Gerekçeli sonuç
    pub reasoned_conclusion: Option<String>,
    // 22. Rapor tarihi (report_date)
    // 23. İmza
    pub signature_block: SignatureBlock,
    // 24. Ekler
    pub attachments: Vec<Attachment>,
    /// Zorunlu alan kontrolü sonucu eksikler.
    pub missing_fields: Vec<String>,
}

/// Bilirkişi bilgileri.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExpertInfo {
    pub full_name: Option<String>,
    pub title: Option<String>,
    pub registry_number: Option<String>,
}

impl ExpertInfo {
    pub fn is_complete(&self) -> bool {
        self.full_name.is_some() && self.title.is_some() && self.registry_number.is_some()
    }
}

/// İmza bloğu. Gerçek e-imza üretilmez; yalnızca imza alanı tanımlanır.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignatureBlock {
    pub expert_line: String,
    pub note: String,
}

impl Default for SignatureBlock {
    fn default() -> Self {
        Self {
            expert_line: "Bilirkişi\nAdı Soyadı / Unvan / Sicil No\nİmza".to_string(),
            note: "Bu raporun resmî imzası UYAP Editör / e-imza altyapısı ile yapılır.".to_string(),
        }
    }
}

/// Rapor eki (şartname 11.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    pub id: AttachmentId,
    /// Ek numarası (1 tabanlı).
    pub number: u32,
    pub title: String,
    pub kind: AttachmentKind,
    /// İlgili ses varlığı (transkript ekleri için).
    pub audio_asset_id: Option<AudioAssetId>,
    pub audio_file_name: Option<String>,
    pub material_number: Option<u32>,
    pub sha256: Option<String>,
    pub duration_seconds: Option<f64>,
    pub transcript_id: Option<TranscriptId>,
    /// Bilirkişi inceleme durumu.
    pub review_status: TranscriptStatus,
}

/// Ek türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Transcript,
    AudioInventory,
    HashManifest,
    Other,
}

impl AttachmentKind {
    pub fn label_tr(&self) -> &'static str {
        match self {
            AttachmentKind::Transcript => "Transkript",
            AttachmentKind::AudioInventory => "Ses teknik envanteri",
            AttachmentKind::HashManifest => "Hash manifestosu",
            AttachmentKind::Other => "Diğer",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(i: usize, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            index: i,
            start_seconds: i as f64,
            end_seconds: i as f64 + 1.0,
            text: text.to_string(),
            machine_text: text.to_string(),
            quality: QualityFlag::Normal,
            issues: vec![],
            marked_unintelligible: false,
            speaker: None,
            note: None,
            word_level_timestamps: false,
        }
    }

    #[test]
    fn transcript_preserves_machine_output_after_edit() {
        let mut t = Transcript::new_machine(
            AudioAssetId::new(),
            "test",
            vec![seg(0, "merhaba")],
            Some("tr".into()),
        );
        let machine = t.versions[0].segments[0].machine_text.clone();
        let mut edited = t.current().segments.clone();
        edited[0].text = "Merhaba.".to_string();
        t.add_version(edited, "bilirkişi", TranscriptStatus::ExpertEdited, None);
        assert!(t.preserves_machine_output());
        assert_eq!(t.versions[0].segments[0].machine_text, machine);
        assert_eq!(t.current_version, 2);
        assert_eq!(t.current().segments[0].text, "Merhaba.");
    }

    #[test]
    fn assignment_missing_fields_detects_gaps() {
        let a = Assignment::default();
        let missing = a.missing_required_fields();
        assert!(missing.contains(&"Dosya numarası"));
        assert!(missing.contains(&"Bilirkişiye yöneltilen sorular"));
    }

    #[test]
    fn case_tracks_verified_materials() {
        let mut c = Case::new("/tmp/case");
        assert_eq!(c.verified_material_count(), 0);
        c.materials.push(Material {
            id: MaterialId::new(),
            source_device_id: DeviceId::new(),
            source_path: "x".into(),
            file_name: "x".into(),
            file_type: "wav".into(),
            size_bytes: 1,
            source_sha256: Some("a".into()),
            target_sha256: Some("a".into()),
            stored_relative_path: "x".into(),
            acquired_at: Utc::now(),
            verification: VerificationStatus::Verified,
            duplicate_of: None,
            warnings: vec![],
        });
        assert_eq!(c.verified_material_count(), 1);
    }

    #[test]
    fn expert_info_completeness() {
        let mut e = ExpertInfo::default();
        assert!(!e.is_complete());
        e.full_name = Some("A".into());
        e.title = Some("B".into());
        e.registry_number = Some("C".into());
        assert!(e.is_complete());
    }
}
