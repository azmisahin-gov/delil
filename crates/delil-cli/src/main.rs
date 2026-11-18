//! DELİL komut satırı arayüzü.
//!
//! Bu arayüz, masaüstü uygulamasının kullandığı aynı uçtan uca iş akışını
//! çalıştırır. İnceleme başlatma, doğrulama, UDF üretme ve vaka deposu
//! işlemleri komut satırından yürütülebilir; böylece işlevler gerçek veriyle
//! test edilebilir.

use clap::{Parser, Subcommand};
use delil_core::{Case, PipelineStep, StepState};
use delil_pipeline::{PipelineConfig, PipelineEvent, PipelineRunner};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "delil",
    version,
    about = "DELİL — 56.12 bilirkişileri için otomatik ses çözümleme ve UDF raporlama"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Yeni bilirkişi incelemesi başlatır ve UDF raporu üretir.
    Inceleme {
        /// Vaka kök dizini.
        #[arg(long)]
        vaka: PathBuf,
        /// Görevlendirme belgesi (PDF/UDF/DOCX/TXT/görsel).
        #[arg(long)]
        gorevlendirme: Option<PathBuf>,
        /// Materyal kaynağı (klasör, USB/SD bağlama noktası).
        #[arg(long)]
        materyal: Option<PathBuf>,
        /// Bilirkişi adı soyadı.
        #[arg(long)]
        bilirkisi: Option<String>,
        /// Bilirkişi unvanı.
        #[arg(long)]
        unvan: Option<String>,
        /// Bilirkişi sicil numarası.
        #[arg(long)]
        sicil: Option<String>,
        /// Transkripsiyon yapılmasın (yalnızca edinim ve rapor).
        #[arg(long, default_value_t = false)]
        transkripsiyon_yok: bool,
        /// Dil (ISO 639-1); varsayılan tr.
        #[arg(long, default_value = "tr")]
        dil: String,
    },
    /// Bağlı aygıtları listeler.
    Aygitlar,
    /// Bir ses dosyasının teknik envanterini gösterir.
    Envanter {
        /// Ses dosyası yolu.
        dosya: PathBuf,
    },
    /// Bir UDF dosyasını okur ve metnini gösterir.
    UdfOku {
        /// UDF dosyası yolu.
        dosya: PathBuf,
    },
    /// Bir UDF dosyasını doğrular.
    UdfDogrula {
        /// UDF dosyası yolu.
        dosya: PathBuf,
    },
    /// Bir UDF dosyasında metin arar.
    UdfAra {
        /// UDF dosyası yolu.
        dosya: PathBuf,
        /// Aranacak metin.
        metin: String,
    },
    /// Vaka deposunu listeler (özet).
    Vakalar {
        /// Kök dizin.
        #[arg(long, default_value = ".")]
        kok: PathBuf,
    },
    /// Vaka deposunu parola korumalı yedeğe dönüştürür.
    Yedekle {
        /// Vaka kök dizini.
        vaka: PathBuf,
        /// Hedef yedek dosyası.
        hedef: PathBuf,
    },
    /// Parola korumalı yedeği geri yükler.
    GeriYukle {
        /// Yedek dosyası.
        yedek: PathBuf,
        /// Hedef dizin.
        hedef: PathBuf,
    },
    /// Sürüm ve sistem bilgisini gösterir.
    Surum,
}

fn main() {
    let cli = Cli::parse();
    let code = match run(cli) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Hata: {e}");
            1
        }
    };
    std::process::exit(code);
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Inceleme {
            vaka,
            gorevlendirme,
            materyal,
            bilirkisi,
            unvan,
            sicil,
            transkripsiyon_yok,
            dil,
        } => {
            let mut cfg = PipelineConfig::new(&vaka);
            cfg.assignment_path = gorevlendirme;
            if let Some(m) = materyal {
                cfg.material_source = Some(m.clone());
                cfg.source = Some(delil_evidence::local_folder_source(&m));
            }
            cfg.transcription.language = Some(dil);
            cfg.transcribe = !transkripsiyon_yok;
            cfg.expert = delil_core::ExpertInfo {
                full_name: bilirkisi,
                title: unvan,
                registry_number: sicil,
            };

            println!("DELİL — otomatik işlem başlatıldı.");
            let mut runner = PipelineRunner::new(cfg);
            let mut cb = |e: PipelineEvent| {
                let durum = match &e.state {
                    StepState::Pending => "bekliyor".to_string(),
                    StepState::Running { progress } => {
                        format!("çalışıyor (%{:.0})", progress * 100.0)
                    }
                    StepState::Done => "tamamlandı".to_string(),
                    StepState::Failed { message } => format!("başarısız: {message}"),
                    StepState::NeedsDecision { message } => format!("karar gerekli: {message}"),
                };
                println!("  [{}] {} — {}", e.step.label_tr(), e.message, durum);
            };
            runner.events = Some(&mut cb);
            let res = runner.run()?;

            println!();
            println!("=== SONUÇ ===");
            println!("UDF raporu: {}", res.udf_path.display());
            if let Some(p) = &res.pdf_path {
                println!("PDF kopyası: {}", p.display());
            }
            println!(
                "Materyal: {} dosya ({} doğrulandı)",
                res.case.materials.len(),
                res.case.verified_material_count()
            );
            println!("Ses dosyası: {}", res.case.audio_assets.len());
            println!("Transkript: {}", res.case.transcripts.len());
            println!("UDF doğrulama: {}", res.validation.summary_tr());
            if !res.case.exceptions.is_empty() {
                println!("İncelenecek konular ({}):", res.case.exceptions.len());
                for ex in &res.case.exceptions {
                    println!("  - {}", ex.description);
                }
            }
            let missing = res
                .case
                .report
                .as_ref()
                .map(|r| r.missing_fields.clone())
                .unwrap_or_default();
            if !missing.is_empty() {
                println!("Rapor için eksik alanlar:");
                for m in missing {
                    println!("  - {m}");
                }
            }
            Ok(())
        }
        Command::Aygitlar => {
            let devices = delil_evidence::detect_devices();
            if devices.is_empty() {
                println!("Bağlı harici aygıt bulunamadı.");
            } else {
                println!("{:<24} {:<12} {:<6} Çıkarılabilir", "Kök", "Tür", "Sistem");
                for d in &devices {
                    println!(
                        "{:<24} {:<12} {:<6} {}",
                        d.root,
                        d.kind.label_tr(),
                        if d.is_system_disk { "evet" } else { "hayır" },
                        if d.removable { "evet" } else { "hayır" }
                    );
                }
            }
            match delil_evidence::resolve_source_ambiguity(&devices) {
                Ok(d) => println!("\nOtomatik seçilecek kaynak: {}", d.root),
                Err(e) => println!("\nKaynak seçimi: {e}"),
            }
            Ok(())
        }
        Command::Envanter { dosya } => {
            let container = delil_audio::detect_container_from_path(&dosya)?;
            match container {
                Some(c) => println!(
                    "İçerik tabanlı algılanan biçim: {} ({})",
                    c.label(),
                    if c.is_video_container() {
                        "video kapsayıcı; yalnızca ses akışı"
                    } else {
                        "ses"
                    }
                ),
                None => println!("İçerik ses biçimi olarak tanınamadı."),
            }
            let info = delil_audio::inventory(&dosya)?;
            println!("Kapsayıcı: {}", info.container);
            println!("Codec: {}", info.codec);
            println!("Süre: {:.2} saniye", info.duration_seconds);
            println!("Örnekleme frekansı: {:?} Hz", info.sample_rate_hz);
            println!("Bit derinliği: {:?}", info.bit_depth);
            println!("Kanal: {:?} ({:?})", info.channels, info.channel_layout);
            println!("Okunabilir: {}", info.readable);
            for w in &info.warnings {
                println!("Uyarı: {w}");
            }
            Ok(())
        }
        Command::UdfOku { dosya } => {
            let doc = delil_udf::read_udf(&dosya)?;
            println!("{}", doc.plain_text());
            println!("\n--- Özet ---");
            println!("Metin havuzu: {} karakter", doc.pool.chars().count());
            println!(
                "Blok: {}, tablo: {}, resim: {}",
                doc.blocks.len(),
                doc.table_count,
                doc.image_count
            );
            Ok(())
        }
        Command::UdfDogrula { dosya } => {
            let report = delil_udf::validate_udf(&dosya)?;
            for c in &report.checks {
                println!(
                    "[{}] {} — {}",
                    if c.passed { "OK" } else { "!!" },
                    c.name,
                    c.detail
                );
            }
            println!();
            println!("{}", report.summary_tr());
            if !report.passed() {
                anyhow::bail!("UDF doğrulaması başarısız");
            }
            Ok(())
        }
        Command::UdfAra { dosya, metin } => {
            let doc = delil_udf::read_udf(&dosya)?;
            let hits = doc.search(&metin);
            if hits.is_empty() {
                println!("“{metin}” bulunamadı.");
            } else {
                println!("“{metin}” için {} eşleşme:", hits.len());
                for h in hits {
                    println!("  [blok {}, {}] {}", h.block_index, h.kind, h.context);
                }
            }
            Ok(())
        }
        Command::Vakalar { kok } => {
            let mut count = 0;
            for entry in std::fs::read_dir(&kok)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() && entry.path().join("case.json.enc").exists() {
                    count += 1;
                    println!("Vaka: {}", entry.path().display());
                }
            }
            println!("Toplam {count} vaka bulundu.");
            Ok(())
        }
        Command::Yedekle { vaka, hedef } => {
            let store = delil_case_store::CaseStore::open(&vaka)?;
            let password = read_password()?;
            let manifest = store.backup(&password, &hedef)?;
            println!("Yedek oluşturuldu: {}", hedef.display());
            println!(
                "{} dosya, bütünlük manifesti kaydedildi.",
                manifest.entries.len()
            );
            Ok(())
        }
        Command::GeriYukle { yedek, hedef } => {
            let password = read_password()?;
            let manifest = delil_case_store::CaseStore::restore(&yedek, &password, &hedef)?;
            println!("Geri yükleme tamamlandı: {}", hedef.display());
            println!(
                "{} dosya bütünlük doğrulamasından geçti.",
                manifest.entries.len()
            );
            Ok(())
        }
        Command::Surum => {
            println!("{} {}", delil_core::APP_NAME, delil_core::APP_VERSION);
            println!("Model: {}", delil_core::WHISPER_MODEL_NAME);
            println!("UDF format sürümü: {}", delil_core::UDF_FORMAT_ID);
            println!("Platform: {}", std::env::consts::OS);
            Ok(())
        }
    }
}

/// Parolayı gizli biçimde okur.
fn read_password() -> anyhow::Result<String> {
    use std::io::{BufRead, Write};
    print!("Parola: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    let pw = line.trim_end_matches(['\n', '\r']).to_string();
    if pw.is_empty() {
        anyhow::bail!("Parola boş olamaz.");
    }
    Ok(pw)
}

// `Case` ve `PipelineStep` importları, alt komutlarda kullanılmasa da API
// yüzeyini belgelemek için tutulur.
#[allow(dead_code)]
fn _type_refs(_: &Case, _: PipelineStep) {}
