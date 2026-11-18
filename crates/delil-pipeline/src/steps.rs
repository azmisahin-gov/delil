//! İş akışı adımlarının uygulanması.

use crate::{PipelineError, Result};
use delil_core::{
    Case, PendingException, PipelineStep, QualityFlag, StepState, Transcript, TranscriptSegment,
    TranscriptStatus,
};
use delil_transcription::engine::{ControlFlag, TranscriptionEngine, TranscriptionOptions};
use delil_transcription::queue::{Queue, QueueItem, QueueOutcome};
use std::path::{Path, PathBuf};

/// İş akışı yapılandırması.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Vaka kök dizini.
    pub case_root: PathBuf,
    /// Görevlendirme belgesi yolu (opsiyonel).
    pub assignment_path: Option<PathBuf>,
    /// Materyal kaynak yolu (aygıt/klasör).
    pub material_source: Option<PathBuf>,
    /// Materyal kaynağının tanımı.
    pub source: Option<delil_core::EvidenceSource>,
    /// Edinim seçenekleri.
    pub acquire: delil_evidence::AcquireOptions,
    /// Transkripsiyon seçenekleri.
    pub transcription: TranscriptionOptions,
    /// Bilirkişi bilgileri.
    pub expert: delil_core::ExpertInfo,
    /// Otomatik transkripsiyon yapılsın mı?
    pub transcribe: bool,
}

impl PipelineConfig {
    pub fn new(case_root: impl Into<PathBuf>) -> Self {
        Self {
            case_root: case_root.into(),
            assignment_path: None,
            material_source: None,
            source: None,
            acquire: delil_evidence::AcquireOptions::default(),
            transcription: TranscriptionOptions::default(),
            expert: delil_core::ExpertInfo::default(),
            transcribe: true,
        }
    }
}

/// İş akışı olayı: kullanıcıya bildirilen adım durumu.
#[derive(Debug, Clone)]
pub struct PipelineEvent {
    pub step: PipelineStep,
    pub state: StepState,
    pub message: String,
}

/// İş akışı sonucu.
#[derive(Debug, Clone)]
pub struct PipelineResult {
    pub case: Case,
    /// UDF raporunun yolu.
    pub udf_path: PathBuf,
    /// PDF kopyasının yolu.
    pub pdf_path: Option<PathBuf>,
    /// UDF doğrulama raporu.
    pub validation: delil_udf::ValidationReport,
    /// Kullanıcı kararı bekleyen istisnalar.
    pub exceptions: Vec<PendingException>,
    /// Transkripsiyon durumu. Model yoksa veya iş başarısızsa `Unavailable`/
    /// `Failed` olur ve rapor **tamamlanmış sayılmaz**.
    pub transcription: TranscriptionOutcome,
}

/// Transkripsiyonun bu çalıştırmadaki gerçek durumu.
///
/// Bu alan, "model yoktu ama rapor üretildi" durumunun sessizce başarı gibi
/// gösterilmesini engeller. Kullanıcı arayüzü ve rapor bu durumu açıkça
/// göstermek zorundadır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptionOutcome {
    /// Transkripsiyon istenmedi (kullanıcı kapatmış veya ses yok).
    NotRequested,
    /// Tüm sesler başarıyla metne dönüştürüldü.
    Completed { transcribed: usize, total: usize },
    /// Motor/model yok; hiçbir ses metne dönüştürülemedi.
    Unavailable { reason: String },
    /// Motor vardı ancak bazı/tüm işler başarısız oldu.
    Failed { failed: usize, total: usize },
}

impl TranscriptionOutcome {
    /// Rapor, transkripsiyon açısından tamamlanmış sayılabilir mi?
    ///
    /// Yalnızca istenmediyse veya tüm sesler başarıyla çözüldüyse `true`.
    pub fn is_satisfactory(&self) -> bool {
        matches!(
            self,
            TranscriptionOutcome::NotRequested | TranscriptionOutcome::Completed { .. }
        )
    }

    /// Kullanıcıya gösterilecek açıklama.
    pub fn label_tr(&self) -> String {
        match self {
            TranscriptionOutcome::NotRequested => "Transkripsiyon istenmedi.".into(),
            TranscriptionOutcome::Completed { transcribed, total } => {
                format!("{transcribed}/{total} ses başarıyla metne dönüştürüldü.")
            }
            TranscriptionOutcome::Unavailable { reason } => {
                format!("Transkripsiyon yapılamadı: {reason}")
            }
            TranscriptionOutcome::Failed { failed, total } => {
                format!("{total} sesin {failed} tanesi metne dönüştürülemedi.")
            }
        }
    }
}

/// Olay geri çağrısı.
pub type EventFn<'a> = &'a mut dyn FnMut(PipelineEvent);

/// İş akışı yürütücüsü.
pub struct PipelineRunner<'a> {
    pub config: PipelineConfig,
    pub control: ControlFlag,
    pub events: Option<EventFn<'a>>,
    /// Transkripsiyon motoru. Verilmezse model dizininden otomatik yüklenmeye
    /// çalışılır; model yoksa açık hata verilir (simülasyon yapılmaz).
    pub engine: Option<Box<dyn TranscriptionEngine>>,
    /// Model dizini (motor verilmediğinde kullanılır).
    pub model_dir: Option<PathBuf>,
}

impl<'a> PipelineRunner<'a> {
    pub fn new(config: PipelineConfig) -> Self {
        Self {
            config,
            control: ControlFlag::new(),
            events: None,
            engine: None,
            model_dir: std::env::var("DELIL_MODEL_DIR").ok().map(PathBuf::from),
        }
    }

    fn emit(&mut self, step: PipelineStep, state: StepState, message: impl Into<String>) {
        if let Some(cb) = self.events.as_deref_mut() {
            cb(PipelineEvent {
                step,
                state,
                message: message.into(),
            });
        }
    }

    fn check_cancel(&self) -> Result<()> {
        if self.control.is_cancelled() {
            return Err(PipelineError::Cancelled);
        }
        Ok(())
    }

    /// Tam iş akışını çalıştırır.
    pub fn run(&mut self) -> Result<PipelineResult> {
        let store = delil_case_store::CaseStore::create(&self.config.case_root)?;
        let mut case = Case::new(self.config.case_root.to_string_lossy().to_string());

        // 1-5. Görevlendirmeyi oku ve bilgileri çıkar.
        self.emit(
            PipelineStep::ReadingAssignment,
            StepState::Running { progress: 0.0 },
            "Görevlendirme okunuyor",
        );
        if let Some(path) = self.config.assignment_path.clone() {
            match delil_assignment::extract_assignment(&path) {
                Ok(a) => {
                    case.assignment = a;
                    // Görevlendirme belgesini vaka deposuna kopyala.
                    let dest = store
                        .root
                        .join("assignment")
                        .join(path.file_name().unwrap_or_default());
                    let _ = std::fs::copy(&path, &dest);
                    case.add_audit("sistem", "görevlendirme", "Görevlendirme belgesi okundu");
                    if case.assignment.case_number.is_none() {
                        case.exceptions.push(PendingException {
                            kind: delil_core::ExceptionKind::CaseNumberUnreadable,
                            description:
                                "Görevlendirme belgesindeki dosya numarası okunamadı. Lütfen kontrol edin."
                                    .into(),
                            related: Some(path.to_string_lossy().into_owned()),
                            decision: None,
                            resolved: false,
                        });
                    }
                }
                Err(e) => {
                    self.emit(
                        PipelineStep::ReadingAssignment,
                        StepState::NeedsDecision {
                            message: format!("Görevlendirme okunamadı: {e}"),
                        },
                        "Görevlendirme okunamadı",
                    );
                }
            }
        }
        self.emit(
            PipelineStep::ReadingAssignment,
            StepState::Done,
            "Görevlendirme okundu",
        );
        self.check_cancel()?;

        // 6-10. Materyali al ve doğrula.
        self.emit(
            PipelineStep::AcquiringMaterial,
            StepState::Running { progress: 0.0 },
            "Materyal alınıyor",
        );
        let materials_root = store.root.join("materials");
        std::fs::create_dir_all(&materials_root)?;
        if let (Some(src), Some(source)) = (
            self.config.material_source.clone(),
            self.config.source.clone(),
        ) {
            // Kaynağın gerçek yazma koruması durumunu tespit et ve kaydet
            // (donanımsal SD/USB kilidi veya salt okunur bağlama). Yazılımsal
            // koruma donanımsal engelleyici ile eşdeğer gösterilmez.
            let mut source = source;
            source.write_protection = delil_evidence::attempt_software_write_protection(&src);
            case.add_audit(
                "sistem",
                "yazma-koruması",
                source.write_protection.label_tr().to_string(),
            );
            if !case.sources.iter().any(|s| s.id == source.id) {
                case.sources.push(source.clone());
            }
            let mats = delil_evidence::acquire_files(
                &src,
                &source,
                &materials_root,
                &self.config.acquire,
                None,
            )?;
            let verified = mats.iter().filter(|m| m.verification.is_verified()).count();
            case.materials = mats;
            case.add_audit(
                "sistem",
                "edinim",
                format!(
                    "{} dosya alındı, {} doğrulandı",
                    case.materials.len(),
                    verified
                ),
            );
            self.emit(
                PipelineStep::AcquiringMaterial,
                StepState::Done,
                format!("{} dosya alındı", case.materials.len()),
            );
            self.emit(
                PipelineStep::VerifyingFiles,
                StepState::Running { progress: 1.0 },
                "Dosyalar doğrulanıyor",
            );
            for m in &case.materials {
                if m.verification == delil_core::VerificationStatus::Mismatch {
                    case.exceptions.push(PendingException {
                        kind: delil_core::ExceptionKind::CorruptAudio,
                        description: format!(
                            "“{}” dosyasının kaynak ve hedef hash değeri uyuşmadı; dosya doğrulanmadı.",
                            m.file_name
                        ),
                        related: Some(m.stored_relative_path.clone()),
                        decision: None,
                        resolved: false,
                    });
                }
            }
            self.emit(
                PipelineStep::VerifyingFiles,
                StepState::Done,
                "Dosyalar doğrulandı",
            );
        } else {
            self.emit(
                PipelineStep::AcquiringMaterial,
                StepState::Done,
                "Materyal kaynağı verilmedi",
            );
        }
        self.check_cancel()?;

        // 11-12. Ses dosyalarını bul, envanteri çıkar, kuyruğa al.
        self.emit(
            PipelineStep::SearchingAudio,
            StepState::Running { progress: 0.0 },
            "Sesler aranıyor",
        );
        case.audio_assets = delil_evidence::build_audio_inventory(&case.materials, &materials_root);
        for a in case.audio_assets.iter_mut() {
            a.queued = self.config.transcribe && a.unsupported_reason.is_none();
        }
        case.add_audit(
            "sistem",
            "ses-envanteri",
            format!("{} ses dosyası bulundu", case.audio_assets.len()),
        );
        self.emit(
            PipelineStep::SearchingAudio,
            StepState::Done,
            format!("{} ses dosyası bulundu", case.audio_assets.len()),
        );
        self.check_cancel()?;

        // 13-15. Transkripsiyon ve kalite kontrolü.
        let transcription_outcome: TranscriptionOutcome =
            if self.config.transcribe && !case.audio_assets.is_empty() {
                self.emit(
                    PipelineStep::Transcribing,
                    StepState::Running { progress: 0.0 },
                    "Transkripsiyon yapılıyor",
                );
                let outcome = self.run_transcription(&mut case, &store, &materials_root)?;
                match &outcome {
                    TranscriptionOutcome::Completed { transcribed, total } => {
                        self.emit(
                            PipelineStep::Transcribing,
                            StepState::Done,
                            format!("{transcribed}/{total} ses metne dönüştürüldü"),
                        );
                    }
                    TranscriptionOutcome::Unavailable { reason } => {
                        // Model yok: bu bir başarı değildir. Kullanıcıya açıkça
                        // bildirilir ve istisna kaydı oluşturulur.
                        self.emit(
                            PipelineStep::Transcribing,
                            StepState::Failed {
                                message: reason.clone(),
                            },
                            format!("Transkripsiyon yapılamadı: {reason}"),
                        );
                        case.exceptions.push(PendingException {
                            kind: delil_core::ExceptionKind::SuspiciousModelOutput,
                            description: format!(
                            "Ses kayıtları metne dönüştürülemedi: {reason}. \
                             Rapor transkript içermemektedir; bu bir tamamlanmış inceleme değildir."
                        ),
                            related: None,
                            decision: None,
                            resolved: false,
                        });
                        case.add_audit(
                            "sistem",
                            "transkripsiyon-başarısız",
                            format!("Transkripsiyon yapılamadı: {reason}"),
                        );
                    }
                    TranscriptionOutcome::Failed { failed, total } => {
                        self.emit(
                            PipelineStep::Transcribing,
                            StepState::Failed {
                                message: format!("{failed}/{total} ses işlenemedi"),
                            },
                            format!("{total} sesin {failed} tanesi metne dönüştürülemedi"),
                        );
                    }
                    TranscriptionOutcome::NotRequested => {}
                }
                outcome
            } else {
                self.emit(
                    PipelineStep::Transcribing,
                    StepState::Done,
                    "Transkripsiyon istenmedi",
                );
                TranscriptionOutcome::NotRequested
            };
        self.check_cancel()?;

        // 16-17. Kalite kontrolü.
        self.emit(
            PipelineStep::QualityControl,
            StepState::Running { progress: 0.0 },
            "Kalite kontrolü yapılıyor",
        );
        self.run_quality_control(&mut case, &materials_root);
        self.emit(
            PipelineStep::QualityControl,
            StepState::Done,
            "Kalite kontrolü tamamlandı",
        );
        self.check_cancel()?;

        // 18. Rapor taslağı.
        self.emit(
            PipelineStep::PreparingReport,
            StepState::Running { progress: 0.0 },
            "Rapor hazırlanıyor",
        );
        let opts = delil_report::ReportOptions {
            expert: self.config.expert.clone(),
            ..Default::default()
        };
        let report = delil_report::generate_report(&case, &opts)?;
        // Eksik zorunlu alanlar için istisna kaydı.
        for m in &report.missing_fields {
            case.exceptions.push(PendingException {
                kind: if m.contains("yanıtı") || m.contains("dayanağı") {
                    delil_core::ExceptionKind::ExpertOpinionMissing
                } else {
                    delil_core::ExceptionKind::CaseNumberUnreadable
                },
                description: format!("Rapor için eksik bilgi: {m}"),
                related: None,
                decision: None,
                resolved: false,
            });
        }
        case.report = Some(report.clone());
        case.add_audit("sistem", "rapor", "Rapor taslağı oluşturuldu");
        self.emit(
            PipelineStep::PreparingReport,
            StepState::Done,
            "Rapor taslağı oluşturuldu",
        );
        self.check_cancel()?;

        // 19-20. UDF üret ve doğrula.
        self.emit(
            PipelineStep::ValidatingUdf,
            StepState::Running { progress: 0.0 },
            "UDF oluşturuluyor",
        );
        let udf_path = store.root.join("report").join("BILIRKISI_RAPORU.udf");
        let udf_doc = delil_report_to_udf::report_to_udf_document(&report);
        delil_udf::write_udf(&udf_doc, &udf_path)?;
        case.add_audit("sistem", "udf", "UDF raporu oluşturuldu");

        let validation = delil_udf::validate_udf(&udf_path)?;
        if !validation.passed() {
            case.exceptions.push(PendingException {
                kind: delil_core::ExceptionKind::SuspiciousModelOutput,
                description: format!(
                    "UDF yapısal doğrulaması başarısız: {}",
                    validation.errors.join("; ")
                ),
                related: Some(udf_path.to_string_lossy().into_owned()),
                decision: None,
                resolved: false,
            });
        }
        self.emit(
            PipelineStep::ValidatingUdf,
            StepState::Done,
            "UDF doğrulandı",
        );

        // Yardımcı çıktılar (PDF, manifest, TXT transkriptler).
        let out_dir = store.root.join("report");
        let artifacts = delil_export::export_all(&case, &report, &out_dir)?;

        // Vakayı kaydet.
        let key = store.load_or_create_master_key()?;
        store.save_case(&case, &key)?;

        Ok(PipelineResult {
            case,
            udf_path,
            pdf_path: Some(artifacts.pdf_path),
            validation,
            exceptions: Vec::new(),
            transcription: transcription_outcome,
        })
    }

    /// Transkripsiyon kuyruğunu çalıştırır ve gerçek durumu döndürür.
    fn run_transcription(
        &mut self,
        case: &mut Case,
        store: &delil_case_store::CaseStore,
        materials_root: &Path,
    ) -> Result<TranscriptionOutcome> {
        // Kuyruk kalıcılığı: kesintiden devam için queue.json.
        let queue_path = store.root.join("logs").join("queue.json");
        let mut queue = Queue::with_persistence(&queue_path)?;
        queue.resume_interrupted();

        for a in &case.audio_assets {
            if !a.queued {
                continue;
            }
            let material = case.materials.iter().find(|m| m.id == a.material_id);
            let rel = material
                .map(|m| m.stored_relative_path.clone())
                .unwrap_or_else(|| a.file_name.clone());
            let input = materials_root.join(rel);
            let item = QueueItem::new(
                a.id.to_string(),
                input,
                a.sha256.clone(),
                &self.config.transcription,
            );
            queue.enqueue(item);
        }

        // Motor: çağıran tarafından verildiyse onu kullan; yoksa model
        // dizininden gerçek Candle Whisper motorunu yüklemeyi dene. Model
        // yoksa **transkripsiyon yapılamaz**; bu durum açıkça bildirilir ve
        // rapor tamamlanmış sayılmaz. Simülasyon yapılmaz.
        let decode = |p: &Path| -> delil_transcription::Result<Vec<f32>> {
            delil_transcription::decode_audio_file(p)
        };

        let mut owned_engine: Option<Box<dyn TranscriptionEngine>> = self.engine.take();
        let mut load_error: Option<String> = None;
        if owned_engine.is_none() {
            match self.model_dir.clone() {
                Some(dir) => match load_candle_engine(&dir) {
                    Ok(e) => owned_engine = Some(e),
                    Err(e) => load_error = Some(e.to_string()),
                },
                None => {
                    load_error = Some(
                        "Whisper model dizini yapılandırılmamış (DELIL_MODEL_DIR veya kurulum models/ dizini)."
                            .into(),
                    );
                }
            }
        }

        let queued_total = queue.items().len();
        let mut engine: Box<dyn TranscriptionEngine> = match owned_engine {
            Some(e) => e,
            None => {
                // Motor yok: hiçbir iş çalıştırılmaz, kuyruk işaretlenmez.
                let reason =
                    load_error.unwrap_or_else(|| "Yerel Whisper modeli yüklenemedi.".to_string());
                return Ok(TranscriptionOutcome::Unavailable { reason });
            }
        };

        let outcomes = queue.run_all(
            &mut *engine,
            &self.config.transcription,
            &self.control,
            &decode,
        );

        let mut transcribed = 0usize;
        let mut failed = 0usize;
        for (item, outcome) in queue.items().iter().zip(outcomes.iter()) {
            match outcome {
                QueueOutcome::Done(r) => {
                    let segments: Vec<TranscriptSegment> = r
                        .segments
                        .iter()
                        .enumerate()
                        .map(|(i, s)| TranscriptSegment {
                            index: i,
                            start_seconds: s.start,
                            end_seconds: s.end,
                            text: s.text.clone(),
                            machine_text: s.text.clone(),
                            quality: QualityFlag::Normal,
                            issues: vec![],
                            marked_unintelligible: false,
                            speaker: None,
                            note: None,
                            word_level_timestamps: false,
                        })
                        .collect();
                    let mut t = Transcript::new_machine(
                        delil_core::AudioAssetId::from_raw(item.audio_asset_id.clone()),
                        engine.version(),
                        segments,
                        r.detected_language.clone(),
                    );
                    t.id = delil_core::TranscriptId::new();
                    case.transcripts.push(t);
                    transcribed += 1;
                }
                QueueOutcome::Failed(msg) => {
                    failed += 1;
                    case.exceptions.push(PendingException {
                        kind: delil_core::ExceptionKind::CorruptAudio,
                        description: format!("Bir ses dosyası işlenemedi: {msg}"),
                        related: Some(item.input_path.to_string_lossy().into_owned()),
                        decision: None,
                        resolved: false,
                    });
                }
                _ => {}
            }
        }
        case.add_audit(
            "sistem",
            "transkripsiyon",
            format!("{transcribed}/{queued_total} ses metne dönüştürüldü, {failed} başarısız"),
        );

        if failed > 0 {
            Ok(TranscriptionOutcome::Failed {
                failed,
                total: queued_total,
            })
        } else {
            Ok(TranscriptionOutcome::Completed {
                transcribed,
                total: queued_total,
            })
        }
    }

    /// Kalite kontrolünü çalıştırır ve bayrakları işler.
    fn run_quality_control(&mut self, case: &mut Case, materials_root: &Path) {
        for t in case.transcripts.iter_mut() {
            let asset = case.audio_assets.iter().find(|a| a.id == t.audio_asset_id);
            let material =
                asset.and_then(|a| case.materials.iter().find(|m| m.id == a.material_id));
            let samples = material
                .map(|m| materials_root.join(&m.stored_relative_path))
                .and_then(|p| delil_transcription::decode_audio_file(&p).ok())
                .unwrap_or_default();
            let stats = delil_transcription::engine::analyze_signal(&samples);
            let silences = delil_transcription::engine::detect_silences(&samples, 0.5, 0.01);
            let qstats = delil_quality::AudioSignalStats {
                duration_seconds: stats.duration_seconds,
                silences,
                rms: stats.rms,
                peak: stats.peak,
                overlap_regions: vec![],
                corrupt: false,
            };
            let report = delil_quality::check_transcript(t, &qstats, Default::default());
            // Bayrakları segmentlere yaz.
            let mut segs = t.current().segments.clone();
            delil_quality::apply_flags(&mut segs, &report);
            let status = if report.is_clean() {
                TranscriptStatus::QualityChecked
            } else {
                TranscriptStatus::AwaitingReview
            };
            t.add_version(segs, "sistem", status, Some(report.summary_tr.clone()));
        }
        case.add_audit("sistem", "kalite-kontrolü", "Kalite kontrolü tamamlandı");
    }
}

/// Rapor modelini UDF belge modeline çevirir (şartname 11.2).
pub mod delil_report_to_udf {
    use delil_core::Report;
    use delil_udf::model::*;

    /// Raporu 24 bölümlü UDF belgesine çevirir.
    pub fn report_to_udf_document(report: &Report) -> UdfDocument {
        let mut doc = UdfDocument::default();
        let bold12 = |s: &str| Paragraph::with_run(TextRun::new(s).bold().size(12.0));
        let para = |s: &str| Paragraph::text(s);

        // 1. Kapak
        doc.push_paragraph(
            Paragraph::with_run(TextRun::new(&report.cover_title).bold().size(16.0)).centered(),
        );
        if let Some(sub) = &report.cover_subtitle {
            doc.push_paragraph(para(sub).centered());
        }
        doc.push_paragraph(Paragraph::empty());
        if let Some(cn) = &report.case_number {
            doc.push_paragraph(para(&format!("Dosya No: {cn}")).centered());
        }

        // 2-3. Merci ve dosya numarası
        doc.push_page_break();
        doc.push_paragraph(bold12("GÖREVLENDİREN MERCİ VE DOSYA BİLGİLERİ"));
        doc.push_paragraph(para(&format!(
            "Görevlendiren merci: {}",
            report.authority.as_deref().unwrap_or("(belirtilmemiş)")
        )));
        doc.push_paragraph(para(&format!(
            "Dosya numarası: {}",
            report.case_number.as_deref().unwrap_or("(belirtilmemiş)")
        )));

        // 4. Taraflar
        doc.push_paragraph(bold12("TARAFLAR"));
        for p in &report.parties {
            doc.push_paragraph(para(&format!("{}: {}", p.role, p.name)));
        }

        // 5. Bilirkişi bilgileri
        doc.push_paragraph(bold12("BİLİRKİŞİ BİLGİLERİ"));
        doc.push_paragraph(para(&format!(
            "Adı Soyadı: {}",
            report
                .expert
                .full_name
                .as_deref()
                .unwrap_or("(belirtilmemiş)")
        )));
        doc.push_paragraph(para(&format!(
            "Unvanı: {}",
            report.expert.title.as_deref().unwrap_or("(belirtilmemiş)")
        )));
        doc.push_paragraph(para(&format!(
            "Sicil No: {}",
            report
                .expert
                .registry_number
                .as_deref()
                .unwrap_or("(belirtilmemiş)")
        )));

        // 6. Görevlendirme tarihi ve süresi
        doc.push_paragraph(bold12("GÖREVLENDİRME TARİHİ VE SÜRESİ"));
        doc.push_paragraph(para(&format!(
            "Görevlendirme tarihi: {}",
            report.assignment_date_text.as_deref().unwrap_or("-")
        )));
        doc.push_paragraph(para(&format!(
            "Rapor teslim süresi: {}",
            report.due_date_text.as_deref().unwrap_or("-")
        )));

        // 7. İnceleme konusu
        doc.push_paragraph(bold12("İNCELEME KONUSU"));
        doc.push_paragraph(para(report.subject.as_deref().unwrap_or("(belirtilmemiş)")));

        // 8. Sorular
        doc.push_paragraph(bold12("BİLİRKİŞİYE YÖNELTİLEN SORULAR"));
        for q in &report.questions {
            doc.push_paragraph(Paragraph::with_run(
                TextRun::new(format!("{}. {}", q.ordinal, q.original_text)).bold(),
            ));
        }

        // 9. İncelenen materyaller (tablo)
        doc.push_paragraph(bold12("İNCELENEN MATERYALLER"));
        if !report.materials.is_empty() {
            let mut rows = vec![TableRow::new(vec![
                TableCell::text("Dosya"),
                TableCell::text("Boyut (bayt)"),
                TableCell::text("Durum"),
                TableCell::text("SHA-256"),
            ])];
            for m in &report.materials {
                rows.push(TableRow::new(vec![
                    TableCell::text(&m.file_name),
                    TableCell::text(m.size_bytes.to_string()),
                    TableCell::text(m.verification.label_tr()),
                    TableCell::text(m.target_sha256.as_deref().unwrap_or("-")),
                ]));
            }
            doc.push_table(Table::new(vec![120.0, 60.0, 60.0, 200.0], rows));
        }

        // 10. Edinim açıklaması
        doc.push_paragraph(bold12("MATERYAL TESLİM/EDİNİM AÇIKLAMASI"));
        doc.push_paragraph(para(&report.acquisition_note));

        // 11. İnceleme yöntemi
        doc.push_paragraph(bold12("İNCELEME YÖNTEMİ"));
        doc.push_paragraph(para(&report.method));

        // 12. Yazılım/model bilgisi
        doc.push_paragraph(bold12("KULLANILAN YAZILIM VE MODEL BİLGİSİ"));
        doc.push_paragraph(para(&format!(
            "{} sürüm {}. Model: {} ({}). İşletim sistemi: {}.",
            report.software.app_name,
            report.software.app_version,
            report.software.model_name,
            report.software.model_version,
            report.software.os
        )));

        // 13. Transkripsiyon yöntemi
        doc.push_paragraph(bold12("TRANSKRİPSİYON YÖNTEMİ"));
        doc.push_paragraph(para(&report.transcription_method));

        // 14. Teknik ses envanteri (tablo)
        doc.push_paragraph(bold12("TEKNİK SES ENVANTERİ"));
        if !report.audio_inventory.is_empty() {
            let mut rows = vec![TableRow::new(vec![
                TableCell::text("Dosya"),
                TableCell::text("Kapsayıcı"),
                TableCell::text("Codec"),
                TableCell::text("Süre (sn)"),
                TableCell::text("Kanal"),
            ])];
            for a in &report.audio_inventory {
                rows.push(TableRow::new(vec![
                    TableCell::text(&a.file_name),
                    TableCell::text(&a.technical.container),
                    TableCell::text(&a.technical.codec),
                    TableCell::text(format!("{:.1}", a.technical.duration_seconds)),
                    TableCell::text(
                        a.technical
                            .channels
                            .map(|c| c.to_string())
                            .unwrap_or_default(),
                    ),
                ]));
            }
            doc.push_table(Table::new(vec![120.0, 70.0, 90.0, 60.0, 40.0], rows));
        }

        // 15-16. Transkript içeriği ve zaman kodları
        doc.push_paragraph(bold12(
            "SES KAYITLARININ METİN HÂLİNE DÖNÜŞTÜRÜLMÜŞ İÇERİĞİ",
        ));
        for t in &report.transcripts {
            let name = report
                .audio_inventory
                .iter()
                .find(|a| a.id == t.audio_asset_id)
                .map(|a| a.file_name.clone())
                .unwrap_or_else(|| t.audio_asset_id.to_string());
            doc.push_paragraph(Paragraph::with_run(TextRun::new(&name).italic()));
            for s in &t.current().segments {
                let line = if report.include_timestamps {
                    format!(
                        "[{:.1}s – {:.1}s] {}",
                        s.start_seconds, s.end_seconds, s.text
                    )
                } else {
                    s.text.clone()
                };
                doc.push_paragraph(para(&line));
            }
        }

        // 17. Belirsiz bölümler
        doc.push_paragraph(bold12("BELİRSİZ/ANLAŞILAMAYAN BÖLÜMLER"));
        if report.unintelligible_summary.is_empty() {
            doc.push_paragraph(para("Belirsiz veya anlaşılamayan bölüm işaretlenmemiştir."));
        } else {
            for u in &report.unintelligible_summary {
                doc.push_paragraph(para(u));
            }
        }

        // 18-19. Soru yanıtları ve dayanaklar
        doc.push_page_break();
        doc.push_paragraph(bold12("BİLİRKİŞİ SORULARININ YANITLARI VE DAYANAKLARI"));
        for q in &report.questions {
            doc.push_paragraph(Paragraph::with_run(
                TextRun::new(format!("Soru {}. {}", q.ordinal, q.original_text)).bold(),
            ));
            doc.push_paragraph(para(&format!(
                "Yanıt: {}",
                q.expert_answer.as_deref().unwrap_or("(yanıtlanmadı)")
            )));
            for ev in &q.evidence_references {
                doc.push_paragraph(para(&format!(
                    "Dayanak: “{}” (zaman: {:.1}s – {:.1}s)",
                    ev.quote, ev.start_seconds, ev.end_seconds
                )));
            }
        }

        // 20. Yöntem sınırlılıkları
        doc.push_paragraph(bold12("YÖNTEM SINIRLILIKLARI"));
        for l in &report.limitations {
            doc.push_paragraph(para(l));
        }

        // 21. Gerekçeli sonuç
        doc.push_paragraph(bold12("GEREKÇELİ SONUÇ"));
        doc.push_paragraph(para(
            report
                .reasoned_conclusion
                .as_deref()
                .unwrap_or("(bilirkişi tarafından doldurulacaktır)"),
        ));

        // 22-23. Rapor tarihi ve imza
        doc.push_paragraph(bold12("RAPOR TARİHİ VE İMZA"));
        doc.push_paragraph(para(&format!(
            "Rapor tarihi: {}",
            report.report_date.format("%d.%m.%Y")
        )));
        doc.push_paragraph(Paragraph::empty());
        for line in report.signature_block.expert_line.lines() {
            doc.push_paragraph(para(line).centered());
        }
        doc.push_paragraph(para(&report.signature_block.note));

        // 24. Ekler
        doc.push_page_break();
        doc.push_paragraph(bold12("EKLER"));
        for att in &report.attachments {
            doc.push_paragraph(para(&format!(
                "EK-{}: {} ({})",
                att.number,
                att.title,
                att.kind.label_tr()
            )));
            if let Some(fname) = &att.audio_file_name {
                doc.push_paragraph(para(&format!("Ses dosyası: {fname}")));
            }
            if let Some(sha) = &att.sha256 {
                doc.push_paragraph(para(&format!("SHA-256: {sha}")));
            }
            if let Some(d) = att.duration_seconds {
                doc.push_paragraph(para(&format!("Süre: {d:.1} saniye")));
            }
            doc.push_paragraph(para(&format!(
                "Bilirkişi inceleme durumu: {}",
                att.review_status.label_tr()
            )));
        }

        doc
    }
}

/// Gerçek Candle Whisper motorunu yükler (motor `candle` özelliğiyle derlenmişse).
///
/// Özellik kapalıysa veya model dosyaları eksikse hata döner; çağıran taraf
/// yer tutucu motora düşer ve bu durum kullanıcıya açıkça bildirilir.
#[cfg(feature = "candle")]
fn load_candle_engine(dir: &Path) -> Result<Box<dyn TranscriptionEngine>> {
    use delil_transcription::candle_whisper::{CandleWhisper, WhisperEngineConfig};
    let cfg = WhisperEngineConfig::new(dir);
    let engine = CandleWhisper::load(&cfg).map_err(|e| PipelineError::Transcription(e))?;
    Ok(Box::new(engine))
}

#[cfg(not(feature = "candle"))]
fn load_candle_engine(_dir: &Path) -> Result<Box<dyn TranscriptionEngine>> {
    Err(PipelineError::Transcription(
        delil_transcription::TranscriptionError::ModelMissing(
            "Whisper motoru bu derlemede etkin değil (candle özelliği kapalı).".into(),
        ),
    ))
}

/// Tam iş akışını çalıştırır (kısayol).
pub fn run_full_pipeline(mut config: PipelineConfig) -> Result<PipelineResult> {
    let mut runner = PipelineRunner::new(std::mem::replace(&mut config, PipelineConfig::new(".")));
    runner.run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use delil_core::{ExpertInfo, Party, QuestionId};
    use std::io::Write;

    fn write_wav(path: &Path, seconds: f64) {
        let sr = 16000u32;
        let samples = (sr as f64 * seconds) as u32;
        let data_len = samples * 2;
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data_len).to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&sr.to_le_bytes());
        v.extend_from_slice(&(sr * 2).to_le_bytes());
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_len.to_le_bytes());
        v.extend(std::iter::repeat_n(0u8, data_len as usize));
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(&v).unwrap();
    }

    const ASSIGNMENT: &str = "Dosya No: 2024/55 Esas\nMahkeme: Test Mahkemesi\nKonu: Ses incelemesi\nBilirkişiye Sorulan Sorular:\n1. Kayıtta ne konuşuluyor?\n";

    #[test]
    fn report_to_udf_contains_all_sections() {
        let case = Case::new("/tmp/c");
        let opts = delil_report::ReportOptions {
            expert: ExpertInfo {
                full_name: Some("Dr. X".into()),
                title: Some("Bilirkişi".into()),
                registry_number: Some("99".into()),
            },
            ..Default::default()
        };
        let mut c = case;
        c.assignment.case_number = Some("2024/1".into());
        c.assignment.parties = vec![Party {
            role: "Şüpheli".into(),
            name: "A".into(),
            raw_text: None,
        }];
        c.assignment.questions = vec![delil_core::Question {
            id: QuestionId::new(),
            ordinal: 1,
            original_text: "Ne konuşuluyor?".into(),
            material_ids: vec![],
            audio_asset_ids: vec![],
            evidence_references: vec![],
            expert_answer: Some("Görüşme".into()),
            answer_approved: true,
        }];
        let report = delil_report::generate_report(&c, &opts).unwrap();
        let doc = delil_report_to_udf::report_to_udf_document(&report);
        let text = doc.plain_text();
        for section in [
            "GÖREVLENDİREN MERCİ",
            "TARAFLAR",
            "BİLİRKİŞİ BİLGİLERİ",
            "İNCELEME KONUSU",
            "BİLİRKİŞİYE YÖNTELTİLEN SORULAR"
                .replace("YÖNTELTİLEN", "YÖNELTİLEN")
                .as_str(),
            "İNCELENEN MATERYALLER",
            "İNCELEME YÖNTEMİ",
            "TEKNİK SES ENVANTERİ",
            "YÖNTEM SINIRLILIKLARI",
            "GEREKÇELİ SONUÇ",
            "EKLER",
        ] {
            assert!(text.contains(section), "eksik bölüm: {section}");
        }
    }

    #[test]
    fn full_pipeline_produces_valid_udf() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("usb");
        std::fs::create_dir_all(&src).unwrap();
        write_wav(&src.join("kayit1.wav"), 1.0);
        write_wav(&src.join("kayit2.wav"), 0.5);
        let asg = dir.path().join("gorev.txt");
        std::fs::write(&asg, ASSIGNMENT).unwrap();

        let mut cfg = PipelineConfig::new(dir.path().join("case"));
        cfg.assignment_path = Some(asg);
        cfg.material_source = Some(src.clone());
        cfg.source = Some(delil_evidence::local_folder_source(&src));
        cfg.transcribe = false; // model yok; transkripsiyon atlanır
        cfg.expert = ExpertInfo {
            full_name: Some("Dr. Test".into()),
            title: Some("Bilirkişi".into()),
            registry_number: Some("123".into()),
        };

        let res = run_full_pipeline(cfg).unwrap();
        // UDF oluştu ve doğrulandı.
        assert!(res.udf_path.exists());
        assert!(res.validation.passed(), "{:?}", res.validation.errors);
        // Sesler envantere alındı.
        assert_eq!(res.case.audio_assets.len(), 2);
        assert_eq!(res.case.verified_material_count(), 2);
        // UDF gerçekten okunabilir.
        let parsed = delil_udf::read_udf(&res.udf_path).unwrap();
        assert!(parsed.plain_text().contains("GEREKÇELİ SONUÇ"));
        assert!(parsed.plain_text().contains("2024/55"));
        // PDF üretildi.
        assert!(res.pdf_path.unwrap().exists());
    }

    #[test]
    fn pipeline_emits_all_steps() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        write_wav(&src.join("a.wav"), 0.5);

        let mut cfg = PipelineConfig::new(dir.path().join("case"));
        cfg.material_source = Some(src.clone());
        cfg.source = Some(delil_evidence::local_folder_source(&src));
        cfg.transcribe = false;

        let mut seen: Vec<PipelineStep> = Vec::new();
        {
            let mut runner = PipelineRunner::new(cfg);
            let mut cb = |e: PipelineEvent| {
                if e.step != PipelineStep::VerifyingFiles || !seen.contains(&e.step) {
                    seen.push(e.step);
                }
            };
            runner.events = Some(&mut cb);
            runner.run().unwrap();
        }
        // Sekiz adımın tamamı görülmüş olmalı.
        for step in PipelineStep::ALL {
            assert!(seen.contains(&step), "adım görülmedi: {step:?}");
        }
    }

    #[test]
    fn missing_model_reports_unavailable_and_not_complete() {
        // Model yokken transkripsiyon istenirse: sonuç "Unavailable" olmalı,
        // hiç transkript üretilmemeli ve sonuç tatmin edici SAYILMAMALI.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        write_wav(&src.join("a.wav"), 0.5);

        let mut cfg = PipelineConfig::new(dir.path().join("case"));
        cfg.material_source = Some(src.clone());
        cfg.source = Some(delil_evidence::local_folder_source(&src));
        cfg.transcribe = true; // transkripsiyon istendi
                               // Model dizini yok.

        let mut runner = PipelineRunner::new(cfg);
        let res = runner.run().unwrap();
        assert!(
            matches!(res.transcription, TranscriptionOutcome::Unavailable { .. }),
            "model yokken Unavailable beklenir, bulunan: {:?}",
            res.transcription
        );
        assert!(!res.transcription.is_satisfactory());
        assert_eq!(res.case.transcripts.len(), 0, "hiç transkript üretilmemeli");
        // İstisna kaydı oluşmalı ve raporda eksik olarak görünmeli.
        assert!(res
            .case
            .exceptions
            .iter()
            .any(|e| e.description.contains("metne dönüştürülemedi")));
    }

    #[test]
    fn transcription_not_requested_is_satisfactory() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = PipelineConfig::new(dir.path().join("case"));
        cfg.transcribe = false;
        let res = run_full_pipeline(cfg).unwrap();
        assert_eq!(res.transcription, TranscriptionOutcome::NotRequested);
        assert!(res.transcription.is_satisfactory());
    }

    #[test]
    fn pipeline_handles_missing_assignment() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = PipelineConfig::new(dir.path().join("case"));
        cfg.transcribe = false;
        let res = run_full_pipeline(cfg).unwrap();
        assert!(res.udf_path.exists());
        // Eksik alanlar istisna olarak bildirilir.
        assert!(!res.case.report.as_ref().unwrap().missing_fields.is_empty());
    }
}
