//! UDF görevlendirme belgesinden metin çıkarma.
//!
//! Görevlendirme bir UDF olarak verildiğinde, DELİL'in kendi UDF okuyucusu
//! kullanılarak metin çıkarılır.

use super::{AssignmentError, ExtractedText};
use delil_core::DocumentReadMethod;
use std::path::Path;

/// UDF belgesinden metin çıkarır.
pub fn extract(path: &Path) -> Result<ExtractedText, AssignmentError> {
    let doc = delil_udf::read_udf(path)?;
    let text = doc.plain_text();
    Ok(ExtractedText {
        pages: vec![text],
        method: DocumentReadMethod::Udf,
        document_kind: "UDF".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use delil_udf::model::{Paragraph, UdfDocument};
    use delil_udf::write_udf;

    #[test]
    fn udf_assignment_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("gorev.udf");
        let mut doc = UdfDocument::default();
        doc.push_paragraph(Paragraph::text("Dosya No: 2024/777 Esas"));
        doc.push_paragraph(Paragraph::text(
            "Bilirkişiye Sorulan Sorular:",
        ));
        doc.push_paragraph(Paragraph::text("1. Kayıtta ne konuşuluyor?"));
        write_udf(&doc, &p).unwrap();

        let t = extract(&p).unwrap();
        assert_eq!(t.document_kind, "UDF");
        assert!(t.full_text().contains("2024/777"));
    }
}
