# AGENTS.md — DELİL geliştirici notları

Bu dosya, DELİL deposunda çalışan geliştirici ajanları için kalıcı bağlamdır.

## Ürün

DELİL, 56.12 bilirkişileri (ses kayıtlarının metin hâline dönüştürülmesi) için
yerel, çevrimdışı, otomatik ve istisna odaklı bir Windows masaüstü
uygulamasıdır. Üç temel işlev:

1. **DELİLİ AL** — materyali algıla, envantere al, kopyala, SHA-256 ile doğrula.
2. **SESİ ÇÖZ** — ses dosyalarını bul, yerel Whisper ile metne dönüştür, eşleştir.
3. **UDF RAPORUNU OLUŞTUR** — raporu birleştir, doğrudan UYAP UDF üret.

## Mimari

Rust workspace, `crates/` altında şu crate'lerden oluşur:

| Crate | Sorumluluk |
|-------|-----------|
| `delil-core` | Alan modeli (vaka, görevlendirme, materyal, ses, transkript, rapor), durum türleri, kimlikler |
| `delil-hash` | SHA-256, kaynak/hedef bütünlük doğrulaması |
| `delil-udf` | UYAP UDF yazıcı/okuyucu/doğrulayıcı (gerçek format) |
| `delil-audio` | İçerik tabanlı ses algılama, teknik envanter, çözme |
| `delil-evidence` | Aygıt algılama, dosya bazlı edinim, doğrulama |
| `delil-assignment` | Görevlendirme belgesi okuma + çıkarım + OCR |
| `delil-transcription` | Whisper/Candle motoru + otomatik kuyruk |
| `delil-quality` | Otomatik kalite kontrolü |
| `delil-report` | Rapor modeli + otomatik üretim |
| `delil-export` | PDF/TXT/JSON yardımcı çıktılar |
| `delil-case-store` | Şifreli yerel vaka deposu, yedekleme/geri yükleme |
| `delil-update` | İmzalı güncelleme yöneticisi |
| `delil-pipeline` | Uçtan uca iş akışı orkestrasyonu |
| `delil-cli` | Komut satırı arayüzü |
| `delil-app` | egui/eframe masaüstü arayüzü |

## UYAP UDF formatı (kritik)

UDF = Deflate ZIP arşivi; içinde **tek dosya**: `content.xml` (UTF-8).

```xml
<?xml version="1.0" encoding="UTF-8" ?>
<template format_id="1.8">
<content><![CDATA[metin havuzu]]></content>
<properties><pageFormat mediaSizeName="1" leftMargin="..." .../></properties>
<elements resolver="hvl-default">
  <paragraph Alignment="0" LeftIndent="0.0" RightIndent="0.0">
    <content startOffset="0" length="5" family="Times New Roman" size="11" />
  </paragraph>
</elements>
<styles><style name="default" .../><style name="hvl-default" .../></styles>
</template>
```

Kurallar:
- Tüm metin tek CDATA havuzunda; önce üstbilgi, sonra gövde, en son altbilgi.
- `startOffset`/`length` **karakter (rune)** cinsindendir, bayt değil.
- Paragraflar arası `\n`. Resim yer tutucusu `\uFFFC`. Sekme `\t`.
- Boş paragraf `\u200B` (sıfır genişlikli boşluk).
- `format_id` her zaman `"1.8"`.
- Renkler işaretli 32-bit ARGB tam sayı (siyah = `-16777216`, beyaz = `-1`).
- A4: 595.28 × 841.89 punto.

Kaynak: `saidsurucu/UDF-Toolkit` Docs.md ve `mistih/UdfParser`.

## Geliştirme komutları

```bash
cargo build --workspace          # derle
cargo test --workspace           # tüm testler
cargo test -p delil-udf          # tek crate testi
cargo clippy --workspace         # lint
```

Ağır özellikler (Candle transkripsiyon, OCR) varsayılan kapalıdır; ilgili
özelliklerle açılır: `--features candle`, `--features ocr`.

## İlkeler

- Kullanıcıdan model indirmesi veya teknik bağımlılık kurması istenmez.
- Hiçbir veri buluta gönderilmez; tüm işlem yereldir.
- Yazılımsal yazma koruması, donanımsal yazma engelleyici ile eşdeğer
  gösterilmez.
- Model olasılıkları gerçek doğruluk yüzdesi olarak sunulmaz.
- İlk makine transkript çıktısı hiçbir zaman kaybedilmez (sürümleme).
- Gerçekleştirilemeyen bir özellik gizlenmez veya simüle edilmez.
