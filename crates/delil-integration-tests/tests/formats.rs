//! Ses biçimi ve codec testleri (şartname 19.2) ile UDF testleri (19.3).
//!
//! Her desteklenen biçim için: açılma, süre, kanal, oynatma (çözme) ve
//! transkripsiyona gönderme adımları sınanır. UDF için: açma, kaydetme,
//! kapatma, tekrar açma, Türkçe karakter, tablo ve sayfa düzeni korunması.

use delil_integration_tests::make_wav;
use delil_udf::model::{Paragraph, Table, TableCell, TableRow, TextRun, UdfDocument};

#[test]
fn all_wav_variants_open_and_report_duration() {
    let dir = tempfile::tempdir().unwrap();
    let cases: &[(u32, u16, f64)] = &[
        (8000, 1, 1.0),
        (16000, 1, 2.0),
        (22050, 2, 1.5),
        (44100, 2, 1.0),
        (48000, 1, 0.5),
    ];
    for (sr, ch, secs) in cases {
        let name = format!("s{sr}_c{ch}.wav");
        let p = make_wav(&dir.path().join(&name), *sr, *ch, *secs, 440.0);
        // Açılma.
        let container = delil_audio::detect_container_from_path(&p).unwrap();
        assert_eq!(
            container,
            Some(delil_audio::Container::Wav),
            "{name} WAV olarak algılanmalı"
        );
        // Süre ve kanal.
        let info = delil_audio::inventory(&p).unwrap();
        assert!(
            (info.duration_seconds - secs).abs() < 0.05,
            "{name}: süre {:.3} beklenen {:.3}",
            info.duration_seconds,
            secs
        );
        assert_eq!(info.channels, Some(*ch));
        assert_eq!(info.sample_rate_hz, Some(*sr));
        // Çözme (oynatma) ve transkripsiyon girdisi.
        let samples = delil_transcription::decode_audio_file(&p).unwrap();
        let expected = (*sr as f64 * secs) as usize;
        // Yeniden örnekleme sonrası 16 kHz'e iner.
        let expected_16k = (16000.0 * secs) as usize;
        assert!(
            (samples.len() as i64 - expected_16k as i64).abs() < 200,
            "{name}: çözülen örnek sayısı {} beklenen ~{} (kaynak {expected})",
            samples.len(),
            expected_16k
        );
    }
}

#[test]
fn container_detection_covers_documented_formats() {
    // İmza tabanlı algılama; her biçim için minimal başlık (en az 12 bayt).
    let cases: Vec<(Vec<u8>, delil_audio::Container)> = vec![
        (
            b"RIFF\0\0\0\0WAVEfmt \0\0\0\0".to_vec(),
            delil_audio::Container::Wav,
        ),
        (
            b"fLaC\0\0\0\0\0\0\0\0".to_vec(),
            delil_audio::Container::Flac,
        ),
        (
            b"OggS\0\0\0\0\0\0\0\0\0\0\0\0".to_vec(),
            delil_audio::Container::Ogg,
        ),
        (
            b"caff\0\0\0\0\0\0\0\0\0\0\0\0".to_vec(),
            delil_audio::Container::Caf,
        ),
        (
            b"ID3\x04\0\0\0\0\0\0\0\0\0\0".to_vec(),
            delil_audio::Container::Mp3,
        ),
        (
            b"#!AMR\x00\x00\x00\x00\x00\x00\x00\x00\x00".to_vec(),
            delil_audio::Container::Amr,
        ),
        (
            b"FORM\0\0\0\0AIFF\0\0\0\0".to_vec(),
            delil_audio::Container::Aiff,
        ),
    ];
    for (bytes, expected) in cases {
        assert_eq!(
            delil_audio::detect_container(&bytes),
            Some(expected),
            "başlık: {:?}",
            &bytes[..4]
        );
    }
}

#[test]
fn video_containers_flagged_audio_only_extraction() {
    // Video kapsayıcıları yalnızca ses akışı için işlenmeli.
    let mp4 = {
        let mut v = vec![0, 0, 0, 0x20];
        v.extend_from_slice(b"ftypisom");
        v.extend(std::iter::repeat(0).take(64));
        v
    };
    let c = delil_audio::detect_container(&mp4).unwrap();
    assert!(c.is_video_container());
}

#[test]
fn udf_full_roundtrip_with_tables_and_turkish() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rapor.udf");

    let mut doc = UdfDocument::default();
    doc.push_paragraph(Paragraph::with_run(
        TextRun::new("BİLİRKİŞİ RAPORU — Türkçe: ıİşŞğĞüÜöÖçÇ")
            .bold()
            .size(14.0),
    ));
    doc.push_paragraph(Paragraph::text("Gövde metni: 12.05.2024 tarihli 5000 TL."));
    doc.push_table(Table::new(
        vec![150.0, 100.0, 100.0],
        vec![
            TableRow::new(vec![
                TableCell::text("Dosya"),
                TableCell::text("Süre"),
                TableCell::text("SHA-256"),
            ]),
            TableRow::new(vec![
                TableCell::text("kayıt.wav"),
                TableCell::text("2.0"),
                TableCell::text("abc123"),
            ]),
        ],
    ));
    doc.push_page_break();
    doc.push_paragraph(Paragraph::text("İkinci sayfa içeriği."));

    // Yaz.
    delil_udf::write_udf(&doc, &path).unwrap();
    assert!(path.exists());

    // Yapısal doğrulama.
    let report = delil_udf::validate_udf(&path).unwrap();
    assert!(report.passed(), "{:?}", report.errors);
    assert!(report.turkish_ok);
    assert_eq!(report.table_count, 1);

    // Kapat ve tekrar aç.
    let parsed = delil_udf::read_udf(&path).unwrap();
    assert!(parsed.contains_text("BİLİRKİŞİ RAPORU"));
    assert!(parsed.contains_text("ıİşŞğĞüÜöÖçÇ"));
    assert!(parsed.contains_text("12.05.2024"));
    assert_eq!(parsed.table_count, 1);
    assert_eq!(parsed.page_break_count, 1);
    assert_eq!(parsed.page_format.media_size_name, 1);

    // Tablo içeriği korundu mu?
    let table = parsed
        .blocks
        .iter()
        .find_map(|b| match b {
            delil_udf::reader::ParsedBlock::Table { rows, column_count } => {
                Some((rows.clone(), *column_count))
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(table.1, 3);
    assert_eq!(table.0[1][0], "kayıt.wav");

    // Yeniden yaz (kaydet-aç döngüsü) ve yine doğrula.
    let path2 = dir.path().join("rapor2.udf");
    delil_udf::write_udf(&doc, &path2).unwrap();
    let report2 = delil_udf::validate_udf(&path2).unwrap();
    assert!(report2.passed());
}

#[test]
fn udf_offsets_are_character_based() {
    // Türkçe karakterler bayt olarak farklı uzunlukta olsa da ofsetler
    // karakter cinsinden olmalı.
    let mut doc = UdfDocument::default();
    doc.push_paragraph(Paragraph::text("Çağrı"));
    doc.push_paragraph(Paragraph::text("İkinci"));
    let xml = delil_udf::writer::document_to_content_xml(&doc);
    // "Çağrı" = 5 karakter (bayt 7), ayırıcı 1 -> ikinci ofset 6.
    assert!(xml.contains("startOffset=\"0\" length=\"5\""));
    assert!(xml.contains("startOffset=\"6\" length=\"6\""));
}

#[test]
fn udf_validator_rejects_corrupt_package() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bozuk.udf");
    std::fs::write(&bad, b"bu bir udf degil").unwrap();
    assert!(delil_udf::validate_udf(&bad).is_err());
    assert!(delil_udf::read_udf(&bad).is_err());
}
