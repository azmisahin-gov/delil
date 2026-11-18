#!/usr/bin/env bash
# DELİL Windows 11 x64 sürüm derlemesi ve paketleme betiği.
#
# Bu betik Windows'ta (PowerShell yerine Git Bash/WSL) veya çapraz derleme
# ortamında çalıştırılabilir. Son kullanıcıya teslim edilecek paketi üretir:
#   dist/
#     delil.exe                     (masaüstü arayüzü)
#     delil-cli.exe                 (komut satırı)
#     models/                       (Whisper + OCR modelleri)
#     LICENSE, README.md, docs/
#     DELIL-Setup-<sürüm>-x64.exe   (kurulum dosyası; Inno Setup ile)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
VERSION="$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')"
DIST="$ROOT/dist"
TARGET="${TARGET:-x86_64-pc-windows-msvc}"

echo "==> DELİL $VERSION — hedef: $TARGET"

# 1. Derleme (tüm ağır özellikler açık).
echo "==> Derleniyor (candle + decode + ocr)…"
cargo build --release \
  --target "$TARGET" \
  -p delil-app \
  -p delil-cli

# Not: OCR özelliği delil-assignment'ta; arayüz üzerinden derlenirken
# ilgili özellikleri etkinleştirmek için aşağıdaki satırı kullanın:
#   cargo build --release --target "$TARGET" -p delil-assignment --features ocr

# 2. Paket dizinini hazırla.
echo "==> Paket dizini hazırlanıyor…"
rm -rf "$DIST"
mkdir -p "$DIST/models/ocr"
mkdir -p "$DIST/docs"

cp "$ROOT/target/$TARGET/release/delil-app.exe" "$DIST/delil.exe"
cp "$ROOT/target/$TARGET/release/delil.exe" "$DIST/delil-cli.exe"

# 3. Belgeler.
cp LICENSE README.md "$DIST/"
cp docs/*.md "$DIST/docs/" 2>/dev/null || true
cp docs/SBOM.json "$DIST/docs/" 2>/dev/null || true

# 4. Model bileşenleri (kurulum paketine dahil edilir).
#    Gerçek dağıtımda bu dosyalar sürüm kontrolü dışında tutulur ve paketleme
#    sırasında bir model deposundan kopyalanır.
if [ -d "$ROOT/models" ]; then
  cp -r "$ROOT/models/." "$DIST/models/" 2>/dev/null || true
else
  echo "UYARI: $ROOT/models bulunamadı. Whisper ve OCR model dosyaları paket dışı kaldı."
  echo "       Kurulum paketi model içermelidir; kullanıcıdan indirme istenmez."
fi

# 5. Kurulum dosyası (Inno Setup; Windows'ta iscc gerekir).
if command -v iscc >/dev/null 2>&1; then
  echo "==> Inno Setup ile kurulum dosyası üretiliyor…"
  iscc /DMyAppVersion="$VERSION" /DMyDistDir="$DIST" \
    "$ROOT/packaging/windows/delil.iss"
  echo "==> Kurulum dosyası: dist/DELIL-Setup-$VERSION-x64.exe"
else
  echo "NOT: Inno Setup (iscc) bulunamadı; kurulum dosyası üretilemedi."
  echo "     Windows'ta: winget install JRSoftware.InnoSetup"
  echo "     Ardından: iscc /DMyAppVersion=$VERSION /DMyDistDir=$DIST packaging/windows/delil.iss"
fi

echo "==> Tamamlandı. Paket: $DIST"
ls -la "$DIST"
