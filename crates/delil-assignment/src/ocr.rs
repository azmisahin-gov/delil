//! Yerel OCR (şartname 5.1).
//!
//! Taranmış/görsel belgeler için `ocrs` motoru kullanılır. OCR tamamen
//! yereldir; internet gerektirmez. Türkçe karakterler desteklenir.
//!
//! Model dosyaları kurulum paketine dahil edilir; kullanıcıdan ayrı indirme
//! istenmez. Model yolu `DELIL_OCR_MODEL_DIR` ortam değişkeniyle veya
//! yürütülebilir dosyanın yanındaki `models/ocr` diziniyle belirlenir.
//!
//! Beklenen dosya adları (ocrs modelleri, `robertknight/ocrs`):
//! * `text-detection.rten`
//! * `text-recognition.rten`
//!
//! `.onnx` uzantılı sürümler de kabul edilir. Modeller **CC-BY-SA-4.0**
//! lisanslıdır; yeniden dağıtımda atıf ve lisans bilgisi korunur
//! (bkz. `docs/OCR_LISANS.md`).

use super::{AssignmentError, ExtractedText};
use delil_core::DocumentReadMethod;
use std::path::{Path, PathBuf};

/// OCR model dosya adı adayları: (tespit, tanıma).
///
/// İlk eşleşen çift kullanılır. `.rten` tercih edilir (saf Rust çalışma
/// zamanı); `.onnx` yedeği de desteklenir.
const MODEL_NAME_CANDIDATES: &[(&str, &str)] = &[
    ("text-detection.rten", "text-recognition.rten"),
    (
        "text-detection-ssfbcj81.rten",
        "text-rec-checkpoint-s52qdbqt.rten",
    ),
    ("text-detection.onnx", "text-recognition.onnx"),
    (
        "text-detection-ssfbcj81.onnx",
        "text-rec-checkpoint-s52qdbqt.onnx",
    ),
];

/// OCR model dizinini belirler.
pub fn model_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("DELIL_OCR_MODEL_DIR") {
        let p = PathBuf::from(dir);
        if p.exists() {
            return Some(p);
        }
    }
    // Yürütülebilir dosyanın yanında.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let p = parent.join("models").join("ocr");
            if p.exists() {
                return Some(p);
            }
            let p2 = parent.join("models");
            if p2.exists() {
                return Some(p2);
            }
        }
    }
    // Geliştirme: depo kökündeki models/ocr.
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("models")
        .join("ocr");
    if p.exists() {
        return Some(p);
    }
    None
}

/// Model dizinindeki tespit ve tanıma modeli yollarını bulur.
pub fn resolve_model_files(dir: &Path) -> Option<(PathBuf, PathBuf)> {
    for (det, rec) in MODEL_NAME_CANDIDATES {
        let d = dir.join(det);
        let r = dir.join(rec);
        if d.exists() && r.exists() {
            return Some((d, r));
        }
    }
    None
}

/// Görsel belgeyi OCR ile okur.
pub fn extract(path: &Path) -> Result<ExtractedText, AssignmentError> {
    let text = run_ocr(path)?;
    Ok(ExtractedText {
        pages: vec![text],
        method: DocumentReadMethod::Ocr,
        document_kind: "OCR".into(),
    })
}

/// Görsel dosya üzerinde OCR çalıştırır.
pub fn run_ocr(path: &Path) -> Result<String, AssignmentError> {
    use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
    use rten::Model;

    let dir = model_dir().ok_or_else(|| {
        AssignmentError::Parse(
            "OCR model dizini bulunamadı. Kurulum paketi OCR modellerini içermelidir.".into(),
        )
    })?;
    let (det, rec) = resolve_model_files(&dir).ok_or_else(|| {
        AssignmentError::Parse(format!(
            "OCR model dosyaları eksik ({}). Beklenen: text-detection.rten ve text-recognition.rten.",
            dir.display()
        ))
    })?;

    let detection_model = Model::load_file(&det)
        .map_err(|e| AssignmentError::Parse(format!("Tespit modeli yüklenemedi: {e}")))?;
    let recognition_model = Model::load_file(&rec)
        .map_err(|e| AssignmentError::Parse(format!("Tanıma modeli yüklenemedi: {e}")))?;

    let engine = OcrEngine::new(OcrEngineParams {
        detection_model: Some(detection_model),
        recognition_model: Some(recognition_model),
        ..Default::default()
    })
    .map_err(|e| AssignmentError::Parse(format!("OCR motoru başlatılamadı: {e}")))?;

    let img = image::open(path)
        .map_err(|e| AssignmentError::Parse(format!("Görsel açılamadı: {e}")))?
        .to_rgb8();
    let (w, h) = img.dimensions();

    let img_source = ImageSource::from_bytes(img.as_raw(), (w, h))
        .map_err(|e| AssignmentError::Parse(format!("Görsel kaynağı hazırlanamadı: {e}")))?;
    let input = engine
        .prepare_input(img_source)
        .map_err(|e| AssignmentError::Parse(format!("OCR girdisi hazırlanamadı: {e}")))?;
    let text = engine
        .get_text(&input)
        .map_err(|e| AssignmentError::Parse(format!("OCR metni alınamadı: {e}")))?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_image_reports_error() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("yok.png");
        // Görsel yoksa veya model yoksa hata döner; ikisi de kabul edilebilir.
        assert!(extract(&p).is_err());
    }

    #[test]
    fn missing_models_reports_clear_error() {
        // Model dizini yoksa açık bir hata beklenir.
        std::env::set_var("DELIL_OCR_MODEL_DIR", "/nonexistent/ocr/models");
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.png");
        // Küçük geçerli bir PNG yaz.
        let img = image::RgbImage::from_pixel(4, 4, image::Rgb([255, 255, 255]));
        img.save(&p).unwrap();
        let err = run_ocr(&p).unwrap_err();
        // Model yolu geçersiz olduğundan model yüklenemedi hatası beklenir.
        assert!(err.to_string().contains("model") || err.to_string().contains("Model"));
        std::env::remove_var("DELIL_OCR_MODEL_DIR");
    }

    #[test]
    fn resolve_model_files_finds_rten_pair() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("text-detection.rten"), b"x").unwrap();
        std::fs::write(dir.path().join("text-recognition.rten"), b"y").unwrap();
        let (d, r) = resolve_model_files(dir.path()).unwrap();
        assert!(d.ends_with("text-detection.rten"));
        assert!(r.ends_with("text-recognition.rten"));
    }

    #[test]
    fn resolve_model_files_none_when_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("text-detection.rten"), b"x").unwrap();
        assert!(resolve_model_files(dir.path()).is_none());
    }
}
