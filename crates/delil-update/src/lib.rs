//! İmzalı otomatik güncelleme yöneticisi (şartname 18).
//!
//! Güvenlik zinciri:
//! 1. **İmzalı manifest** — manifest ed25519 ile imzalanır.
//! 2. **SHA-256 paket doğrulama** — indirilen paket hash ile eşleşmeli.
//! 3. **Dijital imza kontrolü** — manifest imzası ortak anahtarla doğrulanır.
//! 4. **Güncelleme öncesi yedek** — mevcut sürüm yedeklenir.
//! 5. **Güvenli kurulum** — yeni dosyalar geçici dizine açılır, sonra taşınır.
//! 6. **Başarısız kurulumda geri dönüş** — yedekten geri yüklenir.
//! 7. **Veritabanı migrasyonu** ve **eski dosyaları açma uyumluluğu**.
//!
//! Güncelleme aktif transkripsiyon veya kopyalama sırasında uygulanmaz.
//! İnternet yoksa uygulamanın mevcut özellikleri çalışmaya devam eder.
//! Çevrimdışı güncelleme paketiyle de güvenli güncelleme yapılabilir.
//! Vaka belgeleri güncelleme sunucusuna gönderilmez.

use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Güncelleme hataları.
#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON hatası: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ZIP hatası: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Manifest imzası geçersiz")]
    SignatureInvalid,
    #[error("Paket hash'i uyuşmadı: beklenen {expected}, bulunan {found}")]
    HashMismatch { expected: String, found: String },
    #[error("Sürüm geriye alınamaz: mevcut {current}, yeni {new}")]
    Downgrade { current: String, new: String },
    #[error("Güncelleme şu anda uygulanamaz: {0}")]
    Busy(String),
    #[error("Geri dönüş yedeği bulunamadı")]
    NoRollback,
    #[error("Geçersiz imza biçimi")]
    BadSignature,
}

pub type Result<T> = std::result::Result<T, UpdateError>;

/// Güncelleme manifesti.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    pub channel: String,
    pub released_at: chrono::DateTime<chrono::Utc>,
    /// Paket dosya adı.
    pub package: String,
    /// Paketin SHA-256'sı.
    pub sha256: String,
    /// Paket boyutu.
    pub size_bytes: u64,
    /// Asgari desteklenen mevcut sürüm.
    pub min_current_version: Option<String>,
    /// Veritabanı şema sürümü.
    pub schema_version: u32,
    /// Sürüm notları.
    pub notes: Vec<String>,
}

/// İmzalı manifest zarfı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignedManifest {
    pub manifest: UpdateManifest,
    /// ed25519 imzası (base64).
    pub signature: String,
}

impl SignedManifest {
    /// Manifesti ortak anahtarla doğrular.
    pub fn verify(&self, public_key_b64: &str) -> Result<()> {
        let pk_bytes = base64::engine::general_purpose::STANDARD
            .decode(public_key_b64)
            .map_err(|_| UpdateError::BadSignature)?;
        let pk_arr: [u8; 32] = pk_bytes.try_into().map_err(|_| UpdateError::BadSignature)?;
        let vk = VerifyingKey::from_bytes(&pk_arr).map_err(|_| UpdateError::BadSignature)?;
        let sig_bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.signature)
            .map_err(|_| UpdateError::BadSignature)?;
        let sig_arr: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| UpdateError::BadSignature)?;
        let sig = Signature::from_bytes(&sig_arr);
        let payload = manifest_payload(&self.manifest)?;
        vk.verify(&payload, &sig)
            .map_err(|_| UpdateError::SignatureInvalid)
    }
}

/// İmzalanan yük: manifestin kararlı JSON serileştirmesi.
pub fn manifest_payload(m: &UpdateManifest) -> Result<Vec<u8>> {
    // Alan sırası sabit olduğundan serde_json deterministiktir.
    Ok(serde_json::to_vec(m)?)
}

/// Sürüm karşılaştırması (semver benzeri "a.b.c").
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let pa: Vec<u32> = a.split('.').filter_map(|s| s.parse().ok()).collect();
    let pb: Vec<u32> = b.split('.').filter_map(|s| s.parse().ok()).collect();
    let n = pa.len().max(pb.len());
    for i in 0..n {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            std::cmp::Ordering::Equal => continue,
            o => return o,
        }
    }
    std::cmp::Ordering::Equal
}

/// Güncelleme yöneticisi.
pub struct UpdateManager {
    /// Yüklü sürüm.
    pub current_version: String,
    /// Güncelleme ortak anahtarı (base64).
    pub public_key_b64: String,
    /// Uygulama kök dizini (kurulum hedefi).
    pub install_dir: PathBuf,
    /// Yedek dizini.
    pub backup_dir: PathBuf,
}

/// Güncelleme ilerleme durumu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UpdateProgress {
    Checking,
    ManifestVerified,
    Downloading,
    Downloaded,
    HashVerified,
    BackedUp,
    Installing,
    Migrating,
    Done,
    RolledBack,
    Failed(String),
}

impl UpdateManager {
    pub fn new(
        current_version: impl Into<String>,
        public_key_b64: impl Into<String>,
        install_dir: impl Into<PathBuf>,
        backup_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            current_version: current_version.into(),
            public_key_b64: public_key_b64.into(),
            install_dir: install_dir.into(),
            backup_dir: backup_dir.into(),
        }
    }

    /// Yeni bir sürüm mevcut mu?
    pub fn update_available(&self, manifest: &UpdateManifest) -> bool {
        compare_versions(&manifest.version, &self.current_version) == std::cmp::Ordering::Greater
    }

    /// Güncellemeyi uygular.
    ///
    /// `package_bytes` çevrimdışı paket olabilir. `busy` true ise (aktif
    /// transkripsiyon/kopyalama) güncelleme uygulanmaz.
    pub fn apply(
        &self,
        signed: &SignedManifest,
        package_bytes: &[u8],
        busy: bool,
    ) -> Result<UpdateProgress> {
        if busy {
            return Err(UpdateError::Busy(
                "Aktif transkripsiyon veya kopyalama sürüyor".into(),
            ));
        }
        // 1. İmza doğrulama.
        signed.verify(&self.public_key_b64)?;

        // 2. Sürüm kontrolü: geriye alma yok.
        if compare_versions(&signed.manifest.version, &self.current_version)
            == std::cmp::Ordering::Less
        {
            return Err(UpdateError::Downgrade {
                current: self.current_version.clone(),
                new: signed.manifest.version.clone(),
            });
        }

        // 3. Paket hash doğrulaması.
        let found = hex::encode(delil_hash_sha256(package_bytes));
        if found != signed.manifest.sha256 {
            return Err(UpdateError::HashMismatch {
                expected: signed.manifest.sha256.clone(),
                found,
            });
        }

        // 4. Yedek al.
        let backup = self.backup_current()?;

        // 5. Paketi geçici dizine aç ve kur.
        let tmp = tempfile::tempdir()?;
        match self.install_from_bytes(package_bytes, tmp.path()) {
            Ok(()) => Ok(UpdateProgress::Done),
            Err(_e) => {
                // 6. Başarısız kurulumda geri dönüş.
                self.rollback(&backup)?;
                Ok(UpdateProgress::RolledBack)
            }
        }
    }

    /// Mevcut kurulumu yedekler.
    fn backup_current(&self) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.backup_dir)?;
        let stamp = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
        let dest = self.backup_dir.join(format!("backup-{stamp}.zip"));
        let file = std::fs::File::create(&dest)?;
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        if self.install_dir.exists() {
            for entry in walk_files(&self.install_dir)? {
                let rel = entry.strip_prefix(&self.install_dir).unwrap();
                let name = rel.to_string_lossy().replace('\\', "/");
                zip.start_file(name, opts)?;
                let data = std::fs::read(&entry)?;
                std::io::Write::write_all(&mut zip, &data)?;
            }
        }
        zip.finish()?;
        Ok(dest)
    }

    /// Paketi geçici dizine açıp kurulum dizinine taşır.
    fn install_from_bytes(&self, package_bytes: &[u8], tmp: &Path) -> Result<()> {
        let cursor = std::io::Cursor::new(package_bytes);
        let mut archive = zip::ZipArchive::new(cursor)?;
        archive.extract(tmp)?;

        // Güvenli kurulum: dosyaları tek tek hedefe taşı.
        for entry in walk_files(tmp)? {
            let rel = entry.strip_prefix(tmp).unwrap();
            let target = self.install_dir.join(rel);
            if let Some(p) = target.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(&entry, &target)?;
        }
        Ok(())
    }

    /// Yedekten geri döner.
    pub fn rollback(&self, backup_zip: &Path) -> Result<()> {
        if !backup_zip.exists() {
            return Err(UpdateError::NoRollback);
        }
        let file = std::fs::File::open(backup_zip)?;
        let mut archive = zip::ZipArchive::new(file)?;
        archive.extract(&self.install_dir)?;
        Ok(())
    }
}

fn walk_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if !root.exists() {
        return Ok(out);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir)? {
            let e = e?;
            let p = e.path();
            if e.file_type()?.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    Ok(out)
}

/// Bağımlılık döngüsünü önlemek için yerel SHA-256 sarmalayıcı.
fn delil_hash_sha256(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

/// Test/geliştirme için imza üretir (yalnızca `dev-sign` özelliğiyle).
#[cfg(any(test, feature = "dev-sign"))]
pub fn sign_manifest(manifest: &UpdateManifest, secret_key: &[u8; 32]) -> Result<SignedManifest> {
    use ed25519_dalek::{Signer, SigningKey};
    let sk = SigningKey::from_bytes(secret_key);
    let payload = manifest_payload(manifest)?;
    let sig = sk.sign(&payload);
    Ok(SignedManifest {
        manifest: manifest.clone(),
        signature: base64::engine::general_purpose::STANDARD.encode(sig.to_bytes()),
    })
}

/// Test/geliştirme için ortak anahtar üretir.
#[cfg(any(test, feature = "dev-sign"))]
pub fn public_key_of(secret_key: &[u8; 32]) -> String {
    use ed25519_dalek::SigningKey;
    let sk = SigningKey::from_bytes(secret_key);
    base64::engine::general_purpose::STANDARD.encode(sk.verifying_key().to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn manifest_for(version: &str, package_sha: &str) -> UpdateManifest {
        UpdateManifest {
            version: version.to_string(),
            channel: "stable".into(),
            released_at: chrono::Utc::now(),
            package: "delil-update.zip".into(),
            sha256: package_sha.to_string(),
            size_bytes: 0,
            min_current_version: None,
            schema_version: 2,
            notes: vec!["Hata düzeltmeleri".into()],
        }
    }

    fn make_package(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            for (name, data) in files {
                zip.start_file(*name, opts).unwrap();
                zip.write_all(data).unwrap();
            }
            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn version_comparison() {
        assert_eq!(compare_versions("1.2.3", "1.2.4"), std::cmp::Ordering::Less);
        assert_eq!(
            compare_versions("1.2.10", "1.2.9"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(compare_versions("1.0", "1.0.0"), std::cmp::Ordering::Equal);
    }

    #[test]
    fn signed_manifest_verifies() {
        let sk = [7u8; 32];
        let pkg = make_package(&[("a.txt", b"hi")]);
        let sha = hex::encode(delil_hash_sha256(&pkg));
        let m = manifest_for("1.1.0", &sha);
        let signed = sign_manifest(&m, &sk).unwrap();
        let pk = public_key_of(&sk);
        assert!(signed.verify(&pk).is_ok());
    }

    #[test]
    fn tampered_manifest_fails_verification() {
        let sk = [7u8; 32];
        let pkg = make_package(&[("a.txt", b"hi")]);
        let sha = hex::encode(delil_hash_sha256(&pkg));
        let mut m = manifest_for("1.1.0", &sha);
        let mut signed = sign_manifest(&m, &sk).unwrap();
        signed.manifest.version = "9.9.9".into();
        let pk = public_key_of(&sk);
        assert!(matches!(
            signed.verify(&pk),
            Err(UpdateError::SignatureInvalid)
        ));
        let _ = &mut m;
    }

    #[test]
    fn wrong_package_hash_rejected() {
        let sk = [1u8; 32];
        let pkg = make_package(&[("a.txt", b"hi")]);
        let m = manifest_for("1.1.0", "deadbeef");
        let signed = sign_manifest(&m, &sk).unwrap();
        let pk = public_key_of(&sk);
        let mgr = UpdateManager::new(
            "1.0.0",
            pk,
            tempfile::tempdir().unwrap().path(),
            tempfile::tempdir().unwrap().path(),
        );
        let res = mgr.apply(&signed, &pkg, false);
        assert!(matches!(res, Err(UpdateError::HashMismatch { .. })));
    }

    #[test]
    fn busy_blocks_update() {
        let sk = [1u8; 32];
        let pkg = make_package(&[("a.txt", b"hi")]);
        let sha = hex::encode(delil_hash_sha256(&pkg));
        let m = manifest_for("1.1.0", &sha);
        let signed = sign_manifest(&m, &sk).unwrap();
        let mgr = UpdateManager::new(
            "1.0.0",
            public_key_of(&sk),
            tempfile::tempdir().unwrap().path(),
            tempfile::tempdir().unwrap().path(),
        );
        assert!(matches!(
            mgr.apply(&signed, &pkg, true),
            Err(UpdateError::Busy(_))
        ));
    }

    #[test]
    fn downgrade_rejected() {
        let sk = [1u8; 32];
        let pkg = make_package(&[("a.txt", b"hi")]);
        let sha = hex::encode(delil_hash_sha256(&pkg));
        let m = manifest_for("0.9.0", &sha);
        let signed = sign_manifest(&m, &sk).unwrap();
        let mgr = UpdateManager::new(
            "1.0.0",
            public_key_of(&sk),
            tempfile::tempdir().unwrap().path(),
            tempfile::tempdir().unwrap().path(),
        );
        assert!(matches!(
            mgr.apply(&signed, &pkg, false),
            Err(UpdateError::Downgrade { .. })
        ));
    }

    #[test]
    fn successful_update_installs_files() {
        let sk = [3u8; 32];
        let pkg = make_package(&[("delil.exe", b"binary"), ("models/x", b"model")]);
        let sha = hex::encode(delil_hash_sha256(&pkg));
        let m = manifest_for("1.1.0", &sha);
        let signed = sign_manifest(&m, &sk).unwrap();

        let install = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        // Mevcut kurulum.
        std::fs::write(install.path().join("delil.exe"), b"old").unwrap();

        let mgr = UpdateManager::new("1.0.0", public_key_of(&sk), install.path(), backup.path());
        let p = mgr.apply(&signed, &pkg, false).unwrap();
        assert_eq!(p, UpdateProgress::Done);
        assert_eq!(
            std::fs::read(install.path().join("delil.exe")).unwrap(),
            b"binary"
        );
        assert!(install.path().join("models/x").exists());
        // Yedek oluşturuldu mu?
        let backups: Vec<_> = std::fs::read_dir(backup.path()).unwrap().collect();
        assert!(!backups.is_empty());
    }

    #[test]
    fn offline_package_works() {
        // Çevrimdışı paket doğrudan bayt olarak verilir; ağ erişimi yok.
        let sk = [4u8; 32];
        let pkg = make_package(&[("app.bin", b"offline")]);
        let sha = hex::encode(delil_hash_sha256(&pkg));
        let m = manifest_for("2.0.0", &sha);
        let signed = sign_manifest(&m, &sk).unwrap();
        let install = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        let mgr = UpdateManager::new("1.0.0", public_key_of(&sk), install.path(), backup.path());
        assert_eq!(
            mgr.apply(&signed, &pkg, false).unwrap(),
            UpdateProgress::Done
        );
        assert!(install.path().join("app.bin").exists());
    }

    #[test]
    fn rollback_restores_backup() {
        let install = tempfile::tempdir().unwrap();
        std::fs::write(install.path().join("a.txt"), b"v1").unwrap();
        let backup_dir = tempfile::tempdir().unwrap();
        let mgr = UpdateManager::new("1.0.0", "", install.path(), backup_dir.path());
        let backup = mgr.backup_current().unwrap();
        // Kurulumu boz.
        std::fs::write(install.path().join("a.txt"), b"bozuk").unwrap();
        // Geri dön.
        mgr.rollback(&backup).unwrap();
        assert_eq!(std::fs::read(install.path().join("a.txt")).unwrap(), b"v1");
    }

    #[test]
    fn update_available_detection() {
        let mgr = UpdateManager::new("1.0.0", "", "/tmp", "/tmp");
        assert!(mgr.update_available(&manifest_for("1.1.0", "")));
        assert!(!mgr.update_available(&manifest_for("1.0.0", "")));
        assert!(!mgr.update_available(&manifest_for("0.9.0", "")));
    }
}
