//! Yardımcı çıktılar: PDF rapor kopyası, TXT transkript kopyaları ve JSON
//! işlem/metaveri manifesti (şartname 12.6).
//!
//! Birincil çıktı UDF'dir; DOCX birincil veya zorunlu çıktı değildir.
//! Bu yardımcı çıktılar inceleme ve arşivleme kolaylığı sağlar.

use delil_core::Case;
use delil_core::Report;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Dışa aktarma hataları.
#[derive(Debug, Error)]
pub enum ExportError {
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("PDF hatası: {0}")]
    Pdf(String),
    #[error("JSON hatası: {0}")]
    Json(#[from] serde_json::Error),
}

pub mod manifest;
pub mod pdf;
pub mod transcript_txt;

pub use manifest::ProcessingManifest;
pub use pdf::export_pdf;
pub use transcript_txt::export_transcript_txt;

/// İşlem/metaveri manifesti: vaka, materyal, hash ve işlem kaydı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FullManifest {
    pub app_name: String,
    pub app_version: String,
    pub case_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub exported_at: chrono::DateTime<chrono::Utc>,
    pub materials: Vec<MaterialEntry>,
    pub audio: Vec<AudioEntry>,
    pub audit: Vec<delil_core::AuditEntry>,
}

/// Manifestteki materyal kaydı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialEntry {
    pub file_name: String,
    pub source_path: String,
    pub size_bytes: u64,
    pub source_sha256: Option<String>,
    pub target_sha256: Option<String>,
    pub verification: String,
}

/// Manifestteki ses kaydı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioEntry {
    pub file_name: String,
    pub container: String,
    pub codec: String,
    pub duration_seconds: f64,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u16>,
    pub sha256: Option<String>,
}

/// Vakadan tam manifest oluşturur.
pub fn build_full_manifest(case: &Case) -> FullManifest {
    FullManifest {
        app_name: delil_core::APP_NAME.to_string(),
        app_version: delil_core::APP_VERSION.to_string(),
        case_id: case.id.to_string(),
        created_at: case.created_at,
        exported_at: chrono::Utc::now(),
        materials: case
            .materials
            .iter()
            .map(|m| MaterialEntry {
                file_name: m.file_name.clone(),
                source_path: m.source_path.clone(),
                size_bytes: m.size_bytes,
                source_sha256: m.source_sha256.clone(),
                target_sha256: m.target_sha256.clone(),
                verification: m.verification.label_tr().to_string(),
            })
            .collect(),
        audio: case
            .audio_assets
            .iter()
            .map(|a| AudioEntry {
                file_name: a.file_name.clone(),
                container: a.technical.container.clone(),
                codec: a.technical.codec.clone(),
                duration_seconds: a.technical.duration_seconds,
                sample_rate_hz: a.technical.sample_rate_hz,
                channels: a.technical.channels,
                sha256: a.sha256.clone(),
            })
            .collect(),
        audit: case.audit.clone(),
    }
}

/// Manifesti JSON olarak yazar.
pub fn write_full_manifest(case: &Case, path: &std::path::Path) -> Result<(), ExportError> {
    let m = build_full_manifest(case);
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(&m)?)?;
    Ok(())
}

/// Raporu tüm yardımcı biçimlerde dışa aktarır.
pub fn export_all(
    case: &Case,
    report: &Report,
    out_dir: &std::path::Path,
) -> Result<ExportArtifacts, ExportError> {
    std::fs::create_dir_all(out_dir)?;
    let pdf_path = out_dir.join("BILIRKISI_RAPORU.pdf");
    export_pdf(report, &pdf_path)?;

    let manifest_path = out_dir.join("ISLEM_MANIFESTI.json");
    write_full_manifest(case, &manifest_path)?;

    let mut transcript_paths = Vec::new();
    for (i, t) in report.transcripts.iter().enumerate() {
        let name = format!("TRANSKRIPT_EK_{}.txt", i + 1);
        let p = out_dir.join(name);
        export_transcript_txt(t, &p)?;
        transcript_paths.push(p);
    }

    Ok(ExportArtifacts {
        pdf_path,
        manifest_path,
        transcript_paths,
    })
}

/// Üretilen yardımcı çıktıların yolları.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportArtifacts {
    pub pdf_path: std::path::PathBuf,
    pub manifest_path: std::path::PathBuf,
    pub transcript_paths: Vec<std::path::PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use delil_core::*;

    fn sample_case() -> Case {
        let mut c = Case::new("/tmp/c");
        c.assignment.case_number = Some("2024/1".into());
        c.materials.push(Material {
            id: MaterialId::new(),
            source_device_id: DeviceId::new(),
            source_path: "/src/a.wav".into(),
            file_name: "a.wav".into(),
            file_type: "wav".into(),
            size_bytes: 1234,
            source_sha256: Some("aa".into()),
            target_sha256: Some("aa".into()),
            stored_relative_path: "a.wav".into(),
            acquired_at: chrono::Utc::now(),
            verification: VerificationStatus::Verified,
            duplicate_of: None,
            warnings: vec![],
        });
        c.audio_assets.push(AudioAsset {
            id: AudioAssetId::new(),
            material_id: MaterialId::new(),
            file_name: "a.wav".into(),
            sha256: Some("aa".into()),
            extracted_from_video: false,
            technical: AudioTechnicalInfo {
                container: "WAV".into(),
                codec: "PCM".into(),
                duration_seconds: 3.0,
                sample_rate_hz: Some(16000),
                channels: Some(1),
                readable: true,
                ..Default::default()
            },
            queued: true,
            unsupported_reason: None,
        });
        c
    }

    #[test]
    fn full_manifest_contains_materials_and_audio() {
        let c = sample_case();
        let m = build_full_manifest(&c);
        assert_eq!(m.materials.len(), 1);
        assert_eq!(m.audio.len(), 1);
        assert_eq!(m.materials[0].verification, "Doğrulandı");
        assert_eq!(m.app_name, "DELİL");
    }

    #[test]
    fn write_and_read_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let c = sample_case();
        let p = dir.path().join("m.json");
        write_full_manifest(&c, &p).unwrap();
        let back: FullManifest = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        assert_eq!(back.materials.len(), 1);
    }
}
