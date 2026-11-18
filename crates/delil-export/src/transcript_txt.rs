//! Transkript TXT kopyası üretimi.

use super::ExportError;
use delil_core::Transcript;
use std::path::Path;

/// Bir transkripti TXT olarak yazar.
///
/// Başlıkta ses dosyası, süre, model ve sürüm bilgileri; gövdede zaman kodlu
/// (istenirse) transkript satırları yer alır.
pub fn export_transcript_txt(transcript: &Transcript, path: &Path) -> Result<(), ExportError> {
    let text = render_transcript_txt(transcript);
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(path, text)?;
    Ok(())
}

/// Transkripti TXT metnine çevirir (saf işlev).
pub fn render_transcript_txt(transcript: &Transcript) -> String {
    let v = transcript.current();
    let mut out = String::new();
    out.push_str("DELİL — TRANSKRİPT KOPYASI\n");
    out.push_str("==========================\n");
    out.push_str(&format!("Transkript kimliği: {}\n", transcript.id));
    out.push_str(&format!("Ses varlığı: {}\n", transcript.audio_asset_id));
    out.push_str(&format!(
        "Model: {} ({})\n",
        transcript.model_name, transcript.model_version
    ));
    if let Some(l) = &transcript.detected_language {
        out.push_str(&format!("Algılanan dil: {l}\n"));
    }
    if let Some(o) = &transcript.language_user_override {
        out.push_str(&format!("Kullanıcı dil düzeltmesi: {o}\n"));
    }
    out.push_str(&format!("Sürüm: {} ({})\n", v.version, v.status.label_tr()));
    out.push_str(&format!(
        "Oluşturulma: {}\n",
        v.created_at.format("%d.%m.%Y %H:%M")
    ));
    out.push_str(&format!(
        "Birebir (verbatim): {}\n",
        if transcript.verbatim {
            "evet"
        } else {
            "hayır"
        }
    ));
    out.push_str("==========================\n\n");

    for s in &v.segments {
        let speaker = s
            .speaker
            .as_deref()
            .map(|sp| format!("{sp}: "))
            .unwrap_or_default();
        out.push_str(&format!(
            "[{:.2} - {:.2}] {}{}\n",
            s.start_seconds, s.end_seconds, speaker, s.text
        ));
        if let Some(note) = &s.note {
            out.push_str(&format!("    (Not: {note})\n"));
        }
        if s.marked_unintelligible {
            out.push_str("    (Anlaşılamayan bölüm olarak işaretlendi)\n");
        }
    }

    out.push('\n');
    out.push_str("Not: Bu kopya bilirkişi incelemesine uygun birebir transkript taslağıdır.\n");
    out.push_str("Konuşmacı etiketleri gerçek kişi kimliği anlamına gelmez.\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use delil_core::*;

    #[test]
    fn renders_transcript_with_timestamps() {
        let seg = TranscriptSegment {
            index: 0,
            start_seconds: 0.0,
            end_seconds: 2.5,
            text: "Merhaba dünya".into(),
            machine_text: "Merhaba dünya".into(),
            quality: QualityFlag::Normal,
            issues: vec![],
            marked_unintelligible: false,
            speaker: Some("Konuşmacı 1".into()),
            note: None,
            word_level_timestamps: false,
        };
        let t = Transcript::new_machine(AudioAssetId::new(), "test", vec![seg], Some("tr".into()));
        let out = render_transcript_txt(&t);
        assert!(out.contains("TRANSKRİPT KOPYASI"));
        assert!(out.contains("[0.00 - 2.50] Konuşmacı 1: Merhaba dünya"));
        assert!(out.contains("birebir"));
        assert!(out.contains("gerçek kişi kimliği"));
    }

    #[test]
    fn writes_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.txt");
        let t = Transcript::new_machine(AudioAssetId::new(), "test", vec![], Some("tr".into()));
        export_transcript_txt(&t, &p).unwrap();
        assert!(p.exists());
    }

    #[test]
    fn marks_unintelligible_and_notes() {
        let seg = TranscriptSegment {
            index: 0,
            start_seconds: 0.0,
            end_seconds: 1.0,
            text: "".into(),
            machine_text: "".into(),
            quality: QualityFlag::Unintelligible,
            issues: vec![QualityIssueKind::UnintelligibleSegment],
            marked_unintelligible: true,
            speaker: None,
            note: Some("kontrol edildi".into()),
            word_level_timestamps: false,
        };
        let t = Transcript::new_machine(AudioAssetId::new(), "test", vec![seg], Some("tr".into()));
        let out = render_transcript_txt(&t);
        assert!(out.contains("Anlaşılamayan bölüm"));
        assert!(out.contains("kontrol edildi"));
    }
}
