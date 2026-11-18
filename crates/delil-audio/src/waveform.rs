//! Dalga biçimi (waveform) hesabı ve ses oynatma durumu (şartname 10).
//!
//! Dalga biçimi, çözülmüş 16 kHz mono örneklerden iniş/çıkış (min/max) zarfı
//! olarak hesaplanır. Bu, arayüzde zaman çizelgesi üzerinde gösterilir.
//!
//! Oynatma durumu; oynat/duraklat, geri/ileri sarma, hız değiştirme ve kanal
//! seçimi bilgilerini tutar. Gerçek ses çıkışı platforma bağlıdır; bu modül
//! durumu ve konumlandırmayı yönetir.

use serde::{Deserialize, Serialize};

/// Dalga biçimi zarfı: her kutu için en düşük ve en yüksek genlik.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Waveform {
    /// Kutu başına en düşük genlik (-1.0..1.0).
    pub min: Vec<f32>,
    /// Kutu başına en yüksek genlik (-1.0..1.0).
    pub max: Vec<f32>,
    /// Ses süresi (saniye).
    pub duration_seconds: f64,
    /// Kutu (bucket) sayısı.
    pub buckets: usize,
}

impl Waveform {
    /// Belirtilen kutu sayısı için örneklerden zarf hesaplar.
    pub fn from_samples(samples: &[f32], buckets: usize) -> Self {
        if samples.is_empty() || buckets == 0 {
            return Self {
                min: Vec::new(),
                max: Vec::new(),
                duration_seconds: 0.0,
                buckets: 0,
            };
        }
        let per = (samples.len() as f64 / buckets as f64).ceil() as usize;
        let mut min = Vec::with_capacity(buckets);
        let mut max = Vec::with_capacity(buckets);
        for b in 0..buckets {
            let start = b * per;
            if start >= samples.len() {
                min.push(0.0);
                max.push(0.0);
                continue;
            }
            let end = ((b + 1) * per).min(samples.len());
            let mut lo = f32::MAX;
            let mut hi = f32::MIN;
            for &s in &samples[start..end] {
                lo = lo.min(s);
                hi = hi.max(s);
            }
            min.push(lo);
            max.push(hi);
        }
        Self {
            min,
            max,
            duration_seconds: samples.len() as f64 / 16000.0,
            buckets,
        }
    }

    /// Zaman kodunu kutu dizinine çevirir.
    pub fn bucket_for_time(&self, seconds: f64) -> usize {
        if self.duration_seconds <= 0.0 {
            return 0;
        }
        let frac = (seconds / self.duration_seconds).clamp(0.0, 1.0);
        ((frac * self.buckets as f64) as usize).min(self.buckets.saturating_sub(1))
    }

    /// Belirli bir zaman aralığının azami genliği (sessizlik kontrolü).
    pub fn max_amplitude_in(&self, start_s: f64, end_s: f64) -> f32 {
        let a = self.bucket_for_time(start_s);
        let b = self.bucket_for_time(end_s);
        let mut m = 0f32;
        for i in a..=b.min(self.buckets.saturating_sub(1)) {
            if i < self.max.len() {
                m = m.max(self.max[i].abs()).max(self.min[i].abs());
            }
        }
        m
    }
}

/// Oynatma hızı.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlaybackRate(pub f32);

impl Default for PlaybackRate {
    fn default() -> Self {
        Self(1.0)
    }
}

impl PlaybackRate {
    /// Bilinen hız adımları (arayüzde hızlı/yavaş oynatma).
    pub const STEPS: [f32; 7] = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];

    pub fn next(&mut self) {
        let idx = Self::STEPS
            .iter()
            .position(|s| (*s - self.0).abs() < 0.001)
            .unwrap_or(2);
        self.0 = Self::STEPS[(idx + 1).min(Self::STEPS.len() - 1)];
    }

    pub fn prev(&mut self) {
        let idx = Self::STEPS
            .iter()
            .position(|s| (*s - self.0).abs() < 0.001)
            .unwrap_or(2);
        self.0 = Self::STEPS[idx.saturating_sub(1)];
    }
}

/// Kanal seçimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelSelection {
    /// Tüm kanallar (mono karışım).
    Mixed,
    /// Sol kanal.
    Left,
    /// Sağ kanal.
    Right,
    /// Belirli kanal (0 tabanlı).
    Index(u16),
}

impl ChannelSelection {
    pub fn label_tr(&self) -> String {
        match self {
            ChannelSelection::Mixed => "Karışım (mono)".into(),
            ChannelSelection::Left => "Sol kanal".into(),
            ChannelSelection::Right => "Sağ kanal".into(),
            ChannelSelection::Index(i) => format!("Kanal {}", i + 1),
        }
    }
}

/// İşaret (bookmark) kaydı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    /// Zaman (saniye).
    pub seconds: f64,
    /// Etiket.
    pub label: String,
    /// İlişkili transkript segmenti (varsa).
    pub segment_index: Option<usize>,
}

/// Oynatma durumu (arayüz tarafından yönetilir).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackState {
    pub playing: bool,
    /// Geçerli konum (saniye).
    pub position_seconds: f64,
    /// Toplam süre (saniye).
    pub duration_seconds: f64,
    pub rate: PlaybackRate,
    pub channel: ChannelSelection,
    pub markers: Vec<Marker>,
    /// Tekrar dinleme için başlangıç noktası (saniye).
    pub loop_start: Option<f64>,
    pub loop_end: Option<f64>,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            playing: false,
            position_seconds: 0.0,
            duration_seconds: 0.0,
            rate: PlaybackRate::default(),
            channel: ChannelSelection::Mixed,
            markers: Vec::new(),
            loop_start: None,
            loop_end: None,
        }
    }
}

impl PlaybackState {
    pub fn play(&mut self) {
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn toggle(&mut self) {
        self.playing = !self.playing;
    }

    /// Belirli bir zamana atlar.
    pub fn seek(&mut self, seconds: f64) {
        self.position_seconds = seconds.clamp(0.0, self.duration_seconds.max(0.0));
    }

    /// Geri sarar (saniye).
    pub fn rewind(&mut self, seconds: f64) {
        self.seek(self.position_seconds - seconds);
    }

    /// İleri sarar (saniye).
    pub fn forward(&mut self, seconds: f64) {
        self.seek(self.position_seconds + seconds);
    }

    /// Metin satırına tıklandığında oynatıcıyı o noktaya götürür.
    pub fn seek_to_segment(&mut self, start_seconds: f64) {
        self.seek(start_seconds);
    }

    /// Oynatma sırasında geçerli segmenti bulur (vurgulama için).
    pub fn active_segment(&self, starts: &[f64], ends: &[f64]) -> Option<usize> {
        for (i, (s, e)) in starts.iter().zip(ends.iter()).enumerate() {
            if self.position_seconds >= *s && self.position_seconds < *e {
                return Some(i);
            }
        }
        None
    }

    /// İşaret ekler.
    pub fn add_marker(&mut self, label: impl Into<String>, segment_index: Option<usize>) {
        self.markers.push(Marker {
            seconds: self.position_seconds,
            label: label.into(),
            segment_index,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waveform_envelope_from_samples() {
        let samples: Vec<f32> = (0..16000)
            .map(|i| (i as f32 * 0.05).sin() * 0.8)
            .collect();
        let wf = Waveform::from_samples(&samples, 100);
        assert_eq!(wf.buckets, 100);
        assert_eq!(wf.min.len(), 100);
        assert_eq!(wf.max.len(), 100);
        assert!((wf.duration_seconds - 1.0).abs() < 0.01);
        assert!(wf.max.iter().any(|m| *m > 0.5));
    }

    #[test]
    fn waveform_empty_is_safe() {
        let wf = Waveform::from_samples(&[], 100);
        assert_eq!(wf.buckets, 0);
        assert!(wf.min.is_empty());
    }

    #[test]
    fn bucket_for_time_maps_correctly() {
        let wf = Waveform {
            min: vec![0.0; 100],
            max: vec![0.0; 100],
            duration_seconds: 10.0,
            buckets: 100,
        };
        assert_eq!(wf.bucket_for_time(0.0), 0);
        assert_eq!(wf.bucket_for_time(5.0), 50);
        assert_eq!(wf.bucket_for_time(10.0), 99);
    }

    #[test]
    fn amplitude_in_silent_region_is_low() {
        let mut samples = vec![0.8f32; 16000];
        for s in samples.iter_mut().take(8000).skip(4000) {
            *s = 0.0;
        }
        let wf = Waveform::from_samples(&samples, 32);
        let silent = wf.max_amplitude_in(0.30, 0.45);
        let loud = wf.max_amplitude_in(0.60, 0.90);
        assert!(silent < loud, "sessiz {silent} < yüksek {loud} olmalı");
        assert!(silent < 0.01);
    }

    #[test]
    fn playback_seek_and_rewind() {
        let mut p = PlaybackState {
            duration_seconds: 100.0,
            ..Default::default()
        };
        p.seek(50.0);
        assert_eq!(p.position_seconds, 50.0);
        p.rewind(10.0);
        assert_eq!(p.position_seconds, 40.0);
        p.forward(5.0);
        assert_eq!(p.position_seconds, 45.0);
        p.seek(-10.0);
        assert_eq!(p.position_seconds, 0.0);
        p.seek(200.0);
        assert_eq!(p.position_seconds, 100.0);
    }

    #[test]
    fn playback_rate_steps() {
        let mut r = PlaybackRate::default();
        assert_eq!(r.0, 1.0);
        r.next();
        assert_eq!(r.0, 1.25);
        r.prev();
        assert_eq!(r.0, 1.0);
    }

    #[test]
    fn active_segment_detection() {
        let p = PlaybackState {
            position_seconds: 3.5,
            ..Default::default()
        };
        let starts = vec![0.0, 2.0, 5.0];
        let ends = vec![2.0, 5.0, 8.0];
        assert_eq!(p.active_segment(&starts, &ends), Some(1));
    }

    #[test]
    fn markers_are_recorded() {
        let mut p = PlaybackState {
            duration_seconds: 10.0,
            ..Default::default()
        };
        p.seek(4.0);
        p.add_marker("şüpheli ifade", Some(2));
        assert_eq!(p.markers.len(), 1);
        assert_eq!(p.markers[0].seconds, 4.0);
        assert_eq!(p.markers[0].segment_index, Some(2));
    }
}
