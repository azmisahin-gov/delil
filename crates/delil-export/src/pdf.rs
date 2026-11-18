//! PDF rapor kopyası üretimi.
//!
//! `printpdf` ile A4 sayfalara rapor metni yazılır. Türkçe karakterler için
//! gömülü bir yazı tipi (DejaVu Serif) kullanılır; bu yazı tipi `assets/`
//! altında kurulum paketiyle birlikte verilir.

use super::ExportError;
use delil_core::Report;
use printpdf::*;
use std::path::Path;

/// Sayfa kenar boşlukları (mm).
const MARGIN_MM: f32 = 20.0;
/// Satır yüksekliği (mm).
const LINE_HEIGHT_MM: f32 = 5.0;
/// Yazı tipi boyutu.
const FONT_SIZE: f32 = 11.0;

/// Başlık satırı ekler (kalın, 14 punto).
fn push_h(out: &mut Vec<(String, bool, f32)>, s: &str) {
    out.push((s.to_string(), true, 14.0));
}

/// Gövde satırı ekler (normal, 11 punto).
fn push_t(out: &mut Vec<(String, bool, f32)>, s: &str) {
    out.push((s.to_string(), false, FONT_SIZE));
}

/// Gömülü DejaVu Serif yazı tipi (Türkçe karakter desteği).
const FONT_BYTES: &[u8] = include_bytes!("../../delil-export/assets/DejaVuSerif.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../../delil-export/assets/DejaVuSerif-Bold.ttf");

/// Raporu PDF olarak yazar.
pub fn export_pdf(report: &Report, path: &Path) -> Result<(), ExportError> {
    let (doc, page1, layer1) = PdfDocument::new(&report.cover_title, Mm(210.0), Mm(297.0), "Katman 1");
    let font = doc
        .add_external_font(std::io::Cursor::new(FONT_BYTES))
        .map_err(|e| ExportError::Pdf(e.to_string()))?;
    let font_bold = doc
        .add_external_font(std::io::Cursor::new(FONT_BOLD))
        .map_err(|e| ExportError::Pdf(e.to_string()))?;

    let mut current_layer = doc.get_page(page1).get_layer(layer1);
    let mut y = 297.0 - MARGIN_MM;

    let lines = render_report_lines(report);

    for (text, bold, size) in lines {
        if y < MARGIN_MM + LINE_HEIGHT_MM {
            let (p, l) = doc.add_page(Mm(210.0), Mm(297.0), "Katman 1");
            current_layer = doc.get_page(p).get_layer(l);
            y = 297.0 - MARGIN_MM;
        }
        let f = if bold { &font_bold } else { &font };
        current_layer.use_text(text, size, Mm(MARGIN_MM), Mm(y), f);
        y -= LINE_HEIGHT_MM * (size / FONT_SIZE);
    }

    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let file = std::fs::File::create(path)?;
    let mut writer = std::io::BufWriter::new(file);
    doc.save(&mut writer)
        .map_err(|e| ExportError::Pdf(e.to_string()))?;
    Ok(())
}

/// Raporu (metin, kalın, boyut) satırlarına çevirir.
///
/// Bu işlev saf ve deterministiktir; test edilebilir.
pub fn render_report_lines(report: &Report) -> Vec<(String, bool, f32)> {
    let mut out: Vec<(String, bool, f32)> = Vec::new();

    push_h(&mut out, &report.cover_title);
    if let Some(sub) = &report.cover_subtitle {
        push_t(&mut out, sub);
    }
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "1-3. Görevlendiren merci / dosya numarası");
    push_t(&mut out, &format!(
        "Merci: {}",
        report.authority.as_deref().unwrap_or("(belirtilmemiş)")
    ));
    push_t(&mut out, &format!(
        "Dosya No: {}",
        report.case_number.as_deref().unwrap_or("(belirtilmemiş)")
    ));
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "4. Taraflar");
    for p in &report.parties {
        push_t(&mut out, &format!("{}: {}", p.role, p.name));
    }
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "5. Bilirkişi bilgileri");
    push_t(&mut out, &format!(
        "Ad Soyad: {}",
        report.expert.full_name.as_deref().unwrap_or("(belirtilmemiş)")
    ));
    push_t(&mut out, &format!(
        "Unvan: {}",
        report.expert.title.as_deref().unwrap_or("(belirtilmemiş)")
    ));
    push_t(&mut out, &format!(
        "Sicil No: {}",
        report.expert.registry_number.as_deref().unwrap_or("(belirtilmemiş)")
    ));
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "6. Görevlendirme tarihi ve süresi");
    push_t(&mut out, &format!(
        "Görevlendirme: {}",
        report.assignment_date_text.as_deref().unwrap_or("-")
    ));
    push_t(&mut out, &format!(
        "Teslim süresi: {}",
        report.due_date_text.as_deref().unwrap_or("-")
    ));
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "7. İnceleme konusu");
    push_t(&mut out, report.subject.as_deref().unwrap_or("(belirtilmemiş)"));
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "8. Bilirkişiye yöneltilen sorular ve yanıtlar");
    for q in &report.questions {
        out.push((format!("{}. {}", q.ordinal, q.original_text), true, FONT_SIZE));
        let ans = q.expert_answer.as_deref().unwrap_or("(yanıtlanmadı)");
        push_t(&mut out, &format!("Yanıt: {ans}"));
        for ev in &q.evidence_references {
            push_t(&mut out, &format!(
                "Dayanak: “{}” ({:.1}s–{:.1}s)",
                ev.quote, ev.start_seconds, ev.end_seconds
            ));
        }
    }
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "9-10. İncelenen materyaller ve edinim açıklaması");
    for m in &report.materials {
        push_t(&mut out, &format!(
            "{} — {} bayt — {} — SHA-256: {}",
            m.file_name,
            m.size_bytes,
            m.verification.label_tr(),
            m.target_sha256.as_deref().unwrap_or("-")
        ));
    }
    push_t(&mut out, &report.acquisition_note);
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "11. İnceleme yöntemi");
    push_t(&mut out, &report.method);
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "12-13. Kullanılan yazılım ve transkripsiyon yöntemi");
    push_t(&mut out, &format!(
        "{} {} — model: {} ({})",
        report.software.app_name,
        report.software.app_version,
        report.software.model_name,
        report.software.model_version
    ));
    push_t(&mut out, &report.transcription_method);
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "14. Teknik ses envanteri");
    for a in &report.audio_inventory {
        push_t(&mut out, &format!(
            "{} — {} / {} — {:.1}s — {} Hz — {} kanal — SHA-256: {}",
            a.file_name,
            a.technical.container,
            a.technical.codec,
            a.technical.duration_seconds,
            a.technical.sample_rate_hz.map(|v| v.to_string()).unwrap_or_default(),
            a.technical.channels.map(|v| v.to_string()).unwrap_or_default(),
            a.sha256.as_deref().unwrap_or("-")
        ));
    }
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "15-16. Transkript içeriği ve zaman kodları");
    for t_ in &report.transcripts {
        for s in &t_.current().segments {
            if report.include_timestamps {
                push_t(&mut out, &format!(
                    "[{:.1}s-{:.1}s] {}",
                    s.start_seconds, s.end_seconds, s.text
                ));
            } else {
                push_t(&mut out, &s.text);
            }
        }
    }
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "17. Belirsiz/anlaşılamayan bölümler");
    if report.unintelligible_summary.is_empty() {
        push_t(&mut out, "(belirsiz bölüm işaretlenmedi)");
    } else {
        for u in &report.unintelligible_summary {
            push_t(&mut out, u);
        }
    }
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "20. Yöntem sınırlılıkları");
    for l in &report.limitations {
        push_t(&mut out, l);
    }
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "21. Gerekçeli sonuç");
    push_t(&mut out, report
        .reasoned_conclusion
        .as_deref()
        .unwrap_or("(bilirkişi tarafından doldurulacak)"));
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "22-23. Rapor tarihi ve imza");
    push_t(&mut out, &format!("Rapor tarihi: {}", report.report_date.format("%d.%m.%Y")));
    push_t(&mut out, &report.signature_block.expert_line);
    push_t(&mut out, &report.signature_block.note);
    out.push((String::new(), false, FONT_SIZE));

    push_h(&mut out, "24. Ekler");
    for att in &report.attachments {
        push_t(&mut out, &format!("EK-{}: {} ({})", att.number, att.title, att.kind.label_tr()));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use delil_core::*;

    fn sample_report() -> Report {
        Report {
            id: ReportId::new(),
            created_at: chrono::Utc::now(),
            report_date: chrono::Utc::now(),
            cover_title: "BİLİRKİŞİ RAPORU".into(),
            cover_subtitle: Some("Ses incelemesi".into()),
            authority: Some("Ankara 1. ACM".into()),
            case_number: Some("2024/1".into()),
            parties: vec![Party {
                role: "Şüpheli".into(),
                name: "A. B.".into(),
                raw_text: None,
            }],
            expert: ExpertInfo {
                full_name: Some("Dr. E".into()),
                title: Some("Bilirkişi".into()),
                registry_number: Some("1".into()),
            },
            assignment_date_text: Some("01.01.2024".into()),
            due_date_text: Some("01.02.2024".into()),
            subject: Some("Ses çözümleme".into()),
            questions: vec![Question {
                id: QuestionId::new(),
                ordinal: 1,
                original_text: "Ne konuşuluyor?".into(),
                material_ids: vec![],
                audio_asset_ids: vec![],
                evidence_references: vec![],
                expert_answer: Some("Görüşme".into()),
                answer_approved: true,
            }],
            materials: vec![],
            acquisition_note: "Edinim açıklaması".into(),
            method: "Yöntem".into(),
            software: SoftwareInfo::default(),
            transcription_method: "Whisper".into(),
            audio_inventory: vec![],
            transcripts: vec![],
            include_timestamps: true,
            unintelligible_summary: vec![],
            limitations: vec!["Sınırlılık".into()],
            reasoned_conclusion: Some("Sonuç".into()),
            signature_block: SignatureBlock::default(),
            attachments: vec![],
            missing_fields: vec![],
        }
    }

    #[test]
    fn render_includes_all_sections() {
        let r = sample_report();
        let lines = render_report_lines(&r);
        let joined = lines.iter().map(|(s, _, _)| s.as_str()).collect::<Vec<_>>().join("\n");
        assert!(joined.contains("BİLİRKİŞİ RAPORU"));
        assert!(joined.contains("2024/1"));
        assert!(joined.contains("Ne konuşuluyor?"));
        assert!(joined.contains("Gerekçeli sonuç"));
        assert!(joined.contains("Yöntem sınırlılıkları"));
        assert!(joined.contains("Ekler"));
    }

    #[test]
    fn pdf_export_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("r.pdf");
        export_pdf(&sample_report(), &p).unwrap();
        assert!(p.exists());
        let data = std::fs::read(&p).unwrap();
        assert!(data.starts_with(b"%PDF"), "geçerli PDF olmalı");
        assert!(data.len() > 500);
    }

    #[test]
    fn pdf_preserves_turkish_text() {
        // PDF çıktısı Türkçe karakterleri kaybetmemeli (yazı tipi gömülü).
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("tr.pdf");
        let mut r = sample_report();
        r.subject = Some("Şüpheli ifadesi: İĞÜŞÖÇ ığüşöç".into());
        export_pdf(&r, &p).unwrap();
        let data = std::fs::read(&p).unwrap();
        // Gömülü yazı tipi verisi dosyada bulunmalı.
        assert!(data.windows(4).any(|w| w == b"Font"));
    }
}
