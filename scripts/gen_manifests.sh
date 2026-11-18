#!/usr/bin/env bash
# Generates Cargo.toml for every workspace crate.
set -u
cd "$(dirname "$0")/../crates"

w() { mkdir -p "$(dirname "$1")"; cat > "$1"; }

w delil-core/Cargo.toml <<'EOF'
[package]
name = "delil-core"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "DELİL çekirdek alan modeli"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
EOF

w delil-hash/Cargo.toml <<'EOF'
[package]
name = "delil-hash"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "SHA-256 ve bütünlük doğrulama"

[dependencies]
sha2 = { workspace = true }
hex = { workspace = true }
thiserror = { workspace = true }
serde = { workspace = true }
delil-core = { workspace = true }
EOF

w delil-udf/Cargo.toml <<'EOF'
[package]
name = "delil-udf"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "UYAP UDF okuyucu, yazıcı ve doğrulayıcı"

[dependencies]
zip = { workspace = true }
quick-xml = { workspace = true }
base64 = { workspace = true }
thiserror = { workspace = true }
serde = { workspace = true }
delil-core = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
EOF

w delil-case-store/Cargo.toml <<'EOF'
[package]
name = "delil-case-store"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Şifreli yerel vaka deposu, yedekleme ve geri yükleme"

[dependencies]
aes-gcm = { workspace = true }
argon2 = { workspace = true }
rand = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
zip = { workspace = true }
sha2 = { workspace = true }
hex = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
delil-core = { workspace = true }
delil-hash = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
EOF

w delil-audio/Cargo.toml <<'EOF'
[package]
name = "delil-audio"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Ses biçimi algılama, çözme ve teknik envanter"

[dependencies]
thiserror = { workspace = true }
serde = { workspace = true }
delil-core = { workspace = true }

[dependencies.symphonia]
version = "0.5"
optional = true
features = ["all"]

[features]
default = []
decode = ["dep:symphonia"]

[dev-dependencies]
tempfile = { workspace = true }
EOF

w delil-evidence/Cargo.toml <<'EOF'
[package]
name = "delil-evidence"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Aygıt algılama, dosya bazlı edinim ve bütünlük doğrulama"

[dependencies]
walkdir = { workspace = true }
sha2 = { workspace = true }
hex = { workspace = true }
thiserror = { workspace = true }
serde = { workspace = true }
chrono = { workspace = true }
delil-core = { workspace = true }
delil-hash = { workspace = true }
delil-audio = { workspace = true }

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = ["Win32_Storage_FileSystem", "Win32_System_IO", "Win32_Foundation", "Win32_Devices_DeviceAndDriverInstallation", "Win32_UI_Shell"] }

[dev-dependencies]
tempfile = { workspace = true }
EOF

w delil-assignment/Cargo.toml <<'EOF'
[package]
name = "delil-assignment"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Görevlendirme belgesi okuma ve bilgi çıkarma"

[dependencies]
zip = { workspace = true }
quick-xml = { workspace = true }
thiserror = { workspace = true }
serde = { workspace = true }
chrono = { workspace = true }
delil-core = { workspace = true }
delil-udf = { workspace = true }

[features]
default = []
ocr = ["dep:ocrs"]

[dependencies.ocrs]
version = "0.9"
optional = true
default-features = false

[dev-dependencies]
tempfile = { workspace = true }
EOF

w delil-transcription/Cargo.toml <<'EOF'
[package]
name = "delil-transcription"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Yerel Whisper transkripsiyon motoru ve kuyruk"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
delil-core = { workspace = true }

[dependencies.candle-core]
version = "0.8"
optional = true
[dependencies.candle-nn]
version = "0.8"
optional = true
[dependencies.candle-transformers]
version = "0.8"
optional = true
[dependencies.tokenizers]
version = "0.21"
optional = true

[features]
default = []
candle = ["dep:candle-core", "dep:candle-nn", "dep:candle-transformers", "dep:tokenizers"]

[dev-dependencies]
tempfile = { workspace = true }
EOF

w delil-quality/Cargo.toml <<'EOF'
[package]
name = "delil-quality"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Otomatik transkripsiyon kalite kontrolü"

[dependencies]
serde = { workspace = true }
thiserror = { workspace = true }
delil-core = { workspace = true }
EOF

w delil-report/Cargo.toml <<'EOF'
[package]
name = "delil-report"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Bilirkişi rapor modeli ve otomatik üretimi"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
chrono = { workspace = true }
delil-core = { workspace = true }
EOF

w delil-export/Cargo.toml <<'EOF'
[package]
name = "delil-export"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "PDF, TXT ve JSON yardımcı çıktılar"

[dependencies]
printpdf = { version = "0.7", default-features = false }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
delil-core = { workspace = true }
delil-report = { workspace = true }
delil-udf = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
EOF

w delil-update/Cargo.toml <<'EOF'
[package]
name = "delil-update"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "İmzalı otomatik güncelleme yöneticisi"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
sha2 = { workspace = true }
hex = { workspace = true }
ed25519-dalek = { version = "2", features = ["rand_core"] }
base64 = { workspace = true }
thiserror = { workspace = true }
zip = { workspace = true }
tempfile = { workspace = true }
delil-core = { workspace = true }

[dev-dependencies]
rand = { workspace = true }
EOF

w delil-pipeline/Cargo.toml <<'EOF'
[package]
name = "delil-pipeline"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Uçtan uca otomatik iş akışı orkestrasyonu"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
delil-core = { workspace = true }
delil-hash = { workspace = true }
delil-case-store = { workspace = true }
delil-evidence = { workspace = true }
delil-audio = { workspace = true }
delil-assignment = { workspace = true }
delil-transcription = { workspace = true }
delil-quality = { workspace = true }
delil-report = { workspace = true }
delil-udf = { workspace = true }
delil-export = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
EOF

w delil-cli/Cargo.toml <<'EOF'
[package]
name = "delil-cli"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "DELİL komut satırı arayüzü"

[dependencies]
clap = { version = "4", features = ["derive"] }
anyhow = { workspace = true }
serde_json = { workspace = true }
delil-core = { workspace = true }
delil-pipeline = { workspace = true }
delil-case-store = { workspace = true }
delil-report = { workspace = true }
delil-udf = { workspace = true }

[[bin]]
name = "delil"
path = "src/main.rs"
EOF

w delil-app/Cargo.toml <<'EOF'
[package]
name = "delil-app"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "DELİL Windows masaüstü arayüzü"

[dependencies]
eframe = { version = "0.28", default-features = false, features = ["default_fonts", "glow", "wayland", "x11"] }
egui = "0.28"
anyhow = { workspace = true }
serde_json = { workspace = true }
delil-core = { workspace = true }
delil-pipeline = { workspace = true }
delil-case-store = { workspace = true }

[target.'cfg(windows)'.build-dependencies]
winresource = "0.1"
EOF

echo "manifests written: $(ls */Cargo.toml | wc -l)"
