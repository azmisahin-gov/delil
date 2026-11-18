# DELİL — Model Bileşenleri ve Lisansları

DELİL kurulum paketi, kullanıcıdan ayrı indirme istemeden çalışabilmesi için
gerekli model bileşenlerini **içerir**. Bu dosya, bu bileşenlerin kaynağını,
sürümünü, lisansını ve yeniden dağıtım koşullarını kaydeder.

## Whisper large-v3-turbo

| | |
|---|---|
| Kaynak | `openai/whisper-large-v3-turbo` (Hugging Face) |
| Sabitlenmiş revizyon | `41f01f3fe87f28c78e2fbf8b568835947dd65ed9` |
| Lisans | **MIT** |
| Dosyalar | `config.json`, `tokenizer.json`, `model.safetensors` |
| Boyut | ~1.6 GB |
| Yeniden dağıtım | **İzin verilir** (MIT, atıf ile) |

MIT lisansı, telif hakkı bildirimi ve lisans metni korunduğu sürece ticari
dahil her türlü kullanım ve yeniden dağıtıma izin verir. DELİL kurulum paketi
`LICENSE` dosyasında bu bildirimleri içerir.

## OCR modelleri (ocrs)

| | |
|---|---|
| Kaynak | `robertknight/ocrs` (Hugging Face) |
| Sabitlenmiş revizyon | `df0edd170279ab971b53e094c627255a87e1a503` |
| Lisans | **CC-BY-SA-4.0** |
| Dosyalar | `text-detection.rten`, `text-recognition.rten` |
| Boyut | ~12 MB |
| Yeniden dağıtım | **İzin verilir**, **atıf ve ShareAlike koşullarıyla** |

### CC-BY-SA-4.0 koşulları ve DELİL'in uyumu

CC-BY-SA-4.0 iki temel koşul getirir:

1. **Atıf (BY):** Eserin kaynağı belirtilmeli, lisans bağlantısı verilmeli ve
   değişiklik yapıldıysa belirtilmelidir.
2. **Aynı lisansla paylaşım (SA):** Eserin uyarlamaları aynı lisansla
   dağıtılmalıdır.

DELİL'in uyumu:

- **Atıf:** Model dosyaları **değiştirilmeden** dağıtılır. Kaynak, revizyon ve
  lisans bilgisi bu dosyada ve `models/MODEL_MANIFEST.json` içinde kayıtlıdır.
  Kurulum paketi bu bilgiyi içerir.
- **ShareAlike kapsamı:** CC-BY-SA-4.0 yalnızca **model dosyalarının
  kendisini ve uyarlamalarını** kapsar. Model dosyaları değiştirilmediği için
  "uyarlama" oluşmaz. DELİL'in kaynak kodu (MIT) ayrı bir eserdir ve
  CC-BY-SA'nın ShareAlike koşulu **kaynak koda yayılmaz**; yalnızca model
  dosyaları CC-BY-SA-4.0 altında kalır.
- DELİL, model dosyalarını kendi lisansı altında yeniden lisanslamaz.

### Önemli not

CC-BY-SA-4.0, veri setleri için "açık ve kısıtlayıcı olmayan" lisanslar
tercih edilerek eğitilmiş modeller içindir. Ticari kullanımda hukuki
danışmanlık önerilir. Eğer kurumsal dağıtımda CC-BY-SA kabul edilemezse,
alternatif olarak:

- OCR modelleri paket dışında bırakılabilir ve yalnızca tespit/tanıma
  gerektiğinde kullanıcı tarafından temin edilebilir (ancak bu, "kullanıcıdan
  indirme isteme" ilkesiyle çelişir), **veya**
- ocrs modelleri yerine MIT/Apache lisanslı bir OCR çözümü
  değerlendirilebilir.

Bu karar ürün sahibine aittir; DELİL varsayılan olarak atıf koşulunu yerine
getirerek CC-BY-SA-4.0 modellerini dağıtır.

## Model bütünlüğü

Tüm model dosyalarının SHA-256 değerleri `models/MODEL_MANIFEST.json` içinde
kayıtlıdır. Paketleme betiği (`scripts/package-windows.sh`) bu hash'leri
doğrulamadan kurulum paketi üretmez.

## Diğer çalışma zamanı bileşenleri

| Bileşen | Lisans | Not |
|---|---|---|
| Rust standart kütüphanesi | MIT/Apache-2.0 | — |
| Candle | MIT/Apache-2.0 | Whisper çıkarımı |
| Symphonia | MPL-2.0 | Ses çözme (dosya düzeyi copyleft) |
| rodio | MIT/Apache-2.0 | Ses oynatma |
| egui/eframe | MIT/Apache-2.0 | Arayüz |
| ocrs | MIT/Apache-2.0 | OCR motoru |
| rten | MIT/Apache-2.0 | Çıkarım çalışma zamanı |
| DejaVu Serif | Bitstream Vera / DejaVu (izin verici) | PDF Türkçe yazı tipi |

Tam liste ve sürümler için `docs/SBOM.json` ve `docs/LISANSLAR.md`.

Hiçbir bağımlılık güçlü copyleft (GPL/AGPL) değildir. MPL-2.0 (Symphonia),
dosya düzeyinde copyleft uygular; kaynak dosyaları değiştirmediğimiz sürece
dağıtım kısıtı doğurmaz.
