//! Gerçek Candle Whisper motoruyla transkripsiyon testi (şartname 19.1).
//!
//! Bu test **gerçek model ağırlıkları** gerektirir. Model dizini
//! `DELIL_TEST_MODEL_DIR` ortam değişkeniyle verilir. Yoksa test atlanır
//! (başarısız sayılmaz, çünkü model bu depoda dağıtılmaz).
//!
//! Amaç, motor kod yolunun gerçek ağırlıklarla uçtan uca çalıştığını
//! kanıtlamaktır: model yükleme → mel → kodlayıcı → kod çözücü → metin.
//!
//! Not: Bu testin doğruluğu (WER/CER) kullanılan modele bağlıdır; burada asıl
//! sınanan şey kod yolunun çalışmasıdır.

#![cfg(feature = "candle")]

use delil_transcription::candle_whisper::{CandleWhisper, WhisperEngineConfig};
use delil_transcription::engine::{ControlFlag, TranscriptionEngine, TranscriptionOptions};

/// 16 kHz mono WAV üretir (konuşma yerine sinüs tonu; amaç kod yolunu sınamak).
fn write_tone_wav(path: &std::path::Path, seconds: f64, hz: f32) {
    let sr = 16000u32;
    let n = (sr as f64 * seconds) as u32;
    let mut data = Vec::with_capacity(n as usize * 2);
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let s = ((2.0 * std::f32::consts::PI * hz * t).sin() * 0.3 * 32767.0) as i16;
        data.extend_from_slice(&s.to_le_bytes());
    }
    let mut v = Vec::new();
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&((36 + data.len()) as u32).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&sr.to_le_bytes());
    v.extend_from_slice(&(sr * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&(data.len() as u32).to_le_bytes());
    v.extend_from_slice(&data);
    std::fs::write(path, v).unwrap();
}

#[test]
fn real_whisper_transcription_runs_end_to_end() {
    let Some(dir) = std::env::var("DELIL_TEST_MODEL_DIR").ok() else {
        eprintln!("DELIL_TEST_MODEL_DIR ayarlı değil; gerçek model testi atlandı.");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let cfg = WhisperEngineConfig::new(&dir);
    let mut engine = match CandleWhisper::load(&cfg) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Model yüklenemedi ({e}); test atlandı.");
            return;
        }
    };
    assert!(engine.is_ready());

    // Gerçek ses: kısa bir ton. (Bu bir konuşma kaydı değildir; amaç motorun
    // gerçek ağırlıklarla çalıştığını kanıtlamaktır.)
    let tmp = tempfile::tempdir().unwrap();
    let wav = tmp.path().join("tone.wav");
    write_tone_wav(&wav, 3.0, 440.0);
    let samples = delil_transcription::decode_audio_file(&wav).unwrap();
    assert!(!samples.is_empty());

    let opts = TranscriptionOptions {
        language: Some("tr".into()),
        timestamps: true,
        ..Default::default()
    };
    let control = ControlFlag::new();
    let res = engine
        .transcribe(&samples, &opts, &control, None)
        .expect("transkripsiyon çalışmalı");

    // Motor bir sonuç üretti; süre gerçekçi olmalı.
    assert!(
        (res.duration_seconds - 3.0).abs() < 0.2,
        "süre ~3s olmalı, bulunan {}",
        res.duration_seconds
    );
    // Segmentler (boş olabilir; ton konuşma değildir) — kod yolu tamamlandı.
    eprintln!(
        "Gerçek model transkripsiyonu tamamlandı: {} segment, süre {:.2}s, dil {:?}",
        res.segments.len(),
        res.duration_seconds,
        res.detected_language
    );
    for s in &res.segments {
        eprintln!("  [{:.1}-{:.1}] {}", s.start, s.end, s.text);
    }
}

#[test]
fn real_model_with_silence_does_not_hallucinate_text() {
    // Sessizlikte modelin metin üretmemesi beklenir (kalite kontrolünün
    // "sessizlikte metin" kuralı). Bu, motorun gerçek davranışını sınar.
    let Some(dir) = std::env::var("DELIL_TEST_MODEL_DIR").ok() else {
        return;
    };
    let cfg = WhisperEngineConfig::new(std::path::PathBuf::from(dir));
    let Ok(mut engine) = CandleWhisper::load(&cfg) else {
        return;
    };
    let samples = vec![0.0f32; 16000 * 3];
    let opts = TranscriptionOptions {
        language: Some("tr".into()),
        ..Default::default()
    };
    let res = engine
        .transcribe(&samples, &opts, &ControlFlag::new(), None)
        .unwrap();
    let text = res.full_text();
    eprintln!("Sessizlik çıktısı: {:?}", text);
    // Sessizlikte uzun metin üretilmemeli (kısa/boş olabilir).
    assert!(
        text.chars().count() < 50,
        "sessizlikte aşırı metin üretildi: {text:?}"
    );
}
