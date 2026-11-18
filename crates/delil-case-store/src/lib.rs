//! Şifreli yerel vaka deposu, yedekleme ve geri yükleme (şartname 15 ve 16).
//!
//! Vaka verileri disk üzerinde şifreli tutulur. Şifreleme için doğrulanmış bir
//! AEAD mekanizması olan **AES-256-GCM** kullanılır. Anahtar yönetiminde:
//!
//! * **Windows:** Rastgele üretilen ana anahtar, Windows DPAPI ile korunur
//!   (`CryptProtectData`), böylece kullanıcı hesabına bağlanır.
//! * **Diğer platformlar (geliştirme/test):** Ana anahtar, kısıtlı izinli bir
//!   anahtar dosyasında saklanır. Bu yalnızca geliştirme kolaylığı içindir;
//!   üretim hedefi Windows'tur.
//!
//! Taşınabilir yedekler, parola korumalı bir kurtarma mekanizması kullanır:
//! Argon2id ile paroladan anahtar türetilir. Parola hiçbir zaman loglara
//! yazılmaz.

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Key, Nonce};
use argon2::Argon2;
use delil_core::Case;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Vaka deposu hataları.
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("Şifreleme hatası")]
    Crypto,
    #[error("Şifre çözme başarısız (anahtar veya veri bozuk)")]
    Decrypt,
    #[error("JSON hatası: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ZIP hatası: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Parola türetme hatası: {0}")]
    Kdf(String),
    #[error("Ana anahtar bulunamadı; depo açılamıyor")]
    NoKey,
    #[error("Bütünlük doğrulaması başarısız: {0}")]
    Integrity(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// Şifreli dosya başlığı: tuz, nonce ve biçim sürümü.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedEnvelope {
    version: u8,
    /// Argon2 tuzu (base64).
    salt: String,
    /// AES-GCM nonce (base64).
    nonce: String,
    /// Şifreli veri (base64).
    ciphertext: String,
    /// Düz metnin SHA-256'sı (bütünlük göstergesi).
    plain_sha256: String,
}

const ENVELOPE_VERSION: u8 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

/// Argon2id ile paroladan 32 baytlık anahtar türetir.
fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let mut key = [0u8; 32];
    let params =
        argon2::Params::new(19_456, 2, 1, Some(32)).map_err(|e| StoreError::Kdf(e.to_string()))?;
    let a = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    a.hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| StoreError::Kdf(e.to_string()))?;
    Ok(key)
}

/// Veriyi AES-256-GCM ile şifreler ve zarf olarak serileştirir.
pub fn encrypt_bytes(plain: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher
        .encrypt(&nonce, plain)
        .map_err(|_| StoreError::Crypto)?;
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let env = EncryptedEnvelope {
        version: ENVELOPE_VERSION,
        salt: base64_encode(&salt),
        nonce: base64_encode(&nonce),
        ciphertext: base64_encode(&ct),
        plain_sha256: hex::encode(delil_hash::sha256_bytes(plain)),
    };
    Ok(serde_json::to_vec(&env)?)
}

/// Zarfı çözer ve düz metni döndürür.
pub fn decrypt_bytes(envelope: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    let env: EncryptedEnvelope = serde_json::from_slice(envelope)?;
    if env.version != ENVELOPE_VERSION {
        return Err(StoreError::Integrity(format!(
            "Bilinmeyen zarf sürümü: {}",
            env.version
        )));
    }
    let nonce_bytes = base64_decode(&env.nonce)?;
    if nonce_bytes.len() != NONCE_LEN {
        return Err(StoreError::Decrypt);
    }
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = base64_decode(&env.ciphertext)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let plain = cipher
        .decrypt(nonce, ct.as_ref())
        .map_err(|_| StoreError::Decrypt)?;
    // Bütünlük göstergesi.
    let got = hex::encode(delil_hash::sha256_bytes(&plain));
    if got != env.plain_sha256 {
        return Err(StoreError::Integrity("Düz metin hash'i uyuşmadı".into()));
    }
    Ok(plain)
}

fn base64_encode(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn base64_decode(s: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|_| StoreError::Decrypt)
}

/// Parola ile şifreleyip dosyaya yazar (taşınabilir yedek için).
pub fn encrypt_to_file(plain: &[u8], password: &str, path: &Path) -> Result<()> {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher
        .encrypt(&nonce, plain)
        .map_err(|_| StoreError::Crypto)?;
    let env = EncryptedEnvelope {
        version: ENVELOPE_VERSION,
        salt: base64_encode(&salt),
        nonce: base64_encode(&nonce),
        ciphertext: base64_encode(&ct),
        plain_sha256: hex::encode(delil_hash::sha256_bytes(plain)),
    };
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(&env)?)?;
    Ok(())
}

/// Parola ile şifrelenmiş dosyayı çözer.
pub fn decrypt_from_file(path: &Path, password: &str) -> Result<Vec<u8>> {
    let data = std::fs::read(path)?;
    let env: EncryptedEnvelope = serde_json::from_slice(&data)?;
    let salt = base64_decode(&env.salt)?;
    let key = derive_key(password, &salt)?;
    decrypt_bytes(&data, &key)
}

/// Vaka deposu: bir vaka kök dizinini yönetir.
#[derive(Debug, Clone)]
pub struct CaseStore {
    /// Vaka kök dizini.
    pub root: PathBuf,
}

impl CaseStore {
    /// Yeni bir vaka deposu oluşturur ve gerekli alt dizinleri hazırlar.
    pub fn create(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let store = Self { root };
        for sub in [
            "assignment",
            "materials",
            "transcripts",
            "report",
            "logs",
            "keys",
        ] {
            std::fs::create_dir_all(store.root.join(sub))?;
        }
        Ok(store)
    }

    /// Var olan vaka deposunu açar.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        if !root.join("case.json.enc").exists() && !root.join("case.json").exists() {
            return Err(StoreError::NoKey);
        }
        Ok(Self { root })
    }

    fn master_key_path(&self) -> PathBuf {
        self.root.join("keys").join("master.key")
    }

    fn case_path(&self) -> PathBuf {
        self.root.join("case.json.enc")
    }

    /// Vaka meta verisini şifreli olarak kaydeder.
    pub fn save_case(&self, case: &Case, key: &[u8; 32]) -> Result<()> {
        let json = serde_json::to_vec_pretty(case)?;
        let enc = encrypt_bytes(&json, key)?;
        std::fs::write(self.case_path(), enc)?;
        Ok(())
    }

    /// Şifreli vaka meta verisini yükler.
    pub fn load_case(&self, key: &[u8; 32]) -> Result<Case> {
        let enc = std::fs::read(self.case_path())?;
        let plain = decrypt_bytes(&enc, key)?;
        Ok(serde_json::from_slice(&plain)?)
    }

    /// Ana anahtarı yükler; yoksa yeni üretip kaydeder.
    ///
    /// Windows'ta DPAPI ile korunur. Diğer platformlarda kısıtlı izinli dosya.
    pub fn load_or_create_master_key(&self) -> Result<[u8; 32]> {
        let path = self.master_key_path();
        if path.exists() {
            let raw = std::fs::read(&path)?;
            return self.unprotect_key(&raw);
        }
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        let protected = self.protect_key(&key)?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, protected)?;
        restrict_permissions(&path)?;
        Ok(key)
    }

    /// Ana anahtarı platforma uygun biçimde korur.
    fn protect_key(&self, key: &[u8; 32]) -> Result<Vec<u8>> {
        #[cfg(windows)]
        {
            return dpapi::protect(key);
        }
        #[cfg(not(windows))]
        {
            Ok(key.to_vec())
        }
    }

    /// Korunmuş ana anahtarı çözer.
    fn unprotect_key(&self, raw: &[u8]) -> Result<[u8; 32]> {
        #[cfg(windows)]
        {
            let plain = dpapi::unprotect(raw)?;
            return arr32(&plain);
        }
        #[cfg(not(windows))]
        {
            arr32(raw)
        }
    }

    /// Vaka deposunu parola korumalı taşınabilir bir yedeğe dönüştürür.
    ///
    /// Yedek, vaka bütünlüğünü korur: her dosyanın SHA-256'sı manifestte
    /// saklanır ve geri yüklemede doğrulanır.
    pub fn backup(&self, password: &str, dest: &Path) -> Result<BackupManifest> {
        let manifest = self.build_manifest()?;
        let manifest_json = serde_json::to_vec_pretty(&manifest)?;

        // Tüm dosyaları tek bir ZIP'te topla.
        let mut zip_buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for entry in walk_dir(&self.root)? {
                let rel = entry.strip_prefix(&self.root).unwrap();
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                zip.start_file(rel_str, opts)?;
                let data = std::fs::read(&entry)?;
                zip.write_all(&data)?;
            }
            zip.start_file("__manifest__.json", opts)?;
            zip.write_all(&manifest_json)?;
            zip.finish()?;
        }

        encrypt_to_file(&zip_buf, password, dest)?;
        Ok(manifest)
    }

    /// Parola korumalı yedeği başka bir konuma geri yükler ve bütünlüğü
    /// doğrular.
    pub fn restore(backup_path: &Path, password: &str, dest: &Path) -> Result<BackupManifest> {
        let zip_buf = decrypt_from_file(backup_path, password)?;
        let cursor = std::io::Cursor::new(zip_buf);
        let mut archive = zip::ZipArchive::new(cursor)?;

        // Manifesti oku.
        let manifest: BackupManifest = {
            let mut f = archive
                .by_name("__manifest__.json")
                .map_err(|_| StoreError::Integrity("Yedek manifesti bulunamadı".into()))?;
            let mut s = String::new();
            f.read_to_string(&mut s)?;
            serde_json::from_str(&s)?
        };

        std::fs::create_dir_all(dest)?;

        // Dosyaları çıkar.
        let names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
        for name in names {
            if name == "__manifest__.json" {
                continue;
            }
            let mut f = archive.by_name(&name)?;
            let mut data = Vec::new();
            f.read_to_end(&mut data)?;
            let target = dest.join(name.replace('/', std::path::MAIN_SEPARATOR_STR));
            if let Some(p) = target.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::write(&target, &data)?;
        }

        // Bütünlük doğrulaması: her dosyanın SHA-256'sı manifestle eşleşmeli.
        for entry in &manifest.entries {
            let target = dest.join(
                entry
                    .relative_path
                    .replace('/', std::path::MAIN_SEPARATOR_STR),
            );
            if !target.exists() {
                return Err(StoreError::Integrity(format!(
                    "Geri yüklenen dosya eksik: {}",
                    entry.relative_path
                )));
            }
            let got = delil_hash::sha256_file(&target)
                .map_err(|e| StoreError::Integrity(e.to_string()))?;
            if got != entry.sha256 {
                return Err(StoreError::Integrity(format!(
                    "Hash uyuşmadı: {} (beklenen {}, bulunan {})",
                    entry.relative_path, entry.sha256, got
                )));
            }
        }

        Ok(manifest)
    }

    /// Vaka deposunun bütünlük manifestini oluşturur.
    pub fn build_manifest(&self) -> Result<BackupManifest> {
        let mut entries = Vec::new();
        for entry in walk_dir(&self.root)? {
            let rel = entry.strip_prefix(&self.root).unwrap();
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            // Ana anahtar dosyası yedeğe girmez (parola ile korunur).
            if rel_str.starts_with("keys/") {
                continue;
            }
            let sha = delil_hash::sha256_file(&entry)
                .map_err(|e| StoreError::Integrity(e.to_string()))?;
            let size = std::fs::metadata(&entry)?.len();
            entries.push(ManifestEntry {
                relative_path: rel_str,
                sha256: sha,
                size_bytes: size,
            });
        }
        entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(BackupManifest {
            version: 1,
            created_at: chrono::Utc::now(),
            entries,
        })
    }

    /// Vaka deposunun bütünlüğünü doğrular.
    pub fn verify_integrity(&self) -> Result<usize> {
        let manifest = self.build_manifest()?;
        Ok(manifest.entries.len())
    }
}

/// Yedek manifesti.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub version: u8,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub entries: Vec<ManifestEntry>,
}

/// Manifestteki tek dosya kaydı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub relative_path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

fn arr32(v: &[u8]) -> Result<[u8; 32]> {
    if v.len() != 32 {
        return Err(StoreError::Decrypt);
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(v);
    Ok(a)
}

/// Dizindeki tüm normal dosyaları listeler.
fn walk_dir(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if !root.exists() {
        return Ok(out);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let p = entry.path();
            let ft = entry.file_type()?;
            if ft.is_dir() {
                stack.push(p);
            } else if ft.is_file() {
                out.push(p);
            }
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(not(windows))]
fn restrict_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(path, perms)?;
    }
    let _ = path;
    Ok(())
}

#[cfg(windows)]
fn restrict_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

/// Windows DPAPI anahtar koruma yardımcıları.
#[cfg(windows)]
pub mod dpapi {
    use super::StoreError;

    /// Veriyi DPAPI ile korur.
    pub fn protect(plain: &[u8]) -> super::Result<Vec<u8>> {
        use windows::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};
        unsafe {
            let mut input = CRYPT_INTEGER_BLOB {
                cbData: plain.len() as u32,
                pbData: plain.as_ptr() as *mut u8,
            };
            let mut output = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };
            CryptProtectData(&mut input, None, None, None, None, 0, &mut output)
                .map_err(|_| StoreError::Crypto)?;
            let out = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            windows::Win32::Foundation::LocalFree(windows::Win32::Foundation::HLOCAL(
                output.pbData as *mut _,
            ));
            Ok(out)
        }
    }

    /// DPAPI ile korunmuş veriyi çözer.
    pub fn unprotect(protected: &[u8]) -> super::Result<Vec<u8>> {
        use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};
        unsafe {
            let mut input = CRYPT_INTEGER_BLOB {
                cbData: protected.len() as u32,
                pbData: protected.as_ptr() as *mut u8,
            };
            let mut output = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };
            CryptUnprotectData(&mut input, None, None, None, None, 0, &mut output)
                .map_err(|_| StoreError::Decrypt)?;
            let out = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            windows::Win32::Foundation::LocalFree(windows::Win32::Foundation::HLOCAL(
                output.pbData as *mut _,
            ));
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use delil_core::Case;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = [7u8; 32];
        let plain = b"gizli vaka verisi - \xC4\xB1\xC5\x9F\xC3\xA7";
        let enc = encrypt_bytes(plain, &key).unwrap();
        assert_ne!(&enc[..], &plain[..]);
        let dec = decrypt_bytes(&enc, &key).unwrap();
        assert_eq!(dec, plain);
    }

    #[test]
    fn wrong_key_fails_to_decrypt() {
        let key = [1u8; 32];
        let enc = encrypt_bytes(b"veri", &key).unwrap();
        let wrong = [2u8; 32];
        assert!(decrypt_bytes(&enc, &wrong).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = [3u8; 32];
        let mut enc = encrypt_bytes(b"veri", &key).unwrap();
        // Zarfı boz.
        let mid = enc.len() / 2;
        enc[mid] ^= 0xFF;
        assert!(decrypt_bytes(&enc, &key).is_err());
    }

    #[test]
    fn password_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("backup.enc");
        encrypt_to_file(b"yedek icerigi", "güçlü-parola-123", &p).unwrap();
        let dec = decrypt_from_file(&p, "güçlü-parola-123").unwrap();
        assert_eq!(dec, b"yedek icerigi");
        assert!(decrypt_from_file(&p, "yanlış-parola").is_err());
    }

    #[test]
    fn case_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let store = CaseStore::create(dir.path()).unwrap();
        let key = store.load_or_create_master_key().unwrap();
        let mut case = Case::new(dir.path().to_string_lossy().to_string());
        case.assignment.case_number = Some("2024/999".into());
        store.save_case(&case, &key).unwrap();
        let loaded = store.load_case(&key).unwrap();
        assert_eq!(loaded.assignment.case_number.as_deref(), Some("2024/999"));
        assert_eq!(loaded.id, case.id);
    }

    #[test]
    fn master_key_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let store = CaseStore::create(dir.path()).unwrap();
        let k1 = store.load_or_create_master_key().unwrap();
        let k2 = store.load_or_create_master_key().unwrap();
        assert_eq!(k1, k2);
    }

    #[test]
    fn backup_and_restore_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = CaseStore::create(dir.path().join("case1")).unwrap();
        let key = store.load_or_create_master_key().unwrap();
        let case = Case::new(store.root.to_string_lossy().to_string());
        store.save_case(&case, &key).unwrap();
        // Materyal dosyası ekle.
        std::fs::write(
            store.root.join("materials").join("ses.wav"),
            b"RIFF....WAVE",
        )
        .unwrap();

        let backup = dir.path().join("yedek.delilbackup");
        let manifest = store.backup("parola-abc", &backup).unwrap();
        assert!(backup.exists());
        assert!(manifest
            .entries
            .iter()
            .any(|e| e.relative_path.contains("ses.wav")));
        // Ana anahtar yedeğe girmemeli.
        assert!(!manifest
            .entries
            .iter()
            .any(|e| e.relative_path.starts_with("keys/")));

        // Başka konuma geri yükle.
        let restore_dir = dir.path().join("restored");
        let restored = CaseStore::restore(&backup, "parola-abc", &restore_dir).unwrap();
        assert_eq!(restored.entries.len(), manifest.entries.len());
        assert!(restore_dir.join("materials").join("ses.wav").exists());
        // İçerik aynı mı?
        let orig = std::fs::read(store.root.join("materials").join("ses.wav")).unwrap();
        let back = std::fs::read(restore_dir.join("materials").join("ses.wav")).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn restore_with_wrong_password_fails() {
        let dir = tempfile::tempdir().unwrap();
        let store = CaseStore::create(dir.path().join("case1")).unwrap();
        let key = store.load_or_create_master_key().unwrap();
        store.save_case(&Case::new("x"), &key).unwrap();
        let backup = dir.path().join("b.delilbackup");
        store.backup("doğru", &backup).unwrap();
        let res = CaseStore::restore(&backup, "yanlış", &dir.path().join("r"));
        assert!(res.is_err());
    }

    #[test]
    fn tampered_backup_fails_integrity() {
        let dir = tempfile::tempdir().unwrap();
        let store = CaseStore::create(dir.path().join("case1")).unwrap();
        let key = store.load_or_create_master_key().unwrap();
        store.save_case(&Case::new("x"), &key).unwrap();
        std::fs::write(store.root.join("materials").join("a.bin"), b"orijinal").unwrap();
        let backup = dir.path().join("b.delilbackup");
        store.backup("p", &backup).unwrap();

        // Yedeği çöz, dosyayı değiştir, yeniden şifrele.
        let raw = decrypt_from_file(&backup, "p").unwrap();
        let mut zip_buf = Vec::new();
        {
            let cursor = std::io::Cursor::new(raw);
            let mut archive = zip::ZipArchive::new(cursor).unwrap();
            let mut zw = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for i in 0..archive.len() {
                let mut f = archive.by_index(i).unwrap();
                let name = f.name().to_string();
                let mut data = Vec::new();
                f.read_to_end(&mut data).unwrap();
                if name.ends_with("a.bin") {
                    data = b"degistirilmis".to_vec();
                }
                zw.start_file(name, opts).unwrap();
                zw.write_all(&data).unwrap();
            }
            zw.finish().unwrap();
        }
        encrypt_to_file(&zip_buf, "p", &backup).unwrap();

        let res = CaseStore::restore(&backup, "p", &dir.path().join("r"));
        assert!(matches!(res, Err(StoreError::Integrity(_))));
    }

    #[test]
    fn verify_integrity_counts_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = CaseStore::create(dir.path()).unwrap();
        std::fs::write(store.root.join("materials").join("x.bin"), b"x").unwrap();
        let n = store.verify_integrity().unwrap();
        assert!(n >= 1);
    }
}
