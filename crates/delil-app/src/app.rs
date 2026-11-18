//! Uygulama durumu ve ana döngü.
//!
//! İş akışı bir arka plan iş parçacığında çalıştırılır; ilerleme kanal
//! üzerinden arayüze aktarılır. Böylece arayüz donmaz ve kullanıcı işlemi
//! izleyebilir.

use crate::screens::{Screen, WorkflowState};
use delil_core::{Case, PipelineStep, TranscriptEditor};
use delil_pipeline::{PipelineConfig, PipelineEvent, PipelineResult, PipelineRunner};
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
    /// İş akışı tamamlandığında gelen sonuç.
    result_rx: Option<Receiver<Result<PipelineResult, String>>>,
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
    /// Kontrol ekranında seçili transkript sırası.
    pub selected_transcript: usize,
    /// Etkin transkript düzenleyicisi.
    pub editor: Option<TranscriptEditor>,
    /// Etkin transkriptin geçerli dalga biçimi.
    pub waveform: Option<delil_audio::Waveform>,
    /// Oynatma durumu.
    pub playback: delil_audio::PlaybackState,
    /// Metin düzeltme alanı (seçili segment).
    pub edit_buffer: String,
    /// Düzeltilen segment sırası.
    pub edit_index: usize,
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
            result_rx: None,
            case: None,
            udf_path: None,
            validation_summary: String::new(),
            error: None,
            recent_reports: Vec::new(),
            selected_transcript: 0,
            editor: None,
            waveform: None,
            playback: delil_audio::PlaybackState::default(),
            edit_buffer: String::new(),
            edit_index: 0,
        }
    }

    /// Bir transkripti düzenlemek üzere yükler; dalga biçimini hazırlar.
    pub fn load_transcript_for_edit(&mut self, index: usize) {
        let Some(case) = &self.case else { return };
        let Some(t) = case.transcripts.get(index) else {
            return;
        };
        self.selected_transcript = index;
        self.editor = Some(TranscriptEditor::from_transcript(t, "bilirkişi"));
        // Dalga biçimi için ses örneğini çöz.
        let asset = case
            .audio_assets
            .iter()
            .find(|a| a.id == t.audio_asset_id)
            .cloned();
        let material = asset.and_then(|a| {
            case.materials
                .iter()
                .find(|m| m.id == a.material_id)
                .cloned()
        });
        if let Some(m) = material {
            let root = std::path::PathBuf::from(&case.case_root).join("materials");
            let p = root.join(&m.stored_relative_path);
            if let Ok(samples) = delil_transcription::decode_audio_file(&p) {
                self.playback.duration_seconds = samples.len() as f64 / 16000.0;
                self.waveform = Some(delil_audio::Waveform::from_samples(&samples, 800));
            }
        }
        if let Some(ed) = &self.editor {
            if let Some(seg) = ed.segments.first() {
                self.edit_buffer = seg.text.clone();
            }
        }
    }

    /// Seçili segmentin metin düzeltmesini uygular.
    pub fn apply_text_edit(&mut self) {
        if let Some(ed) = self.editor.as_mut() {
            let idx = self.edit_index;
            let text = self.edit_buffer.clone();
            if ed
                .apply(delil_core::EditOperation::SetText { index: idx, text })
                .is_ok()
            {
                self.log(format!("Segment {idx} düzeltildi."));
            }
        }
    }

    /// Düzenlemeleri transkripte yeni sürüm olarak kaydeder.
    pub fn commit_edits(&mut self) {
        let (Some(ed), Some(case)) = (self.editor.as_ref(), self.case.as_mut()) else {
            return;
        };
        let idx = self.selected_transcript;
        if let Some(t) = case.transcripts.get_mut(idx) {
            let v = ed.commit_to(t);
            self.log(format!("Transkript {idx} yeni sürüm: {v}"));
        }
    }

    /// Bilirkişi onayını kaydeder.
    pub fn approve_transcript(&mut self) {
        let (Some(ed), Some(case)) = (self.editor.as_ref(), self.case.as_mut()) else {
            return;
        };
        let idx = self.selected_transcript;
        if let Some(t) = case.transcripts.get_mut(idx) {
            let v = ed.approve(t);
            self.log(format!("Transkript {idx} onaylandı (sürüm {v})"));
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
        let (rtx, rrx) = channel();
        self.rx = Some(rx);
        self.result_rx = Some(rrx);
        std::thread::spawn(move || {
            let mut runner = PipelineRunner::new(cfg);
            let mut cb = |e: PipelineEvent| {
                let _ = tx.send(e);
            };
            runner.events = Some(&mut cb);
            match runner.run() {
                Ok(res) => {
                    let _ = rtx.send(Ok(res));
                }
                Err(e) => {
                    let _ = tx.send(PipelineEvent {
                        step: PipelineStep::ValidatingUdf,
                        state: delil_core::StepState::Failed {
                            message: e.to_string(),
                        },
                        message: format!("İşlem durdu: {e}"),
                    });
                    let _ = rtx.send(Err(e.to_string()));
                }
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
        // Sonuç geldiyse vakayı yükle.
        let mut results = Vec::new();
        if let Some(rx) = &self.result_rx {
            while let Ok(r) = rx.try_recv() {
                results.push(r);
            }
        }
        for r in results {
            match r {
                Ok(res) => {
                    self.validation_summary = res.validation.summary_tr();
                    self.udf_path = Some(res.udf_path.clone());
                    self.recent_reports.push(format!(
                        "{} — {}",
                        res.case
                            .assignment
                            .case_number
                            .clone()
                            .unwrap_or_else(|| "(dosya no yok)".into()),
                        res.udf_path.display()
                    ));
                    self.case = Some(res.case);
                    self.workflow.finished = true;
                    self.log("İşlem tamamlandı; UDF üretildi.".to_string());
                }
                Err(e) => {
                    self.error = Some(e.clone());
                    self.log(format!("Hata: {e}"));
                }
            }
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
