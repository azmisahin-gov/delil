//! Transkripsiyon motoru soyutlaması.
//!
//! `TranscriptionEngine` trait'i, gerçek Whisper motoru ile test amaçlı
//! motorların aynı arayüzü kullanmasını sağlar. Böylece kuyruk ve kalite
//! kontrolü, ağır model olmadan da test edilebilir.

use crate::{Result, SAMPLE_RATE};
use serde::{Deserialize, Serialize};

/// Motorun ürettiği tek bir segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
    /// Ortalama log-olasılık (ham; doğruluk yüzdesi değildir).
    pub avg_logprob: Option<f64>,
    /// Konuşma yok olasılığı (ham).
    pub no_speech_prob: Option<f64>,
}

/// Transkripsiyon seçenekleri. Kullanıcıdan bu parametreleri istemek yerine
/// uygulama bunları otomatik yönetir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionOptions {
    /// Dil ipucu (ISO 639-1). `None` ise otomatik algılanır.
    pub language: Option<String>,
    /// Zaman kodu üretilsin mi?
    pub timestamps: bool,
    /// Çeviri yerine transkripsiyon (her zaman true olmalı).
    pub task_transcribe: bool,
    /// Azami parça uzunluğu (saniye).
    pub max_chunk_seconds: usize,
    /// Sıcaklık geri düşüşü uygulansın mı (kalite için).
    pub temperature_fallback: bool,
}

impl Default for TranscriptionOptions {
    fn default() -> Self {
        Self {
            language: Some("tr".to_string()),
            timestamps: true,
            task_transcribe: true,
            max_chunk_seconds: 30,
            temperature_fallback: true,
        }
    }
}

/// Transkripsiyon sonucu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub segments: Vec<Segment>,
    pub detected_language: Option<String>,
    /// Ses süresi (saniye).
    pub duration_seconds: f64,
    /// Ortalama sinyal seviyesi (RMS) — kalite kontrolü için.
    pub rms: f32,
    pub peak: f32,
    /// Ses çözülemedi mi?
    pub corrupt: bool,
}

impl TranscriptionResult {
    /// Tüm segmentlerin metnini birleştirir (araya boşluk koyarak).
    pub fn full_text(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.text.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// İptal/duraklatma bayrağı. Uzun işlemler bu bayrağı düzenli aralıklarla
/// kontrol eder.
#[derive(Debug, Clone, Default)]
pub struct ControlFlag {
    inner: std::sync::Arc<std::sync::atomic::AtomicU8>,
}

/// Kontrol durumu değerleri.
pub const CONTROL_RUN: u8 = 0;
pub const CONTROL_PAUSE: u8 = 1;
pub const CONTROL_CANCEL: u8 = 2;

impl ControlFlag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pause(&self) {
        self.inner
            .store(CONTROL_PAUSE, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.inner
            .store(CONTROL_RUN, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn cancel(&self) {
        self.inner
            .store(CONTROL_CANCEL, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.load(std::sync::atomic::Ordering::SeqCst) == CONTROL_CANCEL
    }

    pub fn is_paused(&self) -> bool {
        self.inner.load(std::sync::atomic::Ordering::SeqCst) == CONTROL_PAUSE
    }

    /// Duraklatma bitene kadar bekler; iptal edilirse hata döner.
    pub fn wait_if_paused(&self) -> Result<()> {
        while self.is_paused() {
            if self.is_cancelled() {
                return Err(crate::TranscriptionError::Cancelled);
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if self.is_cancelled() {
            return Err(crate::TranscriptionError::Cancelled);
        }
        Ok(())
    }
}

/// İlerleme geri çağrısı: (tamamlanan saniye, toplam saniye).
pub type EngineProgress<'a> = &'a mut dyn FnMut(f64, f64);

/// Transkripsiyon motoru arayüzü.
pub trait TranscriptionEngine {
    /// Motorun adı (ör. "whisper-large-v3-turbo").
    fn name(&self) -> &str;
    /// Model sürümü.
    fn version(&self) -> &str;
    /// Motor hazır mı (model yüklendi mi)?
    fn is_ready(&self) -> bool;

    /// 16 kHz mono f32 örnekleri metne dönüştürür.
    fn transcribe(
        &mut self,
        samples: &[f32],
        options: &TranscriptionOptions,
        control: &ControlFlag,
        progress: Option<EngineProgress<'_>>,
    ) -> Result<TranscriptionResult>;
}

/// 16 kHz mono f32 örneklerden sinyal istatistikleri çıkarır (kalite kontrolü
/// ve sessizlik tespiti için). Model gerektirmez.
pub fn analyze_signal(samples: &[f32]) -> SignalStats {
    if samples.is_empty() {
        return SignalStats::default();
    }
    let mut sum_sq = 0f64;
    let mut peak = 0f32;
    for &s in samples {
        sum_sq += (s as f64) * (s as f64);
        peak = peak.max(s.abs());
    }
    let rms = (sum_sq / samples.len() as f64).sqrt() as f32;
    SignalStats {
        rms,
        peak,
        duration_seconds: samples.len() as f64 / SAMPLE_RATE as f64,
    }
}

/// Sinyal istatistikleri.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SignalStats {
    pub rms: f32,
    pub peak: f32,
    pub duration_seconds: f64,
}

/// Örneklerdeki sessizlik aralıklarını kaba biçimde tespit eder (RMS pencere
/// yöntemi). Ses sinyali işleme model gerektirmez.
pub fn detect_silences(samples: &[f32], window_seconds: f64, threshold: f32) -> Vec<(f64, f64)> {
    let win = (SAMPLE_RATE as f64 * window_seconds) as usize;
    if win == 0 || samples.len() < win {
        return Vec::new();
    }
    let mut silences = Vec::new();
    let mut start: Option<usize> = None;
    let mut i = 0;
    while i + win <= samples.len() {
        let mut sum = 0f64;
        for &s in &samples[i..i + win] {
            sum += (s as f64) * (s as f64);
        }
        let rms = (sum / win as f64).sqrt() as f32;
        if rms < threshold {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            silences.push((
                s as f64 / SAMPLE_RATE as f64,
                i as f64 / SAMPLE_RATE as f64,
            ));
        }
        i += win;
    }
    if let Some(s) = start {
        silences.push((
            s as f64 / SAMPLE_RATE as f64,
            samples.len() as f64 / SAMPLE_RATE as f64,
        ));
    }
    silences
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_stats_of_silence() {
        let samples = vec![0f32; 16000];
        let s = analyze_signal(&samples);
        assert_eq!(s.rms, 0.0);
        assert_eq!(s.peak, 0.0);
        assert!((s.duration_seconds - 1.0).abs() < 0.001);
    }

    #[test]
    fn signal_stats_of_tone() {
        let samples: Vec<f32> = (0..16000)
            .map(|i| (i as f32 * 0.1).sin() * 0.5)
            .collect();
        let s = analyze_signal(&samples);
        assert!(s.rms > 0.3);
        assert!(s.peak > 0.4);
    }

    #[test]
    fn detects_silence_region() {
        let mut samples = vec![0.5f32; 16000 * 2];
        // 2. ve 3. saniyeler sessiz.
        for s in samples.iter_mut().skip(16000).take(16000) {
            *s = 0.0;
        }
        let silences = detect_silences(&samples, 0.5, 0.01);
        assert!(!silences.is_empty());
        assert!(silences.iter().any(|(s, e)| *s >= 0.9 && *e <= 3.1));
    }

    #[test]
    fn control_flag_cancel() {
        let c = ControlFlag::new();
        assert!(!c.is_cancelled());
        c.cancel();
        assert!(c.is_cancelled());
        assert!(c.wait_if_paused().is_err());
    }

    #[test]
    fn control_flag_pause_resume() {
        let c = ControlFlag::new();
        c.pause();
        assert!(c.is_paused());
        c.resume();
        assert!(!c.is_paused());
        assert!(c.wait_if_paused().is_ok());
    }

    #[test]
    fn full_text_joins_segments() {
        let r = TranscriptionResult {
            segments: vec![
                Segment {
                    start: 0.0,
                    end: 1.0,
                    text: "Merhaba".into(),
                    avg_logprob: None,
                    no_speech_prob: None,
                },
                Segment {
                    start: 1.0,
                    end: 2.0,
                    text: "dünya".into(),
                    avg_logprob: None,
                    no_speech_prob: None,
                },
            ],
            detected_language: Some("tr".into()),
            duration_seconds: 2.0,
            rms: 0.2,
            peak: 0.5,
            corrupt: false,
        };
        assert_eq!(r.full_text(), "Merhaba dünya");
    }
}
