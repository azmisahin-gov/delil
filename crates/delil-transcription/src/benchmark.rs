//! Türkçe transkripsiyon değerlendirmesi: WER/CER ve hata sınıfları
//! (şartname 19.1).
//!
//! Bu modül, insan tarafından doğrulanmış bir referans metin ile motor
//! çıktısını karşılaştırır. Sonuçlar **kesin doğruluk garantisi olarak**
//! sunulmaz; yalnızca ölçüm ve izleme amaçlıdır.

use serde::{Deserialize, Serialize};

/// Kelime hatası oranı ve diğer ölçümler.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WerReport {
    pub reference_words: usize,
    pub substitutions: usize,
    pub deletions: usize,
    pub insertions: usize,
    /// WER = (S + D + I) / N
    pub wer: f64,
    /// Karakter hatası oranı.
    pub cer: f64,
}

/// Ayrı değerlendirilen hata sınıfları (şartname 19.1).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ErrorClasses {
    /// Özel isim hataları (referansta büyük harfle başlayan kelimeler).
    pub proper_noun_errors: usize,
    /// Sayı/tarih/tutar hataları.
    pub number_errors: usize,
    /// Olumsuzluk sözcüğü hataları.
    pub negation_errors: usize,
    /// Referansta olmayan fazladan metin (halüsinasyon).
    pub hallucinated_words: usize,
    /// Sessizlikte üretilen metin (referans boşken çıktı var).
    pub text_in_silence: usize,
    /// Zaman kodu tutarsızlıkları.
    pub timestamp_errors: usize,
}

/// Tam değerlendirme sonucu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub wer: WerReport,
    pub classes: ErrorClasses,
}

const NEGATIONS: &[&str] = &[
    "değil", "değildi", "yok", "yoktu", "asla", "hiçbir", "olmaz", "olmadı", "etmez", "etmedi",
    "yapmadı", "yapmaz", "hayır",
];

/// Metni normalize edip kelimelere ayırır (noktalama ve büyük/küçük harf
/// farkını yok sayar, Türkçe küçük harfe çevirir).
pub fn normalize_words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            // Önce küçük harfe çevir (Türkçe 'İ' -> "i̇"); sonra yalnızca
            // alfanümerik karakterleri tut (birleştirici nokta işareti böylece
            // atılır).
            w.to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Karakter dizisi (boşluklar hariç) üretir.
pub fn normalize_chars(text: &str) -> Vec<char> {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Levenshtein düzenleme mesafesi.
fn edit_distance<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    let n = a.len();
    let m = b.len();
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0usize; m + 1];
    for i in 1..=n {
        cur[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[m]
}

/// WER hesaplar. `reference` insan tarafından doğrulanmış metin, `hypothesis`
/// motor çıktısıdır.
pub fn compute_wer(reference: &str, hypothesis: &str) -> WerReport {
    let r = normalize_words(reference);
    let h = normalize_words(hypothesis);
    let n = r.len();
    let dist = edit_distance(&r, &h);
    // Basitleştirilmiş ayrıştırma: mesafeyi S+D+I olarak raporlarız; tam
    // ayrıştırma için hizalamayı ayrıca izleriz.
    let (s, d, ins) = align_counts(&r, &h);
    WerReport {
        reference_words: n,
        substitutions: s,
        deletions: d,
        insertions: ins,
        wer: if n == 0 {
            if h.is_empty() { 0.0 } else { 1.0 }
        } else {
            dist as f64 / n as f64
        },
        cer: compute_cer(reference, hypothesis),
    }
}

/// Karakter hatası oranı.
pub fn compute_cer(reference: &str, hypothesis: &str) -> f64 {
    let r = normalize_chars(reference);
    let h = normalize_chars(hypothesis);
    if r.is_empty() {
        return if h.is_empty() { 0.0 } else { 1.0 };
    }
    edit_distance(&r, &h) as f64 / r.len() as f64
}

/// Hizalamadan S/D/I sayılarını çıkarır (basit dinamik programlama geri izleme).
fn align_counts(r: &[String], h: &[String]) -> (usize, usize, usize) {
    let n = r.len();
    let m = h.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in 0..=n {
        dp[i][0] = i;
    }
    for j in 0..=m {
        dp[0][j] = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = if r[i - 1] == h[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j - 1] + cost)
                .min(dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1);
        }
    }
    // Geri izleme.
    let (mut i, mut j) = (n, m);
    let (mut s, mut d, mut ins) = (0, 0, 0);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 {
            let cost = if r[i - 1] == h[j - 1] { 0 } else { 1 };
            if dp[i][j] == dp[i - 1][j - 1] + cost {
                if cost == 1 {
                    s += 1;
                }
                i -= 1;
                j -= 1;
                continue;
            }
        }
        if i > 0 && dp[i][j] == dp[i - 1][j] + 1 {
            d += 1;
            i -= 1;
        } else if j > 0 && dp[i][j] == dp[i][j - 1] + 1 {
            ins += 1;
            j -= 1;
        } else {
            break;
        }
    }
    (s, d, ins)
}

/// Hata sınıflarını değerlendirir.
pub fn evaluate_classes(reference: &str, hypothesis: &str, reference_silent: bool) -> ErrorClasses {
    let r = normalize_words(reference);
    let h = normalize_words(hypothesis);
    let rset: std::collections::HashSet<&String> = r.iter().collect();
    let hset: std::collections::HashSet<&String> = h.iter().collect();

    let mut c = ErrorClasses::default();

    // Özel isim: referansta büyük harfle başlayan kelimeler.
    let proper: Vec<&str> = reference
        .split_whitespace()
        .filter(|w| w.chars().next().map(|ch| ch.is_uppercase()).unwrap_or(false))
        .collect();
    for p in proper {
        let pn = normalize_words(p);
        if let Some(w) = pn.first() {
            if !hset.contains(w) {
                c.proper_noun_errors += 1;
            }
        }
    }

    // Sayı/tarih/tutar.
    for w in reference.split_whitespace() {
        if w.chars().any(|ch| ch.is_ascii_digit()) {
            let nw = normalize_words(w);
            if let Some(x) = nw.first() {
                if !hset.contains(x) {
                    c.number_errors += 1;
                }
            }
        }
    }

    // Olumsuzluk (Türkçe ekler: "yapmadım", "değildir" vb. kök eşleşmesi).
    for w in &r {
        let is_neg = NEGATIONS.iter().any(|neg| w == neg || w.starts_with(neg));
        if is_neg {
            let present = h.iter().any(|hw| hw == w || hw.starts_with(w) || w.starts_with(hw.as_str()));
            if !present {
                c.negation_errors += 1;
            }
        }
    }

    // Halüsinasyon: çıktıda olup referansta olmayan kelimeler.
    c.hallucinated_words = h.iter().filter(|w| !rset.contains(*w)).count();

    // Sessizlikte metin.
    if reference_silent && !h.is_empty() {
        c.text_in_silence = h.len();
    }

    c
}

/// Tek bir test kaydının sonucu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleResult {
    pub name: String,
    pub kind: String,
    pub evaluation: EvaluationResult,
}

/// Bir test seti üzerinde toplu değerlendirme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub model_name: String,
    pub model_version: String,
    pub samples: Vec<SampleResult>,
    pub mean_wer: f64,
    pub mean_cer: f64,
    pub note: String,
}

impl BenchmarkReport {
    /// Örneklerden özet rapor üretir.
    pub fn from_samples(
        model_name: &str,
        model_version: &str,
        samples: Vec<SampleResult>,
    ) -> Self {
        let n = samples.len().max(1) as f64;
        let mean_wer = samples.iter().map(|s| s.evaluation.wer.wer).sum::<f64>() / n;
        let mean_cer = samples.iter().map(|s| s.evaluation.wer.cer).sum::<f64>() / n;
        Self {
            model_name: model_name.to_string(),
            model_version: model_version.to_string(),
            samples,
            mean_wer,
            mean_cer,
            note: "Bu ölçümler yalnızca izleme amaçlıdır; kesin doğruluk garantisi \
                   olarak yorumlanamaz. Model güncellemeleri aynı test setiyle \
                   yeniden sınanmalıdır."
                .to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_match_has_zero_error() {
        let w = compute_wer("merhaba dünya nasılsın", "merhaba dünya nasılsın");
        assert_eq!(w.wer, 0.0);
        assert_eq!(w.cer, 0.0);
        assert_eq!(w.substitutions + w.deletions + w.insertions, 0);
    }

    #[test]
    fn substitution_detected() {
        let w = compute_wer("merhaba dünya", "merhaba evren");
        assert_eq!(w.reference_words, 2);
        assert_eq!(w.substitutions, 1);
        assert!((w.wer - 0.5).abs() < 0.001);
    }

    #[test]
    fn deletion_detected() {
        let w = compute_wer("bir iki üç", "bir üç");
        assert_eq!(w.deletions, 1);
        assert!((w.wer - 1.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn insertion_detected() {
        let w = compute_wer("bir iki", "bir iki üç");
        assert_eq!(w.insertions, 1);
    }

    #[test]
    fn turkish_case_normalization() {
        // İ/I normalizasyonu: büyük İ küçük i'ye eşitlenmeli.
        let w = compute_wer("İstanbul güzel", "istanbul güzel");
        assert_eq!(w.wer, 0.0);
    }

    #[test]
    fn cer_computation() {
        let cer = compute_cer("merhaba", "merhab");
        assert!((cer - 1.0 / 7.0).abs() < 0.01);
    }

    #[test]
    fn proper_noun_errors_counted() {
        let c = evaluate_classes("Ahmet Yılmaz geldi", "ahmet geldi", false);
        assert!(c.proper_noun_errors >= 1);
    }

    #[test]
    fn number_errors_counted() {
        let c = evaluate_classes("5000 TL ödedi", "bin TL ödedi", false);
        assert!(c.number_errors >= 1);
    }

    #[test]
    fn negation_errors_counted() {
        let c = evaluate_classes("ben yapmadım", "ben yaptım", false);
        assert!(c.negation_errors >= 1);
    }

    #[test]
    fn hallucination_counted() {
        let c = evaluate_classes("kısa bir cümle", "kısa bir cümle ekstra kelimeler burada", false);
        assert!(c.hallucinated_words >= 3);
    }

    #[test]
    fn text_in_silence_counted() {
        let c = evaluate_classes("", "sessizlikte üretilen metin", true);
        assert!(c.text_in_silence >= 1);
    }

    #[test]
    fn benchmark_aggregates_means() {
        let samples = vec![
            SampleResult {
                name: "a".into(),
                kind: "temiz".into(),
                evaluation: EvaluationResult {
                    wer: compute_wer("bir iki üç", "bir iki üç"),
                    classes: ErrorClasses::default(),
                },
            },
            SampleResult {
                name: "b".into(),
                kind: "gürültülü".into(),
                evaluation: EvaluationResult {
                    wer: compute_wer("bir iki üç dört", "bir iki"),
                    classes: ErrorClasses::default(),
                },
            },
        ];
        let r = BenchmarkReport::from_samples("whisper-large-v3-turbo", "sabit", samples);
        assert!(r.mean_wer > 0.0);
        assert!(r.note.contains("garanti"));
    }

    #[test]
    fn empty_reference_is_handled() {
        let w = compute_wer("", "bir şey");
        assert_eq!(w.wer, 1.0);
        let w2 = compute_wer("", "");
        assert_eq!(w2.wer, 0.0);
    }
}
