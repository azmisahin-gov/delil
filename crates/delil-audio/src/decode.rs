//! Symphonia tabanlı ses çözme. `symphonia` özelliği etkinleştirildiğinde
//! derlenir.
//!
//! Bu modül ses dosyasını 16 kHz mono `f32` örneklemeye indirger; Whisper
//! motoru bu biçimi bekler. Kanal bilgisi kaydedilir; kanallar gelişigüzel
//! birleştirilirse bu bilgi `channel_processing` alanına yazılır.

use delil_core::AudioTechnicalInfo;
use std::path::Path;
use thiserror::Error;

/// Çözme hataları.
#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("Symphonia hatası: {0}")]
    Symphonia(String),
    #[error("Ses akışı bulunamadı")]
    NoAudioStream,
}

/// Çözülmüş ses: 16 kHz mono f32 örnekler ve envanter.
#[derive(Debug, Clone)]
pub struct DecodedAudio {
    /// 16 kHz mono f32 örnekler (-1.0..1.0).
    pub samples: Vec<f32>,
    /// Kaynak envanteri (kanal bilgisi dahil).
    pub info: AudioTechnicalInfo,
    /// Kanal karışımı uygulandıysa parametreler.
    pub channel_processing: Option<String>,
}

/// Hedef örnekleme frekansı (Whisper girişi).
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Ses dosyasını 16 kHz mono f32'ye çözer.
pub fn decode_to_16k_mono(path: &Path) -> Result<DecodedAudio, DecodeError> {
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|e| DecodeError::Symphonia(e.to_string()))?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or(DecodeError::NoAudioStream)?;
    let track_id = track.id;
    let src_rate = track.codec_params.sample_rate.unwrap_or(TARGET_SAMPLE_RATE);
    let src_channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(1);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| DecodeError::Symphonia(e.to_string()))?;

    // Kaynak örnekleme frekansından 16 kHz'e indirgeme oranı.
    let mut mono: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(DecodeError::Symphonia(e.to_string())),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                append_mono(&decoded, src_channels, &mut mono);
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(DecodeError::Symphonia(e.to_string())),
        }
    }

    // Basit doğrusal yeniden örnekleme (16 kHz hedefi).
    let (samples, processing) = if src_rate == TARGET_SAMPLE_RATE {
        (mono, None)
    } else {
        let resampled = resample_linear(&mono, src_rate, TARGET_SAMPLE_RATE);
        (
            resampled,
            Some(format!(
                "Yeniden örnekleme: {src_rate} Hz -> {TARGET_SAMPLE_RATE} Hz (doğrusal)"
            )),
        )
    };

    let mut info = AudioTechnicalInfo {
        sample_rate_hz: Some(src_rate),
        channels: Some(src_channels as u16),
        channel_layout: Some(match src_channels {
            1 => "mono".into(),
            2 => "stereo".into(),
            n => format!("{n} kanal"),
        }),
        readable: true,
        ..Default::default()
    };
    if src_channels > 1 {
        info.channel_processing = Some(format!(
            "{} kanal ortalaması alınarak mono'ya indirgendi",
            src_channels
        ));
    }
    info.duration_seconds = samples.len() as f64 / TARGET_SAMPLE_RATE as f64;

    Ok(DecodedAudio {
        samples,
        info,
        channel_processing: processing,
    })
}

fn append_mono(
    decoded: &symphonia::core::audio::AudioBufferRef,
    _channels: usize,
    out: &mut Vec<f32>,
) {
    use symphonia::core::audio::Signal;
    use symphonia::core::conv::IntoSample;
    macro_rules! mix {
        ($buf:expr) => {{
            let frames = $buf.frames();
            let ch = $buf.spec().channels.count();
            for f in 0..frames {
                let mut acc = 0.0f32;
                for c in 0..ch {
                    let s: f32 = IntoSample::<f32>::into_sample($buf.chan(c)[f]);
                    acc += s;
                }
                out.push(acc / ch as f32);
            }
        }};
    }
    match decoded {
        symphonia::core::audio::AudioBufferRef::F32(b) => mix!(b),
        symphonia::core::audio::AudioBufferRef::F64(b) => mix!(b),
        symphonia::core::audio::AudioBufferRef::S16(b) => mix!(b),
        symphonia::core::audio::AudioBufferRef::S32(b) => mix!(b),
        symphonia::core::audio::AudioBufferRef::U8(b) => mix!(b),
        symphonia::core::audio::AudioBufferRef::U16(b) => mix!(b),
        symphonia::core::audio::AudioBufferRef::U24(b) => mix!(b),
        symphonia::core::audio::AudioBufferRef::U32(b) => mix!(b),
        symphonia::core::audio::AudioBufferRef::S8(b) => mix!(b),
        symphonia::core::audio::AudioBufferRef::S24(b) => mix!(b),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_changes_length() {
        let input: Vec<f32> = (0..16000).map(|i| (i as f32 / 16000.0).sin()).collect();
        let out = resample_linear(&input, 16000, 8000);
        assert!((out.len() as i64 - 8000).abs() <= 1);
    }

    #[test]
    fn resample_same_rate_is_identity() {
        let input = vec![0.1, 0.2, 0.3];
        assert_eq!(resample_linear(&input, 16000, 16000), input);
    }

    #[test]
    fn decode_wav_16k() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.wav");
        // crate::tests::make_wav ulaşılamaz; yerel üretim.
        let sr = 16000u32;
        let secs = 0.1f64;
        let samples = (sr as f64 * secs) as u32;
        let data_len = samples * 2;
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data_len).to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&sr.to_le_bytes());
        v.extend_from_slice(&(sr * 2).to_le_bytes());
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_len.to_le_bytes());
        v.extend(std::iter::repeat(0u8).take(data_len as usize));
        std::fs::write(&p, v).unwrap();
        let decoded = decode_to_16k_mono(&p).unwrap();
        assert!(!decoded.samples.is_empty());
        assert_eq!(decoded.info.sample_rate_hz, Some(16000));
    }
}
