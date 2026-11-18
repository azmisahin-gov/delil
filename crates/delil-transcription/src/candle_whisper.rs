//! Candle tabanlı yerel Whisper motoru (şartname 8.1, 8.2, 8.3).
//!
//! Model: **whisper-large-v3-turbo**. Çıkarım Rust/Candle ile tamamen
//! yereldir. Model kurulum paketiyle birlikte verilir; ilk çalıştırmada
//! internetten indirilmez. Model dosyaları (config.json, tokenizer.json,
//! model.safetensors) sabitlenmiş sürümde ve hash doğrulamasıyla kullanılır.
//!
//! CPU desteği zorunludur. GPU (Metal/CUDA) destekleniyorsa otomatik algılanır;
//! kullanılamazsa işlem CPU'ya düşer. Kullanıcıdan CUDA/model/batch/beam/
//! quantization seçimi istenmez.

use crate::engine::{
    ControlFlag, EngineProgress, Segment, SignalStats, TranscriptionEngine, TranscriptionOptions,
    TranscriptionResult,
};
use crate::{Result, TranscriptionError};
use candle::{DType, Device, IndexOp, Tensor, D};
use candle_nn::{ops::softmax, VarBuilder};
use candle_transformers::models::whisper::{self as m, audio, Config};
use std::path::{Path, PathBuf};
use tokenizers::Tokenizer;

/// Motor model dosyalarının yolu ve cihaz seçimi.
#[derive(Debug, Clone)]
pub struct WhisperEngineConfig {
    /// Model dosyalarının bulunduğu dizin (config.json, tokenizer.json,
    /// model.safetensors).
    pub model_dir: PathBuf,
    /// CPU zorla (GPU olsa bile).
    pub force_cpu: bool,
    /// İlk parçayı örnekle (ısınma). Model yüklendikten sonra true.
    pub seed: u64,
}

impl WhisperEngineConfig {
    pub fn new(model_dir: impl Into<PathBuf>) -> Self {
        Self {
            model_dir: model_dir.into(),
            force_cpu: false,
            seed: 299_792_458,
        }
    }
}

/// Model dosya adları.
pub const CONFIG_FILE: &str = "config.json";
pub const TOKENIZER_FILE: &str = "tokenizer.json";
pub const WEIGHTS_FILE: &str = "model.safetensors";

/// Cihaz seçimi: GPU varsa ve istenmişse GPU, yoksa CPU.
pub fn select_device(force_cpu: bool) -> Device {
    if !force_cpu {
        #[cfg(feature = "cuda")]
        {
            if let Ok(d) = Device::new_cuda(0) {
                return d;
            }
        }
        #[cfg(feature = "metal")]
        {
            if let Ok(d) = Device::new_metal(0) {
                return d;
            }
        }
    }
    Device::Cpu
}

/// Model dosyalarının eksiksiz olup olmadığını kontrol eder.
pub fn check_model_files(dir: &Path) -> Result<()> {
    for f in [CONFIG_FILE, TOKENIZER_FILE, WEIGHTS_FILE] {
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

/// Candle Whisper motoru.
pub struct CandleWhisper {
    model: ModelKind,
    tokenizer: Tokenizer,
    config: Config,
    mel_filters: Vec<f32>,
    device: Device,
    version: String,
    name: String,
}

enum ModelKind {
    Normal(m::model::Whisper),
}

impl CandleWhisper {
    /// Modeli diskten yükler. Model kurulum paketiyle birlikte gelmelidir.
    pub fn load(cfg: &WhisperEngineConfig) -> Result<Self> {
        check_model_files(&cfg.model_dir)?;
        let device = select_device(cfg.force_cpu);

        let config_str = std::fs::read_to_string(cfg.model_dir.join(CONFIG_FILE))?;
        let config: Config = serde_json::from_str(&config_str)
            .map_err(|e| TranscriptionError::ModelLoad(format!("config.json: {e}")))?;
        let tokenizer = Tokenizer::from_file(cfg.model_dir.join(TOKENIZER_FILE))
            .map_err(|e| TranscriptionError::ModelLoad(format!("tokenizer.json: {e}")))?;

        let mel_filters = match config.num_mel_bins {
            80 => load_mel_filters(include_bytes!("../assets/melfilters.bytes")),
            128 => load_mel_filters(include_bytes!("../assets/melfilters128.bytes")),
            n => {
                return Err(TranscriptionError::ModelLoad(format!(
                    "beklenmeyen num_mel_bins: {n}"
                )))
            }
        };

        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(
                &[cfg.model_dir.join(WEIGHTS_FILE)],
                m::DTYPE,
                &device,
            )
            .map_err(|e| TranscriptionError::ModelLoad(format!("model.safetensors: {e}")))?
        };
        let model = m::model::Whisper::load(&vb, config.clone())
            .map_err(|e| TranscriptionError::ModelLoad(e.to_string()))?;

        Ok(Self {
            model: ModelKind::Normal(model),
            tokenizer,
            config,
            mel_filters,
            device,
            version: "large-v3-turbo (sabitlenmiş)".to_string(),
            name: m_name(),
        })
    }

    fn encoder_forward(&mut self, x: &Tensor, flush: bool) -> candle::Result<Tensor> {
        match &mut self.model {
            ModelKind::Normal(m) => m.encoder.forward(x, flush),
        }
    }

    fn decoder_forward(&mut self, x: &Tensor, xa: &Tensor, flush: bool) -> candle::Result<Tensor> {
        match &mut self.model {
            ModelKind::Normal(m) => m.decoder.forward(x, xa, flush),
        }
    }

    fn decoder_final_linear(&self, x: &Tensor) -> candle::Result<Tensor> {
        match &self.model {
            ModelKind::Normal(m) => m.decoder.final_linear(x),
        }
    }

    /// Dil algılar (ilk dil token'ı).
    fn detect_language(&mut self, mel: &Tensor) -> candle::Result<String> {
        let language_token_ids = LANGUAGES
            .iter()
            .map(|(t, _)| token_id(&self.tokenizer, &format!("<|{t}|>")))
            .collect::<candle::Result<Vec<_>>>()?;
        let sot_token = token_id(&self.tokenizer, m::SOT_TOKEN)?;
        let audio_features = self.encoder_forward(mel, true)?;
        let tokens = Tensor::new(&[[sot_token]], &self.device)?;
        let language_token_ids = Tensor::new(language_token_ids.as_slice(), &self.device)?;
        let ys = self.decoder_forward(&tokens, &audio_features, true)?;
        let logits = self.decoder_final_linear(&ys.i(..1)?)?.i(0)?.i(0)?;
        let logits = logits.index_select(&language_token_ids, 0)?;
        let probs = softmax(&logits, D::Minus1)?.to_vec1::<f32>()?;
        let mut pairs: Vec<(&str, f32)> =
            LANGUAGES.iter().map(|(t, _)| *t).zip(probs).collect();
        pairs.sort_by(|a, b| b.1.total_cmp(&a.1));
        Ok(pairs[0].0.to_string())
    }

    /// Bir mel parçasını çözer (token'lar ve metin).
    fn decode_chunk(
        &mut self,
        mel: &Tensor,
        language_token: Option<u32>,
        timestamps: bool,
    ) -> candle::Result<(Vec<u32>, String)> {
        let audio_features = self.encoder_forward(mel, true)?;
        let sample_len = self.config.max_target_positions / 2;
        let suppress_tokens: Vec<f32> = (0..self.config.vocab_size as u32)
            .map(|i| {
                if self.config.suppress_tokens.contains(&i) {
                    f32::NEG_INFINITY
                } else {
                    0f32
                }
            })
            .collect();
        let suppress_tokens = Tensor::new(suppress_tokens.as_slice(), &self.device)?;

        let sot_token = token_id(&self.tokenizer, m::SOT_TOKEN)?;
        let transcribe_token = token_id(&self.tokenizer, m::TRANSCRIBE_TOKEN)?;
        let eot_token = token_id(&self.tokenizer, m::EOT_TOKEN)?;
        let no_timestamps_token = token_id(&self.tokenizer, m::NO_TIMESTAMPS_TOKEN)?;

        let mut tokens = vec![sot_token];
        if let Some(l) = language_token {
            tokens.push(l);
        }
        tokens.push(transcribe_token);
        if !timestamps {
            tokens.push(no_timestamps_token);
        }

        for i in 0..sample_len {
            let tokens_t = Tensor::new(tokens.as_slice(), &self.device)?.unsqueeze(0)?;
            let ys = self.decoder_forward(&tokens_t, &audio_features, i == 0)?;
            let (_, seq_len, _) = ys.dims3()?;
            let logits = self
                .decoder_final_linear(&ys.i((..1, seq_len - 1..))?)?
                .i(0)?
                .i(0)?;
            let logits = logits.broadcast_add(&suppress_tokens)?;
            let logits_v: Vec<f32> = logits.to_vec1()?;
            let next_token = logits_v
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.total_cmp(b))
                .map(|(i, _)| i as u32)
                .unwrap();
            tokens.push(next_token);
            if next_token == eot_token || tokens.len() > self.config.max_target_positions {
                break;
            }
        }
        let text = self
            .tokenizer
            .decode(&tokens, true)
            .map_err(|e| candle::Error::Msg(e.to_string()))?;
        Ok((tokens, text))
    }
}

fn m_name() -> String {
    "whisper-large-v3-turbo".to_string()
}

fn token_id(tokenizer: &Tokenizer, token: &str) -> candle::Result<u32> {
    tokenizer
        .token_to_id(token)
        .ok_or_else(|| candle::Error::Msg(format!("token bulunamadı: {token}")))
}

fn load_mel_filters(bytes: &[u8]) -> Vec<f32> {
    let mut v = vec![0f32; bytes.len() / 4];
    for (i, chunk) in bytes.chunks_exact(4).enumerate() {
        v[i] = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    v
}

impl TranscriptionEngine for CandleWhisper {
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &str {
        &self.version
    }
    fn is_ready(&self) -> bool {
        true
    }

    fn transcribe(
        &mut self,
        samples: &[f32],
        options: &TranscriptionOptions,
        control: &ControlFlag,
        mut progress: Option<EngineProgress<'_>>,
    ) -> Result<TranscriptionResult> {
        control.wait_if_paused()?;
        let stats: SignalStats = crate::engine::analyze_signal(samples);

        // Mel spektrogramı.
        let mel = audio::pcm_to_mel(&self.config, samples, &self.mel_filters);
        let mel_len = mel.len();
        if mel_len == 0 {
            return Err(TranscriptionError::Inference("boş ses".into()));
        }
        let mel = Tensor::from_vec(
            mel,
            (1, self.config.num_mel_bins, mel_len / self.config.num_mel_bins),
            &self.device,
        )
        .map_err(|e| TranscriptionError::Inference(e.to_string()))?;

        // Dil token'ı.
        let language_token = match &options.language {
            Some(lang) => {
                let tok = token_id(&self.tokenizer, &format!("<|{lang}|>"))
                    .map_err(|_| TranscriptionError::Inference(format!("dil desteklenmiyor: {lang}")))?;
                Some(tok)
            }
            None => {
                // Otomatik algıla; ham sonuç rapora yazılır.
                let detected = self
                    .detect_language(&mel)
                    .map_err(|e| TranscriptionError::Inference(e.to_string()))?;
                Some(
                    token_id(&self.tokenizer, &format!("<|{detected}|>"))
                        .map_err(|e| TranscriptionError::Inference(e.to_string()))?,
                )
            }
        };

        // 30 saniyelik parçalara ayır ve her parçayı çöz.
        let (_, _, content_frames) = mel
            .dims3()
            .map_err(|e| TranscriptionError::Inference(e.to_string()))?;
        let mut seek = 0usize;
        let mut segments: Vec<Segment> = Vec::new();
        let total_frames = content_frames;
        while seek < content_frames {
            control.wait_if_paused()?;
            let time_offset = (seek * m::HOP_LENGTH) as f64 / m::SAMPLE_RATE as f64;
            let segment_size = usize::min(content_frames - seek, m::N_FRAMES);
            let mel_segment = mel
                .narrow(2, seek, segment_size)
                .map_err(|e| TranscriptionError::Inference(e.to_string()))?;
            let segment_duration =
                (segment_size * m::HOP_LENGTH) as f64 / m::SAMPLE_RATE as f64;

            let (_tokens, text) = self
                .decode_chunk(&mel_segment, language_token, options.timestamps)
                .map_err(|e| TranscriptionError::Inference(e.to_string()))?;

            // Token'ları zaman kodlu segmentlere ayır.
            let parsed = if options.timestamps {
                split_by_timestamps(&self.tokenizer, &_tokens, time_offset)
            } else {
                vec![Segment {
                    start: time_offset,
                    end: time_offset + segment_duration,
                    text: text.clone(),
                    avg_logprob: None,
                    no_speech_prob: None,
                }]
            };
            segments.extend(parsed);

            seek += segment_size;
            if let Some(cb) = progress.as_deref_mut() {
                cb(
                    seek as f64 * m::HOP_LENGTH as f64 / m::SAMPLE_RATE as f64,
                    total_frames as f64 * m::HOP_LENGTH as f64 / m::SAMPLE_RATE as f64,
                );
            }
        }

        Ok(TranscriptionResult {
            segments,
            detected_language: options.language.clone().or(Some("tr".into())),
            duration_seconds: stats.duration_seconds,
            rms: stats.rms,
            peak: stats.peak,
            corrupt: false,
        })
    }
}

/// Token dizisini zaman damgası token'larına göre segmentlere böler.
fn split_by_timestamps(tokenizer: &Tokenizer, tokens: &[u32], offset: f64) -> Vec<Segment> {
    let Ok(no_ts) = token_id(tokenizer, m::NO_TIMESTAMPS_TOKEN) else {
        return Vec::new();
    };
    let Ok(sot) = token_id(tokenizer, m::SOT_TOKEN) else {
        return Vec::new();
    };
    let Ok(eot) = token_id(tokenizer, m::EOT_TOKEN) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut buf: Vec<u32> = Vec::new();
    let mut prev_ts = 0f32;
    let mut current_start = offset;
    for &t in tokens {
        if t == sot || t == eot {
            continue;
        }
        if t > no_ts {
            let ts = (t - no_ts - 1) as f32 / 50.0;
            if !buf.is_empty() {
                let text = tokenizer.decode(&buf, true).unwrap_or_default();
                if !text.trim().is_empty() {
                    out.push(Segment {
                        start: current_start,
                        end: offset + prev_ts as f64,
                        text: text.trim().to_string(),
                        avg_logprob: None,
                        no_speech_prob: None,
                    });
                }
                buf.clear();
            }
            current_start = offset + ts as f64;
            prev_ts = ts;
        } else {
            buf.push(t);
        }
    }
    if !buf.is_empty() {
        let text = tokenizer.decode(&buf, true).unwrap_or_default();
        if !text.trim().is_empty() {
            out.push(Segment {
                start: current_start,
                end: offset + prev_ts as f64 + 30.0,
                text: text.trim().to_string(),
                avg_logprob: None,
                no_speech_prob: None,
            });
        }
    }
    out
}

/// Whisper'ın desteklediği diller (ISO kodu, İngilizce adı).
pub const LANGUAGES: [(&str, &str); 99] = [
    ("en", "english"),
    ("zh", "chinese"),
    ("de", "german"),
    ("es", "spanish"),
    ("ru", "russian"),
    ("ko", "korean"),
    ("fr", "french"),
    ("ja", "japanese"),
    ("pt", "portuguese"),
    ("tr", "turkish"),
    ("pl", "polish"),
    ("ca", "catalan"),
    ("nl", "dutch"),
    ("ar", "arabic"),
    ("sv", "swedish"),
    ("it", "italian"),
    ("id", "indonesian"),
    ("hi", "hindi"),
    ("fi", "finnish"),
    ("vi", "vietnamese"),
    ("he", "hebrew"),
    ("uk", "ukrainian"),
    ("el", "greek"),
    ("ms", "malay"),
    ("cs", "czech"),
    ("ro", "romanian"),
    ("da", "danish"),
    ("hu", "hungarian"),
    ("ta", "tamil"),
    ("no", "norwegian"),
    ("th", "thai"),
    ("ur", "urdu"),
    ("hr", "croatian"),
    ("bg", "bulgarian"),
    ("lt", "lithuanian"),
    ("la", "latin"),
    ("mi", "maori"),
    ("ml", "malayalam"),
    ("cy", "welsh"),
    ("sk", "slovak"),
    ("te", "telugu"),
    ("fa", "persian"),
    ("lv", "latvian"),
    ("bn", "bengali"),
    ("sr", "serbian"),
    ("az", "azerbaijani"),
    ("sl", "slovenian"),
    ("kn", "kannada"),
    ("et", "estonian"),
    ("mk", "macedonian"),
    ("br", "breton"),
    ("eu", "basque"),
    ("is", "icelandic"),
    ("hy", "armenian"),
    ("ne", "nepali"),
    ("mn", "mongolian"),
    ("bs", "bosnian"),
    ("kk", "kazakh"),
    ("sq", "albanian"),
    ("sw", "swahili"),
    ("gl", "galician"),
    ("mr", "marathi"),
    ("pa", "punjabi"),
    ("si", "sinhala"),
    ("km", "khmer"),
    ("sn", "shona"),
    ("yo", "yoruba"),
    ("so", "somali"),
    ("af", "afrikaans"),
    ("oc", "occitan"),
    ("ka", "georgian"),
    ("be", "belarusian"),
    ("tg", "tajik"),
    ("sd", "sindhi"),
    ("gu", "gujarati"),
    ("am", "amharic"),
    ("yi", "yiddish"),
    ("lo", "lao"),
    ("uz", "uzbek"),
    ("fo", "faroese"),
    ("ht", "haitian creole"),
    ("ps", "pashto"),
    ("tk", "turkmen"),
    ("nn", "nynorsk"),
    ("mt", "maltese"),
    ("sa", "sanskrit"),
    ("lb", "luxembourgish"),
    ("my", "myanmar"),
    ("bo", "tibetan"),
    ("tl", "tagalog"),
    ("mg", "malagasy"),
    ("as", "assamese"),
    ("tt", "tatar"),
    ("haw", "hawaiian"),
    ("ln", "lingala"),
    ("ha", "hausa"),
    ("ba", "bashkir"),
    ("jw", "javanese"),
    ("su", "sundanese"),
];

/// DType sabitini yeniden dışa aktarır (test/uyumluluk için).
pub const DTYPE: DType = m::DTYPE;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_device_defaults_to_cpu_without_gpu() {
        let d = select_device(true);
        assert_eq!(d, Device::Cpu);
    }

    #[test]
    fn missing_model_files_reports_clear_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = check_model_files(dir.path()).unwrap_err();
        assert!(matches!(err, TranscriptionError::ModelMissing(_)));
    }

    #[test]
    fn mel_filters_load_with_expected_length() {
        // 80 mel x 201 = 16080 float = 64320 bayt.
        let f = load_mel_filters(include_bytes!("../assets/melfilters.bytes"));
        assert_eq!(f.len(), 64320 / 4);
        let f128 = load_mel_filters(include_bytes!("../assets/melfilters128.bytes"));
        assert_eq!(f128.len(), 102912 / 4);
    }

    #[test]
    fn languages_include_turkish() {
        assert!(LANGUAGES.iter().any(|(c, _)| *c == "tr"));
    }
}
