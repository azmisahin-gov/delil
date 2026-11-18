//! Yerel Whisper transkripsiyon motoru ve otomatik işlem kuyruğu
//! (şartname 8).
//!
//! Motor tamamen yereldir ve çevrimdışı çalışır. Bulut ASR veya çevrimiçi
//! yapay zekâ API'si kullanılmaz.
//!
//! Bu crate iki katmandan oluşur:
//! * [`engine`] — `TranscriptionEngine` soyutlaması ve (özellik etkinse)
//!   Candle tabanlı Whisper motoru.
//! * [`queue`] — kesintiden devam edebilen, duraklatılabilir, iptal
//!   edilebilir otomatik işlem kuyruğu.

pub mod benchmark;
pub mod engine;
pub mod queue;

pub use benchmark::{compute_cer, compute_wer, BenchmarkReport, EvaluationResult, WerReport};
pub use engine::{
    Segment as EngineSegment, TranscriptionEngine, TranscriptionOptions, TranscriptionResult,
};
pub use queue::{Queue, QueueItem, QueueOutcome, QueuePersist};

#[cfg(feature = "candle")]
pub mod candle_whisper;

use thiserror::Error;

/// Transkripsiyon hataları.
#[derive(Debug, Error)]
pub enum TranscriptionError {
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("Ses çözme hatası: {0}")]
    Decode(String),
    #[error("Model yüklenemedi: {0}")]
    ModelLoad(String),
    #[error("Model dosyaları eksik: {0}")]
    ModelMissing(String),
    #[error("Çıkarım hatası: {0}")]
    Inference(String),
    #[error("İşlem iptal edildi")]
    Cancelled,
    #[error("İşlem duraklatıldı")]
    Paused,
    #[error("JSON hatası: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, TranscriptionError>;

/// Whisper hedef örnekleme frekansı.
pub const SAMPLE_RATE: usize = 16_000;
/// Bir parçanın azami süresi (saniye) — modelin 30 saniyelik penceresi.
pub const CHUNK_SECONDS: usize = 30;

/// Whisper model dosya adları.
pub const MODEL_CONFIG_FILE: &str = "config.json";
pub const MODEL_TOKENIZER_FILE: &str = "tokenizer.json";
pub const MODEL_WEIGHTS_FILE: &str = "model.safetensors";

/// Model dosyalarının eksiksiz olup olmadığını kontrol eder.
///
/// Bu işlev özellik bayrağından bağımsızdır; model dizini verilmediğinde veya
/// dosyalar eksik olduğunda açık bir hata döndürür. Model **simülasyonu
/// yapılmaz**.
pub fn check_model_files(dir: &std::path::Path) -> Result<()> {
    for f in [MODEL_CONFIG_FILE, MODEL_TOKENIZER_FILE, MODEL_WEIGHTS_FILE] {
        let p = dir.join(f);
        if !p.exists() {
            return Err(TranscriptionError::ModelMissing(format!(
                "{} bulunamadı ({})",
                f,
                p.display()
            )));
        }
    }
    Ok(())
}

/// Bir ses dosyasını 16 kHz mono f32 örneklemeye çözer.
///
/// Önce yerleşik WAV çözücü denenir (adli ses materyalinde en yaygın biçim).
/// Başarısız olursa, `decode` özelliği etkinse Symphonia tabanlı çözücü
/// kullanılır. Böylece MP3/FLAC/M4A gibi biçimler de desteklenir.
pub fn decode_audio_file(path: &std::path::Path) -> Result<Vec<f32>> {
    if let Ok(samples) = decode_wav_pcm(path) {
        return Ok(samples);
    }
    #[cfg(feature = "decode")]
    {
        let d = delil_audio::decode::decode_to_16k_mono(path)
            .map_err(|e| TranscriptionError::Decode(e.to_string()))?;
        return Ok(d.samples);
    }
    #[cfg(not(feature = "decode"))]
    {
        Err(TranscriptionError::Decode(format!(
            "{} biçimi bu derlemede çözülemiyor; Symphonia desteği gerekli.",
            path.display()
        )))
    }
}

/// Yerleşik WAV (RIFF/WAVE) PCM çözücü: 16 kHz mono f32 üretir.
///
/// Yalnızca PCM (format 1) ve IEEE float (format 3) desteklenir; diğer
/// durumlarda hata döner ve üst katman Symphonia'ya düşer.
pub fn decode_wav_pcm(path: &std::path::Path) -> Result<Vec<f32>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(TranscriptionError::Decode("WAV değil".into()));
    }
    let mut pos = 12usize;
    let mut fmt: Option<(u16, u16, u32, u16)> = None; // (format, channels, rate, bits)
    let mut data: Option<&[u8]> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size =
            u32::from_le_bytes([bytes[pos + 4], bytes[pos + 5], bytes[pos + 6], bytes[pos + 7]])
                as usize;
        let body = pos + 8;
        if body + size > bytes.len() {
            break;
        }
        match id {
            b"fmt " => {
                if size >= 16 {
                    let format = u16::from_le_bytes([bytes[body], bytes[body + 1]]);
                    let channels = u16::from_le_bytes([bytes[body + 2], bytes[body + 3]]);
                    let rate = u32::from_le_bytes([
                        bytes[body + 4],
                        bytes[body + 5],
                        bytes[body + 6],
                        bytes[body + 7],
                    ]);
                    let bits = u16::from_le_bytes([bytes[body + 14], bytes[body + 15]]);
                    fmt = Some((format, channels, rate, bits));
                }
            }
            b"data" => data = Some(&bytes[body..body + size]),
            _ => {}
        }
        pos = body + size + (size & 1);
    }
    let (format, channels, rate, bits) = fmt.ok_or_else(|| {
        TranscriptionError::Decode("WAV fmt chunk bulunamadı".into())
    })?;
    let data = data.ok_or_else(|| TranscriptionError::Decode("WAV data chunk bulunamadı".into()))?;
    if channels == 0 || rate == 0 {
        return Err(TranscriptionError::Decode("geçersiz WAV başlığı".into()));
    }

    // Örnekleri f32'ye çevir.
    let mut mono: Vec<f32> = Vec::new();
    let ch = channels as usize;
    match (format, bits) {
        (1, 16) => {
            let frames = data.len() / (2 * ch);
            for f in 0..frames {
                let mut acc = 0f32;
                for c in 0..ch {
                    let i = (f * ch + c) * 2;
                    let s = i16::from_le_bytes([data[i], data[i + 1]]);
                    acc += s as f32 / 32768.0;
                }
                mono.push(acc / ch as f32);
            }
        }
        (1, 8) => {
            let frames = data.len() / ch;
            for f in 0..frames {
                let mut acc = 0f32;
                for c in 0..ch {
                    let s = data[f * ch + c];
                    acc += (s as f32 - 128.0) / 128.0;
                }
                mono.push(acc / ch as f32);
            }
        }
        (1, 32) => {
            let frames = data.len() / (4 * ch);
            for f in 0..frames {
                let mut acc = 0f32;
                for c in 0..ch {
                    let i = (f * ch + c) * 4;
                    let s = i32::from_le_bytes([
                        data[i],
                        data[i + 1],
                        data[i + 2],
                        data[i + 3],
                    ]);
                    acc += s as f32 / 2147483648.0;
                }
                mono.push(acc / ch as f32);
            }
        }
        (3, 32) => {
            let frames = data.len() / (4 * ch);
            for f in 0..frames {
                let mut acc = 0f32;
                for c in 0..ch {
                    let i = (f * ch + c) * 4;
                    acc += f32::from_le_bytes([
                        data[i],
                        data[i + 1],
                        data[i + 2],
                        data[i + 3],
                    ]);
                }
                mono.push(acc / ch as f32);
            }
        }
        _ => {
            return Err(TranscriptionError::Decode(format!(
                "desteklenmeyen WAV biçimi: format={format}, bits={bits}"
            )))
        }
    }

    // 16 kHz'e yeniden örnekle.
    if rate == SAMPLE_RATE as u32 {
        Ok(mono)
    } else {
        Ok(resample_linear(&mono, rate, SAMPLE_RATE as u32))
    }
}

/// Basit doğrusal yeniden örnekleme.
pub fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = to as f64 / from as f64;
    let out_len = ((input.len() as f64) * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 / ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let a = input.get(idx).copied().unwrap_or(0.0);
        let b = input.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

/// Metni zaman koduyla cümle/segment düzeyinde birleştirmek için yardımcı.
///
/// Uzun kayıtlar 30 saniyelik parçalara ayrılır; parça sınırında cümle/kelime
/// kaybı olmaması için segmentler birleştirilirken metin kırpılmaz.
pub fn merge_segments(
    mut segments: Vec<EngineSegment>,
) -> Vec<EngineSegment> {
    segments.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_keeps_all_segments() {
        let segs = vec![
            EngineSegment {
                start: 1.0,
                end: 2.0,
                text: "b".into(),
                avg_logprob: None,
                no_speech_prob: None,
            },
            EngineSegment {
                start: 0.0,
                end: 1.0,
                text: "a".into(),
                avg_logprob: None,
                no_speech_prob: None,
            },
        ];
        let merged = merge_segments(segs);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].text, "a");
        assert_eq!(merged[1].text, "b");
    }
}
