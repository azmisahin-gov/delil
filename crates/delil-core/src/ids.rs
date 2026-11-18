//! Tip güvenli kimlikler.
//!
//! Her kimlik türü ayrı bir `newtype` olarak tanımlanır; böylece bir vaka
//! kimliğinin yanlışlıkla materyal kimliği yerine kullanılması derleme
//! zamanında engellenir.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! define_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// Rastgele yeni kimlik üretir.
            pub fn new() -> Self {
                Self(format!("{}-{}", $prefix, uuid::Uuid::new_v4()))
            }

            /// Var olan metinden kimlik oluşturur (geri yükleme/yedek okuma).
            pub fn from_raw(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }
    };
}

define_id!(CaseId, "case");
define_id!(DeviceId, "dev");
define_id!(MaterialId, "mat");
define_id!(AudioAssetId, "aud");
define_id!(TranscriptId, "trn");
define_id!(QuestionId, "qst");
define_id!(ReportId, "rpt");
define_id!(JobId, "job");
define_id!(AttachmentId, "att");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_carry_prefix() {
        assert!(CaseId::new().as_str().starts_with("case-"));
        assert!(DeviceId::new().as_str().starts_with("dev-"));
        assert!(MaterialId::new().as_str().starts_with("mat-"));
        assert!(AudioAssetId::new().as_str().starts_with("aud-"));
        assert!(TranscriptId::new().as_str().starts_with("trn-"));
        assert!(QuestionId::new().as_str().starts_with("qst-"));
        assert!(ReportId::new().as_str().starts_with("rpt-"));
    }

    #[test]
    fn ids_roundtrip_json() {
        let id = CaseId::new();
        let json = serde_json::to_string(&id).unwrap();
        let back: CaseId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn distinct_id_types_are_distinct() {
        // Aynı ham metinden üretilse bile türler ayrıdır ve birbirine atanamaz.
        let raw = "abc".to_string();
        let a = CaseId::from_raw(raw.clone());
        let b = MaterialId::from_raw(raw.clone());
        // Ham metin aynı olsa da türler farklıdır.
        assert_eq!(a.as_str(), b.as_str());
        fn takes_case(_: &CaseId) {}
        fn takes_material(_: &MaterialId) {}
        takes_case(&a);
        takes_material(&b);
        // Farklı önekli üretilen kimlikler ayrışır.
        assert_ne!(CaseId::new().as_str(), MaterialId::new().as_str());
    }
}
