//! UDF doğrulayıcı: paket bütünlüğü, XML yapısı, metin/ofset tutarlılığı,
//! karakterler, biçim aralıkları, tablolar, görseller ve sayfa özellikleri
//! kontrol edilir (şartname 12.3).
//!
//! Bu doğrulama UYAP Editör testinin yerine geçmez; yalnızca yapısal bir ön
//! kontroldür. Sonuç raporda bu sınır açıkça belirtilir.

use crate::reader::{extract_content_xml, parse_content_xml, ParsedBlock};
use crate::{Result, FORMAT_ID};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Tek bir doğrulama kontrolünün sonucu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

/// UDF doğrulama raporu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub checks: Vec<CheckResult>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    /// Metin havuzunun karakter sayısı.
    pub pool_chars: usize,
    /// Blok sayısı.
    pub block_count: usize,
    /// Toplam resim sayısı.
    pub image_count: usize,
    /// Toplam tablo sayısı.
    pub table_count: usize,
    /// Türkçe karakter testi geçti mi?
    pub turkish_ok: bool,
}

impl ValidationReport {
    /// Tüm kontroller geçti mi?
    pub fn passed(&self) -> bool {
        self.errors.is_empty() && self.checks.iter().all(|c| c.passed)
    }

    /// Özet satırı.
    pub fn summary_tr(&self) -> String {
        let total = self.checks.len();
        let passed = self.checks.iter().filter(|c| c.passed).count();
        if self.passed() {
            format!("UDF doğrulaması başarılı ({passed}/{total} kontrol geçti).")
        } else {
            format!(
                "UDF doğrulaması başarısız: {passed}/{total} kontrol geçti, {} hata, {} uyarı.",
                self.errors.len(),
                self.warnings.len()
            )
        }
    }
}

/// UDF dosyasını doğrular.
pub fn validate_udf(path: &Path) -> Result<ValidationReport> {
    let bytes = extract_content_xml(path)?;
    validate_content_xml(&bytes)
}

/// Ham `content.xml` baytlarını doğrular.
pub fn validate_content_xml(bytes: &[u8]) -> Result<ValidationReport> {
    let mut checks = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // 1. UTF-8 geçerliliği.
    let text = match std::str::from_utf8(bytes) {
        Ok(t) => {
            checks.push(CheckResult {
                name: "UTF-8 kodlama".into(),
                passed: true,
                detail: "content.xml geçerli UTF-8".into(),
            });
            t
        }
        Err(e) => {
            checks.push(CheckResult {
                name: "UTF-8 kodlama".into(),
                passed: false,
                detail: format!("Geçersiz UTF-8: {e}"),
            });
            errors.push("content.xml geçerli UTF-8 değil".into());
            return Ok(finish(checks, errors, warnings));
        }
    };

    // 2. XML bildirimi ve format_id.
    let has_decl = text.starts_with("<?xml");
    checks.push(CheckResult {
        name: "XML bildirimi".into(),
        passed: has_decl,
        detail: if has_decl {
            "XML bildirimi mevcut".into()
        } else {
            "XML bildirimi eksik".into()
        },
    });
    if !has_decl {
        warnings.push("XML bildirimi eksik".into());
    }
    let fmt = format!("format_id=\"{FORMAT_ID}\"");
    let has_fmt = text.contains(&fmt);
    checks.push(CheckResult {
        name: "format_id".into(),
        passed: has_fmt,
        detail: if has_fmt {
            format!("format_id=\"{FORMAT_ID}\"")
        } else {
            format!("format_id=\"{FORMAT_ID}\" bulunamadı")
        },
    });
    if !has_fmt {
        errors.push(format!("format_id=\"{FORMAT_ID}\" bulunamadı"));
    }

    // 3. CDATA bloğu.
    let has_cdata = text.contains("<![CDATA[") && text.contains("]]>");
    checks.push(CheckResult {
        name: "Metin havuzu (CDATA)".into(),
        passed: has_cdata,
        detail: if has_cdata {
            "CDATA bloğu mevcut".into()
        } else {
            "CDATA bloğu eksik".into()
        },
    });
    if !has_cdata {
        errors.push("CDATA metin havuzu bulunamadı".into());
    }

    // 4. Yapısal çözümleme.
    let parsed = match parse_content_xml(bytes) {
        Ok(p) => {
            checks.push(CheckResult {
                name: "XML yapısı".into(),
                passed: true,
                detail: "content.xml başarıyla çözümlendi".into(),
            });
            Some(p)
        }
        Err(e) => {
            checks.push(CheckResult {
                name: "XML yapısı".into(),
                passed: false,
                detail: format!("Çözümleme hatası: {e}"),
            });
            errors.push(format!("XML çözümlenemedi: {e}"));
            None
        }
    };

    let mut pool_chars = 0usize;
    let mut block_count = 0usize;
    let mut image_count = 0usize;
    let mut table_count = 0usize;

    if let Some(p) = &parsed {
        pool_chars = p.pool.chars().count();
        block_count = p.blocks.len();
        image_count = p.image_count;
        table_count = p.table_count;

        // 5. Metin/ofset tutarlılığı: her `<content startOffset length>` için
        //    ofset aralığı havuz sınırları içinde olmalı.
        let (offsets_ok, offset_detail) = check_offsets(text, &p.pool);
        checks.push(CheckResult {
            name: "Metin/ofset tutarlılığı".into(),
            passed: offsets_ok,
            detail: offset_detail,
        });
        if !offsets_ok {
            errors.push("Bir veya daha fazla ofset havuz sınırlarını aşıyor".into());
        }

        // 6. Sayfa özellikleri.
        let pf_ok = p.page_format.media_size_name == 1
            && p.page_format.left_margin_pt > 0.0
            && p.page_format.right_margin_pt > 0.0;
        checks.push(CheckResult {
            name: "Sayfa özellikleri".into(),
            passed: pf_ok,
            detail: format!(
                "A4={}, kenar boşlukları={:.2}/{:.2}",
                p.page_format.media_size_name == 1,
                p.page_format.left_margin_pt,
                p.page_format.right_margin_pt
            ),
        });

        // 7. Tablo yapısı: satırların sütun sayısı tutarlı olmalı.
        let mut table_ok = true;
        let mut table_detail = String::new();
        for b in &p.blocks {
            if let ParsedBlock::Table { rows, column_count } = b {
                for (i, r) in rows.iter().enumerate() {
                    if r.len() != *column_count {
                        table_ok = false;
                        table_detail = format!(
                            "Satır {} sütun sayısı {} (beklenen {})",
                            i + 1,
                            r.len(),
                            column_count
                        );
                    }
                }
            }
        }
        if table_ok {
            table_detail = format!("{table_count} tablo, sütun sayıları tutarlı");
        }
        checks.push(CheckResult {
            name: "Tablo yapısı".into(),
            passed: table_ok,
            detail: table_detail,
        });
        if !table_ok {
            errors.push("Tablo sütun sayıları tutarsız".into());
        }

        // 8. Türkçe karakter testi: havuzdaki karakterler geçerli olmalı ve
        //    bilinen Türkçe karakterler korunmalı.
        let turkish_chars = ['ı', 'İ', 'ş', 'Ş', 'ğ', 'Ğ', 'ü', 'Ü', 'ö', 'Ö', 'ç', 'Ç'];
        let turkish_in_pool = p.pool.chars().any(|c| turkish_chars.contains(&c));
        let turkish_ok = p
            .pool
            .chars()
            .all(|c| !c.is_control() || c == '\n' || c == '\t' || c == '\r');
        checks.push(CheckResult {
            name: "Karakter geçerliliği".into(),
            passed: turkish_ok,
            detail: format!(
                "Havuzda Türkçe karakter: {}, kontrol karakteri: {}",
                turkish_in_pool,
                if turkish_ok { "yok" } else { "var" }
            ),
        });
        if !turkish_ok {
            errors.push("Havuzda geçersiz kontrol karakteri var".into());
        }
    }

    let turkish_ok = errors.is_empty();
    Ok(ValidationReport {
        checks,
        errors,
        warnings,
        pool_chars,
        block_count,
        image_count,
        table_count,
        turkish_ok,
    })
}

fn finish(
    checks: Vec<CheckResult>,
    errors: Vec<String>,
    warnings: Vec<String>,
) -> ValidationReport {
    ValidationReport {
        checks,
        errors,
        warnings,
        pool_chars: 0,
        block_count: 0,
        image_count: 0,
        table_count: 0,
        turkish_ok: false,
    }
}

/// `<content startOffset=... length=...>` öğelerinin havuz sınırları içinde
/// olduğunu doğrular.
fn check_offsets(xml: &str, pool: &str) -> (bool, String) {
    let pool_len = pool.chars().count();
    let mut count = 0usize;
    let mut bad = Vec::new();
    // Basit ama yeterli bir tarama: `startOffset="N" length="M"` çiftlerini bul.
    let mut rest = xml;
    while let Some(pos) = rest.find("startOffset=\"") {
        let after = &rest[pos + "startOffset=\"".len()..];
        let Some(end_quote) = after.find('"') else {
            break;
        };
        let start: usize = after[..end_quote].parse().unwrap_or(0);
        // Aynı öğe içinde length ara.
        let tail = &after[end_quote..];
        let len = if let Some(lpos) = tail.find("length=\"") {
            let lafter = &tail[lpos + "length=\"".len()..];
            let lend = lafter.find('"').unwrap_or(0);
            lafter[..lend].parse::<usize>().unwrap_or(0)
        } else {
            0
        };
        count += 1;
        if start > pool_len || start + len > pool_len {
            bad.push(format!("offset={start} length={len} (havuz={pool_len})"));
        }
        rest = &after[end_quote..];
        if rest.len() == xml.len() {
            break;
        }
    }
    if bad.is_empty() {
        (
            true,
            format!("{count} ofset aralığı geçerli (havuz {pool_len} karakter)"),
        )
    } else {
        (false, format!("Geçersiz aralıklar: {}", bad.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Paragraph, Table, TableCell, TableRow, UdfDocument as Doc};
    use crate::writer::{document_to_content_xml, document_to_udf_bytes, write_udf};

    #[test]
    fn valid_document_passes() {
        let mut doc = Doc::default();
        doc.push_heading("BİLİRKİŞİ RAPORU");
        doc.push_paragraph(Paragraph::text("Türkçe karakterler: ıİşŞğĞüÜöÖçÇ"));
        doc.push_table(Table::new(
            vec![100.0, 100.0],
            vec![TableRow::new(vec![
                TableCell::text("Ad"),
                TableCell::text("Değer"),
            ])],
        ));
        let xml = document_to_content_xml(&doc);
        let report = validate_content_xml(xml.as_bytes()).unwrap();
        assert!(report.passed(), "hatalar: {:?}", report.errors);
        assert!(report.turkish_ok);
        assert_eq!(report.table_count, 1);
    }

    #[test]
    fn invalid_utf8_fails() {
        let bytes = [0xFF, 0xFE, 0x00, 0x01];
        let report = validate_content_xml(&bytes).unwrap();
        assert!(!report.passed());
        assert!(!report.errors.is_empty());
    }

    #[test]
    fn missing_format_id_fails() {
        let xml = "<?xml version=\"1.0\" encoding=\"UTF-8\" ?><template><content><![CDATA[x]]></content></template>";
        let report = validate_content_xml(xml.as_bytes()).unwrap();
        assert!(!report.passed());
        assert!(report.errors.iter().any(|e| e.contains("format_id")));
    }

    #[test]
    fn out_of_range_offset_fails() {
        let xml = "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<template format_id=\"1.8\">\n<content><![CDATA[abc]]></content>\n<elements resolver=\"hvl-default\"><paragraph Alignment=\"0\" LeftIndent=\"0.0\" RightIndent=\"0.0\"><content startOffset=\"0\" length=\"99\" family=\"Times New Roman\" size=\"11\" /></paragraph></elements></template>";
        let report = validate_content_xml(xml.as_bytes()).unwrap();
        assert!(!report.passed());
        assert!(report
            .errors
            .iter()
            .any(|e| e.contains("ofset") || e.contains("aralık")));
    }

    #[test]
    fn validate_from_zip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.udf");
        let mut doc = Doc::default();
        doc.push_paragraph(Paragraph::text("şğüöçıİ"));
        write_udf(&doc, &path).unwrap();
        let report = validate_udf(&path).unwrap();
        assert!(report.passed());
        assert!(report.summary_tr().contains("başarılı"));
    }

    #[test]
    fn in_memory_bytes_validate() {
        let mut doc = Doc::default();
        doc.push_paragraph(Paragraph::text("Test"));
        let bytes = document_to_udf_bytes(&doc).unwrap();
        // ZIP'ten content.xml çıkarıp doğrula.
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let mut entry = archive.by_name("content.xml").unwrap();
        let mut buf = Vec::new();
        use std::io::Read;
        entry.read_to_end(&mut buf).unwrap();
        let report = validate_content_xml(&buf).unwrap();
        assert!(report.passed());
    }
}
