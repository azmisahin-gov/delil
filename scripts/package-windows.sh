#!/usr/bin/env bash
# DELİL Windows 11 x64 üretim derlemesi ve paketleme betiği.
#
# Bu betik KATI çalışır: eksik model, yanlış hash, kapalı üretim özelliği veya
# eksik yürütülebilir dosya durumunda **paketleme durdurulur** ve hata döner.
# Yalnızca uyarı yazıp eksik paket üretmez.
#
# Ön koşullar:
#   * Windows x64 hedefi derlenebilir olmalı (windows-latest veya çapraz derleme).
#   * `scripts/fetch-models.sh` çalıştırılmış olmalı (models/ dolu).
#   * Inno Setup (iscc) kurulu olmalı (kurulum EXE'si için).
#
# Çıktı:
#   dist/DELIL-Setup-<sürüm>-x64.exe
#   dist/DELIL-Setup-<sürüm>-x64.exe.sha256
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')"
DIST="$ROOT/dist"
TARGET="${TARGET:-x86_64-pc-windows-msvc}"
APP_EXE="$ROOT/target/$TARGET/release/delil-app.exe"
CLI_EXE="$ROOT/target/$TARGET/release/delil.exe"

fail() { echo "HATA: $*" >&2; exit 1; }

# Taşınabilir Python: Windows Git Bash'te "python", Linux'da "python3".
if command -v python3 >/dev/null 2>&1; then PYTHON=python3; elif command -v python >/dev/null 2>&1; then PYTHON=python; else fail "Python bulunamadı (python3/python)."; fi
step() { echo; echo "==> $*"; }

step "DELİL $VERSION — hedef: $TARGET"

# ---------------------------------------------------------------- 1. Ön koşullar
step "1/8 Ön koşullar kontrol ediliyor"

if ! rustup target list --installed 2>/dev/null | grep -q "^$TARGET$"; then
  fail "Hedef '$TARGET' kurulu değil. Çalıştırın: rustup target add $TARGET"
fi

MODELS="$ROOT/models"
for f in config.json tokenizer.json model.safetensors; do
  [ -f "$MODELS/$f" ] || fail "Whisper model dosyası eksik: models/$f (scripts/fetch-models.sh çalıştırın)"
done
for f in text-detection.rten text-recognition.rten; do
  [ -f "$MODELS/ocr/$f" ] || fail "OCR model dosyası eksik: models/ocr/$f (scripts/fetch-models.sh çalıştırın)"
done
[ -f "$MODELS/MODEL_MANIFEST.json" ] || fail "Model manifesti eksik: models/MODEL_MANIFEST.json"

step "1b Model bütünlüğü doğrulanıyor (SHA-256)"
"$PYTHON" - "$MODELS" <<'PY'
import hashlib, json, os, sys
models = sys.argv[1]
manifest = json.load(open(os.path.join(models, "MODEL_MANIFEST.json")))
bad = 0
for e in manifest["components"]:
    p = os.path.join(models, e["path"])
    if not os.path.exists(p):
        print(f"  EKSİK: {e['path']}"); bad += 1; continue
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    got = h.hexdigest()
    if got != e["sha256"]:
        print(f"  HASH UYUŞMADI: {e['path']}")
        print(f"    beklenen {e['sha256']}")
        print(f"    bulunan  {got}")
        bad += 1
    else:
        print(f"  OK: {e['path']}")
if bad:
    sys.exit(4)
PY
echo "  Model bütünlüğü doğrulandı."

ISS="$ROOT/packaging/windows/delil.iss"
[ -f "$ISS" ] || fail "Inno Setup betiği bulunamadı: $ISS"
GUID="$(grep -m1 '^AppId=' "$ISS" | grep -oE '[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}')"
if ! echo "$GUID" | grep -Eq '^[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}$'; then
  fail "Geçersiz veya eksik AppId GUID: '$GUID' (beklenen 8-4-4-4-12 onaltılık biçim)"
fi
echo "  Inno Setup GUID geçerli: $GUID"

# ---------------------------------------------------------------- 2. Üretim derlemesi
step "2/8 Üretim derlemesi (production: candle + decode + ocr)"
# `delil-app` üzerindeki `production` özelliği şunları etkinleştirir:
#   delil-pipeline/candle     -> gerçek Candle Whisper motoru
#   delil-transcription/full  -> candle + ses çözme (symphonia)
#   delil-assignment/ocr      -> yerel OCR
cargo build --release --target "$TARGET" -p delil-app --features production
cargo build --release --target "$TARGET" -p delil-cli

step "2b Üretim özellikleri ikili dosyada doğrulanıyor"
[ -f "$APP_EXE" ] || fail "Masaüstü uygulaması üretilmedi: $APP_EXE"
[ -f "$CLI_EXE" ] || fail "Komut satırı uygulaması üretilmedi: $CLI_EXE"

APP_SIZE=$(stat -c%s "$APP_EXE" 2>/dev/null || stat -f%z "$APP_EXE")
if [ "$APP_SIZE" -lt 20000000 ]; then
  fail "Uygulama beklenenden küçük ($APP_SIZE bayt); production özellikleri derlemeye girmemiş olabilir."
fi
echo "  delil-app.exe boyutu: $APP_SIZE bayt"

STRINGS_BIN="$(command -v strings || true)"
if [ -n "$STRINGS_BIN" ]; then
  if ! "$STRINGS_BIN" -a "$APP_EXE" 2>/dev/null | grep -qiE 'whisper|candle|safetensors'; then
    fail "Whisper/Candle sembolleri ikilide bulunamadı; 'production' özelliği etkin değil."
  fi
  if ! "$STRINGS_BIN" -a "$APP_EXE" 2>/dev/null | grep -qiE 'ocrs|rten|text-detection'; then
    fail "OCR sembolleri ikilide bulunamadı; 'ocr' özelliği etkin değil."
  fi
  echo "  Whisper/Candle ve OCR sembolleri doğrulandı."
else
  echo "  UYARI: 'strings' yok; sembol doğrulaması atlandı (boyut kontrolü yapıldı)."
fi

# ---------------------------------------------------------------- 3. Paket dizini
step "3/8 Paket dizini hazırlanıyor"
rm -rf "$DIST"
mkdir -p "$DIST/models/ocr" "$DIST/docs"

cp "$APP_EXE" "$DIST/delil.exe"
cp "$CLI_EXE" "$DIST/delil-cli.exe"
cp LICENSE README.md "$DIST/"
cp docs/*.md "$DIST/docs/" 2>/dev/null || true
cp docs/SBOM.json "$DIST/docs/" 2>/dev/null || true
cp "$MODELS/config.json" "$MODELS/tokenizer.json" "$MODELS/model.safetensors" "$DIST/models/"
cp "$MODELS/ocr/text-detection.rten" "$MODELS/ocr/text-recognition.rten" "$DIST/models/ocr/"
cp "$MODELS/MODEL_MANIFEST.json" "$DIST/models/"

# ---------------------------------------------------------------- 4. Bütünlük kontrolü
step "4/8 Paket içeriği kontrol ediliyor"
for f in delil.exe delil-cli.exe models/config.json models/tokenizer.json \
         models/model.safetensors models/ocr/text-detection.rten \
         models/ocr/text-recognition.rten docs/KULLANICI_KILAVUZU.md; do
  [ -f "$DIST/$f" ] || fail "Pakette eksik dosya: $f"
done
echo "  Paket içeriği eksiksiz."

# ---------------------------------------------------------------- 5. Kurulum EXE
step "5/8 Inno Setup ile kurulum dosyası üretiliyor"
if ! command -v iscc >/dev/null 2>&1 && ! command -v ISCC.exe >/dev/null 2>&1; then
  fail "Inno Setup (iscc) bulunamadı. Kurulum EXE'si üretilemez.
     Windows: winget install JRSoftware.InnoSetup
     CI (choco): choco install innosetup"
fi
ISCC_BIN="$(command -v iscc || command -v ISCC.exe)"
# Git Bash (MSYS2) /D... biçimindeki Inno Setup anahtarlarını Windows yoluna
# çevirmeye çalışır; bu, "You may not specify more than one script filename"
# hatasına yol açar. Dönüştürmeyi kapatıyoruz.
if [ -n "${MSYS:-}" ] || [ -n "${MINGW_PREFIX:-}" ] || [ "${OS:-}" = "Windows_NT" ]; then
  export MSYS2_ARG_CONV_EXCL='*'
  export MSYS_NO_PATHCONV=1
fi
# Dizin ayraçlarını Windows biçimine çevir (iscc Windows ikilisidir).
DIST_WIN="$DIST"
ISS_WIN="$ISS"
if command -v cygpath >/dev/null 2>&1; then
  DIST_WIN="$(cygpath -w "$DIST")"
  ISS_WIN="$(cygpath -w "$ISS")"
fi
"$ISCC_BIN" "/DMyAppVersion=$VERSION" "/DMyDistDir=$DIST_WIN" "$ISS_WIN"

INSTALLER="$DIST/DELIL-Setup-$VERSION-x64.exe"
[ -f "$INSTALLER" ] || fail "Kurulum dosyası üretilemedi: $INSTALLER"

# ---------------------------------------------------------------- 6. Checksum
step "6/8 SHA-256 checksum üretiliyor"
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$DIST" && sha256sum "DELIL-Setup-$VERSION-x64.exe" > "DELIL-Setup-$VERSION-x64.exe.sha256")
else
  (cd "$DIST" && shasum -a 256 "DELIL-Setup-$VERSION-x64.exe" > "DELIL-Setup-$VERSION-x64.exe.sha256")
fi
cat "$INSTALLER.sha256"

# ---------------------------------------------------------------- 7. Özet
step "7/8 Paket özeti"
INSTALLER_SIZE=$(stat -c%s "$INSTALLER" 2>/dev/null || stat -f%z "$INSTALLER")
echo "  Kurulum dosyası: $INSTALLER"
echo "  Boyut: $INSTALLER_SIZE bayt ($(( INSTALLER_SIZE / 1024 / 1024 )) MiB)"
echo "  Checksum: $INSTALLER.sha256"

step "8/8 Tamamlandı"
ls -la "$DIST"
