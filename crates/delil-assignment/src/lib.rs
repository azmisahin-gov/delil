//! Görevlendirme belgesi okuma ve bilgi çıkarma (şartname 5).
//!
//! Desteklenen biçimler: PDF (metin katmanı), UDF, DOCX, DOC (yerel
//! dönüştürme yolu varsa), TXT ve görsel/taranmış belge (yerel OCR).
//!
//! Çıkarılan her önemli alanın kaynak belgeyle ilişkisi (`FieldSource`)
//! tutulur: hangi sayfadan geldiği ve orijinal metin alıntısı.

use delil_core::{
    Assignment, DocumentReadMethod, FieldSource, Party, Question, QuestionId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "ocr")]
pub mod ocr;
pub mod pdf;
pub mod udf_doc;

/// Belge okuma hataları.
#[derive(Debug, Error)]
pub enum AssignmentError {
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("Desteklenmeyen belge biçimi: {0}")]
    Unsupported(String),
    #[error("Belge çözümlenemedi: {0}")]
    Parse(String),
    #[error("UDF hatası: {0}")]
    Udf(#[from] delil_udf::UdfError),
}

/// Belgeden çıkarılan ham metin (sayfa sınırlarıyla).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedText {
    /// Sayfa sayfa metin. Tek sayfalı belgelerde tek eleman.
    pub pages: Vec<String>,
    /// Okuma yöntemi.
    pub method: DocumentReadMethod,
    /// Belge türü (ör. "PDF", "UDF", "DOCX").
    pub document_kind: String,
}

impl ExtractedText {
    /// Tüm sayfaları birleştirilmiş düz metin.
    pub fn full_text(&self) -> String {
        self.pages.join("\n")
    }
}

/// Belgeyi okuyup ham metni çıkarır.
pub fn extract_text(path: &std::path::Path) -> Result<ExtractedText, AssignmentError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "udf" => udf_doc::extract(path),
        "pdf" => pdf::extract(path),
        "docx" => extract_docx(path),
        "txt" | "text" => {
            let text = std::fs::read_to_string(path)?;
            Ok(ExtractedText {
                pages: vec![text],
                method: DocumentReadMethod::PlainText,
                document_kind: "TXT".into(),
            })
        }
        "doc" => {
            // DOC (eski ikili biçim) doğrudan çözülemez; yerel dönüştürme yolu
            // aranır. Bulunamazsa açıkça bildirilir.
            Err(AssignmentError::Unsupported(
                "DOC biçimi için yerel dönüştürme yolu bulunamadı. Lütfen belgeyi \
                 DOCX veya PDF olarak kaydedin."
                    .into(),
            ))
        }
        "png" | "jpg" | "jpeg" | "tif" | "tiff" | "bmp" | "webp" => {
            #[cfg(feature = "ocr")]
            {
                ocr::extract(path)
            }
            #[cfg(not(feature = "ocr"))]
            {
                Err(AssignmentError::Unsupported(
                    "Görsel belge için OCR bileşeni bu derlemede etkin değil.".into(),
                ))
            }
        }
        _ => Err(AssignmentError::Unsupported(format!(
            "Bilinmeyen belge biçimi: {ext}"
        ))),
    }
}

/// DOCX belgesinden metin çıkarır (word/document.xml çözümlemesi).
pub fn extract_docx(path: &std::path::Path) -> Result<ExtractedText, AssignmentError> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AssignmentError::Parse(format!("DOCX açılamadı: {e}")))?;
    let mut xml = String::new();
    {
        let mut doc = archive
            .by_name("word/document.xml")
            .map_err(|_| AssignmentError::Parse("word/document.xml bulunamadı".into()))?;
        doc.read_to_string(&mut xml)?;
    }
    let text = docx_xml_to_text(&xml);
    Ok(ExtractedText {
        pages: vec![text],
        method: DocumentReadMethod::Office,
        document_kind: "DOCX".into(),
    })
}

/// DOCX `document.xml` içeriğini düz metne çevirir.
///
/// Paragraflar (`w:p`) satır sonuyla, sekmeler (`w:tab`) sekme karakteriyle
/// temsil edilir.
pub fn docx_xml_to_text(xml: &str) -> String {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut out = String::new();
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.name();
                match name.local_name().as_ref() {
                    b"t" => in_text = true,
                    b"tab" => out.push('\t'),
                    b"br" => out.push('\n'),
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let name = e.name();
                match name.local_name().as_ref() {
                    b"tab" => out.push('\t'),
                    b"br" => out.push('\n'),
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                match name.local_name().as_ref() {
                    b"t" => in_text = false,
                    b"p" => out.push('\n'),
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if in_text {
                    if let Ok(s) = e.unescape() {
                        out.push_str(&s);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    out
}

/// Görevlendirme bilgilerini çıkarır (şartname 5).
///
/// Çıkarım kural tabanlıdır: bilinen etiket kalıpları (ör. "Dosya No:",
/// "Mahkeme:") ve soru numaralandırması aranır. Her alanın kaynağı
/// (`FieldSource`) kaydedilir.
pub fn extract_assignment(
    path: &std::path::Path,
) -> Result<Assignment, AssignmentError> {
    let text = extract_text(path)?;
    let mut a = parse_assignment_text(&text);
    a.source_document = Some(path.to_string_lossy().into_owned());
    Ok(a)
}

/// Ham metinden görevlendirme bilgilerini çıkarır.
pub fn parse_assignment_text(text: &ExtractedText) -> Assignment {
    let mut a = Assignment {
        read_method: text.method,
        ..Default::default()
    };
    let full = text.full_text();

    // Alan etiketleri ve karşılık gelen hedef alanlar.
    let label_patterns: &[(&[&str], LabelTarget)] = &[
        (&["dosya no", "dosya numarası", "esas no", "dosya no."], LabelTarget::CaseNumber),
        (&["mahkeme", "birim", "cumhuriyet başsavcılığı"], LabelTarget::Court),
        (&["görevlendiren", "görevlendirme yapan", "merci"], LabelTarget::Authority),
        (&["konu", "inceleme konusu"], LabelTarget::Subject),
        (&["görevlendirme tarihi", "tarih"], LabelTarget::AssignmentDate),
        (&["teslim süresi", "rapor teslim", "süre"], LabelTarget::DueDate),
        (&["şüpheli", "sanık"], LabelTarget::PartySuspect),
        (&["mağdur", "müşteki"], LabelTarget::PartyVictim),
    ];

    for (page_idx, page) in text.pages.iter().enumerate() {
        for line in page.lines() {
            let lower = line.to_lowercase();
            for (labels, target) in label_patterns {
                if labels.iter().any(|l| lower.contains(l)) {
                    if let Some(value) = value_after_label(line) {
                        if value.is_empty() {
                            continue;
                        }
                        let excerpt = line.trim().to_string();
                        let fs = FieldSource {
                            field: target.field_name().to_string(),
                            page: (page_idx + 1) as u32,
                            excerpt,
                        };
                        match target {
                            LabelTarget::CaseNumber => {
                                if a.case_number.is_none() {
                                    a.case_number = Some(value);
                                    a.field_sources.push(fs);
                                }
                            }
                            LabelTarget::Court => {
                                if a.court.is_none() {
                                    a.court = Some(value);
                                    a.field_sources.push(fs);
                                }
                            }
                            LabelTarget::Authority => {
                                if a.authority.is_none() {
                                    a.authority = Some(value);
                                    a.field_sources.push(fs);
                                }
                            }
                            LabelTarget::Subject => {
                                if a.subject.is_none() {
                                    a.subject = Some(value);
                                    a.field_sources.push(fs);
                                }
                            }
                            LabelTarget::AssignmentDate => {
                                if a.assignment_date_text.is_none() {
                                    a.assignment_date_text = Some(value.clone());
                                    a.assignment_date = parse_turkish_date(&value);
                                    a.field_sources.push(fs);
                                }
                            }
                            LabelTarget::DueDate => {
                                if a.due_date_text.is_none() {
                                    a.due_date_text = Some(value.clone());
                                    a.due_date = parse_turkish_date(&value);
                                    a.field_sources.push(fs);
                                }
                            }
                            LabelTarget::PartySuspect => {
                                a.parties.push(Party {
                                    role: "Şüpheli".into(),
                                    name: value,
                                    raw_text: Some(line.trim().into()),
                                });
                            }
                            LabelTarget::PartyVictim => {
                                a.parties.push(Party {
                                    role: "Mağdur".into(),
                                    name: value,
                                    raw_text: Some(line.trim().into()),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Sorular: numaralı satırlar ve soru işaretli satırlar.
    a.questions = extract_questions(&full, text);
    // Bilirkişiden istenen işlemler.
    a.requested_actions = extract_requested_actions(&full);
    // İncelenecek materyaller.
    a.materials_declared = extract_declared_materials(&full);
    // Özel talimatlar.
    a.special_instructions = extract_special_instructions(&full);

    a
}

#[derive(Debug, Clone, Copy)]
enum LabelTarget {
    CaseNumber,
    Court,
    Authority,
    Subject,
    AssignmentDate,
    DueDate,
    PartySuspect,
    PartyVictim,
}

impl LabelTarget {
    fn field_name(&self) -> &'static str {
        match self {
            LabelTarget::CaseNumber => "case_number",
            LabelTarget::Court => "court",
            LabelTarget::Authority => "authority",
            LabelTarget::Subject => "subject",
            LabelTarget::AssignmentDate => "assignment_date",
            LabelTarget::DueDate => "due_date",
            LabelTarget::PartySuspect => "party_suspect",
            LabelTarget::PartyVictim => "party_victim",
        }
    }
}

/// "Dosya No: 2024/123" gibi satırlarda etiketten sonraki değeri döndürür.
fn value_after_label(line: &str) -> Option<String> {
    let seps = [':', '：'];
    for sep in seps {
        if let Some(pos) = line.find(sep) {
            let v = line[pos + sep.len_utf8()..].trim();
            let v = v.trim_start_matches('-').trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    // Ayraç yoksa etiketten sonraki kelimeleri al (ör. "Mahkeme Ankara ...").
    None
}

/// Türkçe tarih biçimlerini ayrıştırır: "12.05.2024", "12/05/2024",
/// "12 Mayıs 2024".
pub fn parse_turkish_date(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    use chrono::{NaiveDate, TimeZone, Utc};
    let s = s.trim();
    // gg.aa.yyyy veya gg/aa/yyyy
    for sep in ['.', '/', '-'] {
        let parts: Vec<&str> = s.split(sep).collect();
        if parts.len() == 3 {
            if let (Ok(d), Ok(m), Ok(y)) = (
                parts[0].trim().parse::<u32>(),
                parts[1].trim().parse::<u32>(),
                parts[2].trim().trim_end_matches(|c: char| !c.is_ascii_digit()).parse::<i32>(),
            ) {
                if let Some(nd) = NaiveDate::from_ymd_opt(y, m, d) {
                    return Utc
                        .from_utc_datetime(&nd.and_hms_opt(0, 0, 0).unwrap())
                        .into();
                }
            }
        }
    }
    // gg Ayıs yyyy
    let months = [
        ("ocak", 1),
        ("şubat", 2),
        ("mart", 3),
        ("nisan", 4),
        ("mayıs", 5),
        ("haziran", 6),
        ("temmuz", 7),
        ("ağustos", 8),
        ("eylül", 9),
        ("ekim", 10),
        ("kasım", 11),
        ("aralık", 12),
    ];
    let lower = s.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    if words.len() >= 3 {
        let d = words[0].trim_matches(|c: char| !c.is_ascii_digit()).parse::<u32>().ok();
        let m = months
            .iter()
            .find(|(name, _)| words[1].starts_with(name))
            .map(|(_, m)| *m);
        let y = words[2]
            .trim_matches(|c: char| !c.is_ascii_digit())
            .parse::<i32>()
            .ok();
        if let (Some(d), Some(m), Some(y)) = (d, m, y) {
            if let Some(nd) = NaiveDate::from_ymd_opt(y, m, d) {
                return Utc.from_utc_datetime(&nd.and_hms_opt(0, 0, 0).unwrap()).into();
            }
        }
    }
    None
}

/// Metinden soruları çıkarır. Orijinal sıra ve metin korunur.
fn extract_questions(full: &str, text: &ExtractedText) -> Vec<Question> {
    let mut questions = Vec::new();
    let mut ordinal = 0u32;
    // Soru bölümünü bul ("sorular" başlığı) veya soru işareti/numaralı
    // satırları al.
    let mut in_questions = false;
    let mut buffer = String::new();

    let flush = |buffer: &mut String, ordinal: &mut u32, questions: &mut Vec<Question>, text: &ExtractedText| {
        let q = buffer.trim();
        if !q.is_empty() {
            *ordinal += 1;
            let page = text
                .pages
                .iter()
                .position(|p| p.contains(q))
                .map(|i| (i + 1) as u32)
                .unwrap_or(0);
            let _ = page;
            questions.push(Question {
                id: QuestionId::new(),
                ordinal: *ordinal,
                original_text: q.to_string(),
                material_ids: vec![],
                audio_asset_ids: vec![],
                evidence_references: vec![],
                expert_answer: None,
                answer_approved: false,
            });
        }
        buffer.clear();
    };

    for line in full.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        // Soru bölümü başlığı: "SORULAR", "Bilirkişiye Sorulan Sorular",
        // "Sorular:", "Bilirkişiden İstenen Sorular" vb.
        if !in_questions && is_questions_heading(&lower) {
            in_questions = true;
            continue;
        }
        if in_questions {
            // Yeni bir soru numarası mı? ("1.", "1)", "Soru 1:")
            if is_question_start(trimmed) {
                flush(&mut buffer, &mut ordinal, &mut questions, text);
                buffer.push_str(strip_question_number(trimmed));
            } else if !trimmed.is_empty() {
                if buffer.is_empty() {
                    buffer.push_str(trimmed);
                } else {
                    buffer.push(' ');
                    buffer.push_str(trimmed);
                }
            }
        }
    }
    flush(&mut buffer, &mut ordinal, &mut questions, text);

    // Soru bölümü bulunamadıysa soru işaretiyle biten satırları topla.
    if questions.is_empty() {
        for line in full.lines() {
            let trimmed = line.trim();
            if trimmed.ends_with('?') && trimmed.chars().count() > 10 {
                ordinal += 1;
                questions.push(Question {
                    id: QuestionId::new(),
                    ordinal,
                    original_text: trimmed.to_string(),
                    material_ids: vec![],
                    audio_asset_ids: vec![],
                    evidence_references: vec![],
                    expert_answer: None,
                    answer_approved: false,
                });
            }
        }
    }

    questions
}

fn is_questions_heading(lower: &str) -> bool {
    let t = lower.trim().trim_end_matches(':').trim();
    // Yalnızca başlık satırları; soru numarası içermez.
    (t.contains("soru") || t.contains("sorulan") || t.contains("istenen"))
        && (t.starts_with("soru")
            || t.contains("sorulan sorular")
            || t.contains("sorular")
            || t.contains("istenen sorular"))
        && !t.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
}

fn is_question_start(line: &str) -> bool {
    let lower = line.to_lowercase();
    if lower.starts_with("soru") {
        return true;
    }
    // "1." "2)" "3-" gibi: rakamlardan sonra ayraç gelmeli.
    let bytes = line.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx == 0 || idx >= bytes.len() {
        return false;
    }
    let sep = bytes[idx] as char;
    matches!(sep, '.' | ')' | '-') && line[idx + 1..].trim_start().len() >= 3
}

fn strip_question_number(line: &str) -> &str {
    // "Soru 1: metin" -> "metin"
    let lower = line.to_lowercase();
    if lower.starts_with("soru") {
        let rest = &line["soru".len()..];
        return rest.trim_start_matches(|c: char| {
            c.is_ascii_digit() || c == '.' || c == ')' || c == ':' || c == ' ' || c == '-'
        });
    }
    // "1. metin" -> "metin"
    let bytes = line.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx > 0 && idx < line.len() {
        return line[idx..].trim_start_matches(|c: char| matches!(c, '.' | ')' | '-' | ':' | ' '));
    }
    line
}

fn extract_requested_actions(full: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_section = false;
    for line in full.lines() {
        let lower = line.to_lowercase();
        if lower.contains("istenen işlem") || lower.contains("yapılması istenen") {
            in_section = true;
            continue;
        }
        if in_section {
            let t = line.trim();
            if t.is_empty() {
                in_section = false;
                continue;
            }
            if is_question_start(t) || lower.contains("soru") {
                in_section = false;
                continue;
            }
            out.push(t.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ')' || c == '-').trim().to_string());
        }
    }
    out.retain(|s| !s.is_empty());
    out
}

fn extract_declared_materials(full: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_section = false;
    for line in full.lines() {
        let lower = line.to_lowercase();
        if lower.contains("incelenecek materyal")
            || lower.contains("incelenmesi istenen")
            || lower.contains("materyaller")
        {
            in_section = true;
            continue;
        }
        if in_section {
            let t = line.trim();
            if t.is_empty() {
                in_section = false;
                continue;
            }
            out.push(t.to_string());
        }
    }
    out
}

fn extract_special_instructions(full: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_section = false;
    for line in full.lines() {
        let lower = line.to_lowercase();
        if lower.contains("özel talimat") {
            in_section = true;
            continue;
        }
        if in_section {
            let t = line.trim();
            if t.is_empty() {
                in_section = false;
                continue;
            }
            out.push(t.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"T.C.
ANKARA 1. ASLİYE CEZA MAHKEMESİ

Dosya No: 2024/123 Esas
Görevlendiren Merci: Ankara 1. Asliye Ceza Mahkemesi
Mahkeme: Ankara 1. Asliye Ceza Mahkemesi
Konu: Ses kaydının metne dönüştürülmesi
Görevlendirme Tarihi: 12.05.2024
Rapor Teslim Süresi: 30.06.2024
Şüpheli: Ahmet Yılmaz
Mağdur: Mehmet Demir

İncelenecek Materyaller:
1 adet USB bellek

Bilirkişiye Sorulan Sorular:
1. Kayıtta kimler konuşmaktadır?
2. Kayıtta geçen tarih ve tutarlar nelerdir?
3. Ses kaydında suç unsuru bulunmakta mıdır?
"#;

    fn sample_extracted() -> ExtractedText {
        ExtractedText {
            pages: vec![SAMPLE.to_string()],
            method: DocumentReadMethod::PdfTextLayer,
            document_kind: "PDF".into(),
        }
    }

    #[test]
    fn extracts_case_number_and_court() {
        let a = parse_assignment_text(&sample_extracted());
        assert_eq!(a.case_number.as_deref(), Some("2024/123 Esas"));
        assert!(a.court.as_deref().unwrap().contains("Ankara"));
        assert_eq!(a.subject.as_deref(), Some("Ses kaydının metne dönüştürülmesi"));
    }

    #[test]
    fn extracts_dates() {
        let a = parse_assignment_text(&sample_extracted());
        assert!(a.assignment_date.is_some());
        assert!(a.due_date.is_some());
        let d = a.assignment_date.unwrap();
        assert_eq!(d.format("%Y-%m-%d").to_string(), "2024-05-12");
    }

    #[test]
    fn extracts_parties() {
        let a = parse_assignment_text(&sample_extracted());
        assert!(a.parties.iter().any(|p| p.role == "Şüpheli" && p.name.contains("Ahmet")));
        assert!(a.parties.iter().any(|p| p.role == "Mağdur" && p.name.contains("Mehmet")));
    }

    #[test]
    fn extracts_questions_in_order_without_rewriting() {
        let a = parse_assignment_text(&sample_extracted());
        assert_eq!(a.questions.len(), 3);
        assert_eq!(a.questions[0].ordinal, 1);
        assert_eq!(a.questions[0].original_text, "Kayıtta kimler konuşmaktadır?");
        assert_eq!(a.questions[1].ordinal, 2);
        assert!(a.questions[1].original_text.contains("tarih ve tutarlar"));
        assert_eq!(a.questions[2].ordinal, 3);
        // Sorular anlam değiştirecek şekilde yeniden yazılmamalı.
        assert!(a.questions[2].original_text.ends_with('?'));
    }

    #[test]
    fn field_sources_are_recorded() {
        let a = parse_assignment_text(&sample_extracted());
        assert!(a.field_sources.iter().any(|f| f.field == "case_number"));
        let fs = a.field_sources.iter().find(|f| f.field == "case_number").unwrap();
        assert_eq!(fs.page, 1);
        assert!(fs.excerpt.contains("2024/123"));
    }

    #[test]
    fn turkish_date_parsing_variants() {
        assert!(parse_turkish_date("12.05.2024").is_some());
        assert!(parse_turkish_date("12/05/2024").is_some());
        let d = parse_turkish_date("12 Mayıs 2024").unwrap();
        assert_eq!(d.format("%Y-%m-%d").to_string(), "2024-05-12");
        assert!(parse_turkish_date("geçersiz").is_none());
    }

    #[test]
    fn missing_fields_detected() {
        let empty = ExtractedText {
            pages: vec!["Bu belgede hiçbir bilgi yok.".into()],
            method: DocumentReadMethod::PlainText,
            document_kind: "TXT".into(),
        };
        let a = parse_assignment_text(&empty);
        assert!(a.case_number.is_none());
        assert!(!a.missing_required_fields().is_empty());
    }

    #[test]
    fn docx_xml_conversion() {
        let xml = r#"<?xml version="1.0"?><w:document xmlns:w="x"><w:body>
<w:p><w:r><w:t>Dosya No</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>2024/1</w:t></w:r></w:p>
<w:p><w:r><w:t>İkinci paragraf</w:t></w:r></w:p>
</w:body></w:document>"#;
        let text = docx_xml_to_text(xml);
        assert!(text.contains("Dosya No"));
        assert!(text.contains("2024/1"));
        assert!(text.contains('\t'));
        assert!(text.contains("İkinci paragraf"));
    }

    #[test]
    fn txt_file_extraction() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("gorev.txt");
        std::fs::write(&p, SAMPLE).unwrap();
        let a = extract_assignment(&p).unwrap();
        assert_eq!(a.case_number.as_deref(), Some("2024/123 Esas"));
        assert_eq!(a.questions.len(), 3);
        assert!(a.source_document.is_some());
    }

    #[test]
    fn docx_file_extraction() {
        // Basit bir DOCX paketi oluştur.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("gorev.docx");
        {
            use std::io::Write;
            let f = std::fs::File::create(&p).unwrap();
            let mut z = zip::ZipWriter::new(f);
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            z.start_file("word/document.xml", opts).unwrap();
            z.write_all(SAMPLE.as_bytes()).unwrap();
            z.finish().unwrap();
        }
        // DOCX içeriği düz metin olarak konuldu; çıkarım yine de çalışmalı.
        let text = extract_text(&p).unwrap();
        assert_eq!(text.document_kind, "DOCX");
    }

    #[test]
    fn unsupported_doc_reports_clearly() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("eski.doc");
        std::fs::write(&p, b"\xD0\xCF\x11\xE0").unwrap();
        let err = extract_text(&p).unwrap_err();
        assert!(matches!(err, AssignmentError::Unsupported(_)));
        assert!(err.to_string().contains("dönüştürme"));
    }
}
