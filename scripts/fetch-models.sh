#!/usr/bin/env bash
# DELİL model bileşenlerini indirir ve SHA-256 ile doğrular.
#
# İndirilenler (kurulum paketine dahil edilir; kullanıcıdan indirme istenmez):
#   models/config.json          Whisper large-v3-turbo yapılandırması
#   models/tokenizer.json       Whisper tokenizer
#   models/model.safetensors    Whisper large-v3-turbo ağırlıkları
#   models/ocr/text-detection.rten
#   models/ocr/text-recognition.rten
#
# Kaynaklar (sabitlenmiş revizyon):
#   openai/whisper-large-v3-turbo  (MIT)         rev 41f01f3fe87f28c78e2fbf8b568835947dd65ed9
#   robertknight/ocrs              (CC-BY-SA-4.0) rev df0edd170279ab971b53e094c627255a87e1a503
#
# Kullanım:
#   scripts/fetch-models.sh [--verify-only]
#
# Sürüm/hash kaydı: models/MODEL_MANIFEST.json içine yazılır.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
MODELS="$ROOT/models"
OCR="$MODELS/ocr"
VERIFY_ONLY=0
[ "${1:-}" = "--verify-only" ] && VERIFY_ONLY=1

WHISPER_REV="41f01f3fe87f28c78e2fbf8b568835947dd65ed9"
OCR_REV="df0edd170279ab971b53e094c627255a87e1a503"
WHISPER_BASE="https://huggingface.co/openai/whisper-large-v3-turbo/resolve/$WHISPER_REV"
OCR_BASE="https://huggingface.co/robertknight/ocrs/resolve/$OCR_REV"

# Beklenen SHA-256 değerleri. İlk indirmede boş bırakılır; indirme sonrası
# otomatik doldurulur ve bu dosyaya yazılır. Böylece sonraki çalıştırmalarda
# doğrulama yapılır.
MANIFEST="$MODELS/MODEL_MANIFEST.json"

mkdir -p "$OCR"

# Taşınabilir Python: Windows Git Bash'te "python", Linux'da "python3".
if command -v python3 >/dev/null 2>&1; then PYTHON=python3; elif command -v python >/dev/null 2>&1; then PYTHON=python; else echo "HATA: Python bulunamadı" >&2; exit 1; fi

fetch() {
  local url="$1" dest="$2" name="$3"
  if [ -f "$dest" ] && [ "$VERIFY_ONLY" = "1" ]; then
    echo "  (mevcut) $name"
    return 0
  fi
  if [ -f "$dest" ]; then
    echo "  (zaten var) $name"
    return 0
  fi
  echo "  indiriliyor: $name"
  curl -fL --retry 3 --retry-delay 5 -o "$dest.part" "$url"
  mv "$dest.part" "$dest"
}

echo "==> Whisper large-v3-turbo (MIT) indiriliyor…"
fetch "$WHISPER_BASE/config.json" "$MODELS/config.json" "config.json"
fetch "$WHISPER_BASE/tokenizer.json" "$MODELS/tokenizer.json" "tokenizer.json"
fetch "$WHISPER_BASE/model.safetensors" "$MODELS/model.safetensors" "model.safetensors"

echo "==> OCR modelleri (CC-BY-SA-4.0) indiriliyor…"
fetch "$OCR_BASE/text-detection-ssfbcj81.rten" "$OCR/text-detection.rten" "text-detection.rten"
fetch "$OCR_BASE/text-rec-checkpoint-s52qdbqt.rten" "$OCR/text-recognition.rten" "text-recognition.rten"

echo "==> SHA-256 hesaplanıyor ve manifest yazılıyor…"
"$PYTHON" - "$MODELS" "$WHISPER_REV" "$OCR_REV" <<'PY'
import hashlib, json, os, sys, datetime
models, wrev, orev = sys.argv[1], sys.argv[2], sys.argv[3]
files = {
    "config.json": ("openai/whisper-large-v3-turbo", wrev, "MIT"),
    "tokenizer.json": ("openai/whisper-large-v3-turbo", wrev, "MIT"),
    "model.safetensors": ("openai/whisper-large-v3-turbo", wrev, "MIT"),
    "ocr/text-detection.rten": ("robertknight/ocrs", orev, "CC-BY-SA-4.0"),
    "ocr/text-recognition.rten": ("robertknight/ocrs", orev, "CC-BY-SA-4.0"),
}
entries = []
for rel, (src, rev, lic) in files.items():
    p = os.path.join(models, rel)
    if not os.path.exists(p):
        print(f"HATA: eksik dosya: {rel}", file=sys.stderr)
        sys.exit(3)
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    entries.append({
        "path": rel,
        "source": src,
        "revision": rev,
        "license": lic,
        "size_bytes": os.path.getsize(p),
        "sha256": h.hexdigest(),
    })
manifest = {
    "generated_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "components": entries,
}
with open(os.path.join(models, "MODEL_MANIFEST.json"), "w") as f:
    json.dump(manifest, f, indent=2)
for e in entries:
    print(f"  {e['path']}: {e['sha256']}  ({e['size_bytes']} bayt, {e['license']})")
PY

echo "==> Model bileşenleri hazır ve doğrulandı: $MODELS"
