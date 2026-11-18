//! UDF yazıcı: [`UdfDocument`] modelinden gerçek UDF (ZIP + content.xml) üretir.
//!
//! Üretim iki aşamalıdır:
//! 1. Metin havuzu (CDATA) kurulur. Her paragrafın metni sırayla eklenir ve
//!    paragraflar arasına `\n` konur. Resimler `\uFFFC`, sekmeler `\t` olarak
//!    yer kaplar.
//! 2. Her paragraf için `<content startOffset length>` öğeleri üretilir.
//!    Ofset ve uzunluk **karakter** cinsindendir.

use crate::model::*;
use crate::{Result, UdfError, FORMAT_ID};
use std::io::Write;
use std::path::Path;

/// Bir `<content>` öğesi için hesaplanan ofset aralığı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    start: usize,
    len: usize,
}

/// UDF belgesi kuran yardımcı.
#[derive(Debug, Default)]
pub struct UdfBuilder {
    doc: UdfDocument,
}

impl UdfBuilder {
    pub fn new() -> Self {
        Self {
            doc: UdfDocument::default(),
        }
    }

    pub fn page_format(mut self, pf: PageFormat) -> Self {
        self.doc.page_format = pf;
        self
    }

    pub fn header(mut self, h: Header) -> Self {
        self.doc.header = Some(h);
        self
    }

    pub fn footer(mut self, f: Footer) -> Self {
        self.doc.footer = Some(f);
        self
    }

    pub fn paragraph(mut self, p: Paragraph) -> Self {
        self.doc.push_paragraph(p);
        self
    }

    pub fn heading(mut self, text: impl Into<String>) -> Self {
        self.doc.push_heading(text);
        self
    }

    pub fn table(mut self, t: Table) -> Self {
        self.doc.push_table(t);
        self
    }

    pub fn page_break(mut self) -> Self {
        self.doc.push_page_break();
        self
    }

    pub fn build(self) -> UdfDocument {
        self.doc
    }

    /// Belgeyi UDF olarak diske yazar.
    pub fn write_to(self, path: &Path) -> Result<()> {
        write_udf(&self.doc, path)
    }
}

/// XML özel karakterlerini kaçırır (metin düğümleri için).
fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Sayıyı UDF'nin beklediği biçimde yazar (gereksiz sıfır kırpılır).
fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{:.1}", v)
    } else {
        // Tam sayıya çok yakınsa tek ondalıkla yaz.
        let s = format!("{}", v);
        s
    }
}

/// CDATA havuzunu ve her paragrafın ofset aralıklarını kurar.
///
/// Paragraflar arasına `\n` konur; ilk paragraftan önce ve son paragraftan
/// sonra ayırıcı bulunmaz. Bu, UDF format dokümantasyonundaki kurala uyar.
struct PoolBuilder {
    pool: String,
}

impl PoolBuilder {
    fn new() -> Self {
        Self {
            pool: String::new(),
        }
    }

    /// Yeni bir paragraf başlatır; havuz boş değilse önce ayırıcı ekler.
    fn begin_paragraph(&mut self) {
        if !self.pool.is_empty() {
            self.pool.push('\n');
        }
    }

    /// Paragrafın içeriğini havuza ekler ve her satır içi öğe için aralık döndürür.
    fn push_paragraph(&mut self, p: &Paragraph) -> Vec<Span> {
        self.begin_paragraph();
        let mut spans = Vec::with_capacity(p.content.len());
        for c in &p.content {
            let text = match c {
                InlineContent::Run(r) => r.text.clone(),
                InlineContent::Tab(_) => crate::TAB_CHAR.to_string(),
                InlineContent::Image(_) => crate::OBJECT_REPLACEMENT_CHAR.to_string(),
            };
            let start = self.pool.chars().count();
            self.pool.push_str(&text);
            let len = text.chars().count();
            spans.push(Span { start, len });
        }
        if spans.is_empty() {
            // Boş paragraf: sıfır genişlikli boşluk.
            let start = self.pool.chars().count();
            self.pool.push(crate::ZERO_WIDTH_SPACE);
            spans.push(Span { start, len: 1 });
        }
        spans
    }
}

/// Tek bir satır içi öğeyi XML'e çevirir.
fn inline_to_xml(c: &InlineContent, span: Span) -> String {
    match c {
        InlineContent::Run(r) => {
            let mut attrs = vec![
                format!("startOffset=\"{}\"", span.start),
                format!("length=\"{}\"", span.len),
                format!("family=\"{}\"", escape_xml(&r.family)),
                format!("size=\"{}\"", fmt_num(r.size_pt)),
            ];
            if r.bold {
                attrs.push("bold=\"true\"".into());
            }
            if r.italic {
                attrs.push("italic=\"true\"".into());
            }
            if r.underline {
                attrs.push("underline=\"true\"".into());
            }
            if r.foreground != crate::color::BLACK {
                attrs.push(format!("foreground=\"{}\"", r.foreground));
            }
            if r.background != crate::color::WHITE {
                attrs.push(format!("background=\"{}\"", r.background));
            }
            format!("<content {} />", attrs.join(" "))
        }
        InlineContent::Tab(r) => format!(
            "<tab startOffset=\"{}\" length=\"1\" family=\"{}\" size=\"{}\" />",
            span.start,
            escape_xml(&r.family),
            fmt_num(r.size_pt)
        ),
        InlineContent::Image(img) => {
            use base64::Engine;
            let data = base64::engine::general_purpose::STANDARD.encode(&img.data);
            format!(
                "<image imageData=\"{}\" startOffset=\"{}\" length=\"1\" width=\"{}\" height=\"{}\" />",
                data,
                span.start,
                fmt_num(img.width_pt),
                fmt_num(img.height_pt)
            )
        }
    }
}

/// Paragraf öğesini XML'e çevirir.
fn paragraph_to_xml(p: &Paragraph, spans: &[Span]) -> String {
    let mut attrs = vec![format!("Alignment=\"{}\"", p.alignment.udf_value())];
    attrs.push(format!("LeftIndent=\"{}\"", fmt_num(p.left_indent_pt)));
    attrs.push(format!("RightIndent=\"{}\"", fmt_num(p.right_indent_pt)));
    if let Some(fi) = p.first_line_indent_pt {
        attrs.push(format!("FirstLineIndent=\"{}\"", fmt_num(fi)));
    }
    if let Some(ls) = p.line_spacing {
        attrs.push(format!("LineSpacing=\"{}\"", fmt_num(ls)));
    }
    if let Some(sa) = p.space_above_pt {
        attrs.push(format!("SpaceAbove=\"{}\"", fmt_num(sa)));
    }
    if let Some(sb) = p.space_below_pt {
        attrs.push(format!("SpaceBelow=\"{}\"", fmt_num(sb)));
    }
    match &p.list {
        Some(ListType::Bullet(b)) => {
            if let Some(id) = p.list_id {
                attrs.push(format!("ListId=\"{}\"", id));
            }
            if let Some(l) = p.list_level {
                attrs.push(format!("ListLevel=\"{}\"", l));
            }
            attrs.push("Bulleted=\"true\"".into());
            attrs.push(format!("BulletType=\"{}\"", b.udf_name()));
        }
        Some(ListType::Numbered(n)) => {
            if let Some(id) = p.list_id {
                attrs.push(format!("ListId=\"{}\"", id));
            }
            if let Some(l) = p.list_level {
                attrs.push(format!("ListLevel=\"{}\"", l));
            }
            attrs.push("Numbered=\"true\"".into());
            attrs.push(format!("NumberType=\"{}\"", n.udf_name()));
        }
        None => {}
    }

    let mut inner = String::new();
    for (c, span) in p.content.iter().zip(spans.iter()) {
        inner.push_str(&inline_to_xml(c, *span));
    }
    if inner.is_empty() {
        // Boş paragraf için sıfır genişlikli boşluk referansı.
        inner.push_str(&format!(
            "<content startOffset=\"{}\" length=\"1\" family=\"Times New Roman\" size=\"10\" />",
            spans[0].start
        ));
    }
    format!("<paragraph {}>{}</paragraph>", attrs.join(" "), inner)
}

/// Tablo hücrelerinin paragraf ofsetleri: satır -> hücre -> paragraf -> aralıklar.
type CellSpans = Vec<Vec<Vec<Vec<Span>>>>;

/// Tablo öğesini XML'e çevirir. Her hücre paragrafı havuza eklenmiş olmalıdır.
fn table_to_xml(t: &Table, cell_spans: &CellSpans) -> String {
    let spans_str = t
        .column_widths_pt
        .iter()
        .map(|w| fmt_num(*w))
        .collect::<Vec<_>>()
        .join(",");
    let border = if t.bordered { "borderCell" } else { "borderNone" };
    let name = t.name.clone().unwrap_or_else(|| "Sabit".to_string());
    let mut rows_xml = String::new();
    for (ri, row) in t.rows.iter().enumerate() {
        let mut cells_xml = String::new();
        for (ci, cell) in row.cells.iter().enumerate() {
            let mut paras_xml = String::new();
            for (pi, p) in cell.paragraphs.iter().enumerate() {
                let spans: &[Span] = &cell_spans[ri][ci][pi];
                paras_xml.push_str(&paragraph_to_xml(p, spans));
            }
            let mut cattr = format!("colspan=\"{}\" align=\"top\"", cell.colspan);
            if cell.fill_color != crate::color::WHITE {
                cattr.push_str(&format!(" fillColor=\"{}\"", cell.fill_color));
            }
            cattr.push_str(&format!(" border=\"{}\" borderSpec=\"{}\"", border, cell.border_spec));
            cells_xml.push_str(&format!("<cell {}>{}</cell>", cattr, paras_xml));
        }
        let height = row.height_pt.unwrap_or(0.0);
        rows_xml.push_str(&format!(
            "<row rowName=\"row{}\" rowType=\"dataRow\" height=\"{}\">{}</row>",
            ri + 1,
            fmt_num(height),
            cells_xml
        ));
    }
    format!(
        "<table tableName=\"{}\" columnCount=\"{}\" columnSpans=\"{}\" border=\"{}\">{}</table>",
        escape_xml(&name),
        t.column_count(),
        spans_str,
        border,
        rows_xml
    )
}

/// Belgeyi XML metnine çevirir. Bu işlev saf ve deterministiktir; test
/// edilebilirliği yüksektir.
pub fn document_to_xml(doc: &UdfDocument) -> String {
    let mut pool = PoolBuilder::new();
    let mut elements_xml = String::new();

    // Üstbilgi.
    if let Some(header) = &doc.header {
        let mut inner = String::new();
        for p in &header.paragraphs {
            let spans = pool.push_paragraph(p);
            inner.push_str(&paragraph_to_xml(p, &spans));
        }
        if !inner.is_empty() {
            elements_xml.push_str(&format!("<header>{}</header>", inner));
        }
    }

    // Gövde.
    for block in &doc.blocks {
        match block {
            Block::Paragraph(p) => {
                let spans = pool.push_paragraph(p);
                elements_xml.push_str(&paragraph_to_xml(p, &spans));
            }
            Block::Table(t) => {
                let mut cell_spans: CellSpans = Vec::new();
                for row in &t.rows {
                    let mut row_spans = Vec::new();
                    for cell in &row.cells {
                        let mut cell_ps = Vec::new();
                        for p in &cell.paragraphs {
                            let spans = pool.push_paragraph(p);
                            cell_ps.push(spans);
                        }
                        row_spans.push(cell_ps);
                    }
                    cell_spans.push(row_spans);
                }
                elements_xml.push_str(&table_to_xml(t, &cell_spans));
            }
            Block::PageBreak => {
                let p = Paragraph::empty();
                let spans = pool.push_paragraph(&p);
                elements_xml.push_str(&format!(
                    "<page-break>{}</page-break>",
                    paragraph_to_xml(&p, &spans)
                ));
            }
        }
    }

    // Altbilgi.
    if let Some(footer) = &doc.footer {
        let mut inner = String::new();
        for p in &footer.paragraphs {
            let spans = pool.push_paragraph(p);
            inner.push_str(&paragraph_to_xml(p, &spans));
        }
        let mut fattrs = String::new();
        if let Some(spec) = &footer.page_number_spec {
            fattrs.push_str(&format!(" pageNumber-spec=\"{}\"", escape_xml(spec)));
        }
        if let Some(face) = &footer.page_number_font_face {
            fattrs.push_str(&format!(" pageNumber-fontFace=\"{}\"", escape_xml(face)));
        }
        if let Some(size) = footer.page_number_font_size {
            fattrs.push_str(&format!(" pageNumber-fontSize=\"{}\"", fmt_num(size)));
        }
        if let Some(color) = footer.page_number_color {
            fattrs.push_str(&format!(" pageNumber-color=\"{}\"", color));
        }
        if !inner.is_empty() || !fattrs.is_empty() {
            elements_xml.push_str(&format!("<footer{}>{}</footer>", fattrs, inner));
        }
    }

    // Boş belge: en az bir paragraf gerekir.
    if elements_xml.is_empty() {
        let p = Paragraph::empty();
        let spans = pool.push_paragraph(&p);
        elements_xml.push_str(&paragraph_to_xml(&p, &spans));
    }

    // Sayfa özellikleri.
    let pf = &doc.page_format;
    let properties = format!(
        "<properties><pageFormat mediaSizeName=\"{}\" leftMargin=\"{}\" rightMargin=\"{}\" topMargin=\"{}\" bottomMargin=\"{}\" paperOrientation=\"{}\" headerFOffset=\"{}\" footerFOffset=\"{}\" /></properties>",
        pf.media_size_name,
        fmt_num(pf.left_margin_pt),
        fmt_num(pf.right_margin_pt),
        fmt_num(pf.top_margin_pt),
        fmt_num(pf.bottom_margin_pt),
        pf.paper_orientation,
        fmt_num(pf.header_offset_pt),
        fmt_num(pf.footer_offset_pt),
    );

    // Stiller.
    let mut styles_xml = String::new();
    for s in &doc.styles {
        let mut attrs = vec![
            format!("name=\"{}\"", escape_xml(&s.name)),
            format!("family=\"{}\"", escape_xml(&s.family)),
            format!("size=\"{}\"", fmt_num(s.size_pt)),
        ];
        if let Some(d) = &s.description {
            attrs.push(format!("description=\"{}\"", escape_xml(d)));
        }
        attrs.push(format!("bold=\"{}\"", s.bold));
        attrs.push(format!("italic=\"{}\"", s.italic));
        attrs.push(format!("foreground=\"{}\"", s.foreground));
        if s.name == "default" {
            attrs.push(
                "FONT_ATTRIBUTE_KEY=\"javax.swing.plaf.FontUIResource[family=Dialog,name=Dialog,style=plain,size=12]\""
                    .to_string(),
            );
        }
        styles_xml.push_str(&format!("<style {} />", attrs.join(" ")));
    }

    let content = pool.pool;
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<template format_id=\"{}\">\n<content><![CDATA[{}]]></content>\n{}\n<elements resolver=\"hvl-default\">\n{}\n</elements>\n<styles>{}</styles>\n</template>",
        FORMAT_ID, content, properties, elements_xml, styles_xml
    )
}

/// Belgeyi UDF (ZIP + content.xml) olarak diske yazar.
pub fn write_udf(doc: &UdfDocument, path: &Path) -> Result<()> {
    let xml = document_to_xml(doc);
    write_udf_bytes(xml.as_bytes(), path)
}

/// Ham `content.xml` baytlarını UDF paketi olarak yazar.
pub fn write_udf_bytes(content_xml: &[u8], path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("content.xml", options)?;
    zip.write_all(content_xml)?;
    zip.finish()?;
    Ok(())
}

/// Belgeyi bellekte UDF baytlarına çevirir (test ve doğrulama için).
pub fn document_to_udf_bytes(doc: &UdfDocument) -> Result<Vec<u8>> {
    let xml = document_to_xml(doc);
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("content.xml", options)?;
        zip.write_all(xml.as_bytes())?;
        zip.finish()?;
    }
    Ok(buf)
}

/// Belgeyi bellekte `content.xml` baytlarına çevirir.
pub fn document_to_content_xml(doc: &UdfDocument) -> String {
    document_to_xml(doc)
}

#[allow(dead_code)]
fn unused(_: UdfError) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_document_produces_valid_structure() {
        let doc = UdfDocument::default();
        let xml = document_to_xml(&doc);
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\" ?>"));
        assert!(xml.contains("<template format_id=\"1.8\">"));
        assert!(xml.contains("<content><![CDATA["));
        assert!(xml.contains("</template>"));
    }

    #[test]
    fn offsets_are_character_based_with_turkish() {
        // "Çağrı" 5 karakterdir; ikinci paragraf ofseti 5 + 1 (\n) = 6 olmalı.
        let mut doc = UdfDocument::default();
        doc.push_paragraph(Paragraph::text("Çağrı"));
        doc.push_paragraph(Paragraph::text("İkinci"));
        let xml = document_to_xml(&doc);
        // İlk paragraf: offset 0 length 5
        assert!(xml.contains("startOffset=\"0\" length=\"5\""));
        // İkinci paragraf: offset 6 length 6
        assert!(xml.contains("startOffset=\"6\" length=\"6\""));
    }

    #[test]
    fn cdata_contains_turkish_text_verbatim() {
        let mut doc = UdfDocument::default();
        doc.push_paragraph(Paragraph::text("Şüpheli ifadesi: İĞÜŞÖÇ ığüşöç"));
        let xml = document_to_xml(&doc);
        assert!(xml.contains("Şüpheli ifadesi: İĞÜŞÖÇ ığüşöç"));
    }

    #[test]
    fn table_generates_cells_and_rows() {
        let mut doc = UdfDocument::default();
        doc.push_table(Table::new(
            vec![150.0, 150.0],
            vec![
                TableRow::new(vec![TableCell::text("Ad"), TableCell::text("Değer")]),
                TableRow::new(vec![TableCell::text("Süre"), TableCell::text("12 s")]),
            ],
        ));
        let xml = document_to_xml(&doc);
        assert!(xml.contains("<table tableName=\"Sabit\" columnCount=\"2\""));
        assert!(xml.contains("<row rowName=\"row1\""));
        assert!(xml.contains("<cell "));
        assert!(xml.contains("columnSpans=\"150.0,150.0\""));
    }

    #[test]
    fn image_placeholder_occupies_one_char() {
        let mut doc = UdfDocument::default();
        let mut p = Paragraph::text("önce");
        p.content.push(InlineContent::Image(UdfImage {
            data: vec![0x89, 0x50, 0x4E, 0x47],
            width_pt: 100.0,
            height_pt: 50.0,
        }));
        p.content.push(InlineContent::Run(TextRun::new("sonra")));
        doc.push_paragraph(p);
        let xml = document_to_xml(&doc);
        // "önce" = 4 karakter (offset 0..4), resim offset 4 length 1, "sonra" offset 5 length 5
        assert!(xml.contains("startOffset=\"0\" length=\"4\""));
        assert!(xml.contains("startOffset=\"4\" length=\"1\" width=\"100.0\" height=\"50.0\""));
        assert!(xml.contains("startOffset=\"5\" length=\"5\""));
    }

    #[test]
    fn list_attributes_are_emitted() {
        let mut doc = UdfDocument::default();
        let mut p = Paragraph::text("madde");
        p.list = Some(ListType::Bullet(BulletType::Ellipse));
        p.list_id = Some(1);
        p.list_level = Some(0);
        doc.push_paragraph(p);
        let xml = document_to_xml(&doc);
        assert!(xml.contains("Bulleted=\"true\""));
        assert!(xml.contains("BulletType=\"BULLET_TYPE_ELLIPSE\""));
        assert!(xml.contains("ListId=\"1\""));
    }

    #[test]
    fn write_and_read_back_zip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.udf");
        let mut doc = UdfDocument::default();
        doc.push_heading("Rapor");
        doc.push_paragraph(Paragraph::text("İçerik"));
        write_udf(&doc, &path).unwrap();
        assert!(path.exists());
        // ZIP olduğunu doğrula.
        let f = std::fs::File::open(&path).unwrap();
        let archive = zip::ZipArchive::new(f).unwrap();
        assert!(archive.file_names().any(|n| n == "content.xml"));
    }

    #[test]
    fn offsets_cover_full_pool() {
        let mut doc = UdfDocument::default();
        doc.push_paragraph(Paragraph::text("abc"));
        doc.push_paragraph(Paragraph::text("de"));
        let xml = document_to_xml(&doc);
        // Havuz "abc\nde" olmalı.
        assert!(xml.contains("<![CDATA[abc\nde]]>"));
    }
}
