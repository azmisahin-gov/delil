//! Otomatik transkripsiyon kuyruğu (şartname 8.4).
//!
//! Kuyruk özellikleri: işlem durumu, yüzde, başlama/bitiş zamanı, hata
//! bilgisi, otomatik yeniden deneme, duraklatma, devam, iptal ve kesintiden
//! devam. Kuyruk durumu JSON olarak diske yazılır; uygulama kapansa veya
//! bilgisayar yeniden başlasa bile işlem kaldığı yerden devam eder.
//!
//! Aynı dosyanın aynı ayarlarla tekrar işlenmesi mükerrer transkript
//! oluşturmaz: aynı (dosya hash'i + ayar imzası) anahtarıyla tamamlanmış bir
//! iş varsa atlanır. Yeni bir işlem istenirse yeni bir sürüm üretilir.

use crate::engine::{ControlFlag, TranscriptionEngine, TranscriptionOptions};
use crate::{Result, TranscriptionError};
use delil_core::JobState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Kuyruktaki tek bir iş.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: String,
    /// Ses varlığı kimliği (metinsel).
    pub audio_asset_id: String,
    /// Girdi dosyası (yerel vaka deposundaki kopya).
    pub input_path: PathBuf,
    /// Dosyanın SHA-256'sı (mükerrerlik tespiti için).
    pub sha256: Option<String>,
    /// Ayar imzası (dil, zaman kodu vb.).
    pub options_signature: String,
    pub state: JobState,
    pub attempts: u32,
    /// Azami deneme sayısı.
    pub max_attempts: u32,
    /// İşlem başlangıç/bitiş zamanı.
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Tamamlanan saniye (kesintiden devam için).
    pub completed_seconds: f64,
    /// Toplam süre (saniye).
    pub total_seconds: f64,
    /// Hata mesajı (varsa).
    pub error: Option<String>,
}

impl QueueItem {
    pub fn new(
        audio_asset_id: impl Into<String>,
        input_path: PathBuf,
        sha256: Option<String>,
        options: &TranscriptionOptions,
    ) -> Self {
        Self {
            id: delil_core::new_id(),
            audio_asset_id: audio_asset_id.into(),
            input_path,
            sha256,
            options_signature: options_signature(options),
            state: JobState::Queued,
            attempts: 0,
            max_attempts: 3,
            started_at: None,
            finished_at: None,
            completed_seconds: 0.0,
            total_seconds: 0.0,
            error: None,
        }
    }

    /// Mükerrerlik anahtarı.
    pub fn dedup_key(&self) -> String {
        let id = match &self.sha256 {
            Some(s) => s.clone(),
            None => self.input_path.to_string_lossy().into_owned(),
        };
        format!("{}::{}", id, self.options_signature)
    }

    /// İşlem yüzdesi (0.0..1.0).
    pub fn progress(&self) -> f32 {
        if self.total_seconds <= 0.0 {
            return 0.0;
        }
        (self.completed_seconds / self.total_seconds).clamp(0.0, 1.0) as f32
    }
}

/// Ayar imzası: dil + zaman kodu + parça uzunluğu.
pub fn options_signature(o: &TranscriptionOptions) -> String {
    format!(
        "lang={};ts={};chunk={}",
        o.language.as_deref().unwrap_or("auto"),
        o.timestamps,
        o.max_chunk_seconds
    )
}

/// Kuyruk kalıcılığı için serileştirilmiş durum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuePersist {
    pub version: u8,
    pub items: Vec<QueueItem>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Bir işin sonucu.
#[derive(Debug, Clone, PartialEq)]
pub enum QueueOutcome {
    /// İş tamamlandı; motor sonucu.
    Done(crate::engine::TranscriptionResult),
    /// Mükerrer olduğu için atlandı.
    SkippedDuplicate,
    /// Hata nedeniyle başarısız.
    Failed(String),
    /// İptal edildi.
    Cancelled,
    /// Duraklatıldı.
    Paused,
}

/// Otomatik transkripsiyon kuyruğu.
pub struct Queue {
    items: Vec<QueueItem>,
    /// Tamamlanmış işlerin mükerrerlik anahtarları.
    completed_keys: HashMap<String, String>,
    persist_path: Option<PathBuf>,
}

impl Queue {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            completed_keys: HashMap::new(),
            persist_path: None,
        }
    }

    /// Kalıcı kuyruk: belirtilen dosyadan yükler veya yeni oluşturur.
    pub fn with_persistence(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let mut q = Self {
            items: Vec::new(),
            completed_keys: HashMap::new(),
            persist_path: Some(path.clone()),
        };
        if path.exists() {
            let data = std::fs::read(&path)?;
            let p: QueuePersist = serde_json::from_slice(&data)?;
            q.items = p.items;
            for it in &q.items {
                if it.state == JobState::Done {
                    q.completed_keys.insert(it.dedup_key(), it.id.clone());
                }
            }
        }
        Ok(q)
    }

    /// Kuyruğa iş ekler. Mükerrer (aynı hash + ayar) ise eklenmez.
    pub fn enqueue(&mut self, item: QueueItem) -> bool {
        let key = item.dedup_key();
        if self.completed_keys.contains_key(&key) {
            return false;
        }
        // Kuyrukta aynı anahtarla bekleyen iş varsa tekrar ekleme.
        if self
            .items
            .iter()
            .any(|i| i.dedup_key() == key && !i.state.is_terminal())
        {
            return false;
        }
        self.items.push(item);
        let _ = self.save();
        true
    }

    /// Bekleyen iş sayısı.
    pub fn pending_count(&self) -> usize {
        self.items.iter().filter(|i| !i.state.is_terminal()).count()
    }

    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut [QueueItem] {
        &mut self.items
    }

    /// Kesintiden devam: yarıda kalan (Running) işleri yeniden kuyruğa alır.
    pub fn resume_interrupted(&mut self) {
        for it in self.items.iter_mut() {
            if matches!(it.state, JobState::Running { .. } | JobState::Paused { .. }) {
                it.state = JobState::Queued;
            }
        }
        let _ = self.save();
    }

    /// Kuyruktaki tüm bekleyen işleri sırayla işler.
    ///
    /// `control` bayrağı ile duraklatma/iptal edilebilir. Her iş için motor
    /// çağrılır; hata olursa otomatik yeniden deneme uygulanır.
    pub fn run_all(
        &mut self,
        engine: &mut dyn TranscriptionEngine,
        options: &TranscriptionOptions,
        control: &ControlFlag,
        decode: &dyn Fn(&Path) -> Result<Vec<f32>>,
    ) -> Vec<QueueOutcome> {
        let mut outcomes = Vec::new();
        let ids: Vec<String> = self
            .items
            .iter()
            .filter(|i| !i.state.is_terminal())
            .map(|i| i.id.clone())
            .collect();
        for id in ids {
            if control.is_cancelled() {
                outcomes.push(QueueOutcome::Cancelled);
                break;
            }
            if control.is_paused() {
                if let Some(it) = self.items.iter_mut().find(|i| i.id == id) {
                    let p = it.progress();
                    it.state = JobState::Paused { progress: p };
                }
                let _ = self.save();
                outcomes.push(QueueOutcome::Paused);
                break;
            }
            let outcome = self.run_one(&id, engine, options, control, decode);
            // Otomatik yeniden deneme: iş hâlâ kuyrukta (Queued) ise ve azami
            // deneme sayısı aşılmadıysa aynı çağrı içinde tekrar denenir.
            let mut outcome = outcome;
            loop {
                if !matches!(outcome, QueueOutcome::Failed(_)) {
                    break;
                }
                let requeued = self
                    .items
                    .iter()
                    .find(|i| i.id == id)
                    .map(|i| i.state == JobState::Queued && i.attempts < i.max_attempts)
                    .unwrap_or(false);
                if !requeued {
                    break;
                }
                outcome = self.run_one(&id, engine, options, control, decode);
            }
            match &outcome {
                QueueOutcome::Done(_) => {
                    if let Some(it) = self.items.iter().find(|i| i.id == id) {
                        self.completed_keys.insert(it.dedup_key(), it.id.clone());
                    }
                }
                QueueOutcome::SkippedDuplicate => {}
                _ => {}
            }
            let terminal = matches!(outcome, QueueOutcome::Cancelled | QueueOutcome::Paused);
            outcomes.push(outcome);
            if terminal {
                break;
            }
        }
        let _ = self.save();
        outcomes
    }

    /// Tek bir işi çalıştırır.
    pub fn run_one(
        &mut self,
        id: &str,
        engine: &mut dyn TranscriptionEngine,
        options: &TranscriptionOptions,
        control: &ControlFlag,
        decode: &dyn Fn(&Path) -> Result<Vec<f32>>,
    ) -> QueueOutcome {
        // İşi bul.
        let idx = match self.items.iter().position(|i| i.id == id) {
            Some(i) => i,
            None => return QueueOutcome::Failed("İş bulunamadı".into()),
        };
        // Mükerrer kontrolü.
        {
            let key = self.items[idx].dedup_key();
            if self.completed_keys.contains_key(&key) {
                self.items[idx].state = JobState::Done;
                return QueueOutcome::SkippedDuplicate;
            }
        }

        // Duraklatma/iptal kontrolü.
        if let Err(e) = control.wait_if_paused() {
            let p = self.items[idx].progress();
            self.items[idx].state = match e {
                TranscriptionError::Cancelled => JobState::Cancelled,
                _ => JobState::Paused { progress: p },
            };
            let _ = self.save();
            return match e {
                TranscriptionError::Cancelled => QueueOutcome::Cancelled,
                _ => QueueOutcome::Paused,
            };
        }

        let input = self.items[idx].input_path.clone();
        let asset_id = self.items[idx].audio_asset_id.clone();
        let _ = asset_id;
        let attempt = self.items[idx].attempts + 1;
        let max_attempts = self.items[idx].max_attempts;
        self.items[idx].attempts = attempt;
        self.items[idx].started_at = Some(chrono::Utc::now());
        self.items[idx].state = JobState::Running { progress: 0.0 };
        let _ = self.save();

        // Ses çözme.
        let samples = match decode(&input) {
            Ok(s) => s,
            Err(e) => {
                return self.fail(idx, attempt, max_attempts, e.to_string());
            }
        };
        let total = samples.len() as f64 / crate::SAMPLE_RATE as f64;
        self.items[idx].total_seconds = total;

        // Transkripsiyon.
        let mut last_sec = 0.0f64;
        let mut progress_cb = |done: f64, _tot: f64| {
            last_sec = done;
        };
        let res = engine.transcribe(&samples, options, control, Some(&mut progress_cb));
        self.items[idx].completed_seconds = if last_sec > 0.0 { last_sec } else { total };

        match res {
            Ok(r) => {
                self.items[idx].state = JobState::Done;
                self.items[idx].finished_at = Some(chrono::Utc::now());
                self.items[idx].error = None;
                let _ = self.save();
                QueueOutcome::Done(r)
            }
            Err(TranscriptionError::Cancelled) => {
                self.items[idx].state = JobState::Cancelled;
                let _ = self.save();
                QueueOutcome::Cancelled
            }
            Err(TranscriptionError::Paused) => {
                let p = self.items[idx].progress();
                self.items[idx].state = JobState::Paused { progress: p };
                let _ = self.save();
                QueueOutcome::Paused
            }
            Err(e) => self.fail(idx, attempt, max_attempts, e.to_string()),
        }
    }

    fn fail(
        &mut self,
        idx: usize,
        attempt: u32,
        max_attempts: u32,
        message: String,
    ) -> QueueOutcome {
        if attempt < max_attempts {
            // Otomatik yeniden deneme: durumu kuyruğa geri al.
            self.items[idx].state = JobState::Queued;
            self.items[idx].error = Some(message.clone());
        } else {
            self.items[idx].state = JobState::Failed {
                message: message.clone(),
                attempts: attempt,
            };
            self.items[idx].finished_at = Some(chrono::Utc::now());
        }
        let _ = self.save();
        QueueOutcome::Failed(message)
    }

    /// Kuyruğu diske yazar.
    pub fn save(&self) -> Result<()> {
        if let Some(path) = &self.persist_path {
            let p = QueuePersist {
                version: 1,
                items: self.items.clone(),
                updated_at: chrono::Utc::now(),
            };
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, serde_json::to_vec_pretty(&p)?)?;
        }
        Ok(())
    }
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{EngineProgress, Segment, TranscriptionResult};

    /// Test motoru: örnekleri bilinen bir metne dönüştürür.
    struct FakeEngine {
        fail_times: std::cell::Cell<u32>,
        text: String,
    }

    impl FakeEngine {
        fn new(text: &str) -> Self {
            Self {
                fail_times: std::cell::Cell::new(0),
                text: text.to_string(),
            }
        }
        fn failing(text: &str, times: u32) -> Self {
            Self {
                fail_times: std::cell::Cell::new(times),
                text: text.to_string(),
            }
        }
    }

    impl TranscriptionEngine for FakeEngine {
        fn name(&self) -> &str {
            "fake"
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
            mut progress: Option<EngineProgress<'_>>,
        ) -> Result<TranscriptionResult> {
            control.wait_if_paused()?;
            if self.fail_times.get() > 0 {
                self.fail_times.set(self.fail_times.get() - 1);
                return Err(TranscriptionError::Inference("geçici hata".into()));
            }
            let dur = samples.len() as f64 / crate::SAMPLE_RATE as f64;
            if let Some(cb) = progress.as_mut() {
                cb(dur, dur);
            }
            Ok(TranscriptionResult {
                segments: vec![Segment {
                    start: 0.0,
                    end: dur,
                    text: self.text.clone(),
                    avg_logprob: Some(-0.2),
                    no_speech_prob: Some(0.01),
                }],
                detected_language: Some("tr".into()),
                duration_seconds: dur,
                rms: 0.2,
                peak: 0.5,
                corrupt: false,
            })
        }
    }

    fn decode_ok(_p: &Path) -> Result<Vec<f32>> {
        Ok(vec![0.1f32; crate::SAMPLE_RATE]) // 1 saniye
    }

    fn item(name: &str, sha: &str) -> QueueItem {
        QueueItem::new(
            name,
            PathBuf::from(format!("/tmp/{name}.wav")),
            Some(sha.to_string()),
            &TranscriptionOptions::default(),
        )
    }

    #[test]
    fn queue_processes_all_items() {
        let mut q = Queue::new();
        assert!(q.enqueue(item("a", "sha-a")));
        assert!(q.enqueue(item("b", "sha-b")));
        assert_eq!(q.pending_count(), 2);
        let mut eng = FakeEngine::new("Merhaba dünya");
        let control = ControlFlag::new();
        let out = q.run_all(
            &mut eng,
            &TranscriptionOptions::default(),
            &control,
            &decode_ok,
        );
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|o| matches!(o, QueueOutcome::Done(_))));
        assert_eq!(q.pending_count(), 0);
    }

    #[test]
    fn duplicate_same_hash_and_options_is_skipped() {
        let mut q = Queue::new();
        assert!(q.enqueue(item("a", "same-sha")));
        let mut eng = FakeEngine::new("x");
        let control = ControlFlag::new();
        q.run_all(
            &mut eng,
            &TranscriptionOptions::default(),
            &control,
            &decode_ok,
        );
        // Aynı hash ve ayarla tekrar ekleme reddedilir.
        assert!(!q.enqueue(item("a2", "same-sha")));
        assert_eq!(q.pending_count(), 0);
    }

    #[test]
    fn different_options_produce_new_job() {
        let mut q = Queue::new();
        assert!(q.enqueue(item("a", "sha-x")));
        let mut eng = FakeEngine::new("x");
        let control = ControlFlag::new();
        q.run_all(
            &mut eng,
            &TranscriptionOptions::default(),
            &control,
            &decode_ok,
        );

        // Farklı ayar imzası -> yeni iş.
        let opts = TranscriptionOptions {
            language: Some("en".into()),
            ..Default::default()
        };
        let it = QueueItem::new(
            "a",
            PathBuf::from("/tmp/a.wav"),
            Some("sha-x".into()),
            &opts,
        );
        assert!(q.enqueue(it));
    }

    #[test]
    fn auto_retry_on_transient_failure() {
        let mut q = Queue::new();
        assert!(q.enqueue(item("a", "sha-r")));
        // İlk 2 deneme başarısız, 3. başarılı; kuyruk tek çağrıda yeniden dener.
        let mut eng = FakeEngine::failing("başarılı", 2);
        let control = ControlFlag::new();
        let out = q.run_all(
            &mut eng,
            &TranscriptionOptions::default(),
            &control,
            &decode_ok,
        );
        assert!(
            matches!(out[0], QueueOutcome::Done(_)),
            "otomatik yeniden deneme başarılı olmalı: {:?}",
            out[0]
        );
        assert_eq!(q.pending_count(), 0);
        // Deneme sayısı 3 olmalı.
        assert_eq!(q.items()[0].attempts, 3);
    }

    #[test]
    fn exhausts_retries_and_fails() {
        let mut q = Queue::new();
        assert!(q.enqueue(item("a", "sha-rf")));
        let mut eng = FakeEngine::failing("asla", 100);
        let control = ControlFlag::new();
        let out = q.run_all(
            &mut eng,
            &TranscriptionOptions::default(),
            &control,
            &decode_ok,
        );
        assert!(matches!(out[0], QueueOutcome::Failed(_)));
        assert!(matches!(q.items()[0].state, JobState::Failed { .. }));
    }

    #[test]
    fn cancel_stops_queue() {
        let mut q = Queue::new();
        q.enqueue(item("a", "sha-c1"));
        q.enqueue(item("b", "sha-c2"));
        let mut eng = FakeEngine::new("x");
        let control = ControlFlag::new();
        control.cancel();
        let out = q.run_all(
            &mut eng,
            &TranscriptionOptions::default(),
            &control,
            &decode_ok,
        );
        assert!(matches!(out[0], QueueOutcome::Cancelled));
    }

    #[test]
    fn pause_stops_queue() {
        let mut q = Queue::new();
        q.enqueue(item("a", "sha-p1"));
        let mut eng = FakeEngine::new("x");
        let control = ControlFlag::new();
        control.pause();
        let out = q.run_all(
            &mut eng,
            &TranscriptionOptions::default(),
            &control,
            &decode_ok,
        );
        assert!(matches!(out[0], QueueOutcome::Paused));
        // Duraklatılmış iş kuyrukta kalır.
        assert_eq!(q.pending_count(), 1);
    }

    #[test]
    fn persistence_and_resume() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("queue.json");
        {
            let mut q = Queue::with_persistence(&path).unwrap();
            q.enqueue(item("a", "sha-p"));
            // İşi yarıda bırak (Running).
            q.items_mut()[0].state = JobState::Running { progress: 0.4 };
            q.items_mut()[0].completed_seconds = 0.4;
            q.save().unwrap();
        }
        // Yeniden yükle.
        let mut q = Queue::with_persistence(&path).unwrap();
        assert_eq!(q.items().len(), 1);
        q.resume_interrupted();
        assert_eq!(q.items()[0].state, JobState::Queued);
        // Kaldığı yerden devam eder.
        let mut eng = FakeEngine::new("devam");
        let control = ControlFlag::new();
        let out = q.run_all(
            &mut eng,
            &TranscriptionOptions::default(),
            &control,
            &decode_ok,
        );
        assert!(matches!(out[0], QueueOutcome::Done(_)));
    }

    #[test]
    fn decode_failure_is_recorded() {
        let mut q = Queue::new();
        q.enqueue(item("a", "sha-d"));
        let mut eng = FakeEngine::new("x");
        let control = ControlFlag::new();
        let fail_decode =
            |_p: &Path| -> Result<Vec<f32>> { Err(TranscriptionError::Decode("bozuk ses".into())) };
        let out = q.run_all(
            &mut eng,
            &TranscriptionOptions::default(),
            &control,
            &fail_decode,
        );
        assert!(matches!(out[0], QueueOutcome::Failed(_)));
    }

    #[test]
    fn progress_percentage() {
        let mut it = item("a", "sha");
        it.total_seconds = 100.0;
        it.completed_seconds = 25.0;
        assert!((it.progress() - 0.25).abs() < 0.001);
    }
}
