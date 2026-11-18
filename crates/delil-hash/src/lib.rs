//! SHA-256 hesaplama ve bütünlük doğrulama.
//!
//! Bu crate dosya bazlı edinimin güven çekirdeğidir: kaynak ve hedef
//! bağımsız olarak okunur ve hash karşılaştırması yapılır. Doğrulama
//! başarısızsa dosya asla "doğrulandı" olarak işaretlenmez.

use delil_core::VerificationStatus;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use thiserror::Error;

/// Hash işlemlerinde oluşan hatalar.
#[derive(Debug, Error)]
pub enum HashError {
    #[error("Dosya açılamadı: {path}: {source}")]
    Open {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Dosya okunamadı: {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Kaynak-hedef karşılaştırmasının sonucu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HashComparison {
    pub source_sha256: String,
    pub target_sha256: String,
    pub status: VerificationStatus,
}

impl HashComparison {
    pub fn matches(&self) -> bool {
        self.status.is_verified()
    }
}

/// 1 MiB tamponla akış hâlinde SHA-256 hesaplar. Büyük dosyalar için
/// belleği sabit tutar.
pub fn sha256_file(path: &Path) -> Result<String, HashError> {
    let file = File::open(path).map_err(|e| HashError::Open {
        path: path.display().to_string(),
        source: e,
    })?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(|e| HashError::Read {
            path: path.display().to_string(),
            source: e,
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Bayt dilimi için SHA-256.
pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Kaynak ve hedefi bağımsız okuyup karşılaştırır. Hedef yeniden okunur;
/// yalnızca kopyalama sırasında hesaplanan değere güvenilmez.
pub fn compare_files(source: &Path, target: &Path) -> Result<HashComparison, HashError> {
    let s = sha256_file(source)?;
    let t = sha256_file(target)?;
    let status = if s == t {
        VerificationStatus::Verified
    } else {
        VerificationStatus::Mismatch
    };
    Ok(HashComparison {
        source_sha256: s,
        target_sha256: t,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(dir: &Path, name: &str, content: &[u8]) -> std::path::PathBuf {
        let p = dir.join(name);
        let mut f = File::create(&p).unwrap();
        f.write_all(content).unwrap();
        p
    }

    #[test]
    fn known_sha256_of_empty() {
        assert_eq!(
            sha256_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn known_sha256_of_abc() {
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn file_hash_matches_bytes_hash() {
        let dir = tempfile::tempdir().unwrap();
        let content = b"delil test icerigi - turkce karakterler: \xC4\xB1\xC5\x9F\xC3\xA7";
        let p = write_tmp(dir.path(), "a.bin", content);
        assert_eq!(sha256_file(&p).unwrap(), sha256_bytes(content));
    }

    #[test]
    fn identical_files_verify() {
        let dir = tempfile::tempdir().unwrap();
        let content = b"ayni icerik";
        let a = write_tmp(dir.path(), "a.bin", content);
        let b = write_tmp(dir.path(), "b.bin", content);
        let cmp = compare_files(&a, &b).unwrap();
        assert!(cmp.matches());
        assert_eq!(cmp.status, VerificationStatus::Verified);
    }

    #[test]
    fn differing_files_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let a = write_tmp(dir.path(), "a.bin", b"bir");
        let b = write_tmp(dir.path(), "b.bin", b"iki");
        let cmp = compare_files(&a, &b).unwrap();
        assert!(!cmp.matches());
        assert_eq!(cmp.status, VerificationStatus::Mismatch);
    }

    #[test]
    fn missing_file_is_open_error() {
        let err = sha256_file(Path::new("/nonexistent/delil/xyz")).unwrap_err();
        matches!(err, HashError::Open { .. });
    }
}
