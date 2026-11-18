//! PDF belgesinden metin çıkarma.
//!
//! Önce PDF metin katmanı okunur. Metin katmanı yoksa (taranmış PDF) çağıran
//! taraf OCR'a yönlendirir.

use super::{AssignmentError, ExtractedText};
use delil_core::DocumentReadMethod;
use std::path::Path;

/// PDF'ten metin çıkarır.
pub fn extract(path: &Path) -> Result<ExtractedText, AssignmentError> {
    let bytes = std::fs::read(path)?;
    let text = pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| AssignmentError::Parse(format!("PDF metni çıkarılamadı: {e}")))?;
    // Sayfa ayırıcı olarak form feed (\x0c) kullanılıyorsa sayfalara böl.
    let pages: Vec<String> = if text.contains('\u{0c}') {
        text.split('\u{0c}').map(|s| s.to_string()).collect()
    } else {
        vec![text]
    };
    Ok(ExtractedText {
        pages,
        method: DocumentReadMethod::PdfTextLayer,
        document_kind: "PDF".into(),
    })
}

/// PDF'in metin katmanı içerip içermediğini kaba biçimde kontrol eder.
/// Taranmış PDF'lerde metin katmanı bulunmaz.
pub fn has_text_layer(path: &Path) -> bool {
    match extract(path) {
        Ok(t) => t.full_text().trim().chars().count() > 20,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_pdf_reports_error() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("fake.pdf");
        std::fs::write(&p, b"bu bir pdf degil").unwrap();
        assert!(extract(&p).is_err());
        assert!(!has_text_layer(&p));
    }
}
