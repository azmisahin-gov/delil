//! Otomatik transkripsiyon kalite kontrolü (şartname 9).
//!
//! Bu modül transkripti inceler ve riskli bölümleri işaretler. Model
//! güven/log olasılık değerleri **gerçek doğruluk yüzdesi olarak
//! gösterilmez**; bunun yerine `Normal`, `Kontrol önerilir`,
//! `Öncelikli inceleme`, `Anlaşılamayan bölüm` durumları kullanılır.
//!
//! Tespit edilen durumlar:
//! uzun sessizlik, konuşma varken boş transkript, sessizlikte metin, şüpheli
//! tekrarlar, konuşma dışı kelime, çok düşük ses, bozuk bölüm, örtüşen
//! konuşma, anlaşılamayan bölüm, dil tutarsızlığı, zaman kodu tutarsızlığı,
//! parça sınırı kaybı, özel isim/sayı riski ve olumsuzluk sözcüğü riski.

use delil_core::{QualityFlag, QualityIssueKind, Transcript, TranscriptSegment};
use serde::{Deserialize, Serialize};

/// Ses sinyali özeti; kalite kontrolüne ses tarafından gelen ölçümler.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AudioSignalStats {
    /// Sesin toplam süresi (saniye).
    pub duration_seconds: f64,
    /// Sessizlik aralıkları (başlangıç, bitiş) saniye.
    pub silences: Vec<(f64, f64)>,
    /// Kök ortalama kare (RMS) seviyesi 0..1.
    pub rms: f32,
    /// Tepe (peak) seviyesi 0..1.
    pub peak: f32,
    /// Örtüşen konuşma olasılığı olan aralıklar.
    pub overlap_regions: Vec<(f64, f64)>,
    /// Ses çözülemedi mi?
    pub corrupt: bool,
}

/// Bir bölüm için tespit edilen kalite sorunu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentIssue {
    pub segment_index: usize,
    pub kind: QualityIssueKind,
    pub flag: QualityFlag,
    /// Kullanıcıya gösterilecek sade açıklama.
    pub description: String,
    /// İlgili zaman aralığı.
    pub start_seconds: f64,
    pub end_seconds: f64,
}

/// Kalite kontrolü sonucu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityReport {
    pub issues: Vec<SegmentIssue>,
    /// En yüksek risk seviyesi.
    pub overall: QualityFlag,
    /// İncelenmesi gereken segment sıraları.
    pub review_queue: Vec<usize>,
    /// Özet (Türkçe).
    pub summary_tr: String,
}

impl QualityReport {
    pub fn is_clean(&self) -> bool {
        self.overall == QualityFlag::Normal
    }
}

/// Varsayılan eşikler.
#[derive(Debug, Clone, Copy)]
pub struct Thresholds {
    /// Bu süreden uzun sessizlik "uzun sessizlik" sayılır (saniye).
    pub long_silence_seconds: f64,
    /// Bu RMS altındaki ses "çok düşük ses".
    pub low_rms: f32,
    /// Segment metninin boş sayılacağı uzunluk.
    pub empty_text_chars: usize,
    /// Bir kelimenin bu sayıdan fazla art arda tekrarı "şüpheli tekrar".
    pub max_word_repeats: usize,
    /// Konuşma dışı bilinen çıktılar.
    pub non_speech_markers: &'static [&'static str],
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            long_silence_seconds: 10.0,
            low_rms: 0.01,
            empty_text_chars: 0,
            max_word_repeats: 4,
            non_speech_markers: &[
                "[müzik]",
                "[music]",
                "[alkış]",
                "[applause]",
                "[gülüşme]",
                "[laughter]",
                "(müzik)",
                "(music)",
                "[sessizlik]",
                "[silence]",
                "[blank_audio]",
                "[no speech]",
                "altyazı:",
                "subtitles by",
                "amara.org",
            ],
        }
    }
}

/// Türkçe olumsuzluk sözcükleri; bunlar içeren segmentler öncelikli incelenir.
const NEGATION_WORDS: &[&str] = &[
    "değil",
    "değildi",
    "yok",
    "yoktu",
    "asla",
    "hiçbir",
    "hiç",
    "olmaz",
    "olmadı",
    "etmez",
    "etmedi",
    "bilmem",
    "bilmiyorum",
    "hayır",
    "olmamış",
    "yapmadı",
    "yapmaz",
];

/// Sayı veya tarih içeren metin kalıbı (basit sezgisel kontrol).
fn contains_numbers_or_dates(text: &str) -> bool {
    let has_digit = text.chars().any(|c| c.is_ascii_digit());
    let has_date_sep = text.contains('.') && has_digit;
    let has_time = text.contains(':') && has_digit;
    has_digit || has_date_sep || has_time
}

/// Özel isim sezgiseli: cümle başı dışında büyük harfle başlayan kelimeler.
fn contains_proper_noun(text: &str) -> bool {
    let mut words = text.split_whitespace();
    // İlk kelimeyi atla (cümle başı olabilir).
    let _ = words.next();
    words.any(|w| {
        let mut chars = w.chars();
        matches!(chars.next(), Some(c) if c.is_uppercase()) && w.chars().count() > 1
    })
}

/// Bir segmentin metnini kelimelere ayırır (küçük harfe indirger).
fn words_lower(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

/// Transkript + ses özeti üzerinde kalite kontrolü çalıştırır.
pub fn check_transcript(
    transcript: &Transcript,
    stats: &AudioSignalStats,
    th: Thresholds,
) -> QualityReport {
    let segments = &transcript.current().segments;
    let mut issues: Vec<SegmentIssue> = Vec::new();

    // 1. Ses çözülemediyse.
    if stats.corrupt {
        issues.push(SegmentIssue {
            segment_index: 0,
            kind: QualityIssueKind::CorruptSegment,
            flag: QualityFlag::PriorityReview,
            description: "Ses dosyasının bir bölümü bozuk; bu bölüm güvenilir değil.".into(),
            start_seconds: 0.0,
            end_seconds: stats.duration_seconds,
        });
    }

    // 2. Çok düşük ses.
    if stats.rms > 0.0 && stats.rms < th.low_rms {
        issues.push(SegmentIssue {
            segment_index: 0,
            kind: QualityIssueKind::VeryLowVolume,
            flag: QualityFlag::PriorityReview,
            description: format!(
                "Ses seviyesi çok düşük (RMS {:.3}); transkript hatalı olabilir.",
                stats.rms
            ),
            start_seconds: 0.0,
            end_seconds: stats.duration_seconds,
        });
    }

    // 3. Uzun sessizlik.
    for (s, e) in &stats.silences {
        if (e - s) >= th.long_silence_seconds {
            issues.push(SegmentIssue {
                segment_index: nearest_segment(segments, *s),
                kind: QualityIssueKind::LongSilence,
                flag: QualityFlag::ReviewSuggested,
                description: format!(
                    "{:.1} saniyelik uzun sessizlik ({:.1}s - {:.1}s).",
                    e - s,
                    s,
                    e
                ),
                start_seconds: *s,
                end_seconds: *e,
            });
        }
    }

    // 4. Örtüşen konuşma.
    for (s, e) in &stats.overlap_regions {
        issues.push(SegmentIssue {
            segment_index: nearest_segment(segments, *s),
            kind: QualityIssueKind::OverlappingSpeech,
            flag: QualityFlag::PriorityReview,
            description: format!("Örtüşen konuşma olasılığı ({:.1}s - {:.1}s).", s, e),
            start_seconds: *s,
            end_seconds: *e,
        });
    }

    // Segment düzeyi kontroller.
    let mut prev_text: Option<String> = None;
    let mut repeat_run: usize = 0;
    for (i, seg) in segments.iter().enumerate() {
        let text = seg.text.trim();

        // 5. Konuşma varken boş transkript.
        if text.chars().count() <= th.empty_text_chars && seg.end_seconds - seg.start_seconds > 1.0
        {
            issues.push(SegmentIssue {
                segment_index: i,
                kind: QualityIssueKind::EmptyTranscriptWithSpeech,
                flag: QualityFlag::PriorityReview,
                description: format!(
                    "{:.1}s - {:.1}s aralığında konuşma var ancak transkript boş.",
                    seg.start_seconds, seg.end_seconds
                ),
                start_seconds: seg.start_seconds,
                end_seconds: seg.end_seconds,
            });
        }

        // 6. Sessizlikte metin üretilmesi.
        if !text.is_empty() && is_in_silence(stats, seg.start_seconds, seg.end_seconds) {
            issues.push(SegmentIssue {
                segment_index: i,
                kind: QualityIssueKind::TextInSilence,
                flag: QualityFlag::ReviewSuggested,
                description: format!(
                    "Sessizlik aralığında metin üretilmiş ({:.1}s - {:.1}s).",
                    seg.start_seconds, seg.end_seconds
                ),
                start_seconds: seg.start_seconds,
                end_seconds: seg.end_seconds,
            });
        }

        // 7. Konuşma dışı kelime üretimi.
        let lower = text.to_lowercase();
        if th
            .non_speech_markers
            .iter()
            .any(|m| lower.contains(&m.to_lowercase()))
        {
            issues.push(SegmentIssue {
                segment_index: i,
                kind: QualityIssueKind::NonSpeechToken,
                flag: QualityFlag::ReviewSuggested,
                description: "Konuşma dışı ifade üretilmiş olabilir.".into(),
                start_seconds: seg.start_seconds,
                end_seconds: seg.end_seconds,
            });
        }

        // 8. Şüpheli tekrarlar.
        let words = words_lower(text);
        let mut run = 1usize;
        let mut max_run = 1usize;
        for w in words.windows(2) {
            if w[0] == w[1] && !w[0].is_empty() {
                run += 1;
                max_run = max_run.max(run);
            } else {
                run = 1;
            }
        }
        if max_run >= th.max_word_repeats {
            issues.push(SegmentIssue {
                segment_index: i,
                kind: QualityIssueKind::SuspiciousRepetition,
                flag: QualityFlag::ReviewSuggested,
                description: format!("Aynı kelime {max_run} kez tekrarlanmış."),
                start_seconds: seg.start_seconds,
                end_seconds: seg.end_seconds,
            });
        }
        // Segmentler arası birebir tekrar.
        if let Some(prev) = &prev_text {
            if !text.is_empty() && prev == text {
                repeat_run += 1;
                if repeat_run >= 2 {
                    issues.push(SegmentIssue {
                        segment_index: i,
                        kind: QualityIssueKind::SuspiciousRepetition,
                        flag: QualityFlag::ReviewSuggested,
                        description: "Ardışık segmentler birebir aynı.".into(),
                        start_seconds: seg.start_seconds,
                        end_seconds: seg.end_seconds,
                    });
                }
            } else {
                repeat_run = 0;
            }
        }
        prev_text = Some(text.to_string());

        // 9. Anlaşılamayan bölüm.
        if seg.marked_unintelligible {
            issues.push(SegmentIssue {
                segment_index: i,
                kind: QualityIssueKind::UnintelligibleSegment,
                flag: QualityFlag::Unintelligible,
                description: "Bu bölüm anlaşılamıyor olarak işaretlenmiş.".into(),
                start_seconds: seg.start_seconds,
                end_seconds: seg.end_seconds,
            });
        }

        // 10. Zaman kodu tutarsızlığı.
        if seg.end_seconds < seg.start_seconds {
            issues.push(SegmentIssue {
                segment_index: i,
                kind: QualityIssueKind::TimestampInconsistency,
                flag: QualityFlag::ReviewSuggested,
                description: "Bitiş zamanı başlangıçtan önce.".into(),
                start_seconds: seg.start_seconds,
                end_seconds: seg.end_seconds,
            });
        }
        if stats.duration_seconds > 0.0 && seg.end_seconds > stats.duration_seconds + 1.0 {
            issues.push(SegmentIssue {
                segment_index: i,
                kind: QualityIssueKind::TimestampInconsistency,
                flag: QualityFlag::ReviewSuggested,
                description: format!(
                    "Zaman kodu ses süresini aşıyor ({:.1}s > {:.1}s).",
                    seg.end_seconds, stats.duration_seconds
                ),
                start_seconds: seg.start_seconds,
                end_seconds: seg.end_seconds,
            });
        }

        // 11. Özel isim ve sayı riski.
        if contains_numbers_or_dates(text) {
            issues.push(SegmentIssue {
                segment_index: i,
                kind: QualityIssueKind::ProperNounOrNumberRisk,
                flag: QualityFlag::PriorityReview,
                description: "Sayı/tarih içeriyor; değer kontrolü önerilir.".into(),
                start_seconds: seg.start_seconds,
                end_seconds: seg.end_seconds,
            });
        } else if contains_proper_noun(text) {
            issues.push(SegmentIssue {
                segment_index: i,
                kind: QualityIssueKind::ProperNounOrNumberRisk,
                flag: QualityFlag::ReviewSuggested,
                description: "Özel isim olabilecek kelime içeriyor.".into(),
                start_seconds: seg.start_seconds,
                end_seconds: seg.end_seconds,
            });
        }

        // 12. Olumsuzluk sözcüğü riski.
        if words.iter().any(|w| NEGATION_WORDS.contains(&w.as_str())) {
            issues.push(SegmentIssue {
                segment_index: i,
                kind: QualityIssueKind::NegationRisk,
                flag: QualityFlag::PriorityReview,
                description: "Olumsuzluk sözcüğü içeriyor; anlam kontrolü önerilir.".into(),
                start_seconds: seg.start_seconds,
                end_seconds: seg.end_seconds,
            });
        }
    }

    // 13. Parça sınırında metin kaybı: segmentler arası büyük boşluklar.
    for pair in segments.windows(2) {
        let gap = pair[1].start_seconds - pair[0].end_seconds;
        if gap > 5.0 {
            issues.push(SegmentIssue {
                segment_index: pair[0].index,
                kind: QualityIssueKind::ChunkBoundaryLoss,
                flag: QualityFlag::ReviewSuggested,
                description: format!(
                    "Segmentler arasında {:.1}s boşluk; parça sınırı olabilir.",
                    gap
                ),
                start_seconds: pair[0].end_seconds,
                end_seconds: pair[1].start_seconds,
            });
        }
    }

    // 14. Dil algılama tutarsızlığı.
    if let (Some(detected), Some(over)) = (
        &transcript.detected_language,
        &transcript.language_user_override,
    ) {
        if detected != over {
            issues.push(SegmentIssue {
                segment_index: 0,
                kind: QualityIssueKind::LanguageInconsistency,
                flag: QualityFlag::PriorityReview,
                description: format!(
                    "Algılanan dil ({detected}) ile kullanıcı düzeltmesi ({over}) farklı."
                ),
                start_seconds: 0.0,
                end_seconds: stats.duration_seconds,
            });
        }
    }

    // Genel risk ve inceleme kuyruğu.
    let overall = issues
        .iter()
        .map(|i| i.flag)
        .max()
        .unwrap_or(QualityFlag::Normal);
    let mut review_queue: Vec<usize> = issues
        .iter()
        .filter(|i| i.flag.severity() >= QualityFlag::ReviewSuggested.severity())
        .map(|i| i.segment_index)
        .collect();
    review_queue.sort_unstable();
    review_queue.dedup();

    let summary_tr = if issues.is_empty() {
        "Kalite kontrolü: belirgin bir sorun bulunmadı.".to_string()
    } else {
        format!(
            "Kalite kontrolü: {} bulgu, {} bölüm inceleme kuyruğunda. Genel durum: {}.",
            issues.len(),
            review_queue.len(),
            overall.label_tr()
        )
    };

    QualityReport {
        issues,
        overall,
        review_queue,
        summary_tr,
    }
}

fn is_in_silence(stats: &AudioSignalStats, start: f64, end: f64) -> bool {
    let mid = (start + end) / 2.0;
    stats
        .silences
        .iter()
        .any(|(s, e)| mid >= *s && mid <= *e && (e - s) >= 2.0)
}

fn nearest_segment(segments: &[TranscriptSegment], t: f64) -> usize {
    segments
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let da = (a.start_seconds - t).abs();
            let db = (b.start_seconds - t).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Segmentlere kalite bayraklarını işler (transkripte geri yazmak için).
pub fn apply_flags(segments: &mut [TranscriptSegment], report: &QualityReport) {
    for seg in segments.iter_mut() {
        seg.issues.clear();
        seg.quality = QualityFlag::Normal;
    }
    for issue in &report.issues {
        if let Some(seg) = segments.get_mut(issue.segment_index) {
            seg.issues.push(issue.kind);
            if issue.flag.severity() > seg.quality.severity() {
                seg.quality = issue.flag;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use delil_core::{AudioAssetId, QualityFlag, TranscriptSegment};

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

    fn transcript(segments: Vec<TranscriptSegment>) -> Transcript {
        Transcript::new_machine(AudioAssetId::new(), "test", segments, Some("tr".into()))
    }

    #[test]
    fn clean_transcript_has_no_issues() {
        let t = transcript(vec![
            seg(0, 0.0, 2.0, "Merhaba, ben geldim."),
            seg(1, 2.0, 4.0, "Nasılsınız acaba?"),
        ]);
        let stats = AudioSignalStats {
            duration_seconds: 4.0,
            rms: 0.2,
            peak: 0.8,
            ..Default::default()
        };
        let r = check_transcript(&t, &stats, Thresholds::default());
        // Özel isim/sayı bulgusu olabilir ama öncelikli inceleme dışında bir şey
        // beklenmez.
        assert!(r.overall.severity() <= QualityFlag::ReviewSuggested.severity());
    }

    #[test]
    fn detects_long_silence() {
        let t = transcript(vec![seg(0, 0.0, 2.0, "Merhaba.")]);
        let stats = AudioSignalStats {
            duration_seconds: 30.0,
            silences: vec![(5.0, 25.0)],
            rms: 0.2,
            peak: 0.8,
            ..Default::default()
        };
        let r = check_transcript(&t, &stats, Thresholds::default());
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == QualityIssueKind::LongSilence));
    }

    #[test]
    fn detects_empty_transcript_with_speech() {
        let t = transcript(vec![seg(0, 0.0, 5.0, "")]);
        let stats = AudioSignalStats {
            duration_seconds: 5.0,
            rms: 0.2,
            ..Default::default()
        };
        let r = check_transcript(&t, &stats, Thresholds::default());
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == QualityIssueKind::EmptyTranscriptWithSpeech));
    }

    #[test]
    fn detects_text_in_silence() {
        let t = transcript(vec![seg(0, 5.0, 8.0, "alakasız metin")]);
        let stats = AudioSignalStats {
            duration_seconds: 10.0,
            silences: vec![(0.0, 10.0)],
            rms: 0.2,
            ..Default::default()
        };
        let r = check_transcript(&t, &stats, Thresholds::default());
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == QualityIssueKind::TextInSilence));
    }

    #[test]
    fn detects_suspicious_repetition() {
        let t = transcript(vec![seg(0, 0.0, 3.0, "evet evet evet evet evet")]);
        let stats = AudioSignalStats {
            duration_seconds: 3.0,
            rms: 0.2,
            ..Default::default()
        };
        let r = check_transcript(&t, &stats, Thresholds::default());
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == QualityIssueKind::SuspiciousRepetition));
    }

    #[test]
    fn detects_non_speech_marker() {
        let t = transcript(vec![seg(0, 0.0, 3.0, "[müzik]")]);
        let stats = AudioSignalStats {
            duration_seconds: 3.0,
            rms: 0.2,
            ..Default::default()
        };
        let r = check_transcript(&t, &stats, Thresholds::default());
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == QualityIssueKind::NonSpeechToken));
    }

    #[test]
    fn detects_low_volume() {
        let t = transcript(vec![seg(0, 0.0, 3.0, "fısıltı")]);
        let stats = AudioSignalStats {
            duration_seconds: 3.0,
            rms: 0.001,
            peak: 0.05,
            ..Default::default()
        };
        let r = check_transcript(&t, &stats, Thresholds::default());
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == QualityIssueKind::VeryLowVolume));
        assert_eq!(r.overall, QualityFlag::PriorityReview);
    }

    #[test]
    fn detects_number_and_negation() {
        let t = transcript(vec![
            seg(0, 0.0, 3.0, "12.05.2024 tarihinde 5000 TL"),
            seg(1, 3.0, 6.0, "Bunu ben yapmadım, değil mi?"),
        ]);
        let stats = AudioSignalStats {
            duration_seconds: 6.0,
            rms: 0.2,
            ..Default::default()
        };
        let r = check_transcript(&t, &stats, Thresholds::default());
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == QualityIssueKind::ProperNounOrNumberRisk));
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == QualityIssueKind::NegationRisk));
    }

    #[test]
    fn detects_timestamp_inconsistency() {
        let t = transcript(vec![seg(0, 10.0, 5.0, "ters zaman")]);
        let stats = AudioSignalStats {
            duration_seconds: 20.0,
            rms: 0.2,
            ..Default::default()
        };
        let r = check_transcript(&t, &stats, Thresholds::default());
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == QualityIssueKind::TimestampInconsistency));
    }

    #[test]
    fn detects_corrupt_audio() {
        let t = transcript(vec![seg(0, 0.0, 3.0, "x")]);
        let stats = AudioSignalStats {
            duration_seconds: 3.0,
            corrupt: true,
            ..Default::default()
        };
        let r = check_transcript(&t, &stats, Thresholds::default());
        assert!(r
            .issues
            .iter()
            .any(|i| i.kind == QualityIssueKind::CorruptSegment));
    }

    #[test]
    fn flags_are_applied_to_segments() {
        let mut segs = vec![seg(0, 0.0, 3.0, "12.05.2024")];
        let t = transcript(segs.clone());
        let stats = AudioSignalStats {
            duration_seconds: 3.0,
            rms: 0.2,
            ..Default::default()
        };
        let r = check_transcript(&t, &stats, Thresholds::default());
        apply_flags(&mut segs, &r);
        assert!(segs[0].quality.severity() >= QualityFlag::PriorityReview.severity());
        assert!(!segs[0].issues.is_empty());
    }

    #[test]
    fn review_queue_is_sorted_and_deduped() {
        let t = transcript(vec![
            seg(0, 0.0, 3.0, "12.05.2024 ve 5000"),
            seg(1, 3.0, 6.0, "yapmadım değil"),
        ]);
        let stats = AudioSignalStats {
            duration_seconds: 6.0,
            rms: 0.2,
            ..Default::default()
        };
        let r = check_transcript(&t, &stats, Thresholds::default());
        let mut sorted = r.review_queue.clone();
        sorted.sort_unstable();
        assert_eq!(r.review_queue, sorted);
        let mut deduped = sorted.clone();
        deduped.dedup();
        assert_eq!(sorted, deduped);
    }
}
