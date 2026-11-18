# DELİL — Sürüm Notları

## DELİL 0.1.0 (Windows 11 x64)

**Delili tak. Sesi çöz. UDF raporunu al.**

DELİL, 56.12 — Ses Kayıtlarının Metin Hâline Dönüştürülmesi uzmanlık alanındaki
bilirkişiler için geliştirilmiş, tek amaca odaklanmış, yerel ve çevrimdışı bir
Windows masaüstü uygulamasıdır.

### İndirme

| Dosya | Açıklama |
|---|---|
| `DELIL-Setup-0.1.0-x64.exe` | Windows 11 x64 kurulum dosyası (uygulama + modeller dahil) |
| `DELIL-Setup-0.1.0-x64.exe.sha256` | Kurulum dosyasının SHA-256 checksum'ı |
| `MODEL_MANIFEST.json` | Pakete dahil edilen model bileşenlerinin kaynak/sürüm/hash kaydı |

### Bu sürümde neler var

**1. DELİLİ AL**
- Görevlendirme belgesi okuma: PDF (metin katmanı), UDF, DOCX, TXT ve
  görsel/taranmış belge (yerel OCR).
- Mahkeme, dosya numarası, taraflar, tarihler ve soruların otomatik çıkarımı;
  her alanın kaynak belgeyle ilişkisi saklanır.
- USB/SD/harici disk otomatik algılama (takıldığında da).
- Mantıksal dosya edinimi; kaynak materyale asla yazılmaz.
- SHA-256 ile kaynak/hedef bütünlük doğrulaması.

**2. SESİ ÇÖZ**
- Ses dosyalarının **içerikten** algılanması (uzantıya güvenilmez).
- Whisper **large-v3-turbo** ile yerel, çevrimdışı transkripsiyon (Candle).
- Otomatik işlem kuyruğu; kesintiden devam, duraklatma, iptal, otomatik
  yeniden deneme.
- Sessizlikte üretilen halüsinasyon metninin otomatik elenmesi.
- 14 kalite tespitiyle otomatik kalite kontrolü.
- Ses/transkript eşzamanlı inceleme: dalga biçimi, oynatma, düzeltme,
  sürümleme, konuşmacı etiketi, not.

**3. UDF RAPORUNU OLUŞTUR**
- 24 bölümlü bilirkişi raporunun otomatik üretimi.
- Rapor **doğrudan UYAP UDF** olarak üretilir (DOCX'e çevirme gerekmez).
- UDF yapısal doğrulaması (9 kontrol).
- PDF yardımcı kopyası, TXT transkript ekleri, JSON işlem manifesti.

**Güvenlik ve saklama**
- Tamamen yerel ve çevrimdışı; hiçbir veri buluta gönderilmez.
- AES-256-GCM şifreli yerel vaka deposu (Windows DPAPI korumalı ana anahtar).
- Parola korumalı, bütünlük doğrulamalı yedekleme ve geri yükleme.
- ed25519 imzalı güncelleme manifesti; güncelleme öncesi yedek ve geri dönüş.

### Paket içeriği

- DELİL masaüstü uygulaması (`delil.exe`)
- DELİL komut satırı aracı (`delil-cli.exe`)
- Whisper large-v3-turbo model dosyaları (`models/`)
- OCR motoru ve modelleri (`models/ocr/`)
- Türkçe kullanıcı kılavuzu ve kurulum kılavuzu
- Bağımlılık lisansları ve SBOM

### Desteklenen sistem

- **İşletim sistemi:** Windows 11 x64 (22H2 ve üzeri önerilir)
- **İşlemci:** x86-64, AVX2 destekli, 4+ çekirdek
- **RAM:** en az 8 GB, önerilen 16 GB
- **Disk:** uygulama + modeller için ~2.5 GB, çalışma alanı için materyalin
  ~3 katı
- **GPU:** gerekli değil (CPU yeterli); CUDA/Metal destekleniyorsa otomatik
  kullanılır, yoksa CPU'ya düşer
- **İnternet:** gerekmez (kurulum ve ilk çalıştırma dahil)

### Model ve bileşen sürümleri

| Bileşen | Sürüm | Lisans |
|---|---|---|
| Whisper large-v3-turbo | `41f01f3f` (sabitlenmiş) | MIT |
| ocrs OCR modelleri | `df0edd17` (sabitlenmiş) | CC-BY-SA-4.0 |
| DELİL | 0.1.0 | MIT |

Ayrıntılı hash ve kaynak bilgisi için `MODEL_MANIFEST.json` dosyasına bakın.

### Uygulama imzası

**Bu sürümün yürütülebilir dosyaları İMZALANMAMIŞTIR.** Teslim ortamında
kullanılabilir bir kod imzalama sertifikası bulunmadığından, Windows
SmartScreen ilk çalıştırmada "Bilinmeyen yayımcı" uyarısı gösterebilir.
İmzalanmamış uygulama, imzalıymış gibi gösterilmez. Kurumsal dağıtımda kod
imzalama sertifikası ile imzalanması önerilir.

### Bilinen sınırlamalar

- **Gerçek UYAP Editör uyumluluk testi:** Resmî UYAP Editör Windows'a özeldir.
  UDF biçimi, açık kaynak referans uygulamaları incelenerek doğrulanmıştır;
  üretilen dosyalar DELİL'in kendi doğrulayıcısından geçer. Ancak resmî UYAP
  Editör ile uyumluluk, teslim ortamında **doğrulanmamıştır**. Kullanıcıların
  güncel resmî UYAP Editör ile test etmesi ve sonuçları bildirmesi istenir.
- **Türkçe WER/CER ölçümü:** Ölçüm altyapısı hazırdır; insan tarafından
  doğrulanmış Türkçe test setiyle gerçek ölçüm yapılması önerilir.
- **GPU hızlandırma:** Kod GPU desteğini otomatik algılar; ancak GPU
  hızlandırma gerçek donanımda test edilmeden "destekleniyor" olarak
  bildirilmez. CPU her zaman çalışır.

### Kapsam dışı (bilinçli olarak)

Fiziksel RAW/E01 imaj üretimi, silinmiş veri kurtarma, genel adli bilişim,
cep telefonu incelemesi, konuşmacı biyometrik kimlik tespiti, ses
orijinalliği/montaj incelemesi, deepfake analizi, hukuki değerlendirme.

### Kurulum

1. `DELIL-Setup-0.1.0-x64.exe` dosyasını indirin.
2. SHA-256 checksum'ı doğrulayın.
3. Kurulumu çalıştırın (yönetici izni gerekir).
4. Masaüstündeki DELİL kısayolundan uygulamayı açın.

Kullanıcıdan Rust, Cargo, Python, Node.js, FFmpeg, Whisper modeli veya OCR
modeli kurması **istenmez**; tüm bileşenler kurulum paketine dahildir.

---

_Bu sürüm notları bir AI ajanı (OpenHands) tarafından kullanıcı adına
hazırlanmıştır._
