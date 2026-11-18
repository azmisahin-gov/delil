//! Gerçek ses oynatma (şartname 10).
//!
//! Bu modül `rodio` üzerinden yerel ses çıkışına bağlanır. Ses çıkışı
//! kullanılamıyorsa (ör. başsız ortam) oynatıcı **hata döndürür** ve arayüz
//! durumu (konum, hız, kanal) yine de yönetilebilir; simülasyon yapılmaz.
//!
//! Kayıt hiçbir zaman değiştirilmez; oynatma yalnızca okuma amaçlıdır.

use crate::waveform::PlaybackRate;
use std::path::Path;
use thiserror::Error;

/// Oynatma hataları.
#[derive(Debug, Error)]
pub enum PlaybackError {
    #[error("Ses çıkışı başlatılamadı: {0}")]
    OutputUnavailable(String),
    #[error("Ses dosyası açılamadı: {0}")]
    Decode(String),
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("Sarma desteklenmiyor: {0}")]
    Seek(String),
}

/// Yerel ses oynatıcı.
///
/// `rodio` kaynakları ve çıkış akışını yönetir. Kaynak bir dosyadan çözülür;
/// oynat/duraklat, hız, ses düzeyi ve sarma desteklenir.
pub struct AudioPlayer {
    /// Çıkış akışı (açık tutulmalı).
    _stream: rodio::OutputStream,
    handle: rodio::OutputStreamHandle,
    /// Geçerli kaynak (dosya).
    sink: Option<rodio::Sink>,
    /// Geçerli dosya yolu.
    current: Option<std::path::PathBuf>,
    /// Kaynak süresi.
    duration: std::time::Duration,
    /// Çıkış kullanılabilir mi?
    available: bool,
}

impl AudioPlayer {
    /// Yerel ses çıkışını açar.
    pub fn new() -> Result<Self, PlaybackError> {
        match rodio::OutputStream::try_default() {
            Ok((stream, handle)) => Ok(Self {
                _stream: stream,
                handle,
                sink: None,
                current: None,
                duration: std::time::Duration::ZERO,
                available: true,
            }),
            Err(e) => Err(PlaybackError::OutputUnavailable(e.to_string())),
        }
    }

    /// Ses çıkışı mevcut mu?
    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Geçerli dosya.
    pub fn current_file(&self) -> Option<&Path> {
        self.current.as_deref()
    }

    /// Süre (saniye).
    pub fn duration_seconds(&self) -> f64 {
        self.duration.as_secs_f64()
    }

    /// Bir ses dosyasını yükler ve oynatmaya hazırlar.
    ///
    /// `duration_seconds` çağıran tarafından biliniyorsa (teknik envanterden)
    /// verilir; bilinmiyorsa 0.0 geçilebilir.
    pub fn load(&mut self, path: &Path, duration_seconds: f64) -> Result<(), PlaybackError> {
        // Önceki kaynağı durdur.
        self.stop();
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let source =
            rodio::Decoder::new(reader).map_err(|e| PlaybackError::Decode(e.to_string()))?;
        let sink = rodio::Sink::try_new(&self.handle)
            .map_err(|e| PlaybackError::OutputUnavailable(e.to_string()))?;
        sink.append(source);
        sink.pause();
        self.sink = Some(sink);
        self.current = Some(path.to_path_buf());
        self.duration = std::time::Duration::from_secs_f64(duration_seconds.max(0.0));
        Ok(())
    }

    /// Oynatır.
    pub fn play(&self) {
        if let Some(s) = &self.sink {
            s.play();
        }
    }

    /// Duraklatır.
    pub fn pause(&self) {
        if let Some(s) = &self.sink {
            s.pause();
        }
    }

    /// Oynatma hızını ayarlar.
    pub fn set_rate(&self, rate: &PlaybackRate) {
        if let Some(s) = &self.sink {
            s.set_speed(rate.0);
        }
    }

    /// Ses düzeyini ayarlar (0.0..2.0).
    pub fn set_volume(&self, v: f32) {
        if let Some(s) = &self.sink {
            s.set_volume(v.clamp(0.0, 2.0));
        }
    }

    /// Belirli bir saniyeye sarar.
    pub fn seek(&self, seconds: f64) -> Result<(), PlaybackError> {
        let s = self
            .sink
            .as_ref()
            .ok_or_else(|| PlaybackError::Seek("Kaynak yüklenmedi".into()))?;
        let d = std::time::Duration::from_secs_f64(seconds.max(0.0));
        s.try_seek(d)
            .map_err(|e| PlaybackError::Seek(e.to_string()))
    }

    /// Oynatma bitti mi?
    pub fn is_finished(&self) -> bool {
        self.sink.as_ref().map(|s| s.empty()).unwrap_or(true)
    }

    /// Kaynağı durdurur ve temizler.
    pub fn stop(&mut self) {
        if let Some(s) = self.sink.take() {
            s.stop();
        }
        self.current = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_creation_does_not_panic_without_device() {
        // Başsız ortamda çıkış açılamaz; bu bir hata döndürür ama panik olmaz.
        match AudioPlayer::new() {
            Ok(p) => {
                assert!(p.is_available());
            }
            Err(e) => {
                // Beklenen: ses çıkışı yok.
                assert!(matches!(e, PlaybackError::OutputUnavailable(_)));
            }
        }
    }

    #[test]
    fn load_missing_file_errors() {
        let Ok(mut p) = AudioPlayer::new() else {
            return; // ses çıkışı yoksa test atlanır
        };
        assert!(p
            .load(Path::new("/nonexistent/delil/xyz.wav"), 0.0)
            .is_err());
    }
}
