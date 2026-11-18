//! UDF belge modeli: rapor verisinden UDF üretmek için ara temsil.
//!
//! Bu model, UDF'nin metin havuzu + ofset yaklaşımını soyutlar. Yazar
//! ([`crate::writer`]) bu modeli gerçek `content.xml` yapısına çevirir.

use serde::{Deserialize, Serialize};

/// Bir metin parçasının biçimi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextRun {
    pub text: String,
    pub family: String,
    pub size_pt: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub foreground: i32,
    pub background: i32,
}

impl Default for TextRun {
    fn default() -> Self {
        Self {
            text: String::new(),
            family: "Times New Roman".to_string(),
            size_pt: 11.0,
            bold: false,
            italic: false,
            underline: false,
            foreground: crate::color::BLACK,
            background: crate::color::WHITE,
        }
    }
}

impl TextRun {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    pub fn size(mut self, pt: f64) -> Self {
        self.size_pt = pt;
        self
    }

    pub fn family(mut self, f: impl Into<String>) -> Self {
        self.family = f.into();
        self
    }
}

/// Paragraf hizalaması.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

impl Alignment {
    pub fn udf_value(&self) -> u8 {
        match self {
            Alignment::Left => 0,
            Alignment::Center => 1,
            Alignment::Right => 2,
            Alignment::Justify => 3,
        }
    }
}

/// Liste türü (madde işareti veya numaralı).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum ListType {
    /// Madde işaretli liste.
    Bullet(BulletType),
    /// Numaralı liste.
    Numbered(NumberType),
}

/// Madde işareti türleri.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BulletType {
    Ellipse,
    Rectangle,
    RectangleD,
    Arrow,
    Diamond,
    Triangle,
}

impl BulletType {
    pub fn udf_name(&self) -> &'static str {
        match self {
            BulletType::Ellipse => "BULLET_TYPE_ELLIPSE",
            BulletType::Rectangle => "BULLET_TYPE_RECTANGLE",
            BulletType::RectangleD => "BULLET_TYPE_RECTANGLE_D",
            BulletType::Arrow => "BULLET_TYPE_ARROW",
            BulletType::Diamond => "BULLET_TYPE_DIAMOND",
            BulletType::Triangle => "BULLET_TYPE_TRIANGLE",
        }
    }
}

/// Numara türleri.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NumberType {
    NumberDot,
    NumberParenthesis,
    CharSmallDot,
    CharSmallParenthesis,
    CharBigDot,
    CharBigParenthesis,
    RomanSmallDot,
    RomanSmallParenthesis,
    RomanBigDot,
    RomanBigParenthesis,
}

impl NumberType {
    pub fn udf_name(&self) -> &'static str {
        match self {
            NumberType::NumberDot => "NUMBER_TYPE_NUMBER_DOT",
            NumberType::NumberParenthesis => "NUMBER_TYPE_NUMBER_PARENTHESIS",
            NumberType::CharSmallDot => "NUMBER_TYPE_CHAR_SMALL_DOT",
            NumberType::CharSmallParenthesis => "NUMBER_TYPE_CHAR_SMALL_PARENTHESIS",
            NumberType::CharBigDot => "NUMBER_TYPE_CHAR_BIG_DOT",
            NumberType::CharBigParenthesis => "NUMBER_TYPE_CHAR_BIG_PARENTHESIS",
            NumberType::RomanSmallDot => "NUMBER_TYPE_ROMAN_SMALL_DOT",
            NumberType::RomanSmallParenthesis => "NUMBER_TYPE_ROMAN_SMALL_PARENTHESIS",
            NumberType::RomanBigDot => "NUMBER_TYPE_ROMAN_BIG_DOT",
            NumberType::RomanBigParenthesis => "NUMBER_TYPE_ROMAN_BIG_PARENTHESIS",
        }
    }
}

/// Gömülü resim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UdfImage {
    /// Ham resim baytları (JPEG veya PNG).
    pub data: Vec<u8>,
    pub width_pt: f64,
    pub height_pt: f64,
}

/// Paragraf içi içerik öğesi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum InlineContent {
    Run(TextRun),
    Image(UdfImage),
    Tab(TextRun),
}

/// Bir paragraf.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paragraph {
    pub content: Vec<InlineContent>,
    pub alignment: Alignment,
    pub left_indent_pt: f64,
    pub right_indent_pt: f64,
    pub first_line_indent_pt: Option<f64>,
    pub line_spacing: Option<f64>,
    pub space_above_pt: Option<f64>,
    pub space_below_pt: Option<f64>,
    pub list: Option<ListType>,
    pub list_id: Option<u32>,
    pub list_level: Option<u32>,
}

impl Default for Paragraph {
    fn default() -> Self {
        Self {
            content: Vec::new(),
            alignment: Alignment::Left,
            left_indent_pt: 0.0,
            right_indent_pt: 0.0,
            first_line_indent_pt: None,
            line_spacing: None,
            space_above_pt: None,
            space_below_pt: None,
            list: None,
            list_id: None,
            list_level: None,
        }
    }
}

impl Paragraph {
    /// Düz metin paragrafı.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![InlineContent::Run(TextRun::new(text))],
            ..Default::default()
        }
    }

    /// Biçimli paragraf.
    pub fn with_run(run: TextRun) -> Self {
        Self {
            content: vec![InlineContent::Run(run)],
            ..Default::default()
        }
    }

    /// Boş paragraf.
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn alignment(mut self, a: Alignment) -> Self {
        self.alignment = a;
        self
    }

    pub fn centered(self) -> Self {
        self.alignment(Alignment::Center)
    }

    /// Paragrafın düz metni.
    pub fn plain_text(&self) -> String {
        let mut s = String::new();
        for c in &self.content {
            match c {
                InlineContent::Run(r) => s.push_str(&r.text),
                InlineContent::Tab(_) => s.push('\t'),
                InlineContent::Image(_) => s.push(crate::OBJECT_REPLACEMENT_CHAR),
            }
        }
        s
    }
}

/// Tablo hücresi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    pub paragraphs: Vec<Paragraph>,
    pub colspan: u32,
    pub fill_color: i32,
    pub border_spec: u8,
}

impl Default for TableCell {
    fn default() -> Self {
        Self {
            paragraphs: vec![Paragraph::empty()],
            colspan: 1,
            fill_color: crate::color::WHITE,
            border_spec: 15,
        }
    }
}

impl TableCell {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            paragraphs: vec![Paragraph::text(text)],
            ..Default::default()
        }
    }
}

/// Tablo satırı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    pub height_pt: Option<f64>,
}

impl TableRow {
    pub fn new(cells: Vec<TableCell>) -> Self {
        Self {
            cells,
            height_pt: None,
        }
    }
}

/// Tablo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub name: Option<String>,
    pub column_widths_pt: Vec<f64>,
    pub rows: Vec<TableRow>,
    pub bordered: bool,
}

impl Table {
    pub fn new(column_widths_pt: Vec<f64>, rows: Vec<TableRow>) -> Self {
        Self {
            name: Some("Sabit".to_string()),
            column_widths_pt,
            rows,
            bordered: true,
        }
    }

    pub fn column_count(&self) -> usize {
        self.column_widths_pt.len()
    }
}

/// Belge gövdesi öğesi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Block {
    Paragraph(Paragraph),
    Table(Table),
    PageBreak,
}

/// Sayfa biçimi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageFormat {
    pub media_size_name: u8,
    pub left_margin_pt: f64,
    pub right_margin_pt: f64,
    pub top_margin_pt: f64,
    pub bottom_margin_pt: f64,
    pub paper_orientation: u8,
    pub header_offset_pt: f64,
    pub footer_offset_pt: f64,
}

impl Default for PageFormat {
    fn default() -> Self {
        Self {
            media_size_name: 1,
            left_margin_pt: 42.51968479156494,
            right_margin_pt: 28.34645652770996,
            top_margin_pt: 14.17322826385498,
            bottom_margin_pt: 14.17322826385498,
            paper_orientation: 1,
            header_offset_pt: 20.0,
            footer_offset_pt: 20.0,
        }
    }
}

/// Üstbilgi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Header {
    pub paragraphs: Vec<Paragraph>,
}

/// Altbilgi; sayfa numarası özellikleri taşıyabilir.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Footer {
    pub paragraphs: Vec<Paragraph>,
    pub page_number_spec: Option<String>,
    pub page_number_font_face: Option<String>,
    pub page_number_font_size: Option<f64>,
    pub page_number_color: Option<i32>,
}

/// Stil tanımı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleDef {
    pub name: String,
    pub description: Option<String>,
    pub family: String,
    pub size_pt: f64,
    pub bold: bool,
    pub italic: bool,
    pub foreground: i32,
}

/// Tam UDF belgesi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UdfDocument {
    pub page_format: PageFormat,
    pub header: Option<Header>,
    pub footer: Option<Footer>,
    pub blocks: Vec<Block>,
    pub styles: Vec<StyleDef>,
}

impl Default for UdfDocument {
    fn default() -> Self {
        Self {
            page_format: PageFormat::default(),
            header: None,
            footer: None,
            blocks: Vec::new(),
            styles: vec![
                StyleDef {
                    name: "default".to_string(),
                    description: Some("Geçerli".to_string()),
                    family: "Dialog".to_string(),
                    size_pt: 12.0,
                    bold: false,
                    italic: false,
                    foreground: crate::color::DEFAULT_FOREGROUND,
                },
                StyleDef {
                    name: "hvl-default".to_string(),
                    description: Some("Gövde".to_string()),
                    family: "Times New Roman".to_string(),
                    size_pt: 12.0,
                    bold: false,
                    italic: false,
                    foreground: crate::color::BLACK,
                },
            ],
        }
    }
}

impl UdfDocument {
    pub fn push_paragraph(&mut self, p: Paragraph) {
        self.blocks.push(Block::Paragraph(p));
    }

    pub fn push_table(&mut self, t: Table) {
        self.blocks.push(Block::Table(t));
    }

    pub fn push_page_break(&mut self) {
        self.blocks.push(Block::PageBreak);
    }

    pub fn push_heading(&mut self, text: impl Into<String>) {
        self.push_paragraph(Paragraph::with_run(
            TextRun::new(text).bold().size(12.0),
        ));
    }

    /// Belgenin tüm düz metni (geliştirici/test kolaylığı için).
    pub fn plain_text(&self) -> String {
        let mut lines = Vec::new();
        for b in &self.blocks {
            match b {
                Block::Paragraph(p) => lines.push(p.plain_text()),
                Block::Table(t) => {
                    for row in &t.rows {
                        for cell in &row.cells {
                            for p in &cell.paragraphs {
                                lines.push(p.plain_text());
                            }
                        }
                    }
                }
                Block::PageBreak => lines.push(String::new()),
            }
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_document_has_two_styles() {
        let d = UdfDocument::default();
        assert_eq!(d.styles.len(), 2);
        assert_eq!(d.styles[0].name, "default");
        assert_eq!(d.styles[1].name, "hvl-default");
    }

    #[test]
    fn paragraph_plain_text_includes_tabs_and_images() {
        let mut p = Paragraph::text("A");
        p.content.push(InlineContent::Tab(TextRun::default()));
        p.content.push(InlineContent::Image(UdfImage {
            data: vec![1, 2, 3],
            width_pt: 10.0,
            height_pt: 10.0,
        }));
        p.content.push(InlineContent::Run(TextRun::new("B")));
        let t = p.plain_text();
        assert!(t.contains('\t'));
        assert!(t.contains(crate::OBJECT_REPLACEMENT_CHAR));
        assert!(t.starts_with('A'));
        assert!(t.ends_with('B'));
    }

    #[test]
    fn alignment_values_match_udf() {
        assert_eq!(Alignment::Left.udf_value(), 0);
        assert_eq!(Alignment::Center.udf_value(), 1);
        assert_eq!(Alignment::Right.udf_value(), 2);
        assert_eq!(Alignment::Justify.udf_value(), 3);
    }

    #[test]
    fn document_plain_text_walks_blocks() {
        let mut d = UdfDocument::default();
        d.push_heading("Başlık");
        d.push_paragraph(Paragraph::text("Gövde"));
        d.push_table(Table::new(
            vec![100.0, 100.0],
            vec![TableRow::new(vec![
                TableCell::text("A1"),
                TableCell::text("B1"),
            ])],
        ));
        let t = d.plain_text();
        assert!(t.contains("Başlık"));
        assert!(t.contains("Gövde"));
        assert!(t.contains("A1"));
        assert!(t.contains("B1"));
    }
}
