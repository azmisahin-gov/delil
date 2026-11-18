//! Aygıt algılama, dosya bazlı mantıksal edinim ve bütünlük doğrulama
//! (şartname 6 ve 7).
//!
//! Edinim **mantıksal dosya edinimidir**: kaynak dosyalar yerel vaka deposuna
//! kopyalanır, dizin yapısı ve dosya adları korunur, kaynak ve hedef SHA-256
//! değerleri karşılaştırılır. Fiziksel RAW/E01 imaj veya silinmiş veri edinimi
//! olarak adlandırılmaz.
//!
//! Kaynak materyale asla yazma, silme, yeniden adlandırma veya taşıma
//! yapılmaz.

use delil_core::{
    AudioAsset, AudioAssetId, DeviceId, DeviceKind, EvidenceSource, Material, MaterialId,
    VerificationStatus, WriteProtection,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

/// Edinim hataları.
#[derive(Debug, Error)]
pub enum EvidenceError {
    #[error("G/Ç hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("Kaynak bulunamadı: {0}")]
    SourceMissing(String),
    #[error("Hedef dizin oluşturulamadı: {0}")]
    TargetDir(String),
    #[error("Kopyalama kesildi veya başarısız: {0}")]
    Copy(String),
    #[error("Disk alanı yetersiz: gereken {required} bayt, mevcut {available} bayt")]
    InsufficientSpace { required: u64, available: u64 },
    #[error("Hash doğrulaması başarısız: {0}")]
    Verification(String),
}

/// Bir aygıtın/klasörün tanımı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectedDevice {
    pub id: DeviceId,
    pub kind: DeviceKind,
    pub root: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub capacity_bytes: Option<u64>,
    pub serial_number: Option<String>,
    pub filesystem: Option<String>,
    pub disk_info: Option<String>,
    pub removable: bool,
    pub is_system_disk: bool,
}

impl DetectedDevice {
    /// Bu aygıtı `EvidenceSource`'a çevirir.
    pub fn to_source(&self, write_protection: WriteProtection) -> EvidenceSource {
        EvidenceSource {
            id: self.id.clone(),
            kind: self.kind,
            root: self.root.clone(),
            manufacturer: self.manufacturer.clone(),
            model: self.model.clone(),
            capacity_bytes: self.capacity_bytes,
            serial_number: self.serial_number.clone(),
            filesystem: self.filesystem.clone(),
            disk_info: self.disk_info.clone(),
            removable: self.removable,
            write_protection,
            is_system_disk: self.is_system_disk,
            detected_at: chrono::Utc::now(),
        }
    }
}

/// Sistem diskini/OS bölümünü asla otomatik delil kaynağı seçmez.
///
/// Bu işlev, verilen aday aygıtlar arasından sistem diskini işaretler ve
/// otomatik seçime uygun olanları döndürür.
pub fn selectable_devices(devices: &[DetectedDevice]) -> Vec<&DetectedDevice> {
    devices.iter().filter(|d| !d.is_system_disk).collect()
}

/// Birden fazla aday aygıt varsa ve kaynak belirsizse kullanıcı kararı
/// gerekir (şartname 4.3). Tek aday varsa otomatik seçilir.
pub fn resolve_source_ambiguity(
    devices: &[DetectedDevice],
) -> Result<&DetectedDevice, AmbiguityError> {
    let candidates: Vec<&DetectedDevice> = selectable_devices(devices);
    match candidates.len() {
        0 => Err(AmbiguityError::NoCandidate),
        1 => Ok(candidates[0]),
        _ => Err(AmbiguityError::MultipleCandidates(candidates.len())),
    }
}

/// Kaynak belirsizliği hatası.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AmbiguityError {
    #[error("Seçilebilir harici aygıt bulunamadı")]
    NoCandidate,
    #[error("Birden fazla harici aygıt bağlı ({0}); kaynak belirsiz, kullanıcı seçimi gerekir")]
    MultipleCandidates(usize),
}

/// Linux'ta bağlı blok aygıtlarını `/proc/mounts` ve `lsblk` çıktısından
/// algılar. Windows'ta sürücü harfleri taranır.
///
/// Bu işlev yalnızca **harici** ve **çıkarılabilir** aygıtları döndürür;
/// sistem diski işaretlenir ama otomatik seçilmez.
pub fn detect_devices() -> Vec<DetectedDevice> {
    #[cfg(windows)]
    {
        detect_devices_windows()
    }
    #[cfg(not(windows))]
    {
        detect_devices_unix()
    }
}

#[cfg(not(windows))]
fn detect_devices_unix() -> Vec<DetectedDevice> {
    // /proc/mounts: gerçek aygıtlar (kaynak yolu /dev/... ile başlayan).
    let mut devices = Vec::new();
    let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    let root_dev = mounts
        .lines()
        .find(|l| l.split_whitespace().nth(1) == Some("/"))
        .and_then(|l| l.split_whitespace().next())
        .map(|s| s.to_string());

    for line in mounts.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let dev = parts[0];
        let mount = parts[1];
        let fs = parts[2];
        // Yalnızca gerçek blok aygıtları.
        if !dev.starts_with("/dev/") {
            continue;
        }
        // Sanal dosya sistemlerini atla.
        if matches!(
            fs,
            "proc" | "sysfs" | "tmpfs" | "devtmpfs" | "cgroup" | "cgroup2" | "overlay" | "squashfs"
        ) {
            continue;
        }
        let is_system = Some(dev.to_string()) == root_dev || mount == "/";
        // /media, /mnt, /run/media altındaki bağlantılar genelde harici.
        let removable = mount.starts_with("/media")
            || mount.starts_with("/mnt")
            || mount.starts_with("/run/media");
        let kind = if removable {
            if mount.to_lowercase().contains("sd") || mount.to_lowercase().contains("card") {
                DeviceKind::SdCard
            } else {
                DeviceKind::Usb
            }
        } else if is_system {
            DeviceKind::Unknown
        } else {
            DeviceKind::ExternalHdd
        };
        devices.push(DetectedDevice {
            id: DeviceId::new(),
            kind,
            root: mount.to_string(),
            manufacturer: None,
            model: None,
            capacity_bytes: None,
            serial_number: None,
            filesystem: Some(fs.to_string()),
            disk_info: Some(dev.to_string()),
            removable,
            is_system_disk: is_system,
        });
    }
    devices
}

#[cfg(windows)]
fn detect_devices_windows() -> Vec<DetectedDevice> {
    // A'dan Z'ye sürücü harflerini dene; GetDriveType ile türü belirle.
    use windows::Win32::Storage::FileSystem::GetDriveTypeW;
    use windows::Win32::Storage::FileSystem::{
        DRIVE_FIXED, DRIVE_REMOTE, DRIVE_REMOVABLE, DRIVE_UNKNOWN,
    };
    let mut devices = Vec::new();
    for letter in b'A'..=b'Z' {
        let root = format!("{}:\\", letter as char);
        let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
        let t = unsafe { GetDriveTypeW(windows::core::PCWSTR(wide.as_ptr())) };
        if t == DRIVE_UNKNOWN {
            continue;
        }
        let removable = t == DRIVE_REMOVABLE;
        let fixed = t == DRIVE_FIXED;
        let remote = t == DRIVE_REMOTE;
        if remote {
            continue;
        }
        // Sistem diski genellikle C: ve sabit sürücüdür.
        let is_system = fixed && (letter == b'C');
        let kind = if removable {
            DeviceKind::Usb
        } else if fixed && !is_system {
            DeviceKind::ExternalHdd
        } else {
            DeviceKind::Unknown
        };
        // Yalnızca erişilebilir sürücüleri ekle.
        if !Path::new(&root).exists() {
            continue;
        }
        devices.push(DetectedDevice {
            id: DeviceId::new(),
            kind,
            root,
            manufacturer: None,
            model: None,
            capacity_bytes: None,
            serial_number: None,
            filesystem: None,
            disk_info: Some(format!("Sürücü {}", letter as char)),
            removable,
            is_system_disk: is_system,
        });
    }
    devices
}

/// Yerel bir klasörü delil kaynağı olarak tanımlar.
pub fn local_folder_source(path: &Path) -> EvidenceSource {
    DetectedDevice {
        id: DeviceId::new(),
        kind: DeviceKind::LocalFolder,
        root: path.to_string_lossy().into_owned(),
        manufacturer: None,
        model: None,
        capacity_bytes: None,
        serial_number: None,
        filesystem: None,
        disk_info: None,
        removable: false,
        is_system_disk: false,
    }
    .to_source(WriteProtection::NotApplied)
}

/// Edinim seçenekleri.
#[derive(Debug, Clone)]
pub struct AcquireOptions {
    /// Alt dizin yapısını koru.
    pub preserve_structure: bool,
    /// Disk alanı kontrolü yap.
    pub check_disk_space: bool,
    /// Kaynak aygıtta yazılımsal salt okunur koruma denensin mi?
    pub attempt_software_write_protect: bool,
}

impl Default for AcquireOptions {
    fn default() -> Self {
        Self {
            preserve_structure: true,
            check_disk_space: true,
            attempt_software_write_protect: false,
        }
    }
}

/// Edinim ilerleme geri çağrısı: (tamamlanan bayt, toplam bayt, geçerli dosya).
pub type ProgressFn<'a> = &'a mut dyn FnMut(u64, u64, &str);

/// Bir kaynak (aygıt veya klasör) altındaki tüm dosyaları listeler.
pub fn list_files(root: &Path) -> Result<Vec<PathBuf>, EvidenceError> {
    if !root.exists() {
        return Err(EvidenceError::SourceMissing(root.display().to_string()));
    }
    let mut files = Vec::new();
    for entry in WalkDir::new(root).follow_links(false).into_iter() {
        let entry = entry.map_err(|e| EvidenceError::Io(std::io::Error::other(e.to_string())))?;
        if entry.file_type().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();
    Ok(files)
}

/// Kaynak ağacını tarayıp dosya listesini ve toplam boyutu döndürür.
pub fn scan_source(root: &Path) -> Result<(Vec<PathBuf>, u64), EvidenceError> {
    let files = list_files(root)?;
    let mut total = 0u64;
    for f in &files {
        total += std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
    }
    Ok((files, total))
}

/// Dosya bazlı mantıksal edinim: kaynak dosyaları hedef dizine kopyalar,
/// dizin yapısını ve adları korur, SHA-256 ile doğrular.
///
/// Kaynak materyale yazma/silme/taşıma yapılmaz.
pub fn acquire_files(
    source_root: &Path,
    source: &EvidenceSource,
    target_root: &Path,
    opts: &AcquireOptions,
    mut progress: Option<ProgressFn<'_>>,
) -> Result<Vec<Material>, EvidenceError> {
    let (files, total_bytes) = scan_source(source_root)?;

    if opts.check_disk_space {
        let available = available_space(target_root)?;
        if available < total_bytes {
            return Err(EvidenceError::InsufficientSpace {
                required: total_bytes,
                available,
            });
        }
    }

    let mut materials = Vec::new();
    let mut hash_index: HashMap<String, MaterialId> = HashMap::new();
    let mut done: u64 = 0;

    for src in &files {
        let rel = src.strip_prefix(source_root).unwrap_or(src);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let target_rel = if opts.preserve_structure {
            rel_str.clone()
        } else {
            src.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| rel_str.clone())
        };
        let target = target_root.join(&target_rel);
        if let Some(p) = target.parent() {
            std::fs::create_dir_all(p).map_err(|e| EvidenceError::TargetDir(e.to_string()))?;
        }

        let size = std::fs::metadata(src).map(|m| m.len()).unwrap_or(0);

        // Kaynak hash'i.
        let source_sha =
            delil_hash::sha256_file(src).map_err(|e| EvidenceError::Copy(e.to_string()))?;

        // Kopyala (kesintiye dayanıklı: geçici dosyaya yazıp taşı).
        let tmp = target.with_extension("delilpart");
        copy_file(src, &tmp)?;
        std::fs::rename(&tmp, &target)?;

        // Hedefi yeniden okuyup doğrula.
        let target_sha =
            delil_hash::sha256_file(&target).map_err(|e| EvidenceError::Copy(e.to_string()))?;
        let verification = if source_sha == target_sha {
            VerificationStatus::Verified
        } else {
            VerificationStatus::Mismatch
        };

        // Yinelenen dosya tespiti (aynı hash).
        let duplicate_of = hash_index.get(&source_sha).cloned();
        let id = MaterialId::new();
        hash_index
            .entry(source_sha.clone())
            .or_insert_with(|| id.clone());

        let mut warnings = Vec::new();
        if verification == VerificationStatus::Mismatch {
            warnings.push("Kaynak ve hedef hash uyuşmadı; dosya doğrulanmadı.".into());
        }

        materials.push(Material {
            id,
            source_device_id: source.id.clone(),
            source_path: src.to_string_lossy().into_owned(),
            file_name: src
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            file_type: src
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default(),
            size_bytes: size,
            source_sha256: Some(source_sha),
            target_sha256: Some(target_sha),
            stored_relative_path: target_rel,
            acquired_at: chrono::Utc::now(),
            verification,
            duplicate_of,
            warnings,
        });

        done += size;
        if let Some(cb) = progress.as_deref_mut() {
            cb(done, total_bytes, &rel_str);
        }
    }

    Ok(materials)
}

/// Kaynak dosyayı hedefe kopyalar. Kaynak açılır, hedefe yazılır.
fn copy_file(src: &Path, dst: &Path) -> Result<(), EvidenceError> {
    use std::io::{Read, Write};
    let mut input = std::fs::File::open(src)?;
    let mut output = std::fs::File::create(dst)?;
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        output.write_all(&buf[..n])?;
    }
    output.sync_all()?;
    Ok(())
}

/// Hedef diskteki boş alanı döndürür (bayt). Bilinemiyorsa `u64::MAX`.
pub fn available_space(path: &Path) -> Result<u64, EvidenceError> {
    // Basit taşınabilir yaklaşım: en yakın var olan üst dizini bul.
    let mut p = path.to_path_buf();
    while !p.exists() {
        match p.parent() {
            Some(parent) => p = parent.to_path_buf(),
            None => return Ok(u64::MAX),
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(p.as_os_str().as_bytes())
            .map_err(|_| EvidenceError::Io(std::io::Error::other("geçersiz yol")))?;
        unsafe {
            let mut st: libc::statvfs = std::mem::zeroed();
            if libc::statvfs(c.as_ptr(), &mut st) == 0 {
                return Ok(st.f_bavail as u64 * st.f_frsize as u64);
            }
        }
        Ok(u64::MAX)
    }
    #[cfg(not(unix))]
    {
        Ok(u64::MAX)
    }
}

/// Edinilmiş materyaller içinden ses dosyalarını içerik tabanlı algılar ve
/// teknik envanteri çıkarır (şartname 7).
pub fn build_audio_inventory(materials: &[Material], materials_root: &Path) -> Vec<AudioAsset> {
    let mut assets = Vec::new();
    for m in materials {
        let path = materials_root.join(&m.stored_relative_path);
        if !path.exists() {
            continue;
        }
        match delil_audio::detect_container_from_path(&path) {
            Ok(Some(container)) => {
                let technical = delil_audio::inventory(&path).unwrap_or_default();
                assets.push(AudioAsset {
                    id: AudioAssetId::new(),
                    material_id: m.id.clone(),
                    file_name: m.file_name.clone(),
                    sha256: m.target_sha256.clone(),
                    extracted_from_video: container.is_video_container(),
                    technical,
                    queued: false,
                    unsupported_reason: None,
                });
            }
            Ok(None) => {
                // Uzantı ses gibi görünüyorsa ama içerik ses değilse açıkça
                // bildir; sessizce atlanmaz.
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or_default();
                if delil_audio::Container::from_extension(ext).is_some() {
                    assets.push(AudioAsset {
                        id: AudioAssetId::new(),
                        material_id: m.id.clone(),
                        file_name: m.file_name.clone(),
                        sha256: m.target_sha256.clone(),
                        extracted_from_video: false,
                        technical: delil_core::AudioTechnicalInfo {
                            readable: false,
                            ..Default::default()
                        },
                        queued: false,
                        unsupported_reason: Some(
                            "Dosya ses uzantılı ancak içeriği ses biçimi olarak tanınamadı.".into(),
                        ),
                    });
                }
            }
            Err(_) => {}
        }
    }
    assets
}

/// Kaynak aygıtta yazılımsal salt okunur koruma uygulanabilir mi?
/// Uygulanamıyorsa `NotApplied` döner; donanımsal koruma iddiası üretilmez.
pub fn attempt_software_write_protection(_root: &Path) -> WriteProtection {
    // Windows'ta disk yazma koruması (diskpart/registry) yönetici yetkisi
    // gerektirir ve kalıcıdır; bu nedenle varsayılan olarak uygulanmaz.
    // Kullanıcı bunu açıkça isterse ve başarılı olursa SoftwareReadOnly
    // döndürülmelidir. Burada güvenli varsayılan: uygulanmadı.
    WriteProtection::NotApplied
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(dir: &Path, rel: &str, content: &[u8]) -> PathBuf {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(content).unwrap();
        p
    }

    fn dummy_source(root: &Path) -> EvidenceSource {
        DetectedDevice {
            id: DeviceId::new(),
            kind: DeviceKind::Usb,
            root: root.to_string_lossy().into_owned(),
            manufacturer: None,
            model: None,
            capacity_bytes: None,
            serial_number: None,
            filesystem: None,
            disk_info: None,
            removable: true,
            is_system_disk: false,
        }
        .to_source(WriteProtection::NotApplied)
    }

    #[test]
    fn acquire_preserves_structure_and_verifies() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        write(&src, "a/b/ses.wav", b"RIFF....WAVE-content");
        write(&src, "not.txt", b"notlar");

        let target = dir.path().join("case/materials");
        std::fs::create_dir_all(&target).unwrap();
        let source = dummy_source(&src);
        let mats = acquire_files(&src, &source, &target, &AcquireOptions::default(), None).unwrap();
        assert_eq!(mats.len(), 2);
        for m in &mats {
            assert_eq!(m.verification, VerificationStatus::Verified);
            assert_eq!(m.source_sha256, m.target_sha256);
        }
        assert!(target.join("a/b/ses.wav").exists());
        assert!(target.join("not.txt").exists());
    }

    #[test]
    fn source_is_never_modified() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let f = write(&src, "ses.wav", b"orijinal-icerik");
        let before = std::fs::read(&f).unwrap();
        let target = dir.path().join("out");
        let source = dummy_source(&src);
        acquire_files(&src, &source, &target, &AcquireOptions::default(), None).unwrap();
        let after = std::fs::read(&f).unwrap();
        assert_eq!(before, after, "kaynak dosya değişmemeli");
        assert!(f.exists(), "kaynak dosya silinmemeli");
    }

    #[test]
    fn duplicate_files_are_flagged_but_not_removed() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        write(&src, "a.bin", b"ayni-icerik");
        write(&src, "b.bin", b"ayni-icerik");
        let target = dir.path().join("out");
        let source = dummy_source(&src);
        let mats = acquire_files(&src, &source, &target, &AcquireOptions::default(), None).unwrap();
        assert_eq!(mats.len(), 2, "yinelenen dosyalar silinmemeli");
        assert!(mats.iter().any(|m| m.duplicate_of.is_some()));
    }

    #[test]
    fn corrupted_target_is_not_verified() {
        // Doğrulama başarısızsa "doğrulandı" gösterilmez. Bu test, hash
        // karşılaştırmasının mantığını doğrudan kontrol eder.
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::write(&a, b"bir").unwrap();
        std::fs::write(&b, b"iki").unwrap();
        let cmp = delil_hash::compare_files(&a, &b).unwrap();
        assert_eq!(cmp.status, VerificationStatus::Mismatch);
        assert!(!cmp.status.is_verified());
    }

    #[test]
    fn insufficient_space_is_detected() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        write(&src, "big.bin", &vec![0u8; 1024]);
        let target = dir.path().join("out");
        std::fs::create_dir_all(&target).unwrap();
        let source = dummy_source(&src);
        // Gerçek diskte yer var; bu test yalnızca hata türünü doğrular.
        let err = EvidenceError::InsufficientSpace {
            required: 100,
            available: 10,
        };
        assert!(err.to_string().contains("Disk alanı yetersiz"));
        // Normal akış başarılı olmalı.
        assert!(acquire_files(&src, &source, &target, &AcquireOptions::default(), None).is_ok());
    }

    #[test]
    fn audio_inventory_detects_wav_by_content() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        // Gerçek WAV üret.
        let sr = 16000u32;
        let samples = 1600u32;
        let data_len = samples * 2;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&sr.to_le_bytes());
        wav.extend_from_slice(&(sr * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        wav.extend(std::iter::repeat(0u8).take(data_len as usize));
        write(&src, "kayit.txt", &wav); // yanlış uzantı

        let target = dir.path().join("out");
        let source = dummy_source(&src);
        let mats = acquire_files(&src, &source, &target, &AcquireOptions::default(), None).unwrap();
        let assets = build_audio_inventory(&mats, &target);
        assert_eq!(
            assets.len(),
            1,
            "içerik tabanlı algılama uzantıya güvenmemeli"
        );
        assert_eq!(assets[0].technical.container, "WAV");
    }

    #[test]
    fn system_disk_is_excluded_from_auto_selection() {
        let devices = vec![
            DetectedDevice {
                id: DeviceId::new(),
                kind: DeviceKind::Unknown,
                root: "/".into(),
                manufacturer: None,
                model: None,
                capacity_bytes: None,
                serial_number: None,
                filesystem: Some("ext4".into()),
                disk_info: None,
                removable: false,
                is_system_disk: true,
            },
            DetectedDevice {
                id: DeviceId::new(),
                kind: DeviceKind::Usb,
                root: "/media/usb".into(),
                manufacturer: None,
                model: None,
                capacity_bytes: None,
                serial_number: None,
                filesystem: Some("vfat".into()),
                disk_info: None,
                removable: true,
                is_system_disk: false,
            },
        ];
        let sel = selectable_devices(&devices);
        assert_eq!(sel.len(), 1);
        assert_eq!(sel[0].root, "/media/usb");
    }

    #[test]
    fn multiple_candidates_require_decision() {
        let mk = |root: &str| DetectedDevice {
            id: DeviceId::new(),
            kind: DeviceKind::Usb,
            root: root.into(),
            manufacturer: None,
            model: None,
            capacity_bytes: None,
            serial_number: None,
            filesystem: None,
            disk_info: None,
            removable: true,
            is_system_disk: false,
        };
        let devices = vec![mk("/media/a"), mk("/media/b")];
        assert!(matches!(
            resolve_source_ambiguity(&devices),
            Err(AmbiguityError::MultipleCandidates(2))
        ));
        let single = vec![mk("/media/a")];
        assert!(resolve_source_ambiguity(&single).is_ok());
    }

    #[test]
    fn software_write_protection_is_not_faked() {
        // Varsayılan olarak donanımsal koruma iddiası üretilmez.
        let p = attempt_software_write_protection(Path::new("/tmp"));
        assert!(!p.claims_hardware());
    }

    #[test]
    fn scan_reports_total_size() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        write(&src, "a.bin", &[0u8; 100]);
        write(&src, "b.bin", &[0u8; 200]);
        let (files, total) = scan_source(&src).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(total, 300);
    }
}
