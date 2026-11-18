//! Uygulama durumu ve ana döngü.
//!
//! İş akışı bir arka plan iş parçacığında çalıştırılır; ilerleme kanal
//! üzerinden arayüze aktarılır. Böylece arayüz donmaz ve kullanıcı işlemi
//! izleyebilir.

use crate::screens::{Screen, WorkflowState};
use delil_core::{Case, PipelineStep};
use delil_pipeline::{PipelineConfig, PipelineEvent, PipelineRunner};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

/// Arayüz durumu.
pub struct DelilApp {
    /// Geçerli ekran.
    pub screen: Screen,
    /// İş akışı durumu.
    pub workflow: WorkflowState,
    /// Seçilen görevlendirme belgesi.
    pub assignment_path: Option<PathBuf>,
    /// Seçilen materyal kaynağı.
    pub material_path: Option<PathBuf>,
    /// Bilirkişi bilgileri.
    pub expert_name: String,
    pub expert_title: String,
    pub expert_registry: String,
    /// Algılanan aygıtlar.
    pub devices: Vec<delil_evidence::DetectedDevice>,
    /// Teknik log (ayrıntı paneli).
    pub log: Vec<String>,
    /// Ayrıntı paneli açık mı?
    pub show_logs: bool,
    /// Arka plan işinden gelen olaylar.
    rx: Option<Receiver<PipelineEvent>>,
    /// Tamamlanan vaka.
    pub case: Option<Case>,
    /// Üretilen UDF yolu.
    pub udf_path: Option<PathBuf>,
    /// Doğrulama özeti.
    pub validation_summary: String,
    /// Hata mesajı.
    pub error: Option<String>,
    /// Son oluşturulan raporlar (basit liste).
    pub recent_reports: Vec<String>,
}

impl DelilApp {
    pub fn new() -> Self {
        let devices = delil_evidence::detect_devices();
        Self {
            screen: Screen::Home,
            workflow: WorkflowState::new(),
            assignment_path: None,
            material_path: None,
            expert_name: String::new(),
            expert_title: String::new(),
            expert_registry: String::new(),
            devices,
            log: vec!["DELİL başlatıldı.".to_string()],
            show_logs: false,
            rx: None,
            case: None,
            udf_path: None,
            validation_summary: String::new(),
            error: None,
            recent_reports: Vec::new(),
        }
    }

    /// Aygıt listesini yeniler.
    pub fn refresh_devices(&mut self) {
        self.devices = delil_evidence::detect_devices();
        self.log(format!("{} aygıt algılandı.", self.devices.len()));
    }

    /// İncelemeyi başlatır.
    pub fn start_investigation(&mut self) {
        let mut cfg = PipelineConfig::new(
            dirs_case_root().unwrap_or_else(|| PathBuf::from("./delil-vaka")),
        );
        cfg.assignment_path = self.assignment_path.clone();
        cfg.material_source = self.material_path.clone();
        if let Some(m) = &self.material_path {
            cfg.source = Some(delil_evidence::local_folder_source(m));
        }
        cfg.expert = delil_core::ExpertInfo {
            full_name: non_empty(&self.expert_name),
            title: non_empty(&self.expert_title),
            registry_number: non_empty(&self.expert_registry),
        };
        // Model dizini varsa transkripsiyon açılır; yoksa edinim ve rapor
        // aşamaları çalışır, kullanıcıya model eksikliği bildirilir.
        cfg.transcribe = transcription_model_available();

        self.workflow.reset();
        self.error = None;
        self.screen = Screen::Processing;

        let (tx, rx) = channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let mut runner = PipelineRunner::new(cfg);
            let mut cb = |e: PipelineEvent| {
                let _ = tx.send(e);
            };
            runner.events = Some(&mut cb);
            let result = runner.run();
            // Sonucu özel bir olayla iletmek yerine kanalı kapatırız; hata
            // durumunda log üzerinden bildirilir.
            if let Err(e) = result {
                let _ = tx.send(PipelineEvent {
                    step: PipelineStep::ValidatingUdf,
                    state: delil_core::StepState::Failed {
                        message: e.to_string(),
                    },
                    message: format!("İşlem durdu: {e}"),
                });
            }
        });
    }

    /// Olayları işler.
    pub fn poll_events(&mut self) {
        let mut events = Vec::new();
        if let Some(rx) = &self.rx {
            while let Ok(e) = rx.try_recv() {
                events.push(e);
            }
        }
        for e in events {
            self.log(format!("[{}] {}", e.step.label_tr(), e.message));
            self.workflow.apply(&e);
        }
    }

    /// Log satırı ekler.
    pub fn log(&mut self, line: impl Into<String>) {
        let ts = chrono::Local::now().format("%H:%M:%S");
        self.log.push(format!("{ts}  {}", line.into()));
        if self.log.len() > 2000 {
            self.log.drain(0..500);
        }
    }
}

impl Default for DelilApp {
    fn default() -> Self {
        Self::new()
    }
}

/// Varsayılan vaka kök dizini.
fn dirs_case_root() -> Option<PathBuf> {
    let base = std::env::var("DELIL_CASE_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .ok()
                .map(|h| PathBuf::from(h).join("DELIL").join("vakalar"))
        })?;
    let _ = std::fs::create_dir_all(&base);
    Some(base.join(format!(
        "vaka-{}",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    )))
}

/// Transkripsiyon modeli mevcut mu?
fn transcription_model_available() -> bool {
    for var in ["DELIL_MODEL_DIR"] {
        if let Ok(d) = std::env::var(var) {
            let p = PathBuf::from(d);
            if p.join("model.safetensors").exists() {
                return true;
            }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            if parent.join("models").join("model.safetensors").exists() {
                return true;
            }
        }
    }
    false
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

impl eframe::App for DelilApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();
        if self.screen == Screen::Processing {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        crate::screens::draw(self, ctx);
    }
}
