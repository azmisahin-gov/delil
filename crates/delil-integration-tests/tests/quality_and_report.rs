//! Kalite kontrolü ve rapor testleri (şartname 19.1, 9, 11).
//!
//! Gerçek kullanım koşullarını yansıtan kayıt türleri (temiz konuşma, gürültü,
//! düşük ses, çok konuşmacı, örtüşen konuşma, özel isim, sayı/tarih, olumsuzluk)
//! için kalite kontrolü davranışı sınanır.

use delil_core::*;
use delil_quality::{check_transcript, Thresholds};

fn seg(i: usize, s: f64, e: f64, text: &str) -> TranscriptSegment {
    TranscriptSegment {
        index: i,
        start_seconds: s,
        end_seconds: e,
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

fn transcript(segs: Vec<TranscriptSegment>) -> Transcript {
    Transcript::new_machine(AudioAssetId::new(), "test", segs, Some("tr".into()))
}

#[test]
fn clean_speech_passes_without_priority_flags() {
    let t = transcript(vec![
        seg(0, 0.0, 2.0, "Merhaba, ben geldim."),
        seg(1, 2.0, 4.0, "Nasılsınız acaba?"),
    ]);
    let stats = delil_quality::AudioSignalStats {
        duration_seconds: 4.0,
        rms: 0.25,
        peak: 0.7,
        ..Default::default()
    };
    let r = check_transcript(&t, &stats, Thresholds::default());
    assert!(r.overall.severity() <= QualityFlag::ReviewSuggested.severity());
}

#[test]
fn proper_names_and_numbers_are_flagged_for_review() {
    let t = transcript(vec![seg(
        0,
        0.0,
        3.0,
        "Ahmet Yılmaz 12.05.2024 tarihinde 5000 TL ödedi.",
    )]);
    let stats = delil_quality::AudioSignalStats {
        duration_seconds: 3.0,
        rms: 0.25,
        ..Default::default()
    };
    let r = check_transcript(&t, &stats, Thresholds::default());
    assert!(r
        .issues
        .iter()
        .any(|i| i.kind == QualityIssueKind::ProperNounOrNumberRisk));
    assert_eq!(r.overall, QualityFlag::PriorityReview);
}

#[test]
fn negation_words_are_flagged() {
    let t = transcript(vec![seg(0, 0.0, 3.0, "Bunu ben yapmadım, doğru değil.")]);
    let stats = delil_quality::AudioSignalStats {
        duration_seconds: 3.0,
        rms: 0.25,
        ..Default::default()
    };
    let r = check_transcript(&t, &stats, Thresholds::default());
    assert!(r
        .issues
        .iter()
        .any(|i| i.kind == QualityIssueKind::NegationRisk));
}

#[test]
fn noisy_and_low_volume_recording_flagged() {
    let t = transcript(vec![seg(0, 0.0, 3.0, "çok kısık bir ses")]);
    let stats = delil_quality::AudioSignalStats {
        duration_seconds: 3.0,
        rms: 0.002,
        peak: 0.05,
        ..Default::default()
    };
    let r = check_transcript(&t, &stats, Thresholds::default());
    assert!(r
        .issues
        .iter()
        .any(|i| i.kind == QualityIssueKind::VeryLowVolume));
}

#[test]
fn overlapping_speech_is_flagged() {
    let t = transcript(vec![seg(0, 0.0, 5.0, "iki kişi aynı anda")]);
    let stats = delil_quality::AudioSignalStats {
        duration_seconds: 5.0,
        rms: 0.25,
        overlap_regions: vec![(1.0, 2.5)],
        ..Default::default()
    };
    let r = check_transcript(&t, &stats, Thresholds::default());
    assert!(r
        .issues
        .iter()
        .any(|i| i.kind == QualityIssueKind::OverlappingSpeech));
}

#[test]
fn text_in_silence_is_flagged() {
    let t = transcript(vec![seg(0, 5.0, 8.0, "sessizlikte üretilen metin")]);
    let stats = delil_quality::AudioSignalStats {
        duration_seconds: 10.0,
        silences: vec![(0.0, 10.0)],
        rms: 0.25,
        ..Default::default()
    };
    let r = check_transcript(&t, &stats, Thresholds::default());
    assert!(r
        .issues
        .iter()
        .any(|i| i.kind == QualityIssueKind::TextInSilence));
}

#[test]
fn long_recording_chunk_boundary_loss_flagged() {
    let t = transcript(vec![
        seg(0, 0.0, 30.0, "birinci parça"),
        seg(1, 40.0, 70.0, "ikinci parça"),
    ]);
    let stats = delil_quality::AudioSignalStats {
        duration_seconds: 70.0,
        rms: 0.25,
        ..Default::default()
    };
    let r = check_transcript(&t, &stats, Thresholds::default());
    assert!(r
        .issues
        .iter()
        .any(|i| i.kind == QualityIssueKind::ChunkBoundaryLoss));
}

#[test]
fn quality_report_is_not_a_fake_accuracy_percentage() {
    // Raporda hiçbir alan "doğruluk yüzdesi" sunmamalı; yalnızca anlaşılır
    // durumlar bulunmalı.
    let t = transcript(vec![seg(0, 0.0, 3.0, "test")]);
    let stats = delil_quality::AudioSignalStats {
        duration_seconds: 3.0,
        rms: 0.25,
        ..Default::default()
    };
    let r = check_transcript(&t, &stats, Thresholds::default());
    let json = serde_json::to_string(&r).unwrap();
    assert!(!json.contains("accuracy"));
    assert!(!json.contains("confidence"));
    // Yalnızca bilinen durum etiketleri kullanılır.
    for flag in [
        QualityFlag::Normal,
        QualityFlag::ReviewSuggested,
        QualityFlag::PriorityReview,
        QualityFlag::Unintelligible,
    ] {
        assert!(!flag.label_tr().is_empty());
    }
}

#[test]
fn report_preserves_original_questions_verbatim() {
    let mut case = Case::new("/tmp/c");
    case.assignment.case_number = Some("2024/1".into());
    case.assignment.questions = vec![Question {
        id: QuestionId::new(),
        ordinal: 1,
        original_text: "Kayıtta TEHDİT var mıdır? (aynen)".into(),
        material_ids: vec![],
        audio_asset_ids: vec![],
        evidence_references: vec![],
        expert_answer: Some("Yoktur.".into()),
        answer_approved: true,
    }];
    let opts = delil_report::ReportOptions::default();
    let report = delil_report::generate_report(&case, &opts).unwrap();
    assert_eq!(
        report.questions[0].original_text,
        "Kayıtta TEHDİT var mıdır? (aynen)"
    );
}

#[test]
fn report_includes_limitations_and_method() {
    let case = Case::new("/tmp/c");
    let opts = delil_report::ReportOptions::default();
    let report = delil_report::generate_report(&case, &opts).unwrap();
    assert!(report.limitations.len() >= 5);
    assert!(report
        .limitations
        .iter()
        .any(|l| l.contains("konuşmacı") || l.contains("Konuşmacı")));
    assert!(report.limitations.iter().any(|l| l.contains("garanti")));
    assert!(!report.method.is_empty());
    assert!(!report.transcription_method.is_empty());
}
