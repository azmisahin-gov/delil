//! İşlem/metaveri manifesti.
//!
//! Bu kayıtlar uygulamanın yaptığı işlemleri belgeler. Tek başına hukuki
//! muhafaza zinciri, kaynağın tarihsel orijinalliği veya güvenilir zaman
//! damgası yerine geçmez (şartname 15.2).

use delil_core::Case;
use serde::{Deserialize, Serialize};

/// İşlem manifesti.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessingManifest {
    pub app_name: String,
    pub app_version: String,
    pub case_id: String,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub steps: Vec<ManifestStep>,
    /// Bu manifestin hukuki muhafaza zinciri yerine geçmediğine dair not.
    pub disclaimer: String,
}

/// Manifestteki tek işlem adımı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestStep {
    pub at: chrono::DateTime<chrono::Utc>,
    pub action: String,
    pub detail: String,
    pub actor: String,
}

impl ProcessingManifest {
    /// Vakadan işlem manifesti üretir.
    pub fn from_case(case: &Case) -> Self {
        Self {
            app_name: delil_core::APP_NAME.to_string(),
            app_version: delil_core::APP_VERSION.to_string(),
            case_id: case.id.to_string(),
            generated_at: chrono::Utc::now(),
            steps: case
                .audit
                .iter()
                .map(|a| ManifestStep {
                    at: a.at,
                    action: a.action.clone(),
                    detail: a.detail.clone(),
                    actor: a.actor.clone(),
                })
                .collect(),
            disclaimer: "Bu kayıt uygulamanın yaptığı işlemleri belgeler; tek başına \
                         hukuki muhafaza zinciri, kaynağın tarihsel orijinalliği veya \
                         güvenilir zaman damgası yerine geçmez."
                .to_string(),
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_from_case_includes_audit() {
        let mut c = Case::new("/tmp/c");
        c.add_audit("sistem", "edinim", "5 dosya kopyalandı");
        c.add_audit("bilirkişi", "onay", "rapor onaylandı");
        let m = ProcessingManifest::from_case(&c);
        assert_eq!(m.steps.len(), 2);
        assert!(m.disclaimer.contains("hukuki muhafaza zinciri"));
        assert!(m.to_json().unwrap().contains("edinim"));
    }
}
