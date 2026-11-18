//! Arayüz ekranları ve çizim.
//!
//! Beş ekran: Ana ekran, Yeni inceleme, Otomatik işlem, Kontrol ve Sonuç.

use crate::app::DelilApp;
use delil_core::{PipelineStep, StepState};

/// Geçerli ekran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// Ana ekran.
    Home,
    /// Yeni inceleme.
    NewInvestigation,
    /// Otomatik işlem.
    Processing,
    /// Kontrol ekranı.
    Review,
    /// Sonuç ekranı.
    Result,
}

/// İş akışı adımlarının arayüzdeki durumu.
#[derive(Debug, Clone)]
pub struct WorkflowState {
    pub steps: Vec<(PipelineStep, StepState, String)>,
    /// Tamamlandı mı?
    pub finished: bool,
    /// Başarısız mı?
    pub failed: Option<String>,
    /// Bekleyen kullanıcı kararları.
    pub decisions: Vec<String>,
}

impl WorkflowState {
    pub fn new() -> Self {
        Self {
            steps: PipelineStep::ALL
                .iter()
                .map(|s| (*s, StepState::Pending, String::new()))
                .collect(),
            finished: false,
            failed: None,
            decisions: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Bir olayı duruma uygular.
    pub fn apply(&mut self, e: &delil_pipeline::PipelineEvent) {
        if let Some(entry) = self.steps.iter_mut().find(|(s, _, _)| *s == e.step) {
            entry.1 = e.state.clone();
            entry.2 = e.message.clone();
        }
        match &e.state {
            StepState::Failed { message } => self.failed = Some(message.clone()),
            StepState::NeedsDecision { message } => {
                if !self.decisions.contains(message) {
                    self.decisions.push(message.clone());
                }
            }
            StepState::Done if e.step == PipelineStep::ValidatingUdf => {
                self.finished = true;
            }
            _ => {}
        }
    }

    /// Genel ilerleme yüzdesi.
    pub fn progress(&self) -> f32 {
        let total = self.steps.len() as f32;
        let done = self
            .steps
            .iter()
            .filter(|(_, s, _)| matches!(s, StepState::Done))
            .count() as f32;
        done / total
    }
}

impl Default for WorkflowState {
    fn default() -> Self {
        Self::new()
    }
}

/// Ekranı çizer.
pub fn draw(app: &mut DelilApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("ust").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading("DELİL");
            ui.label("— Delili tak. Sesi çöz. UDF raporunu al.");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Ayrıntı paneli").clicked() {
                    app.show_logs = !app.show_logs;
                }
                if ui.button("Ana ekran").clicked() {
                    app.screen = Screen::Home;
                }
            });
        });
    });

    if app.show_logs {
        egui::SidePanel::right("loglar")
            .resizable(true)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.heading("Teknik ayrıntılar");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for line in &app.log {
                        ui.label(line);
                    }
                });
            });
    }

    egui::CentralPanel::default().show(ctx, |ui| match app.screen {
        Screen::Home => draw_home(app, ui),
        Screen::NewInvestigation => draw_new(app, ui),
        Screen::Processing => draw_processing(app, ui),
        Screen::Review => draw_review(app, ui),
        Screen::Result => draw_result(app, ui),
    });
}

/// Ana ekran (şartname 14.1).
fn draw_home(app: &mut DelilApp, ui: &mut egui::Ui) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        ui.heading("YENİ BİLİRKİŞİ RAPORU OLUŞTUR");
        ui.add_space(8.0);
        if ui
            .add_sized([320.0, 56.0], egui::Button::new("➕  Yeni bilirkişi raporu oluştur"))
            .clicked()
        {
            app.screen = Screen::NewInvestigation;
        }
    });
    ui.add_space(24.0);
    ui.separator();
    ui.heading("Devam eden inceleme");
    ui.label("Devam eden bir inceleme yok.");

    ui.add_space(12.0);
    ui.heading("Son oluşturulan raporlar");
    if app.recent_reports.is_empty() {
        ui.label("Henüz rapor oluşturulmadı.");
    } else {
        for r in &app.recent_reports {
            ui.label(r);
        }
    }

    ui.add_space(12.0);
    ui.heading("Bekleyen inceleme uyarıları");
    if app.devices.is_empty() {
        ui.label("Bağlı harici aygıt yok.");
    } else {
        ui.label(format!("{} harici aygıt algılandı.", app.devices.len()));
    }
}

/// Yeni inceleme ekranı (şartname 14.2).
fn draw_new(app: &mut DelilApp, ui: &mut egui::Ui) {
    ui.heading("Yeni inceleme");
    ui.add_space(8.0);

    ui.group(|ui| {
        ui.heading("Görevlendirme belgesi");
        ui.label("PDF, UDF, DOCX, TXT veya taranmış görsel belge seçin.");
        ui.horizontal(|ui| {
            if ui.button("Belge seç…").clicked() {
                if let Some(p) = rfd_pick_file() {
                    app.log(format!("Görevlendirme: {}", p.display()));
                    app.assignment_path = Some(p);
                }
            }
            match &app.assignment_path {
                Some(p) => ui.label(format!("Seçildi: {}", p.display())),
                None => ui.label("(seçilmedi — rapor zorunlu eksikleri isteyecek)"),
            };
        });
    });

    ui.add_space(8.0);
    ui.group(|ui| {
        ui.heading("Delil materyali");
        ui.horizontal(|ui| {
            if ui.button("Klasör seç…").clicked() {
                if let Some(p) = rfd_pick_folder() {
                    app.log(format!("Materyal: {}", p.display()));
                    app.material_path = Some(p);
                }
            }
            match &app.material_path {
                Some(p) => ui.label(format!("Seçildi: {}", p.display())),
                None => ui.label("(seçilmedi)"),
            };
        });
        ui.separator();
        ui.label("USB/SD algılama durumu:");
        if ui.button("Aygıtları yenile").clicked() {
            app.refresh_devices();
        }
        for d in &app.devices {
            ui.horizontal(|ui| {
                ui.label(format!("• {} — {}", d.root, d.kind.label_tr()));
                if d.is_system_disk {
                    ui.colored_label(egui::Color32::DARK_RED, "(sistem diski — seçilemez)");
                } else if ui.button("Bu aygıtı kullan").clicked() {
                    app.material_path = Some(std::path::PathBuf::from(&d.root));
                }
            });
        }
    });

    ui.add_space(8.0);
    ui.group(|ui| {
        ui.heading("Bilirkişi bilgileri");
        egui::Grid::new("bilirkisi").num_columns(2).show(ui, |ui| {
            ui.label("Adı Soyadı:");
            ui.text_edit_singleline(&mut app.expert_name);
            ui.end_row();
            ui.label("Unvan:");
            ui.text_edit_singleline(&mut app.expert_title);
            ui.end_row();
            ui.label("Sicil No:");
            ui.text_edit_singleline(&mut app.expert_registry);
            ui.end_row();
        });
    });

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui
            .add_sized([220.0, 40.0], egui::Button::new("İncelemeyi başlat"))
            .clicked()
        {
            app.start_investigation();
        }
        if ui.button("Geri").clicked() {
            app.screen = Screen::Home;
        }
    });
}

/// Otomatik işlem ekranı (şartname 14.3).
fn draw_processing(app: &mut DelilApp, ui: &mut egui::Ui) {
    ui.heading("Otomatik işlem");
    ui.add_space(8.0);
    let progress = app.workflow.progress();
    ui.add(egui::ProgressBar::new(progress).show_percentage());
    ui.add_space(8.0);

    for (step, state, msg) in &app.workflow.steps {
        ui.horizontal(|ui| {
            let (icon, color) = match state {
                StepState::Pending => ("○", egui::Color32::GRAY),
                StepState::Running { .. } => ("◐", egui::Color32::from_rgb(0, 100, 200)),
                StepState::Done => ("●", egui::Color32::from_rgb(0, 140, 0)),
                StepState::Failed { .. } => ("✗", egui::Color32::RED),
                StepState::NeedsDecision { .. } => ("!", egui::Color32::from_rgb(200, 120, 0)),
            };
            ui.colored_label(color, icon);
            ui.label(step.label_tr());
            if !msg.is_empty() {
                ui.weak(msg);
            }
        });
    }

    if let Some(f) = &app.workflow.failed {
        ui.add_space(8.0);
        ui.colored_label(egui::Color32::RED, format!("Hata: {f}"));
    }

    if !app.workflow.decisions.is_empty() {
        ui.add_space(12.0);
        ui.heading("Karar gereken konular");
        for d in &app.workflow.decisions {
            ui.label(format!("• {d}"));
        }
    }

    ui.add_space(16.0);
    ui.horizontal(|ui| {
        if app.workflow.finished {
            if ui
                .add_sized([200.0, 36.0], egui::Button::new("Kontrol ekranına geç"))
                .clicked()
            {
                app.screen = Screen::Review;
            }
        }
        if app.workflow.failed.is_some() && ui.button("Geri dön").clicked() {
            app.screen = Screen::NewInvestigation;
        }
    });
}

/// Kontrol ekranı (şartname 14.4).
fn draw_review(app: &mut DelilApp, ui: &mut egui::Ui) {
    ui.heading("Kontrol");
    ui.add_space(8.0);
    ui.label("Belirsiz işaretlenmiş bölümleri dinleyin, transkripti düzeltin ve soru yanıtlarını tamamlayın.");
    ui.add_space(8.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        if let Some(case) = &app.case {
            ui.group(|ui| {
                ui.heading("Ses ve transkript");
                for t in &case.transcripts {
                    let name = case
                        .audio_assets
                        .iter()
                        .find(|a| a.id == t.audio_asset_id)
                        .map(|a| a.file_name.clone())
                        .unwrap_or_default();
                    ui.collapsing(&name, |ui| {
                        ui.label(format!("Durum: {}", t.current().status.label_tr()));
                        ui.label(format!("Model: {} ({})", t.model_name, t.model_version));
                        for s in &t.current().segments {
                            ui.horizontal(|ui| {
                                ui.monospace(format!("[{:.1}s-{:.1}s]", s.start_seconds, s.end_seconds));
                                if s.quality != delil_core::QualityFlag::Normal {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(200, 120, 0),
                                        format!("({})", s.quality.label_tr()),
                                    );
                                }
                                ui.label(&s.text);
                            });
                        }
                    });
                }
                if case.transcripts.is_empty() {
                    ui.label("Transkript üretilmedi (model yüklenmemiş olabilir).");
                }
            });

            ui.add_space(8.0);
            ui.group(|ui| {
                ui.heading("Görevlendirme soruları");
                for q in &case.assignment.questions {
                    ui.label(format!("{}. {}", q.ordinal, q.original_text));
                    ui.label(format!(
                        "Yanıt: {}",
                        q.expert_answer.as_deref().unwrap_or("(yanıtlanmadı)")
                    ));
                }
                if case.assignment.questions.is_empty() {
                    ui.label("Soru bulunamadı.");
                }
            });

            if !case.exceptions.is_empty() {
                ui.add_space(8.0);
                ui.group(|ui| {
                    ui.heading("Karar gereken konular");
                    for e in &case.exceptions {
                        ui.label(format!("• {}", e.description));
                    }
                });
            }
        } else {
            ui.label("Vaka verisi yüklenmedi.");
        }
    });

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui
            .add_sized([200.0, 36.0], egui::Button::new("Sonuç ekranına geç"))
            .clicked()
        {
            app.screen = Screen::Result;
        }
    });
}

/// Sonuç ekranı (şartname 14.5).
fn draw_result(app: &mut DelilApp, ui: &mut egui::Ui) {
    ui.heading("Sonuç");
    ui.add_space(8.0);
    ui.group(|ui| {
        ui.heading("Üretilen dosyalar");
        match &app.udf_path {
            Some(p) => {
                ui.label(format!("UDF raporu: {}", p.display()));
                if ui.button("UDF dosyasını aç").clicked() {
                    open_path(p);
                }
            }
            None => {
                ui.label("UDF üretilmedi.");
            }
        }
        ui.label(&app.validation_summary);
    });

    ui.add_space(8.0);
    ui.group(|ui| {
        ui.heading("Doğrulama");
        ui.label("UDF yapısal doğrulaması tamamlandı.");
        ui.label(
            "Not: Bu önizleme, resmî UYAP Editör kontrolünün yerine geçmez. \
             Raporun son kontrolü UYAP Editör'de yapılmalıdır.",
        );
    });

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui.button("Ana ekran").clicked() {
            app.screen = Screen::Home;
        }
        if ui.button("Yeni inceleme").clicked() {
            app.screen = Screen::NewInvestigation;
        }
    });
}

/// Dosya seçme iletişim kutusu (native).
fn rfd_pick_file() -> Option<std::path::PathBuf> {
    // Native dosya seçici yoksa (başsız ortam), None döner.
    rfd::FileDialog::new()
        .add_filter("Belgeler", &["pdf", "udf", "docx", "doc", "txt", "png", "jpg", "jpeg", "tif", "tiff"])
        .pick_file()
}

fn rfd_pick_folder() -> Option<std::path::PathBuf> {
    rfd::FileDialog::new().pick_folder()
}

/// Dosyayı sistemin varsayılan uygulamasıyla açar.
fn open_path(p: &std::path::Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(p)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(p).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(p).spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use delil_pipeline::PipelineEvent;

    #[test]
    fn workflow_state_starts_pending() {
        let w = WorkflowState::new();
        assert_eq!(w.steps.len(), 8);
        assert!(w.steps.iter().all(|(_, s, _)| matches!(s, StepState::Pending)));
        assert_eq!(w.progress(), 0.0);
    }

    #[test]
    fn workflow_applies_done_events() {
        let mut w = WorkflowState::new();
        for step in PipelineStep::ALL {
            w.apply(&PipelineEvent {
                step,
                state: StepState::Done,
                message: "tamam".into(),
            });
        }
        assert_eq!(w.progress(), 1.0);
        assert!(w.finished);
    }

    #[test]
    fn workflow_records_failure_and_decisions() {
        let mut w = WorkflowState::new();
        w.apply(&PipelineEvent {
            step: PipelineStep::AcquiringMaterial,
            state: StepState::Failed {
                message: "disk dolu".into(),
            },
            message: "hata".into(),
        });
        assert_eq!(w.failed.as_deref(), Some("disk dolu"));
        w.apply(&PipelineEvent {
            step: PipelineStep::ReadingAssignment,
            state: StepState::NeedsDecision {
                message: "dosya no okunamadı".into(),
            },
            message: "karar".into(),
        });
        assert_eq!(w.decisions.len(), 1);
    }
}
