//! DELİL entegrasyon testleri için ortak yardımcılar.
//!
//! Bu crate üretim kodu içermez; yalnızca gerçekçi test verisi üreten ve
//! uçtan uca senaryoları kuran yardımcıları barındırır.

use std::path::{Path, PathBuf};

/// 16-bit PCM WAV dosyası üretir (isteğe bağlı sinüs tonu).
pub fn make_wav(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    seconds: f64,
    tone_hz: f32,
) -> PathBuf {
    let frames = (sample_rate as f64 * seconds) as usize;
    let bytes_per_sample = 2usize;
    let data_len = frames * channels as usize * bytes_per_sample;
    let byte_rate = sample_rate * channels as u32 * bytes_per_sample as u32;

    let mut data = Vec::with_capacity(data_len);
    for i in 0..frames {
        let t = i as f32 / sample_rate as f32;
        let s = if tone_hz > 0.0 {
            ((2.0 * std::f32::consts::PI * tone_hz * t).sin() * 0.4 * 32767.0) as i16
        } else {
            0
        };
        for _ in 0..channels {
            data.extend_from_slice(&s.to_le_bytes());
        }
    }

    let mut v = Vec::new();
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    v.extend_from_slice(b"WAVE");
    v.extend_from_slice(b"fmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&channels.to_le_bytes());
    v.extend_from_slice(&sample_rate.to_le_bytes());
    v.extend_from_slice(&byte_rate.to_le_bytes());
    v.extend_from_slice(&(channels * bytes_per_sample as u16).to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&(data_len as u32).to_le_bytes());
    v.extend_from_slice(&data);

    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::write(path, v).unwrap();
    path.to_path_buf()
}

/// Gerçekçi bir Türkçe görevlendirme belgesi metni.
pub const ASSIGNMENT_TEXT: &str = "T.C.
ANKARA 5. ASLİYE CEZA MAHKEMESİ

Dosya No: 2024/312 Esas
Görevlendiren Merci: Ankara 5. Asliye Ceza Mahkemesi
Mahkeme: Ankara 5. Asliye Ceza Mahkemesi
Konu: Ses kaydının metne dönüştürülmesi
Görevlendirme Tarihi: 03.02.2024
Rapor Teslim Süresi: 03.03.2024
Şüpheli: Kemal Öztürk
Mağdur: Selin Şahin

İncelenecek Materyaller:
1 adet USB bellek içinde ses kayıtları

Bilirkişiye Sorulan Sorular:
1. Kayıtta kimler konuşmaktadır?
2. Kayıtta geçen tarih, saat ve tutarlar nelerdir?
3. Kayıtta tehdit veya hakaret içeren bir ifade var mıdır?
";

/// Test vakası dizini ve görevlendirme dosyasını hazırlar.
pub fn setup_case_dir(root: &Path) -> (PathBuf, PathBuf) {
    let usb = root.join("usb");
    std::fs::create_dir_all(&usb).unwrap();
    let asg = root.join("gorevlendirme.txt");
    std::fs::write(&asg, ASSIGNMENT_TEXT).unwrap();
    (usb, asg)
}

/// İki WAV dosyası ve bir metin dosyası içeren gerçekçi bir USB kaynağı kurar.
pub fn setup_usb_with_audio(usb: &Path) {
    make_wav(&usb.join("kayit_01.wav"), 16000, 1, 2.0, 440.0);
    make_wav(&usb.join("kayit_02.wav"), 44100, 2, 1.5, 880.0);
    std::fs::write(usb.join("notlar.txt"), b"Bu bir metin dosyasidir.").unwrap();
    // İçeriği ses olan ama uzantısı yanlış bir dosya: uzantıya güvenilmemeli.
    make_wav(&usb.join("gizli_ses.dat"), 16000, 1, 0.5, 220.0);
}
