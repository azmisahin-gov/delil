//! UDF okuyucu: UDF paketini çözümleyip metin ve yapıyı çıkarır.
//!
//! Okuyucu, `content.xml` içindeki metin havuzunu ve `<elements>` ağacını
//! ayrıştırır. `startOffset` ve `length` değerleri karakter cinsinden
//! yorumlanır; bu nedenle metin `Vec<char>` üzerinden dilimlenir.

use crate::model::PageFormat;
use crate::{Result, UdfError};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

/// Çözümlenmiş UDF belgesi: düz metin + bloklar + sayfa bilgisi.
#[derive(Debug, Clone, PartialEq)]
pub struct UdfDocument {
    /// Metin havuzu (CDATA içeriği).
    pub pool: String,
    /// Çözümlenen bloklar.
    pub blocks: Vec<ParsedBlock>,
    pub page_format: PageFormat,
    /// Üstbilgi metni.
    pub header_text: Option<String>,
    /// Altbilgi metni.
    pub footer_text: Option<String>,
    /// Sayfa numarası belirteci (varsa).
    pub page_number_spec: Option<String>,
    /// Belgede bulunan resim sayısı.
    pub image_count: usize,
    /// Belgede bulunan tablo sayısı.
    pub table_count: usize,
    /// Belgede bulunan sayfa sonu sayısı.
    pub page_break_count: usize,
}

/// Çözümlenmiş blok (okuma amaçlı basitleştirilmiş).
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedBlock {
    /// Düz metin paragrafı.
    Paragraph {
        text: String,
        alignment: u8,
        list: Option<String>,
    },
    /// Tablo; satır ve hücrelerin düz metni.
    Table {
        rows: Vec<Vec<String>>,
        column_count: usize,
    },
    /// Sayfa sonu.
    PageBreak,
}

impl UdfDocument {
    /// Belgenin tam düz metni (paragraflar satır sonuyla ayrılır).
    pub fn plain_text(&self) -> String {
        let mut out = Vec::new();
        for b in &self.blocks {
            match b {
                ParsedBlock::Paragraph { text, .. } => out.push(text.clone()),
                ParsedBlock::Table { rows, .. } => {
                    for r in rows {
                        out.push(r.join(" | "));
                    }
                }
                ParsedBlock::PageBreak => out.push(String::new()),
            }
        }
        out.join("\n")
    }

    /// Belgede bir metin var mı?
    pub fn contains_text(&self, needle: &str) -> bool {
        self.pool.contains(needle)
    }
}

fn slice_pool(chars: &[char], start: usize, len: usize) -> String {
    if start >= chars.len() {
        return String::new();
    }
    let end = (start + len).min(chars.len());
    chars[start..end].iter().collect()
}

fn attrs(e: &quick_xml::events::BytesStart) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for a in e.attributes().flatten() {
        let key = String::from_utf8_lossy(a.key.as_ref()).into_owned();
        let val = String::from_utf8_lossy(&a.value).into_owned();
        m.insert(key, val);
    }
    m
}

fn attr_f64(m: &HashMap<String, String>, k: &str, default: f64) -> f64 {
    m.get(k).and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn attr_u8(m: &HashMap<String, String>, k: &str, default: u8) -> u8 {
    m.get(k).and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// ZIP paketinden `content.xml` baytlarını çıkarır.
pub fn extract_content_xml(path: &Path) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entry = archive
        .by_name("content.xml")
        .map_err(|_| UdfError::MissingContent)?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

/// UDF paketini okur ve çözümler.
pub fn read_udf(path: &Path) -> Result<UdfDocument> {
    let bytes = extract_content_xml(path)?;
    parse_content_xml(&bytes)
}

/// Paragraf akümülatörü.
struct ParaAcc {
    text: String,
    alignment: u8,
    list: Option<String>,
}

/// Tablo akümülatörü.
#[derive(Default)]
struct TableAcc {
    rows: Vec<Vec<String>>,
    current_row: Option<Vec<String>>,
    current_cell: Option<String>,
    column_count: usize,
}

/// Ayrıştırma sırasında taşınan değişken durum.
struct ParseState {
    pool: Option<String>,
    pool_chars: Vec<char>,
    pool_ready: bool,
    blocks: Vec<ParsedBlock>,
    page_format: PageFormat,
    header_text: Option<String>,
    footer_text: Option<String>,
    page_number_spec: Option<String>,
    image_count: usize,
    table_count: usize,
    page_break_count: usize,
    in_content_pool: bool,
    in_header: bool,
    in_footer: bool,
    in_page_break: bool,
    in_table: bool,
    para: Option<ParaAcc>,
    table: TableAcc,
    header_acc: String,
    footer_acc: String,
}

impl ParseState {
    fn new() -> Self {
        Self {
            pool: None,
            pool_chars: Vec::new(),
            pool_ready: false,
            blocks: Vec::new(),
            page_format: PageFormat::default(),
            header_text: None,
            footer_text: None,
            page_number_spec: None,
            image_count: 0,
            table_count: 0,
            page_break_count: 0,
            in_content_pool: false,
            in_header: false,
            in_footer: false,
            in_page_break: false,
            in_table: false,
            para: None,
            table: TableAcc::default(),
            header_acc: String::new(),
            footer_acc: String::new(),
        }
    }

    fn append_inline(&mut self, s: &str) {
        if let Some(p) = self.para.as_mut() {
            p.text.push_str(s);
        }
    }

    fn ensure_pool_chars(&mut self) {
        if !self.pool_ready {
            self.pool_chars = self.pool.as_deref().unwrap_or("").chars().collect();
            self.pool_ready = true;
        }
    }

    fn handle_start(&mut self, lname: &str, a: &HashMap<String, String>, is_empty: bool) {
        match lname {
            "content" => {
                if let (Some(start_s), Some(len_s)) = (a.get("startOffset"), a.get("length")) {
                    self.ensure_pool_chars();
                    let start = start_s.parse::<usize>().unwrap_or(0);
                    let len = len_s.parse::<usize>().unwrap_or(0);
                    let s = slice_pool(&self.pool_chars, start, len);
                    self.append_inline(&s);
                } else if !is_empty {
                    self.in_content_pool = true;
                }
            }
            "tab" => self.append_inline("\t"),
            "image" => {
                self.image_count += 1;
                self.append_inline(&crate::OBJECT_REPLACEMENT_CHAR.to_string());
            }
            "paragraph" => {
                let list = if a.get("Numbered").map(|v| v == "true").unwrap_or(false) {
                    a.get("NumberType").cloned()
                } else if a.get("Bulleted").map(|v| v == "true").unwrap_or(false) {
                    a.get("BulletType").cloned()
                } else {
                    None
                };
                self.para = Some(ParaAcc {
                    text: String::new(),
                    alignment: a.get("Alignment").and_then(|v| v.parse().ok()).unwrap_or(0),
                    list,
                });
            }
            "table" => {
                self.in_table = true;
                self.table_count += 1;
                self.table.column_count = a
                    .get("columnCount")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                self.table.rows.clear();
            }
            "row" => self.table.current_row = Some(Vec::new()),
            "cell" => self.table.current_cell = Some(String::new()),
            "page-break" => {
                self.in_page_break = true;
                self.page_break_count += 1;
            }
            "header" => self.in_header = true,
            "footer" => {
                self.in_footer = true;
                self.page_number_spec = a.get("pageNumber-spec").cloned();
            }
            "pageFormat" => {
                self.page_format = PageFormat {
                    media_size_name: attr_u8(a, "mediaSizeName", 1),
                    left_margin_pt: attr_f64(a, "leftMargin", 42.52),
                    right_margin_pt: attr_f64(a, "rightMargin", 28.35),
                    top_margin_pt: attr_f64(a, "topMargin", 14.17),
                    bottom_margin_pt: attr_f64(a, "bottomMargin", 14.17),
                    paper_orientation: attr_u8(a, "paperOrientation", 1),
                    header_offset_pt: attr_f64(a, "headerFOffset", 20.0),
                    footer_offset_pt: attr_f64(a, "footerFOffset", 20.0),
                };
            }
            _ => {}
        }
    }

    fn handle_end(&mut self, lname: &str) {
        match lname {
            "paragraph" => {
                if let Some(p) = self.para.take() {
                    if self.in_page_break {
                        self.blocks.push(ParsedBlock::PageBreak);
                        self.in_page_break = false;
                    } else if self.in_header {
                        if !self.header_acc.is_empty() {
                            self.header_acc.push('\n');
                        }
                        self.header_acc.push_str(&p.text);
                    } else if self.in_footer {
                        if !self.footer_acc.is_empty() {
                            self.footer_acc.push('\n');
                        }
                        self.footer_acc.push_str(&p.text);
                    } else if self.in_table {
                        if let Some(cell) = self.table.current_cell.as_mut() {
                            if !cell.is_empty() {
                                cell.push('\n');
                            }
                            cell.push_str(&p.text);
                        }
                    } else {
                        self.blocks.push(ParsedBlock::Paragraph {
                            text: p.text,
                            alignment: p.alignment,
                            list: p.list,
                        });
                    }
                }
            }
            "cell" => {
                if let (Some(cell), Some(row)) =
                    (self.table.current_cell.take(), self.table.current_row.as_mut())
                {
                    row.push(cell);
                }
            }
            "row" => {
                if let Some(row) = self.table.current_row.take() {
                    self.table.rows.push(row);
                }
            }
            "table" => {
                self.in_table = false;
                self.blocks.push(ParsedBlock::Table {
                    rows: std::mem::take(&mut self.table.rows),
                    column_count: self.table.column_count,
                });
            }
            "header" => {
                self.in_header = false;
                if !self.header_acc.is_empty() {
                    self.header_text = Some(self.header_acc.clone());
                }
            }
            "footer" => {
                self.in_footer = false;
                if !self.footer_acc.is_empty() {
                    self.footer_text = Some(self.footer_acc.clone());
                }
            }
            _ => {}
        }
    }
}

/// `content.xml` baytlarını çözümler.
///
/// `<content>` öğesi iki bağlamda görünür: üst düzey metin havuzu ve paragraf
/// içi biçimli metin. Bu ayrım `startOffset` özniteliğinin varlığıyla yapılır.
pub fn parse_content_xml(bytes: &[u8]) -> Result<UdfDocument> {
    let text = std::str::from_utf8(bytes).map_err(|e| UdfError::Xml(e.to_string()))?;
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(false);
    reader.config_mut().expand_empty_elements = false;

    let mut st = ParseState::new();

    loop {
        let ev = reader
            .read_event()
            .map_err(|e| UdfError::Xml(e.to_string()))?;
        match ev {
            Event::Start(e) => {
                let name = e.name();
                let lname = String::from_utf8_lossy(name.local_name().as_ref()).into_owned();
                let a = attrs(&e);
                st.handle_start(&lname, &a, false);
            }
            Event::Empty(e) => {
                let name = e.name();
                let lname = String::from_utf8_lossy(name.local_name().as_ref()).into_owned();
                let a = attrs(&e);
                st.handle_start(&lname, &a, true);
                st.handle_end(&lname);
            }
            Event::End(e) => {
                let name = e.name();
                let lname = String::from_utf8_lossy(name.local_name().as_ref()).into_owned();
                if lname == "content" {
                    st.in_content_pool = false;
                    continue;
                }
                st.handle_end(&lname);
            }
            Event::CData(e) => {
                if st.in_content_pool {
                    st.pool = Some(String::from_utf8_lossy(&e.into_inner()).into_owned());
                    st.pool_ready = false;
                }
            }
            Event::Text(e) => {
                if st.in_content_pool && st.pool.is_none() {
                    let s = e.unescape().map_err(|er| UdfError::Xml(er.to_string()))?;
                    st.pool = Some(s.into_owned());
                    st.pool_ready = false;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    let pool = st.pool.ok_or(UdfError::MissingContent)?;
    Ok(UdfDocument {
        pool,
        blocks: st.blocks,
        page_format: st.page_format,
        header_text: st.header_text,
        footer_text: st.footer_text,
        page_number_spec: st.page_number_spec,
        image_count: st.image_count,
        table_count: st.table_count,
        page_break_count: st.page_break_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Paragraph, Table, TableCell, TableRow, UdfDocument as Doc};
    use crate::writer::{document_to_content_xml, write_udf};

    #[test]
    fn parse_simple_document() {
        let mut doc = Doc::default();
        doc.push_paragraph(Paragraph::text("Birinci paragraf"));
        doc.push_paragraph(Paragraph::text("İkinci paragraf"));
        let xml = document_to_content_xml(&doc);
        let parsed = parse_content_xml(xml.as_bytes()).unwrap();
        assert_eq!(parsed.pool, "Birinci paragraf\nİkinci paragraf");
        assert_eq!(parsed.blocks.len(), 2);
        match &parsed.blocks[0] {
            ParsedBlock::Paragraph { text, .. } => assert_eq!(text, "Birinci paragraf"),
            _ => panic!("paragraf bekleniyordu"),
        }
    }

    #[test]
    fn parse_turkish_characters_exactly() {
        let mut doc = Doc::default();
        doc.push_paragraph(Paragraph::text("Şüpheli: İĞÜŞÖÇ ığüşöç 123"));
        let xml = document_to_content_xml(&doc);
        let parsed = parse_content_xml(xml.as_bytes()).unwrap();
        assert!(parsed.plain_text().contains("Şüpheli: İĞÜŞÖÇ ığüşöç 123"));
    }

    #[test]
    fn roundtrip_table() {
        let mut doc = Doc::default();
        doc.push_table(Table::new(
            vec![100.0, 100.0],
            vec![
                TableRow::new(vec![TableCell::text("A"), TableCell::text("B")]),
                TableRow::new(vec![TableCell::text("1"), TableCell::text("2")]),
            ],
        ));
        let xml = document_to_content_xml(&doc);
        let parsed = parse_content_xml(xml.as_bytes()).unwrap();
        assert_eq!(parsed.table_count, 1);
        match &parsed.blocks[0] {
            ParsedBlock::Table { rows, column_count } => {
                assert_eq!(*column_count, 2);
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0], vec!["A", "B"]);
                assert_eq!(rows[1], vec!["1", "2"]);
            }
            _ => panic!("tablo bekleniyordu"),
        }
    }

    #[test]
    fn read_from_zip_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("r.udf");
        let mut doc = Doc::default();
        doc.push_heading("Rapor Başlığı");
        doc.push_paragraph(Paragraph::text("Gövde metni."));
        write_udf(&doc, &path).unwrap();
        let parsed = read_udf(&path).unwrap();
        assert!(parsed.plain_text().contains("Rapor Başlığı"));
        assert!(parsed.plain_text().contains("Gövde metni."));
    }

    #[test]
    fn missing_content_xml_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.udf");
        let f = std::fs::File::create(&path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        z.start_file::<_, ()>("other.txt", zip::write::FileOptions::default())
            .unwrap();
        use std::io::Write;
        z.write_all(b"x").unwrap();
        z.finish().unwrap();
        assert!(matches!(read_udf(&path), Err(UdfError::MissingContent)));
    }

    #[test]
    fn header_and_footer_are_extracted() {
        let mut doc = Doc::default();
        doc.header = Some(crate::model::Header {
            paragraphs: vec![Paragraph::text("ÜSTBİLGİ")],
        });
        doc.footer = Some(crate::model::Footer {
            paragraphs: vec![Paragraph::text("ALTBİLGİ")],
            page_number_spec: Some("BSP32_40".to_string()),
            ..Default::default()
        });
        doc.push_paragraph(Paragraph::text("gövde"));
        let xml = document_to_content_xml(&doc);
        let parsed = parse_content_xml(xml.as_bytes()).unwrap();
        assert_eq!(parsed.header_text.as_deref(), Some("ÜSTBİLGİ"));
        assert_eq!(parsed.footer_text.as_deref(), Some("ALTBİLGİ"));
        assert_eq!(parsed.page_number_spec.as_deref(), Some("BSP32_40"));
    }
}
