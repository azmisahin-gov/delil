//! Arayüz ekranları ve çizim.
//!
//! Beş ekran: Ana ekran, Yeni inceleme, Otomatik işlem, Kontrol ve Sonuç.

use crate::app::DelilApp;
use delil_core::{PipelineStep, StepState};
use std::path::PathBuf;

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

    // Yazma koruması onay penceresi (kaynağı değiştiren işlem: açık onay).
    draw_write_protect_dialog(app, ctx);
}

/// Yazma koruması uygulama/kaldırma onay penceresi.
///
/// Kaynağı etkileyen işlemler için **açık onay** alınır (şartname 4.3).
fn draw_write_protect_dialog(app: &mut DelilApp, ctx: &egui::Context) {
    let mut action: Option<(PathBuf, bool)> = None; // (yol, uygula?)
    if let Some(p) = app.pending_write_protect.clone() {
        egui::Window::new("Yazma koruması uygula")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("Aygıt: {}", p.display()));
                ui.add_space(6.0);
                ui.colored_label(
                    egui::Color32::from_rgb(255, 200, 120),
                    "Bu işlem aygıtı Windows düzeyinde SALT OKUNUR yapar \
                     (diskpart; yönetici yetkisi gerekir).",
                );
                ui.label(
                    "• Bu, donanımsal yazma engelleyici ile EŞDEĞER DEĞİLDİR.\n\
                     • Aygıt ilk bağlandığında Windows'un yazmış olabileceği \
                     ihtimali ortadan kaldırmaz.\n\
                     • Kaldırılana kadar aygıt salt okunur kalır.",
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Salt okunur yap").clicked() {
                        action = Some((p.clone(), true));
                    }
                    if ui.button("Vazgeç").clicked() {
                        action = Some((p.clone(), false));
                    }
                });
            });
    } else if let Some(p) = app.pending_write_clear.clone() {
        egui::Window::new("Yazma korumasını kaldır")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("Aygıt: {}", p.display()));
                ui.add_space(6.0);
                ui.colored_label(
                    egui::Color32::from_rgb(255, 200, 120),
                    "Bu işlem aygıtın salt okunur korumasını KALDIRIR.",
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Korumayı kaldır").clicked() {
                        action = Some((p.clone(), false));
                    }
                    if ui.button("Vazgeç").clicked() {
                        action = Some((p.clone(), true));
                    }
                });
            });
    }

    if let Some((path, apply)) = action {
        // Onay/iptal: her durumda bekleyen isteği temizle.
        let was_protect = app.pending_write_protect.is_some();
        app.pending_write_protect = None;
        app.pending_write_clear = None;

        if was_protect && apply {
            match delil_evidence::apply_software_write_protection(&path) {
                Ok(wp) => app.log(format!("Yazma koruması uygulandı: {}", wp.label_tr())),
                Err(e) => app.log(format!("Yazma koruması uygulanamadı: {e}")),
            }
            app.refresh_devices();
        } else if !was_protect && !apply {
            match delil_evidence::clear_software_write_protection(&path) {
                Ok(()) => app.log("Yazma koruması kaldırıldı.".to_string()),
                Err(e) => app.log(format!("Koruma kaldırılamadı: {e}")),
            }
            app.refresh_devices();
        }
    }
}

/// Ana ekran (şartname 14.1).
fn draw_home(app: &mut DelilApp, ui: &mut egui::Ui) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        ui.heading("YENİ BİLİRKİŞİ RAPORU OLUŞTUR");
        ui.add_space(8.0);
        if ui
            .add_sized(
                [320.0, 56.0],
                egui::Button::new("➕  Yeni bilirkişi raporu oluştur"),
            )
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
                    app.load_assignment(&p);
                }
            }
            match &app.assignment_path {
                Some(p) => ui.label(format!("Seçildi: {}", p.display())),
                None => ui.label("(seçilmedi — rapor zorunlu eksikleri isteyecek)"),
            };
        });
        // Belgeden çıkarılan bilgileri göster (bilirkişi doğrulayabilsin).
        if let Some(a) = &app.assignment_preview {
            ui.add_space(4.0);
            ui.separator();
            ui.label("Belgeden çıkarılan bilgiler:");
            egui::Grid::new("gorevlendirme_onizleme")
                .num_columns(2)
                .show(ui, |ui| {
                    let row = |ui: &mut egui::Ui, k: &str, v: &Option<String>| {
                        ui.label(k);
                        match v {
                            Some(s) if !s.trim().is_empty() => {
                                ui.label(s);
                            }
                            _ => {
                                ui.colored_label(
                                    egui::Color32::from_rgb(200, 120, 0),
                                    "(okunamadı)",
                                );
                            }
                        }
                        ui.end_row();
                    };
                    row(ui, "Dosya No:", &a.case_number);
                    row(ui, "Mahkeme:", &a.court);
                    row(ui, "Konu:", &a.subject);
                    row(ui, "Görevlendirme tarihi:", &a.assignment_date_text);
                    row(ui, "Teslim süresi:", &a.due_date_text);
                    ui.label("Bilirkişi:");
                    ui.label(a.expert.full_name.clone().unwrap_or_else(|| "—".into()));
                    ui.end_row();
                    ui.label("Sicil No:");
                    ui.label(
                        a.expert
                            .registry_number
                            .clone()
                            .unwrap_or_else(|| "—".into()),
                    );
                    ui.end_row();
                });
            ui.label(format!(
                "{} taraf, {} soru bulundu.",
                a.parties.len(),
                a.questions.len()
            ));
            if !a.questions.is_empty() {
                ui.collapsing("Soruları göster", |ui| {
                    for q in &a.questions {
                        ui.label(format!("{}. {}", q.ordinal, q.original_text));
                    }
                });
            }
        }
        if let Some(e) = &app.assignment_error {
            ui.colored_label(
                egui::Color32::from_rgb(200, 120, 0),
                format!("Belge okunamadı: {e}"),
            );
        }
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
                } else {
                    // Gerçek yazma koruması durumunu göster (donanımsal kilit
                    // veya salt okunur bağlama). İddia değil, ölçümdür.
                    let wp = delil_evidence::attempt_software_write_protection(
                        std::path::Path::new(&d.root),
                    );
                    match wp {
                        delil_core::WriteProtection::SoftwareReadOnly => {
                            ui.colored_label(
                                egui::Color32::from_rgb(0, 140, 0),
                                "🔒 yazmaya kapalı (salt okunur)",
                            );
                        }
                        delil_core::WriteProtection::HardwareWriteBlockerReported => {
                            ui.colored_label(
                                egui::Color32::from_rgb(0, 140, 0),
                                "🔒 donanımsal engelleyici bildirildi",
                            );
                        }
                        delil_core::WriteProtection::NotApplied => {
                            ui.colored_label(
                                egui::Color32::from_rgb(200, 120, 0),
                                "⚠ yazmaya açık (uygulama yine de yazmaz)",
                            );
                        }
                    }
                    if ui.button("Bu aygıtı kullan").clicked() {
                        app.material_path = Some(std::path::PathBuf::from(&d.root));
                    }
                    // Yazılımsal salt okunur korumayı uygula/kaldır (açık istek).
                    if wp == delil_core::WriteProtection::NotApplied {
                        if ui
                            .button("Salt okunur yap")
                            .on_hover_text(
                                "Bu aygıtı Windows düzeyinde salt okunur yapar (diskpart; \
                                 yönetici yetkisi gerekir). Donanımsal yazma engelleyici ile \
                                 eşdeğer değildir.",
                            )
                            .clicked()
                        {
                            app.pending_write_protect = Some(std::path::PathBuf::from(&d.root));
                        }
                    } else {
                        if ui.button("Korumayı kaldır").clicked() {
                            app.pending_write_clear = Some(std::path::PathBuf::from(&d.root));
                        }
                    }
                }
            });
        }
        ui.add_space(4.0);
        ui.label(
            "Not: DELİL kaynak materyale hiçbir koşulda yazmaz; yalnızca kopyalar. \
             Yukarıdaki gösterge, aygıtın kendi yazma koruması durumudur. \
             Yazılımsal koruma, donanımsal yazma engelleyici ile eşdeğer değildir.",
        );
    });

    // Birden fazla harici aygıt bağlıysa kaynak belirsizdir; kullanıcı seçer.
    {
        let external: Vec<&delil_evidence::DetectedDevice> =
            app.devices.iter().filter(|d| !d.is_system_disk).collect();
        if external.len() > 1 {
            ui.add_space(6.0);
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(50, 42, 20))
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 200, 120),
                        format!(
                            "⚠ {} harici aygıt bağlı. Kaynak belirsiz; lütfen delil \
                             materyalini içeren aygıtı yukarıdan 'Bu aygıtı kullan' ile seçin.",
                            external.len()
                        ),
                    );
                });
        }
    }

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
        if app.workflow.finished
            && ui
                .add_sized([200.0, 36.0], egui::Button::new("Kontrol ekranına geç"))
                .clicked()
        {
            app.screen = Screen::Review;
        }
        if app.workflow.failed.is_some() && ui.button("Geri dön").clicked() {
            app.screen = Screen::NewInvestigation;
        }
    });
}

/// Kontrol ekranı (şartname 14.4).
fn draw_review(app: &mut DelilApp, ui: &mut egui::Ui) {
    ui.heading("Kontrol");
    ui.add_space(4.0);
    ui.label("Belirsiz işaretlenmiş bölümleri dinleyin, transkripti düzeltin ve soru yanıtlarını tamamlayın.");
    ui.add_space(8.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        // Vaka verisini kopyala; arayüz `app` üzerinde değişiklik yaparken
        // ödünç alma çakışması olmasın.
        let Some(case) = app.case.clone() else {
            ui.label("Vaka verisi yüklenmedi.");
            return;
        };

        // Transkript seçimi.
        ui.group(|ui| {
            ui.heading("Ses ve transkript");
            if case.transcripts.is_empty() {
                ui.label("Transkript üretilmedi (model yüklenmemiş olabilir).");
                return;
            }
            // (ad, durum) listesini önceden hazırla; `app` üzerinde değişiklik
            // yaparken `case` ödünç alınmış kalmasın.
            let items: Vec<(usize, String)> = case
                .transcripts
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let name = case
                        .audio_assets
                        .iter()
                        .find(|a| a.id == t.audio_asset_id)
                        .map(|a| a.file_name.clone())
                        .unwrap_or_default();
                    (i, format!("{name} — {}", t.current().status.label_tr()))
                })
                .collect();
            let mut clicked: Option<usize> = None;
            for (i, label) in items {
                if ui
                    .selectable_label(app.selected_transcript == i, label)
                    .clicked()
                {
                    clicked = Some(i);
                }
            }
            if let Some(i) = clicked {
                app.load_transcript_for_edit(i);
            }
        });

        // Dalga biçimi ve oynatma.
        ui.add_space(6.0);
        ui.group(|ui| {
            ui.heading("Dalga biçimi ve oynatma");
            if let Some(wf) = &app.waveform {
                draw_waveform(ui, wf, app.playback.position_seconds);
            } else {
                ui.label("Dalga biçimi yüklenmedi.");
            }
            ui.horizontal(|ui| {
                if ui
                    .button(if app.playback.playing {
                        "⏸ Duraklat"
                    } else {
                        "▶ Oynat"
                    })
                    .clicked()
                {
                    app.playback.toggle();
                    if let Some(p) = &app.player {
                        if app.playback.playing {
                            p.play();
                        } else {
                            p.pause();
                        }
                    }
                }
                if ui.button("⏪ 5 sn geri").clicked() {
                    app.playback.rewind(5.0);
                    if let Some(p) = &app.player {
                        let _ = p.seek(app.playback.position_seconds);
                    }
                }
                if ui.button("⏩ 5 sn ileri").clicked() {
                    app.playback.forward(5.0);
                    if let Some(p) = &app.player {
                        let _ = p.seek(app.playback.position_seconds);
                    }
                }
                if ui
                    .button(format!("Hız: {:.2}x", app.playback.rate.0))
                    .clicked()
                {
                    app.playback.rate.next();
                    if let Some(p) = &app.player {
                        p.set_rate(&app.playback.rate);
                    }
                }
                if ui.button("İşaretle").clicked() {
                    app.playback.add_marker("işaret", None);
                }
                ui.label(format!(
                    "{:.1}s / {:.1}s",
                    app.playback.position_seconds, app.playback.duration_seconds
                ));
                if app.player.is_none() {
                    ui.colored_label(egui::Color32::from_rgb(200, 120, 0), "(ses çıkışı yok)");
                }
            });
            ui.label(format!("Kanal: {}", app.playback.channel.label_tr()));
            if !app.playback.markers.is_empty() {
                ui.label(format!("İşaret sayısı: {}", app.playback.markers.len()));
            }
        });

        // Transkript düzenleme.
        ui.add_space(6.0);
        ui.group(|ui| {
            ui.heading("Transkript düzenleme");
            if let Some(ed) = &app.editor {
                egui::Grid::new("segment_edit")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        for (i, s) in ed.segments.iter().enumerate() {
                            // Satıra tıklandığında sesi o noktaya götür.
                            if ui
                                .selectable_label(
                                    app.edit_index == i,
                                    format!("[{:.1}s-{:.1}s]", s.start_seconds, s.end_seconds),
                                )
                                .clicked()
                            {
                                app.edit_index = i;
                                app.edit_buffer = s.text.clone();
                                app.playback.seek_to_segment(s.start_seconds);
                            }
                            if s.quality != delil_core::QualityFlag::Normal {
                                ui.colored_label(
                                    egui::Color32::from_rgb(200, 120, 0),
                                    s.quality.label_tr(),
                                );
                            } else {
                                ui.label("");
                            }
                            ui.label(&s.text);
                            ui.end_row();
                        }
                    });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(format!("Segment {} metni:", app.edit_index));
                    ui.text_edit_singleline(&mut app.edit_buffer);
                });
                ui.horizontal(|ui| {
                    if ui.button("Düzeltmeyi uygula").clicked() {
                        app.apply_text_edit();
                    }
                    if ui.button("Geri al").clicked() {
                        if let Some(ed) = app.editor.as_mut() {
                            ed.undo();
                        }
                    }
                    if ui.button("Yinele").clicked() {
                        if let Some(ed) = app.editor.as_mut() {
                            ed.redo();
                        }
                    }
                    if ui.button("Sürüm olarak kaydet").clicked() {
                        app.commit_edits();
                    }
                    if ui.button("Onayla").clicked() {
                        app.approve_transcript();
                    }
                });
            } else {
                ui.label("Düzenlemek için yukarıdan bir transkript seçin.");
            }
        });

        // Sorular.
        ui.add_space(6.0);
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
            ui.add_space(6.0);
            ui.group(|ui| {
                ui.heading("Karar gereken konular");
                for e in &case.exceptions {
                    ui.label(format!("• {}", e.description));
                }
            });
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

/// Dalga biçimini çizer (zarf).
fn draw_waveform(ui: &mut egui::Ui, wf: &delil_audio::Waveform, position: f64) {
    let desired = egui::vec2(ui.available_width().min(900.0), 80.0);
    let (rect, _resp) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(24, 24, 28));
    let mid = rect.center().y;
    let half = rect.height() / 2.0 - 2.0;
    let n = wf.buckets.max(1);
    let w = rect.width() / n as f32;
    for i in 0..n {
        let x = rect.left() + i as f32 * w;
        let lo = wf.min.get(i).copied().unwrap_or(0.0).clamp(-1.0, 1.0);
        let hi = wf.max.get(i).copied().unwrap_or(0.0).clamp(-1.0, 1.0);
        let y1 = mid - hi * half;
        let y2 = mid - lo * half;
        painter.line_segment(
            [egui::pos2(x, y1), egui::pos2(x, y2.max(y1 + 0.5))],
            egui::Stroke::new(w.max(0.5_f32), egui::Color32::from_rgb(90, 160, 240)),
        );
    }
    // Geçerli konum imleci.
    if wf.duration_seconds > 0.0 {
        let frac = (position / wf.duration_seconds).clamp(0.0, 1.0) as f32;
        let x = rect.left() + frac * rect.width();
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(240, 80, 80)),
        );
    }
}

/// Sonuç ekranı (şartname 14.5).
fn draw_result(app: &mut DelilApp, ui: &mut egui::Ui) {
    ui.heading("Sonuç");

    // Transkripsiyon durumu: model yoksa veya iş başarısızsa AÇIK uyarı.
    if let Some(status) = &app.transcription_status {
        ui.add_space(6.0);
        if status.is_satisfactory() {
            ui.colored_label(egui::Color32::from_rgb(0, 140, 0), status.label_tr());
        } else {
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(60, 20, 20))
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 120, 120),
                        "⚠ DİKKAT: İnceleme tamamlanmadı",
                    );
                    ui.colored_label(egui::Color32::from_rgb(255, 180, 180), status.label_tr());
                    ui.label(
                        "Ses kayıtları metne dönüştürülemediği için rapor transkript içermiyor. \
                         Bu bir tamamlanmış bilirkişi incelemesi değildir. \
                         Model bileşenlerinin kurulu olduğundan emin olun.",
                    );
                });
        }
    }

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
        .add_filter(
            "Belgeler",
            &[
                "pdf", "udf", "docx", "doc", "txt", "png", "jpg", "jpeg", "tif", "tiff",
            ],
        )
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
        assert!(w
            .steps
            .iter()
            .all(|(_, s, _)| matches!(s, StepState::Pending)));
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
