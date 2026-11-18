//! Ses biçimi algılama, teknik envanter ve (isteğe bağlı) çözme.
//!
//! Algılama **dosya uzantısına güvenmez**: dosyanın ilk baytlarındaki imza
//! (magic bytes) incelenir. Böylece yanlış uzantılı bir dosya sessizce
//! atlanmaz; içeriğine göre sınıflandırılır.
//!
//! Teknik envanter (kapsayıcı, codec, süre, örnekleme frekansı vb.) WAV ve
//! FLAC için başlık ayrıştırmasıyla çıkarılır. Ayrıntılı çözme `symphonia`
//! özelliği etkinleştirildiğinde kullanılabilir.

use delil_core::AudioTechnicalInfo;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "playback")]
pub mod player;
pub mod waveform;

#[cfg(feature = "decode")]
pub mod decode;

#[cfg(feature = "playback")]
pub use player::AudioPlayer;
pub use waveform::{ChannelSelection, Marker, PlaybackRate, PlaybackState, Waveform};

/// Algılama ve envanter hataları.
#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Dosya okunamadı: {0}")]
    Io(#[from] std::io::Error),
    #[error("Desteklenmeyen biçim: {0}")]
    Unsupported(String),
}

/// Algılanan kapsayıcı biçimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Container {
    Wav,
    Bwf,
    Mp3,
    M4a,
    Aac,
    Flac,
    Ogg,
    Opus,
    Wma,
    Amr,
    Aiff,
    Caf,
    /// Video kapsayıcı; yalnızca ses akışı çıkarılır.
    Mp4,
    Mov,
    Mkv,
    WebM,
}

impl Container {
    /// Kapsayıcının yalnızca ses içerdiği kesin mi? Video kapsayıcıları için
    /// ses akışı ayrıca doğrulanmalıdır.
    pub fn is_audio_only(&self) -> bool {
        !matches!(
            self,
            Container::Mp4 | Container::Mov | Container::Mkv | Container::WebM
        )
    }

    pub fn is_video_container(&self) -> bool {
        !self.is_audio_only()
    }

    /// Dosya adı uzantısına göre olası kapsayıcı (yalnızca ipucu; asıl
    /// algılama içerikten yapılır).
    pub fn from_extension(ext: &str) -> Option<Container> {
        Some(match ext.to_ascii_lowercase().as_str() {
            "wav" | "wave" => Container::Wav,
            "bwf" => Container::Bwf,
            "mp3" => Container::Mp3,
            "m4a" => Container::M4a,
            "aac" => Container::Aac,
            "flac" => Container::Flac,
            "ogg" | "oga" => Container::Ogg,
            "opus" => Container::Opus,
            "wma" => Container::Wma,
            "amr" => Container::Amr,
            "aiff" | "aif" | "aifc" => Container::Aiff,
            "caf" => Container::Caf,
            "mp4" | "m4v" => Container::Mp4,
            "mov" => Container::Mov,
            "mkv" => Container::Mkv,
            "webm" => Container::WebM,
            _ => return None,
        })
    }

    pub fn label(&self) -> &'static str {
        match self {
            Container::Wav => "WAV",
            Container::Bwf => "BWF",
            Container::Mp3 => "MP3",
            Container::M4a => "M4A",
            Container::Aac => "AAC",
            Container::Flac => "FLAC",
            Container::Ogg => "OGG",
            Container::Opus => "Opus",
            Container::Wma => "WMA",
            Container::Amr => "AMR",
            Container::Aiff => "AIFF",
            Container::Caf => "CAF",
            Container::Mp4 => "MP4",
            Container::Mov => "MOV",
            Container::Mkv => "MKV",
            Container::WebM => "WebM",
        }
    }
}

/// İçerik imzasına göre kapsayıcı algılar. Uzantıya güvenilmez.
///
/// Bu işlev **panik yapmaz**: kısa veya bozuk dosyalarda `None` döner.
/// Her imza kontrolü, gereken uzunluğu ayrıca doğrular.
pub fn detect_container(bytes: &[u8]) -> Option<Container> {
    let b = bytes;
    let at = |off: usize, pat: &[u8]| -> bool {
        b.len() >= off + pat.len() && &b[off..off + pat.len()] == pat
    };

    // RIFF....WAVE  (WAV veya BWF) — en az 12 bayt gerekir.
    if at(0, b"RIFF") && at(8, b"WAVE") {
        let head = &b[..b.len().min(4096)];
        if head.windows(4).any(|w| w == b"bext") {
            return Some(Container::Bwf);
        }
        return Some(Container::Wav);
    }
    // FORM....AIFF / AIFC
    if at(0, b"FORM") && (at(8, b"AIFF") || at(8, b"AIFC")) {
        return Some(Container::Aiff);
    }
    if at(0, b"fLaC") {
        return Some(Container::Flac);
    }
    if at(0, b"OggS") {
        let head = &b[..b.len().min(64)];
        if head.windows(8).any(|w| w == b"OpusHead") {
            return Some(Container::Opus);
        }
        return Some(Container::Ogg);
    }
    if at(0, b"caff") {
        return Some(Container::Caf);
    }
    if at(0, b"ID3") {
        return Some(Container::Mp3);
    }
    // MPEG çerçeve eşitleme: 0xFF 0xEx/0xFx (en az 2 bayt).
    if b.len() >= 2 && b[0] == 0xFF && (b[1] & 0xE0) == 0xE0 {
        return Some(Container::Mp3);
    }
    if at(0, b"#!AMR") {
        return Some(Container::Amr);
    }
    // ASF (WMA) — 16 baytlık GUID.
    if at(
        0,
        b"\x30\x26\xB2\x75\x8E\x66\xCF\x11\xA6\xD9\x00\xAA\x00\x62\xCE\x6C",
    ) {
        return Some(Container::Wma);
    }
    // ISO-BMFF (MP4/M4A/MOV): ....ftyp + 4 baytlık marka (en az 12 bayt).
    if at(4, b"ftyp") && b.len() >= 12 {
        let brand = &b[8..12];
        return Some(match brand {
            // Yalnızca gerçek ses kapsayıcı markaları ses sayılır; `isom`,
            // `mp42` gibi genel ISO-BMFF markaları video kapsayıcıdır.
            b"M4A " | b"M4B " => Container::M4a,
            b"qt  " => Container::Mov,
            _ => Container::Mp4,
        });
    }
    // Matroska/WebM (EBML)
    if at(0, b"\x1A\x45\xDF\xA3") {
        let head = &b[..b.len().min(64)];
        if head.windows(4).any(|w| w == b"webm") {
            return Some(Container::WebM);
        }
        return Some(Container::Mkv);
    }
    // ADTS AAC (0xFF 0xF1 / 0xFF 0xF9) — MP3 kontrolünden sonra.
    if b.len() >= 2 && b[0] == 0xFF && (b[1] == 0xF1 || b[1] == 0xF9) {
        return Some(Container::Aac);
    }
    None
}

/// Dosyanın yalnızca ilk `max` baytını okur (bellek dostu başlık okuma).
///
/// Uzun kayıtların tümünü belleğe almak bellek tükenmesine yol açabilir;
/// kapsayıcı ve teknik envanter için başlık yeterlidir.
pub fn read_header(path: &std::path::Path, max: usize) -> Result<Vec<u8>, AudioError> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut buf = vec![0u8; max];
    let mut filled = 0usize;
    loop {
        let n = f.read(&mut buf[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
        if filled >= max {
            break;
        }
    }
    buf.truncate(filled);
    Ok(buf)
}

/// Dosya yolundan ilk baytları okuyup kapsayıcı algılar.
pub fn detect_container_from_path(path: &std::path::Path) -> Result<Option<Container>, AudioError> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut buf = vec![0u8; 4096];
    let n = f.read(&mut buf)?;
    buf.truncate(n);
    Ok(detect_container(&buf))
}

/// WAV başlığını ayrıştırıp teknik bilgileri çıkarır.
fn parse_wav(bytes: &[u8], info: &mut AudioTechnicalInfo) {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        info.warnings.push("Geçersiz WAV başlığı".into());
        return;
    }
    let mut pos = 12usize;
    let mut data_len: Option<u32> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        let body = pos + 8;
        match id {
            b"fmt " => {
                if body + 16 <= bytes.len() {
                    let channels = u16::from_le_bytes([bytes[body + 2], bytes[body + 3]]);
                    let sample_rate = u32::from_le_bytes([
                        bytes[body + 4],
                        bytes[body + 5],
                        bytes[body + 6],
                        bytes[body + 7],
                    ]);
                    let bits = u16::from_le_bytes([bytes[body + 14], bytes[body + 15]]);
                    info.channels = Some(channels);
                    info.sample_rate_hz = Some(sample_rate);
                    info.bit_depth = Some(bits);
                    info.codec = "PCM".into();
                    info.bit_rate_bps = Some(sample_rate as u64 * channels as u64 * bits as u64);
                    info.channel_layout = Some(match channels {
                        1 => "mono".into(),
                        2 => "stereo".into(),
                        n => format!("{n} kanal"),
                    });
                }
            }
            b"data" => {
                data_len = Some(size as u32);
            }
            b"bext" => {
                info.container = "BWF".into();
            }
            _ => {}
        }
        pos = body + size + (size & 1);
        if size == 0 && id != b"data" {
            break;
        }
    }
    if let (Some(dl), Some(sr), Some(ch), Some(bits)) =
        (data_len, info.sample_rate_hz, info.channels, info.bit_depth)
    {
        if sr > 0 && ch > 0 && bits > 0 {
            info.duration_seconds = dl as f64 / (sr as f64 * ch as f64 * (bits as f64 / 8.0));
        }
    }
}

/// FLAC başlığından teknik bilgi çıkarır (STREAMINFO).
fn parse_flac(bytes: &[u8], info: &mut AudioTechnicalInfo) {
    info.codec = "FLAC".into();
    if bytes.len() < 4 + 4 + 34 || &bytes[0..4] != b"fLaC" {
        info.warnings.push("Eksik FLAC başlığı".into());
        return;
    }
    let si = &bytes[8..8 + 34];
    let sr = ((si[10] as u32) << 12) | ((si[11] as u32) << 4) | ((si[12] as u32) >> 4);
    let channels = (((si[12] >> 1) & 0x07) + 1) as u16;
    let bits = ((((si[12] & 0x01) as u16) << 4) | ((si[13] as u16) >> 4)) + 1;
    let total_samples = (((si[13] as u64) & 0x0F) << 32)
        | ((si[14] as u64) << 24)
        | ((si[15] as u64) << 16)
        | ((si[16] as u64) << 8)
        | (si[17] as u64);
    info.sample_rate_hz = Some(sr);
    info.channels = Some(channels);
    info.bit_depth = Some(bits);
    if sr > 0 {
        info.duration_seconds = total_samples as f64 / sr as f64;
    }
}

/// Dosya için teknik envanter çıkarır.
pub fn inventory(path: &std::path::Path) -> Result<AudioTechnicalInfo, AudioError> {
    let mut info = AudioTechnicalInfo::default();
    // Yalnızca başlığı oku. Uzun kayıtlar (ör. 2 saatlik WAV ~1.4 GB) tümüyle
    // belleğe alınırsa bellek tükenir; teknik envanter için ilk birkaç yüz
    // kilobayt yeterlidir.
    let bytes = read_header(path, 512 * 1024)?;
    let container = match detect_container(&bytes) {
        Some(c) => c,
        None => {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            match Container::from_extension(ext) {
                Some(c) => {
                    info.warnings
                        .push("İçerik imzası algılanamadı; uzantıya göre değerlendirildi".into());
                    c
                }
                None => {
                    info.readable = false;
                    info.warnings.push("Desteklenmeyen biçim".into());
                    return Ok(info);
                }
            }
        }
    };
    info.container = container.label().to_string();
    match container {
        Container::Wav | Container::Bwf => parse_wav(&bytes, &mut info),
        Container::Flac => parse_flac(&bytes, &mut info),
        Container::Mp3 => {
            info.codec = "MPEG Layer III".into();
            info.warnings
                .push("Süre ve bit hızı için tam çözme gerekebilir".into());
        }
        Container::Aac | Container::M4a => info.codec = "AAC".into(),
        Container::Ogg => info.codec = "Vorbis".into(),
        Container::Opus => info.codec = "Opus".into(),
        Container::Wma => info.codec = "WMA".into(),
        Container::Amr => info.codec = "AMR".into(),
        Container::Aiff => info.codec = "PCM (AIFF)".into(),
        Container::Caf => info.codec = "CAF".into(),
        Container::Mp4 | Container::Mov | Container::Mkv | Container::WebM => {
            info.codec = "bilinmiyor (kapsayıcı içi ses akışı)".into();
            info.warnings
                .push("Video kapsayıcısı; yalnızca ses akışı işlenecek".into());
        }
    }
    info.readable = true;
    Ok(info)
}

/// Bir ses dosyasının içeriğine göre gerçekten ses içerip içermediğini
/// söyler. Uzantıya güvenilmez.
pub fn looks_like_audio(path: &std::path::Path) -> bool {
    matches!(detect_container_from_path(path), Ok(Some(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Basit bir 16-bit PCM WAV üretir.
    pub(crate) fn make_wav(sample_rate: u32, channels: u16, bits: u16, seconds: f64) -> Vec<u8> {
        let byte_rate = sample_rate * channels as u32 * (bits / 8) as u32;
        let samples = (sample_rate as f64 * seconds) as u32;
        let data_len = samples * channels as u32 * (bits / 8) as u32;
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data_len).to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&channels.to_le_bytes());
        v.extend_from_slice(&sample_rate.to_le_bytes());
        v.extend_from_slice(&byte_rate.to_le_bytes());
        v.extend_from_slice(&(channels * (bits / 8)).to_le_bytes());
        v.extend_from_slice(&bits.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_len.to_le_bytes());
        v.extend(std::iter::repeat_n(0u8, data_len as usize));
        v
    }

    #[test]
    fn detect_wav_from_content() {
        let wav = make_wav(44100, 2, 16, 1.0);
        assert_eq!(detect_container(&wav), Some(Container::Wav));
    }

    #[test]
    fn detect_bwf_when_bext_present() {
        // bext chunk'ı dosya başında (ilk 4 KiB içinde) olacak şekilde küçük
        // bir WAV üretilir.
        let mut wav = make_wav(48000, 1, 16, 0.01);
        wav.extend_from_slice(b"bext");
        wav.extend_from_slice(&4u32.to_le_bytes());
        wav.extend_from_slice(b"test");
        assert_eq!(detect_container(&wav), Some(Container::Bwf));
    }

    #[test]
    fn detect_mp3_from_id3() {
        let mut v = b"ID3\x04\x00\x00".to_vec();
        v.extend(std::iter::repeat_n(0, 100));
        assert_eq!(detect_container(&v), Some(Container::Mp3));
    }

    #[test]
    fn detect_flac() {
        let mut v = b"fLaC".to_vec();
        v.extend(std::iter::repeat_n(0, 100));
        assert_eq!(detect_container(&v), Some(Container::Flac));
    }

    #[test]
    fn detect_ogg_and_opus() {
        let mut ogg = b"OggS".to_vec();
        ogg.extend(std::iter::repeat_n(0, 100));
        assert_eq!(detect_container(&ogg), Some(Container::Ogg));

        let mut opus = b"OggS".to_vec();
        opus.extend(std::iter::repeat_n(0, 20));
        opus.extend_from_slice(b"OpusHead");
        opus.extend(std::iter::repeat_n(0, 20));
        assert_eq!(detect_container(&opus), Some(Container::Opus));
    }

    #[test]
    fn detect_mp4_and_mov() {
        let mut mp4 = vec![0, 0, 0, 0x20];
        mp4.extend_from_slice(b"ftypM4A ");
        mp4.extend(std::iter::repeat_n(0, 50));
        assert_eq!(detect_container(&mp4), Some(Container::M4a));

        let mut mov = vec![0, 0, 0, 0x20];
        mov.extend_from_slice(b"ftypqt  ");
        mov.extend(std::iter::repeat_n(0, 50));
        assert_eq!(detect_container(&mov), Some(Container::Mov));
    }

    #[test]
    fn detect_matroska_and_webm() {
        let mut mkv = vec![0x1A, 0x45, 0xDF, 0xA3];
        mkv.extend(std::iter::repeat_n(0, 60));
        assert_eq!(detect_container(&mkv), Some(Container::Mkv));

        let mut webm = vec![0x1A, 0x45, 0xDF, 0xA3];
        webm.extend_from_slice(b"webm");
        webm.extend(std::iter::repeat_n(0, 60));
        assert_eq!(detect_container(&webm), Some(Container::WebM));
    }

    #[test]
    fn detect_amr_and_wma() {
        let mut amr = b"#!AMR".to_vec();
        amr.extend(std::iter::repeat_n(0, 20));
        assert_eq!(detect_container(&amr), Some(Container::Amr));

        let mut wma = b"\x30\x26\xB2\x75\x8E\x66\xCF\x11\xA6\xD9\x00\xAA\x00\x62\xCE\x6C".to_vec();
        wma.extend(std::iter::repeat_n(0, 20));
        assert_eq!(detect_container(&wma), Some(Container::Wma));
    }

    #[test]
    fn non_audio_returns_none() {
        assert_eq!(
            detect_container(b"this is a plain text file with no audio"),
            None
        );
        assert_eq!(detect_container(b"%PDF-1.7 abcdefghij"), None);
    }

    #[test]
    fn detect_container_never_panics_on_short_or_arbitrary_input() {
        // Regresyon: 12-15 baytlık kısa dosyalar ASF (16 bayt) kontrolünde
        // panik yapıyordu (gerçek kullanımda SD kartta bulundu).
        // 0..64 bayt arası tüm uzunluklar ve çeşitli içerikler panik yapmamalı.
        for len in 0..=64usize {
            // Sıfırlarla dolu.
            assert!(detect_container(&vec![0u8; len]).is_none() || len >= 12);
            // Bilinen imza önekleri.
            for prefix in [
                &b"RIFF"[..],
                b"FORM",
                b"fLaC",
                b"OggS",
                b"caff",
                b"ID3",
                b"#!AMR",
                b"\x30\x26\xB2\x75\x8E\x66\xCF\x11\xA6\xD9\x00\xAA\x00\x62\xCE\x6C",
                b"\x1A\x45\xDF\xA3",
                b"\xFF\xF1",
                b"\xFF\xE0",
            ] {
                let mut v = prefix.to_vec();
                v.resize(len.max(prefix.len()), 0);
                let v = &v[..len.min(v.len())];
                // Panik yapmamalı; sonuç ne olursa olsun.
                let _ = detect_container(v);
            }
            // Rastgele baytlar.
            let v: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(37)).collect();
            let _ = detect_container(&v);
        }
    }

    #[test]
    fn twelve_byte_asf_prefix_does_not_panic() {
        // Tam olarak panik veren durum: 12 baytlık dosya, ASF kontrolü 16 bayt
        // istiyordu.
        let v = &b"\x30\x26\xB2\x75\x8E\x66\xCF\x11\xA6\xD9\x00\xAA"[..];
        assert_eq!(v.len(), 12);
        assert_eq!(detect_container(v), None);
    }

    #[test]
    fn wrong_extension_is_detected_by_content() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("actually_audio.txt");
        std::fs::write(&p, make_wav(16000, 1, 16, 0.25)).unwrap();
        assert!(looks_like_audio(&p));
    }

    #[test]
    fn wav_inventory_extracts_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.wav");
        std::fs::write(&p, make_wav(44100, 2, 16, 2.0)).unwrap();
        let info = inventory(&p).unwrap();
        assert_eq!(info.container, "WAV");
        assert_eq!(info.codec, "PCM");
        assert_eq!(info.sample_rate_hz, Some(44100));
        assert_eq!(info.channels, Some(2));
        assert_eq!(info.bit_depth, Some(16));
        assert!((info.duration_seconds - 2.0).abs() < 0.01);
        assert!(info.readable);
    }

    #[test]
    fn inventory_does_not_load_whole_file() {
        // Bellek dostu: 200 MB'lık bir WAV'ın envanteri hızlı ve düşük bellekli
        // olmalı. Sadece başlık okunur.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("buyuk.wav");
        // 200 MB WAV üret (sıkıştırılamaz gerçek veri).
        let sr = 16000u32;
        let seconds = 200u32; // ~6.4 MB/s * ... aslında 16000*2 = 32KB/s -> ~6.4MB
        let data_len = sr * seconds * 2;
        let mut v = Vec::with_capacity(data_len as usize + 44);
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data_len).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&sr.to_le_bytes());
        v.extend_from_slice(&(sr * 2).to_le_bytes());
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_len.to_le_bytes());
        v.resize(v.len() + data_len as usize, 0);
        std::fs::write(&p, &v).unwrap();
        drop(v);

        // read_header sınırlı okumalı.
        let head = read_header(&p, 512 * 1024).unwrap();
        assert_eq!(head.len(), 512 * 1024);

        // Envanter doğru süreyi vermeli (dosya boyutundan, tümünü okumadan).
        let info = inventory(&p).unwrap();
        assert_eq!(info.sample_rate_hz, Some(sr));
        assert_eq!(info.channels, Some(1));
        assert!(
            (info.duration_seconds - seconds as f64).abs() < 0.1,
            "süre ~{seconds}s olmalı, bulunan {}",
            info.duration_seconds
        );
    }

    #[test]
    fn flac_inventory_extracts_metadata() {
        let mut si = [0u8; 34];
        let sr: u32 = 44100;
        si[10] = (sr >> 12) as u8;
        si[11] = ((sr >> 4) & 0xFF) as u8;
        si[12] = (((sr & 0x0F) as u8) << 4) | (1u8 << 1);
        si[13] = 15u8 << 4;
        let samples: u64 = 88200;
        si[13] = (si[13] & 0xF0) | ((samples >> 32) as u8 & 0x0F);
        si[14] = ((samples >> 24) & 0xFF) as u8;
        si[15] = ((samples >> 16) & 0xFF) as u8;
        si[16] = ((samples >> 8) & 0xFF) as u8;
        si[17] = (samples & 0xFF) as u8;
        let mut v = b"fLaC".to_vec();
        v.extend_from_slice(&[0x00, 0x00, 0x00, 0x22]);
        v.extend_from_slice(&si);
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.flac");
        std::fs::write(&p, &v).unwrap();
        let info = inventory(&p).unwrap();
        assert_eq!(info.container, "FLAC");
        assert_eq!(info.sample_rate_hz, Some(44100));
        assert_eq!(info.channels, Some(2));
        assert_eq!(info.bit_depth, Some(16));
        assert!((info.duration_seconds - 2.0).abs() < 0.01);
    }

    #[test]
    fn video_container_is_flagged() {
        assert!(Container::Mp4.is_video_container());
        assert!(!Container::Wav.is_video_container());
    }
}
