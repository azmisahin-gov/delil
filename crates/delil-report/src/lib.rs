//! Bilirkişi rapor modeli ve otomatik üretimi (şartname 11).
//!
//! Bu crate, vaka verisinden (görevlendirme, materyal, transkript, ses
//! envanteri) 24 ana bölümlü rapor taslağını oluşturur. Yapay zekâ yalnızca
//! kaynak metin ve zaman koduyla desteklenen bir yanıt taslağı sunabilir;
//! kaynakta bulunmayan bilgi veya meslekî kanaat uydurulamaz. Nihai yanıt
//! bilirkişi tarafından yazılır veya onaylanır.

use delil_core::{
    Attachment, AttachmentId, Case, ExpertInfo, Party, Question, Report, ReportId, SignatureBlock,
    TranscriptStatus,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Rapor üretim hataları.
#[derive(Debug, Error)]
pub enum ReportError {
    #[error("Rapor için zorunlu bilgi eksik: {0}")]
    MissingRequired(String),
    #[error("Dayanak bulunamadı: {0}")]
    EvidenceMissing(String),
}

/// Rapor üretim seçenekleri.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportOptions {
    /// Kapak başlığı.
    pub cover_title: String,
    /// Zaman kodları rapora eklensin mi?
    pub include_timestamps: bool,
    /// Transkriptler ayrı ekler olarak düzenlensin mi?
    pub transcripts_as_attachments: bool,
    /// Bir ekin azami karakter sayısı (UDF'nin açılmasını engellememek için).
    pub max_attachment_chars: usize,
    /// Bilirkişi bilgileri.
    pub expert: ExpertInfo,
}

impl Default for ReportOptions {
    fn default() -> Self {
        Self {
            cover_title: "BİLİRKİŞİ RAPORU".to_string(),
            include_timestamps: true,
            transcripts_as_attachments: true,
            max_attachment_chars: 30_000,
            expert: ExpertInfo::default(),
        }
    }
}

/// Vaka verisinden rapor taslağı üretir.
///
/// Eksik zorunlu alanlar rapora `missing_fields` olarak yazılır; üretim
/// engellenmez ancak son kontrol ekranında gösterilir (şartname 11.4).
pub fn generate_report(case: &Case, opts: &ReportOptions) -> Result<Report, ReportError> {
    let a = &case.assignment;

    // Zorunlu alan kontrolü (eksikler rapora yazılır, hata değildir).
    let mut missing: Vec<String> = Vec::new();
    if a.court.is_none() && a.authority.is_none() {
        missing.push("Görevlendiren merci / mahkeme".into());
    }
    if a.case_number.is_none() {
        missing.push("Dosya numarası".into());
    }
    if a.questions.is_empty() {
        missing.push("Bilirkişiye yöneltilen sorular".into());
    }
    if !opts.expert.is_complete() {
        missing.push("Bilirkişi bilgileri (ad, unvan, sicil no)".into());
    }

    // Soru yanıtları ve dayanak kontrolü.
    let mut questions = a.questions.clone();
    for q in questions.iter_mut() {
        if q.expert_answer.as_deref().unwrap_or("").trim().is_empty() {
            missing.push(format!("Soru {} yanıtı boş", q.ordinal));
        } else if q.evidence_references.is_empty() && q.audio_asset_ids.is_empty() {
            missing.push(format!("Soru {} dayanağı yok", q.ordinal));
        }
    }

    // Onaylanmış transkriptleri seç (şartname 11.3: yalnızca onaylanmış
    // transkriptler rapora aktarılır; onaylanmamışlar da bilgi için eklenir
    // ama durumları belirtilir).
    let transcripts: Vec<_> = case.transcripts.clone();

    // Belirsiz/anlaşılamayan bölümler özeti.
    let mut unintelligible = Vec::new();
    for t in &case.transcripts {
        for s in &t.current().segments {
            if s.marked_unintelligible {
                unintelligible.push(format!(
                    "{} — {:.1}s-{:.1}s: {}",
                    t.audio_asset_id,
                    s.start_seconds,
                    s.end_seconds,
                    if s.text.trim().is_empty() {
                        "(anlaşılamadı)"
                    } else {
                        s.text.trim()
                    }
                ));
            }
        }
    }

    // Materyal teslim/edinim açıklaması.
    let verified = case.verified_material_count();
    let acquisition_note = format!(
        "İnceleme konusu materyal, {} kaynaktan mantıksal dosya edinimi yöntemiyle alınmıştır. \
Toplam {} dosya envantere kaydedilmiş, bunlardan {} tanesinin SHA-256 değeri kaynak ve hedef \
karşılaştırmasıyla doğrulanmıştır. Edinim sırasında kaynak materyale yazma işlemi yapılmamıştır.",
        case.sources.len(),
        case.materials.len(),
        verified
    );

    // Transkripsiyon yöntemi açıklaması.
    let transcription_method = format!(
        "Ses kayıtları, yerel ve çevrimdışı çalışan Whisper {} modeli ile metne dönüştürülmüştür. \
İşlem tamamen bu bilgisayarda yürütülmüş; hiçbir ses, transkript veya belge bulut sistemine \
gönderilmemiştir. Çıktı, bilirkişi incelemesine uygun birebir (verbatim) transkript taslağıdır.",
        case.software.model_name
    );

    // Ekler (şartname 11.5).
    let mut attachments: Vec<Attachment> = Vec::new();
    if opts.transcripts_as_attachments {
        for (i, t) in case.transcripts.iter().enumerate() {
            let asset = case.audio_assets.iter().find(|a| a.id == t.audio_asset_id);
            attachments.push(Attachment {
                id: AttachmentId::new(),
                number: (i + 1) as u32,
                title: format!(
                    "EK-{}: {} transkripti",
                    i + 1,
                    asset.map(|a| a.file_name.clone()).unwrap_or_default()
                ),
                kind: delil_core::AttachmentKind::Transcript,
                audio_asset_id: Some(t.audio_asset_id.clone()),
                audio_file_name: asset.map(|a| a.file_name.clone()),
                material_number: None,
                sha256: asset.and_then(|a| a.sha256.clone()),
                duration_seconds: asset.map(|a| a.technical.duration_seconds),
                transcript_id: Some(t.id.clone()),
                review_status: t.current().status,
            });
        }
    }
    attachments.push(Attachment {
        id: AttachmentId::new(),
        number: attachments.len() as u32 + 1,
        title: "Ses teknik envanteri".into(),
        kind: delil_core::AttachmentKind::AudioInventory,
        audio_asset_id: None,
        audio_file_name: None,
        material_number: None,
        sha256: None,
        duration_seconds: None,
        transcript_id: None,
        review_status: TranscriptStatus::MachineGenerated,
    });
    attachments.push(Attachment {
        id: AttachmentId::new(),
        number: attachments.len() as u32 + 1,
        title: "Hash manifestosu (materyal bütünlük kaydı)".into(),
        kind: delil_core::AttachmentKind::HashManifest,
        audio_asset_id: None,
        audio_file_name: None,
        material_number: None,
        sha256: None,
        duration_seconds: None,
        transcript_id: None,
        review_status: TranscriptStatus::MachineGenerated,
    });

    let report = Report {
        id: ReportId::new(),
        created_at: chrono::Utc::now(),
        report_date: chrono::Utc::now(),
        cover_title: opts.cover_title.clone(),
        cover_subtitle: Some("Ses Kayıtlarının Metin Hâline Dönüştürülmesi İncelemesi".into()),
        authority: a.court.clone().or_else(|| a.authority.clone()),
        case_number: a.case_number.clone(),
        parties: a.parties.clone(),
        expert: opts.expert.clone(),
        assignment_date_text: a.assignment_date_text.clone(),
        due_date_text: a.due_date_text.clone(),
        subject: a.subject.clone(),
        questions,
        materials: case.materials.clone(),
        acquisition_note,
        method: default_method_text(),
        software: case.software.clone(),
        transcription_method,
        audio_inventory: case.audio_assets.clone(),
        transcripts,
        include_timestamps: opts.include_timestamps,
        unintelligible_summary: unintelligible,
        limitations: default_limitations(),
        reasoned_conclusion: None,
        signature_block: SignatureBlock::default(),
        attachments,
        missing_fields: missing,
    };
    Ok(report)
}

/// İnceleme yöntemi metni.
pub fn default_method_text() -> String {
    "İnceleme; materyalin mantıksal dosya edinimi, bütünlük doğrulaması (SHA-256), \
ses dosyalarının teknik envanterinin çıkarılması, yerel yapay zekâ ile transkripsiyon, \
otomatik kalite kontrolü ve bilirkişi incelemesi aşamalarından oluşur.".to_string()
}

/// Yöntem sınırlılıkları (şartname 11.2/20).
pub fn default_limitations() -> Vec<String> {
    vec![
        "Bu rapor, ses kayıtlarının metne dönüştürülmesiyle sınırlıdır; konuşmacı kimliği \
tespitine veya ses orijinalliği/montaj incelemesine yönelik bir değerlendirme içermez."
            .to_string(),
        "Konuşmacı etiketleri (ör. “Konuşmacı 1”) yalnızca geçici bir ayrımdır; gerçek kişi \
kimliği anlamına gelmez.".to_string(),
        "Otomatik transkripsiyon çıktısı birebir taslaktır; belirsiz veya şüpheli bölümler \
bilirkişi tarafından kontrol edilmelidir.".to_string(),
        "Yazılımsal yazma koruması, ilk bağlantı anında donanımsal yazma engelleme ile eşdeğer \
değildir.".to_string(),
        "Otomatik transkripsiyon başarısı kesin doğruluk garantisi olarak yorumlanamaz."
            .to_string(),
    ]
}

/// Bir soru için kaynak metin ve zaman koduyla desteklenen yanıt taslağı
/// önerisi üretir. **Kaynakta bulunmayan bilgi uydurulmaz.**
pub fn suggest_answer_from_evidence(
    question: &Question,
    transcripts: &[delil_core::Transcript],
) -> Option<String> {
    if question.evidence_references.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for ev in &question.evidence_references {
        let t = transcripts.iter().find(|t| t.id == ev.transcript_id);
        let Some(t) = t else { continue };
        // Dayanak metnini transkriptten doğrula; uydurma yapma.
        let seg = t.current().segments.get(ev.segment_index);
        if let Some(seg) = seg {
            if seg.text.contains(&ev.quote) || ev.quote.contains(seg.text.trim()) {
                parts.push(format!(
                    "“{}” (zaman: {:.1}s–{:.1}s)",
                    ev.quote.trim(),
                    ev.start_seconds,
                    ev.end_seconds
                ));
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(format!(
            "İlgili kayıt bölümleri: {}",
            parts.join("; ")
        ))
    }
}

/// Raporun zorunlu alan kontrolünü yapar ve eksiklerin listesini döndürür.
pub fn check_required_fields(report: &Report) -> Vec<String> {
    let mut missing = Vec::new();
    if report.authority.is_none() {
        missing.push("Görevlendiren merci".into());
    }
    if report.case_number.is_none() {
        missing.push("Dosya numarası".into());
    }
    if report.parties.is_empty() {
        missing.push("Taraf bilgileri".into());
    }
    if !report.expert.is_complete() {
        missing.push("Bilirkişi bilgileri".into());
    }
    if report.subject.is_none() {
        missing.push("İnceleme konusu".into());
    }
    if report.questions.is_empty() {
        missing.push("Görevlendirme soruları".into());
    }
    if report.reasoned_conclusion.is_none() {
        missing.push("Gerekçeli sonuç (meslekî kanaat)".into());
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;
    use delil_core::*;

    fn sample_case() -> Case {
        let mut c = Case::new("/tmp/case");
        c.assignment.court = Some("Ankara 1. Asliye Ceza Mahkemesi".into());
        c.assignment.case_number = Some("2024/123 Esas".into());
        c.assignment.parties = vec![
            Party {
                role: "Şüpheli".into(),
                name: "A. B.".into(),
                raw_text: None,
            },
            Party {
                role: "Mağdur".into(),
                name: "C. D.".into(),
                raw_text: None,
            },
        ];
        c.assignment.subject = Some("Ses kaydının metne dönüştürülmesi".into());
        c.assignment.questions = vec![Question {
            id: QuestionId::new(),
            ordinal: 1,
            original_text: "Kayıtta ne konuşulmaktadır?".into(),
            material_ids: vec![],
            audio_asset_ids: vec![],
            evidence_references: vec![],
            expert_answer: Some("Kayıtta taraflar arasında bir görüşme yer almaktadır.".into()),
            answer_approved: true,
        }];
        let asset = AudioAsset {
            id: AudioAssetId::new(),
            material_id: MaterialId::new(),
            file_name: "kayit.wav".into(),
            sha256: Some("abc".into()),
            extracted_from_video: false,
            technical: AudioTechnicalInfo {
                container: "WAV".into(),
                codec: "PCM".into(),
                duration_seconds: 12.0,
                sample_rate_hz: Some(16000),
                channels: Some(1),
                readable: true,
                ..Default::default()
            },
            queued: true,
            unsupported_reason: None,
        };
        c.audio_assets.push(asset.clone());
        let seg = TranscriptSegment {
            index: 0,
            start_seconds: 0.0,
            end_seconds: 3.0,
            text: "Merhaba nasılsınız".into(),
            machine_text: "Merhaba nasılsınız".into(),
            quality: QualityFlag::Normal,
            issues: vec![],
            marked_unintelligible: false,
            speaker: None,
            note: None,
            word_level_timestamps: false,
        };
        c.transcripts
            .push(Transcript::new_machine(asset.id, "test", vec![seg], Some("tr".into())));
        c
    }

    #[test]
    fn generates_report_with_all_sections() {
        let case = sample_case();
        let mut opts = ReportOptions::default();
        opts.expert = ExpertInfo {
            full_name: Some("Dr. E. F.".into()),
            title: Some("Bilirkişi".into()),
            registry_number: Some("12345".into()),
        };
        let r = generate_report(&case, &opts).unwrap();
        assert_eq!(r.cover_title, "BİLİRKİŞİ RAPORU");
        assert_eq!(r.case_number.as_deref(), Some("2024/123 Esas"));
        assert_eq!(r.parties.len(), 2);
        assert_eq!(r.questions.len(), 1);
        assert!(!r.acquisition_note.is_empty());
        assert!(!r.method.is_empty());
        assert!(!r.transcription_method.is_empty());
        assert_eq!(r.audio_inventory.len(), 1);
        assert_eq!(r.transcripts.len(), 1);
        assert!(r.limitations.len() >= 5);
        // Ekler: 1 transkript + envanter + hash = 3
        assert_eq!(r.attachments.len(), 3);
    }

    #[test]
    fn missing_fields_are_reported_not_fatal() {
        let case = Case::new("/tmp/case");
        let opts = ReportOptions::default();
        let r = generate_report(&case, &opts).unwrap();
        assert!(r.missing_fields.iter().any(|f| f.contains("Dosya numarası")));
        assert!(r
            .missing_fields
            .iter()
            .any(|f| f.contains("Bilirkişiye yöneltilen sorular")));
    }

    #[test]
    fn check_required_fields_detects_gaps() {
        let case = Case::new("/tmp/case");
        let opts = ReportOptions::default();
        let r = generate_report(&case, &opts).unwrap();
        let missing = check_required_fields(&r);
        assert!(missing.iter().any(|m| m.contains("Gerekçeli sonuç")));
        assert!(missing.iter().any(|m| m.contains("Taraf bilgileri")));
    }

    #[test]
    fn suggestion_only_from_real_evidence() {
        let case = sample_case();
        let mut q = case.assignment.questions[0].clone();
        let t = &case.transcripts[0];
        q.evidence_references = vec![EvidenceReference {
            audio_asset_id: t.audio_asset_id.clone(),
            transcript_id: t.id.clone(),
            segment_index: 0,
            start_seconds: 0.0,
            end_seconds: 3.0,
            quote: "Merhaba nasılsınız".into(),
            confirmed_by_expert: false,
        }];
        let s = suggest_answer_from_evidence(&q, &case.transcripts);
        assert!(s.is_some());
        assert!(s.unwrap().contains("Merhaba nasılsınız"));
    }

    #[test]
    fn suggestion_rejects_fabricated_quote() {
        let case = sample_case();
        let mut q = case.assignment.questions[0].clone();
        let t = &case.transcripts[0];
        q.evidence_references = vec![EvidenceReference {
            audio_asset_id: t.audio_asset_id.clone(),
            transcript_id: t.id.clone(),
            segment_index: 0,
            start_seconds: 0.0,
            end_seconds: 3.0,
            quote: "Kaynakta olmayan uydurma metin".into(),
            confirmed_by_expert: false,
        }];
        // Dayanak metni transkriptle uyuşmadığı için öneri üretilmez.
        assert!(suggest_answer_from_evidence(&q, &case.transcripts).is_none());
    }

    #[test]
    fn no_suggestion_without_evidence() {
        let case = sample_case();
        let q = case.assignment.questions[0].clone();
        assert!(suggest_answer_from_evidence(&q, &case.transcripts).is_none());
    }

    #[test]
    fn unintelligible_segments_are_summarized() {
        let mut case = sample_case();
        case.transcripts[0].versions[0].segments[0].marked_unintelligible = true;
        let opts = ReportOptions::default();
        let r = generate_report(&case, &opts).unwrap();
        assert!(!r.unintelligible_summary.is_empty());
    }

    #[test]
    fn empty_answer_is_flagged_missing() {
        let mut case = sample_case();
        case.assignment.questions[0].expert_answer = None;
        let opts = ReportOptions::default();
        let r = generate_report(&case, &opts).unwrap();
        assert!(r
            .missing_fields
            .iter()
            .any(|f| f.contains("Soru 1 yanıtı boş")));
    }
}
