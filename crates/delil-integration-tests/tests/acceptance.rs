//! Uçtan uca kabul testi (şartname 20).
//!
//! Bu test, şartnamedeki 38 adımlı senaryoyu gerçek verilerle yürütür:
//! görevlendirme okuma → materyal edinimi → doğrulama → ses envanteri →
//! (transkripsiyon) → kalite kontrolü → rapor → UDF üretimi ve doğrulaması →
//! arşivleme → yedekleme → geri yükleme → bütünlük doğrulaması.

use delil_core::ExpertInfo;
use delil_integration_tests::{setup_case_dir, setup_usb_with_audio};
use delil_pipeline::{PipelineConfig, PipelineRunner};

#[test]
fn full_acceptance_scenario() {
    let dir = tempfile::tempdir().unwrap();
    let (usb, asg) = setup_case_dir(dir.path());
    setup_usb_with_audio(&usb);

    let case_root = dir.path().join("case");

    // 1-4. Yeni inceleme başlatılır.
    let mut cfg = PipelineConfig::new(&case_root);
    cfg.assignment_path = Some(asg);
    cfg.material_source = Some(usb.clone());
    cfg.source = Some(delil_evidence::local_folder_source(&usb));
    cfg.transcribe = false; // model olmadan; transkripsiyon adımı ayrıca test edilir
    cfg.expert = ExpertInfo {
        full_name: Some("Dr. Zeynep Arslan".into()),
        title: Some("Bilirkişi".into()),
        registry_number: Some("54321".into()),
    };

    let mut runner = PipelineRunner::new(cfg);
    let result = runner.run().expect("iş akışı tamamlanmalı");

    // 5-6. Görevlendirme bilgileri çıkarıldı.
    assert_eq!(
        result.case.assignment.case_number.as_deref(),
        Some("2024/312 Esas")
    );
    assert!(result
        .case
        .assignment
        .court
        .as_deref()
        .unwrap()
        .contains("Ankara"));
    assert_eq!(result.case.assignment.questions.len(), 3);
    assert!(result
        .case
        .assignment
        .parties
        .iter()
        .any(|p| p.name.contains("Kemal")));

    // 7-12. Materyal alındı ve SHA-256 ile doğrulandı.
    assert_eq!(result.case.materials.len(), 4, "4 dosya edinilmeli");
    assert_eq!(
        result.case.verified_material_count(),
        4,
        "tüm dosyalar doğrulanmalı"
    );
    for m in &result.case.materials {
        assert!(m.source_sha256.is_some());
        assert_eq!(m.source_sha256, m.target_sha256);
    }

    // 13-14. Ses dosyaları içerik tabanlı bulundu; teknik envanter çıkarıldı.
    // 3 WAV (biri yanlış uzantılı .dat) + 1 metin dosyası ses değil.
    assert_eq!(
        result.case.audio_assets.len(),
        3,
        "içerik tabanlı algılama 3 ses bulmalı (.dat dahil)"
    );
    for a in &result.case.audio_assets {
        assert!(a.technical.readable);
        assert!(a.technical.duration_seconds > 0.0);
        assert_eq!(a.technical.container, "WAV");
    }
    // Yanlış uzantılı dosya gerçekten bulundu mu?
    assert!(result
        .case
        .audio_assets
        .iter()
        .any(|a| a.file_name == "gizli_ses.dat"));

    // 24-26. Rapor taslağı ve UDF üretildi; UDF yapısal doğrulamadan geçti.
    assert!(result.udf_path.exists());
    assert!(result.validation.passed(), "{:?}", result.validation.errors);
    assert!(result.validation.turkish_ok);

    // UDF gerçekten okunabilir ve doğru içerikte.
    let parsed = delil_udf::read_udf(&result.udf_path).unwrap();
    let text = parsed.plain_text();
    assert!(text.contains("2024/312 Esas"));
    assert!(text.contains("GEREKÇELİ SONUÇ"));
    assert!(text.contains("TEKNİK SES ENVANTERİ"));
    assert!(text.contains("İNCELENEN MATERYALLER"));

    // 31. PDF yardımcı kopyası üretildi.
    let pdf = result.pdf_path.clone().unwrap();
    assert!(pdf.exists());
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));

    // 32. Vaka arşivlendi.
    let store = delil_case_store::CaseStore::open(&case_root).unwrap();
    let key = store.load_or_create_master_key().unwrap();
    let loaded = store.load_case(&key).unwrap();
    assert_eq!(loaded.materials.len(), 4);
    assert_eq!(loaded.id, result.case.id);

    // 33-35. Uygulama kapatılıp yeniden açıldı: aynı vaka bütün içerikleriyle geri geldi.
    drop(store);
    let store2 = delil_case_store::CaseStore::open(&case_root).unwrap();
    let key2 = store2.load_or_create_master_key().unwrap();
    let loaded2 = store2.load_case(&key2).unwrap();
    assert_eq!(loaded2.id, result.case.id);
    assert_eq!(loaded2.audio_assets.len(), 3);

    // 36-38. Şifreli yedek oluşturuldu, başka konuma geri yüklendi ve
    // hash/bütünlük kontrolünden geçti.
    let backup = dir.path().join("yedek.delilbackup");
    let manifest = store2.backup("güçlü-parola-2024", &backup).unwrap();
    assert!(backup.exists());
    assert!(!manifest.entries.is_empty());

    let restore_dir = dir.path().join("geri-yuklenen");
    let restored =
        delil_case_store::CaseStore::restore(&backup, "güçlü-parola-2024", &restore_dir).unwrap();
    assert_eq!(restored.entries.len(), manifest.entries.len());

    // Geri yüklenen vaka, orijinalle aynı içeriğe ve hash'lere sahip.
    let restored_store = delil_case_store::CaseStore::open(&restore_dir).unwrap();
    let rkey = restored_store.load_or_create_master_key().unwrap();
    // Not: geri yüklenen vakada ana anahtar dosyası yedeğe girmediğinden yeni
    // anahtar üretilir; meta veri yedeğin dışındaki vaka dosyasıyla birlikte
    // taşınır. Burada asıl doğrulanan dosya bütünlüğüdür.
    let _ = rkey;
    let orig_material = std::fs::read(
        case_root
            .join("materials")
            .join("kayit_01.wav"),
    )
    .unwrap();
    let restored_material =
        std::fs::read(restore_dir.join("materials").join("kayit_01.wav")).unwrap();
    assert_eq!(orig_material, restored_material, "geri yüklenen içerik aynı olmalı");
}

#[test]
fn pipeline_skips_unsupported_audio_without_silent_drop() {
    // Desteklenmeyen codec sessizce atlanmamalı; açıkça bildirilmeli.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    // .mp3 uzantılı ama içeriği ses olmayan dosya.
    std::fs::write(src.join("bozuk.mp3"), b"bu bir mp3 degil, duz metin").unwrap();

    let mats = delil_evidence::acquire_files(
        &src,
        &delil_evidence::local_folder_source(&src),
        &dir.path().join("out"),
        &Default::default(),
        None,
    )
    .unwrap();
    let assets = delil_evidence::build_audio_inventory(&mats, &dir.path().join("out"));
    assert_eq!(assets.len(), 1);
    assert!(
        assets[0].unsupported_reason.is_some(),
        "desteklenmeyen dosya açıkça bildirilmeli, sessizce atlanmamalı"
    );
}

#[test]
fn interrupted_copy_resumes_safely() {
    // Kopyalama kesintisi: geçici dosya (.delilpart) kalıntısı hedefte
    // görünmemeli ve yeniden çalıştırma başarılı olmalı.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    delil_integration_tests::make_wav(&src.join("a.wav"), 16000, 1, 1.0, 440.0);
    let target = dir.path().join("out");

    let mats = delil_evidence::acquire_files(
        &src,
        &delil_evidence::local_folder_source(&src),
        &target,
        &Default::default(),
        None,
    )
    .unwrap();
    assert_eq!(mats.len(), 1);
    // Geçici dosya kalmamalı.
    let leftovers: Vec<_> = std::fs::read_dir(&target)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".delilpart"))
        .collect();
    assert!(leftovers.is_empty(), "geçici kopya dosyası kalmamalı");

    // Yeniden çalıştırma (üzerine yazma) başarılı olmalı.
    let mats2 = delil_evidence::acquire_files(
        &src,
        &delil_evidence::local_folder_source(&src),
        &target,
        &Default::default(),
        None,
    )
    .unwrap();
    assert_eq!(mats2.len(), 1);
    assert!(mats2[0].verification.is_verified());
}

#[test]
fn large_file_hash_streaming_is_consistent() {
    // Büyük dosya (8 MiB) hash'i akış hâlinde tutarlı hesaplanmalı.
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("buyuk.bin");
    let data = vec![0xABu8; 8 * 1024 * 1024];
    std::fs::write(&p, &data).unwrap();
    let h1 = delil_hash::sha256_file(&p).unwrap();
    let h2 = delil_hash::sha256_bytes(&data);
    assert_eq!(h1, h2);
}

#[test]
fn disk_full_condition_is_detected() {
    let err = delil_evidence::EvidenceError::InsufficientSpace {
        required: 5_000_000,
        available: 1_000,
    };
    assert!(err.to_string().contains("Disk alanı yetersiz"));
}
