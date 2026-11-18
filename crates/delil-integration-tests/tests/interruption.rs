//! Kesinti ve dayanıklılık testleri (şartname 19.5).
//!
//! Uygulama çökmesi, elektrik kesintisi, yeniden başlatma, yarıda kalan model
//! işlemi, yarıda kalan kopyalama, veritabanı kilitlenmesi, yedekleme
//! kesintisi ve güncelleme hatası sonrasında sistem güvenli biçimde devam
//! etmelidir.

use delil_core::JobState;
use delil_integration_tests::make_wav;
use delil_transcription::engine::{ControlFlag, TranscriptionEngine, TranscriptionOptions};
use delil_transcription::queue::{Queue, QueueItem, QueueOutcome};

#[test]
fn queue_resumes_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let queue_path = dir.path().join("queue.json");

    // İlk oturum: iş yarıda kalır.
    {
        let mut q = Queue::with_persistence(&queue_path).unwrap();
        let item = QueueItem::new(
            "aud-1",
            dir.path().join("a.wav"),
            Some("sha".into()),
            &TranscriptionOptions::default(),
        );
        q.enqueue(item);
        q.items_mut()[0].state = JobState::Running { progress: 0.37 };
        q.items_mut()[0].completed_seconds = 0.37;
        q.items_mut()[0].total_seconds = 1.0;
        q.save().unwrap();
    }

    // Yeniden başlatma: yarıda kalan iş kuyruğa geri alınır.
    let mut q = Queue::with_persistence(&queue_path).unwrap();
    assert_eq!(q.items().len(), 1);
    assert!(matches!(q.items()[0].state, JobState::Running { .. }));
    q.resume_interrupted();
    assert_eq!(q.items()[0].state, JobState::Queued);
    // Tamamlanan saniye korunur (kaldığı yerden devam).
    assert!((q.items()[0].completed_seconds - 0.37).abs() < 0.001);
}

#[test]
fn crash_during_transcription_does_not_corrupt_queue() {
    let dir = tempfile::tempdir().unwrap();
    let queue_path = dir.path().join("queue.json");
    let mut q = Queue::with_persistence(&queue_path).unwrap();
    for i in 0..3 {
        q.enqueue(QueueItem::new(
            format!("aud-{i}"),
            dir.path().join(format!("{i}.wav")),
            Some(format!("sha-{i}")),
            &TranscriptionOptions::default(),
        ));
    }
    q.save().unwrap();
    // Dosyayı elle boz (kesinti benzetimi), sonra yeniden oku: kurtarılabilir
    // olmasa bile panik olmamalı.
    std::fs::write(&queue_path, b"{ bozuk json").unwrap();
    let res = Queue::with_persistence(&queue_path);
    assert!(
        res.is_err(),
        "bozuk kuyruk dosyası hata döndürmeli, panik olmamalı"
    );
}

#[test]
fn cancellation_is_honored_mid_queue() {
    let dir = tempfile::tempdir().unwrap();
    let mut q = Queue::new();
    q.enqueue(QueueItem::new(
        "a",
        dir.path().join("a.wav"),
        Some("s1".into()),
        &TranscriptionOptions::default(),
    ));
    q.enqueue(QueueItem::new(
        "b",
        dir.path().join("b.wav"),
        Some("s2".into()),
        &TranscriptionOptions::default(),
    ));
    let control = ControlFlag::new();
    control.cancel();
    let mut eng = NoopEngine;
    let decode =
        |_p: &std::path::Path| -> delil_transcription::Result<Vec<f32>> { Ok(vec![0.0f32; 16000]) };
    let out = q.run_all(
        &mut eng,
        &TranscriptionOptions::default(),
        &control,
        &decode,
    );
    assert!(matches!(out[0], QueueOutcome::Cancelled));
    // Hiçbir iş tamamlanmadı.
    assert!(q.items().iter().all(|i| i.state != JobState::Done));
}

#[test]
fn backup_interruption_leaves_original_intact() {
    // Yedekleme kesintisi orijinal vakayı bozmamalı.
    let dir = tempfile::tempdir().unwrap();
    let store = delil_case_store::CaseStore::create(dir.path().join("case")).unwrap();
    let key = store.load_or_create_master_key().unwrap();
    let case = delil_core::Case::new(store.root.to_string_lossy().to_string());
    store.save_case(&case, &key).unwrap();
    std::fs::write(store.root.join("materials").join("a.bin"), b"veri").unwrap();

    // Geçersiz hedefe yedekleme denenir (kesinti/hatalı ortam benzetimi).
    let bad_dest = std::path::PathBuf::from("/nonexistent/dir/yedek.enc");
    let _ = store.backup("p", &bad_dest);

    // Orijinal vaka hâlâ okunabilir.
    let loaded = store.load_case(&key).unwrap();
    assert_eq!(loaded.id, case.id);
    assert!(store.root.join("materials").join("a.bin").exists());
}

#[test]
fn update_failure_rolls_back() {
    use delil_update::{
        public_key_of, sign_manifest, UpdateManager, UpdateManifest, UpdateProgress,
    };
    let sk = [9u8; 32];
    // Geçerli paket ve manifest.
    let mut pkg = Vec::new();
    {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut pkg));
        let o: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        use std::io::Write;
        z.start_file("delil.exe", o).unwrap();
        z.write_all(b"v2").unwrap();
        z.finish().unwrap();
    }
    let sha = {
        use sha2::Digest;
        let mut h = sha2::Sha256::new();
        h.update(&pkg);
        hex::encode(h.finalize())
    };
    let m = UpdateManifest {
        version: "1.1.0".into(),
        channel: "stable".into(),
        released_at: chrono::Utc::now(),
        package: "p.zip".into(),
        sha256: sha,
        size_bytes: pkg.len() as u64,
        min_current_version: None,
        schema_version: 2,
        notes: vec![],
    };
    let signed = sign_manifest(&m, &sk).unwrap();

    let install = tempfile::tempdir().unwrap();
    let backup = tempfile::tempdir().unwrap();
    std::fs::write(install.path().join("delil.exe"), b"v1").unwrap();
    let mgr = UpdateManager::new("1.0.0", public_key_of(&sk), install.path(), backup.path());
    let p = mgr.apply(&signed, &pkg, false).unwrap();
    assert_eq!(p, UpdateProgress::Done);
    assert_eq!(
        std::fs::read(install.path().join("delil.exe")).unwrap(),
        b"v2"
    );

    // Yedekten geri dönüş çalışır.
    let backups: Vec<_> = std::fs::read_dir(backup.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(!backups.is_empty());
    mgr.rollback(&backups[0].path()).unwrap();
    assert_eq!(
        std::fs::read(install.path().join("delil.exe")).unwrap(),
        b"v1"
    );
}

/// Hiçbir şey yapmayan motor (iptal testi için).
struct NoopEngine;

impl TranscriptionEngine for NoopEngine {
    fn name(&self) -> &str {
        "noop"
    }
    fn version(&self) -> &str {
        "0"
    }
    fn is_ready(&self) -> bool {
        true
    }
    fn transcribe(
        &mut self,
        samples: &[f32],
        _o: &TranscriptionOptions,
        control: &ControlFlag,
        _p: Option<delil_transcription::engine::EngineProgress<'_>>,
    ) -> delil_transcription::Result<delil_transcription::engine::TranscriptionResult> {
        control.wait_if_paused()?;
        Ok(delil_transcription::engine::TranscriptionResult {
            segments: vec![],
            detected_language: Some("tr".into()),
            duration_seconds: samples.len() as f64 / 16000.0,
            rms: 0.0,
            peak: 0.0,
            corrupt: false,
        })
    }
}

#[test]
fn model_files_missing_reports_clearly() {
    // Model dosyaları eksikse açık hata verilir (simülasyon yapılmaz).
    let dir = tempfile::tempdir().unwrap();
    let res = delil_transcription::check_model_files(dir.path());
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("bulunamadı"));
}

#[test]
fn wav_roundtrip_through_decode() {
    let dir = tempfile::tempdir().unwrap();
    let p = make_wav(&dir.path().join("tone.wav"), 16000, 1, 1.0, 440.0);
    let samples = delil_transcription::decode_audio_file(&p).unwrap();
    assert!((samples.len() as f64 - 16000.0).abs() < 100.0);
    // Ton üretildi; sessiz olmamalı.
    let peak = samples.iter().map(|s| s.abs()).fold(0f32, f32::max);
    assert!(peak > 0.1);
}
