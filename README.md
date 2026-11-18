# DELİL

**Delili tak. Sesi çöz. UDF raporunu al.**

DELİL, **56.12 — Ses Kayıtlarının Metin Hâline Dönüştürülmesi** uzmanlık
alanındaki bilirkişiler için geliştirilmiş, tek amaca odaklanmış bir Windows
masaüstü uygulamasıdır. Yerel, çevrimdışı, otomatik ve istisna odaklı çalışır.

## İndirme

**Son sürüm:** <https://github.com/azmisahin-gov/delil/releases/latest>

| Dosya | Açıklama |
|---|---|
| `DELIL-Setup-0.1.1-x64.exe` | Windows 11 x64 kurulum dosyası (~1.37 GB; uygulama + Whisper + OCR modelleri dahil) |
| `DELIL-Setup-0.1.1-x64.exe.sha256` | Kurulum dosyasının SHA-256 checksum'ı |
| `MODEL_MANIFEST.json` | Pakete dahil edilen model bileşenlerinin kaynak/sürüm/hash kaydı |

Kurulum dosyası, kullanıcıdan **Rust, Cargo, Python, Node.js, FFmpeg veya ayrı
model indirmesi istemeden** çalışır. Tüm bileşenler pakete dahildir ve kurulum
çevrimdışı tamamlanır.

## Ürünün üç temel işlemi

1. **DELİLİ AL** — Materyali algılar, envantere alır, güvenli biçimde kopyalar
   ve SHA-256 ile bütünlüğünü doğrular.
2. **SESİ ÇÖZ** — Ses dosyalarını içeriklerine göre bulur, yerel Whisper
   large-v3-turbo motoruyla metne dönüştürür ve ses ile metni eşleştirir.
3. **UDF RAPORUNU OLUŞTUR** — Görevlendirme, materyal, transkript, teknik
   yöntem ve bilirkişi değerlendirmesini birleştirir; doğrudan **UYAP UDF**
   üretir.

## Temel ilkeler

- **Tamamen yerel ve çevrimdışı.** Hiçbir ses, transkript, görevlendirme,
  taraf bilgisi, dosya numarası veya rapor buluta gönderilmez.
- **Kullanıcıdan model indirmesi veya teknik bağımlılık kurması istenmez.**
  Model ve OCR bileşenleri kurulum paketiyle birlikte verilir.
- **Kaynak materyale asla yazılmaz.** Edinim mantıksal dosya edinimidir.
- **Yazılımsal yazma koruması, donanımsal yazma engelleyici ile eşdeğer
  gösterilmez.**
- **Model olasılıkları gerçek doğruluk yüzdesi olarak sunulmaz**; yerine
  “Normal / Kontrol önerilir / Öncelikli inceleme / Anlaşılamayan bölüm”
  durumları kullanılır.
- **İlk makine transkript çıktısı hiçbir zaman kaybedilmez** (sürümleme).
- **Gerçekleştirilemeyen bir özellik gizlenmez veya simüle edilmez.**

## Mimari

Rust workspace; `crates/` altında 15 crate. Ayrıntı için [AGENTS.md](AGENTS.md)
ve [docs/MIMARI.md](docs/MIMARI.md).

```
crates/
  delil-core          Alan modeli ve durum türleri
  delil-hash          SHA-256 ve bütünlük doğrulama
  delil-udf           UYAP UDF yazıcı/okuyucu/doğrulayıcı (gerçek format)
  delil-audio         İçerik tabanlı ses algılama, teknik envanter, çözme
  delil-evidence      Aygıt algılama, mantıksal dosya edinimi, doğrulama
  delil-assignment    Görevlendirme belgesi okuma, alan/soru çıkarımı, OCR
  delil-transcription Whisper (Candle) motoru + otomatik kuyruk
  delil-quality       Otomatik kalite kontrolü
  delil-report        Rapor modeli ve otomatik üretim
  delil-export        PDF/TXT/JSON yardımcı çıktılar
  delil-case-store    AES-256-GCM şifreli depo, yedek/geri yükleme
  delil-update        ed25519 imzalı güncelleme yöneticisi
  delil-pipeline      Uçtan uca iş akışı orkestrasyonu
  delil-cli           Komut satırı arayüzü
  delil-app           egui/eframe masaüstü arayüzü
```

## Derleme ve çalıştırma

### Geliştirme

```bash
cargo build --workspace          # derle
cargo test --workspace           # tüm testler
cargo clippy --workspace         # lint

# Ağır özellikleri aç (Whisper + ses çözme + OCR):
cargo build -p delil-transcription --features candle
cargo build -p delil-transcription --features decode
cargo build -p delil-assignment  --features ocr
```

### Komut satırı ile inceleme

```bash
delil inceleme \
  --vaka ./vaka-2024-456 \
  --gorevlendirme ./gorevlendirme.pdf \
  --materyal /media/usb \
  --bilirkisi "Dr. Ayşe Kaya" --unvan "Bilirkişi" --sicil 12345
```

Üretilen çıktılar:

```
vaka-2024-456/
  assignment/        görevlendirme belgesi
  materials/         edinilmiş kopyalar (dizin yapısı korunur)
  transcripts/       transkript sürümleri
  report/
    BILIRKISI_RAPORU.udf     ← birincil çıktı
    BILIRKISI_RAPORU.pdf     ← yardımcı kopya
    TRANSKRIPT_EK_1.txt      ← transkript ekleri
    ISLEM_MANIFESTI.json     ← hash + işlem kaydı
  logs/queue.json            kesintiden devam için kuyruk durumu
  case.json.enc              şifreli vaka meta verisi
  keys/master.key            DPAPI ile korunan ana anahtar
```

### Diğer komutlar

```bash
delil aygitlar                    # bağlı harici aygıtları listele
delil envanter kayit.wav          # ses teknik envanteri
delil udf-oku rapor.udf           # UDF metnini göster
delil udf-dogrula rapor.udf       # UDF yapısal doğrulaması
delil yedekle ./vaka yedek.delilbackup
delil geri-yukle yedek.delilbackup ./geri-yuklenen
delil surum
```

## UYAP UDF çıktısı

DELİL raporu **doğrudan UDF** olarak üretir; kullanıcıdan başka bir programda
DOCX'i UDF'ye çevirmesi istenmez. UDF, Deflate sıkıştırmalı bir ZIP arşividir
ve içinde `content.xml` barındırır (`format_id="1.8"`, karakter tabanlı
`startOffset`/`length`).

Üretilen UDF kendi doğrulayıcımızdan geçer. **Ancak bu, resmî UYAP Editör
testinin yerine geçmez.** Teslim öncesi güncel resmî UYAP Editör sürümüyle
açma/kaydetme/yeniden açma testi yapılmalıdır (bkz. `docs/UDF_UYUMLULUK.md`).

## Belgeler

- [Kullanıcı kılavuzu (Türkçe)](docs/KULLANICI_KILAVUZU.md)
- [Kurulum kılavuzu](docs/KURULUM.md)
- [Mimari](docs/MIMARI.md)
- [Desteklenen dosya biçimleri matrisi](docs/FORMAT_MATRISI.md)
- [UDF uyumluluk raporu](docs/UDF_UYUMLULUK.md)
- [Test raporu](docs/TEST_RAPORU.md)
- [Türkçe transkripsiyon benchmark](docs/TRANSKRIPSİYON_BENCHMARK.md)
- [Bağımlılık lisansları](docs/LISANSLAR.md)
- [SBOM](docs/SBOM.md)

## Lisans

MIT. Ayrıntı için [LICENSE](LICENSE).
