# DELİL — Sürüm Notları

## DELİL 0.2.0 (Windows 11 x64)

**Delili tak. Sesi çöz. UDF raporunu al.**

### Bu sürümde yeni ve düzeltilenler (0.1.3 → 0.2.0)

0. **Aynı incelemeye geri dönme.** Ana ekranda artık vaka listesi var: her vaka
   için dosya no, tarih, materyal sayısı ve UDF durumu gösterilir; **Aç** ile
   incelemeye devam edilir. Vaka üst klasörü değiştirilebilir.
0. **'İncelemeyi başlat' düğmesi** artık üstte ve her zaman görünür (küçük
   pencerelerde kaybolmaz); materyal seçilmeden devre dışıdır ve nedeni
   açıklanır.
0. **Ana ekrana gidince iş kaybolmaz:** üst çubukta 'İncelemeye dön' ve
   'Sonuca dön' düğmeleri var.
0. **Transkript düzenleme** yeniden tasarlandı: geniş, kaydırılabilir segment
   tablosu (zaman / kalite / metin). **Zaman düğmesine tıklayınca ses o ana
   gider ve oynar.** Her düğme (uygula / geri al / yinele / işaretle / kaydet /
   onayla) işlem sonrası görünür durum mesajı verir.
0. **Anlaşılamayan işaretleme** düğmesi eklendi.
0. **Yazılımsal yazma koruması** artık `diskpart` yerine
   `IOCTL_DISK_SET_DISK_ATTRIBUTES` ile uygulanır (daha güvenilir; birim
   düzeyinde). `Persist=FALSE`: aygıt çıkarılana veya yeniden başlatılana
   kadar geçerlidir. Sonuç yeniden ölçülür.

### 0.1.x sürümlerinden devralınanlar (0.1.1 → 0.1.2)


Kullanıcı geri bildirimiyle bulunan sorunlar giderildi:

0. **Model bulunamıyor hatası (kritik).** Kurulumdan sonra transkripsiyon
   "Whisper model dizini yapılandırılmamış" hatası veriyordu: arayüz `models/`
   dizinini buluyordu ama motor yalnızca ortam değişkenine bakıyordu. Artık
   ikisi de aynı ortak çözümleyiciyi kullanır; kurulumdaki model otomatik
   bulunur.
0. **Bilirkişi bilgileri görevlendirme belgesinden çıkarılıyor.** Ad soyad,
   unvan ve sicil no belgede varsa otomatik alınır; arayüzde çıkarılan bilgiler
   gösterilir ve boş alanlar ön-doldurulur (sizin girdiğiniz değer önceliklidir).
0. **Yazılımsal yazma koruması artık uygulanabilir.** Aygıt listesindeki
   "Salt okunur yap" düğmesiyle aygıt Windows düzeyinde salt okunur yapılabilir
   (diskpart; yönetici yetkisi gerekir). Onay penceresiyle açık onay alınır;
   sonuç yeniden ölçülür. Donanımsal engelleyici ile eşdeğer gösterilmez.
0. **Arka plandaki konsol penceresi (siyah ekran) kaldırıldı.** Bu bir GUI
   uygulamasıdır; artık konsol penceresi açılmaz.
0. **Birden fazla harici aygıt bağlıysa** açık uyarı gösterilir ve kaynağı
   siz seçersiniz.

Ayrıca (0.1.1'den devralınan):

1. **Çökme hatası giderildi (kritik).** Kaynak materyalde 12–15 baytlık kısa bir
   dosya bulunduğunda uygulama çöküyordu (içerik algılama kodu, 16 baytlık imza
   kontrolünü yeterli uzunluk doğrulamadan yapıyordu). Gerçek SD kart
   kullanımında ortaya çıktı. Artık **panik yapmaz**; kısa/bozuk dosyalar
   sessizce atlanmaz, uyarı olarak kaydedilir.
2. **Tek bozuk dosya artık tüm işlemi durdurmuyor.** Her dosya bağımsız
   değerlendirilir; sorunlu dosyalar uyarı listesine eklenir.
3. **Güvenlik ağı.** Arka plan işleminde beklenmedik bir hata olsa bile
   uygulama kapanmaz; kullanıcıya açık hata bildirilir.
4. **Yazma koruması durumu artık gerçekten ölçülüyor.** Aygıt listesinde her
   sürücü için salt okunur durumu gösterilir (donanımsal SD kart kilidi veya
   salt okunur bağlama algılanır). Bu bir iddia değil, ölçümdür; yazılımsal
   koruma donanımsal yazma engelleyici ile eşdeğer gösterilmez.

### DELİL kaynak materyale yazar mı?

**Hayır.** DELİL kaynak dosyalara hiçbir koşulda yazmaz, silmez, yeniden
adlandırmaz veya taşımaz; yalnızca yerel vaka deposuna kopyalar. SD kartın
kendi yazma kilidi (fiziksel anahtar) takılıysa veya aygıt salt okunur
bağlanmışsa, uygulama bunu aygıt listesinde "🔒 yazmaya kapalı (salt okunur)"
olarak gösterir. Kilit yoksa "⚠ yazmaya açık (uygulama yine de yazmaz)" yazar.

Yazılımsal yazma koruması, ilk bağlantı anında donanımsal yazma engelleme ile
**eşdeğer değildir**; yüksek güvence gereken edinimlerde donanımsal yazma
engelleyici kullanılması önerilir.

### 0.1.0 sürümünden devralınanlar

**1. DELİLİ AL**
- Görevlendirme belgesi okuma: PDF (metin katmanı), UDF, DOCX, TXT ve
  görsel/taranmış belge (yerel OCR).
- Mahkeme, dosya numarası, taraflar, tarihler ve soruların otomatik çıkarımı;
  her alanın kaynak belgeyle ilişkisi saklanır.
- USB/SD/harici disk otomatik algılama (takıldığında da).
- Mantıksal dosya edinimi; kaynak materyale asla yazılmaz.
- SHA-256 ile kaynak/hedef bütünlük doğrulaması.

**2. SESİ ÇÖZ**
- Ses dosyalarının içerikten algılanması (uzantıya güvenilmez).
- Whisper **large-v3-turbo** ile yerel, çevrimdışı transkripsiyon (Candle).
- Otomatik işlem kuyruğu; kesintiden devam, duraklatma, iptal, otomatik
  yeniden deneme.
- Sessizlikte üretilen halüsinasyon metninin otomatik elenmesi.
- 14 kalite tespitiyle otomatik kalite kontrolü.
- Ses/transkript eşzamanlı inceleme: dalga biçimi, oynatma, düzeltme,
  sürümleme, konuşmacı etiketi, not.

**3. UDF RAPORUNU OLUŞTUR**
- 24 bölümlü bilirkişi raporunun otomatik üretimi.
- Rapor doğrudan UYAP UDF olarak üretilir (DOCX'e çevirme gerekmez).
- UDF yapısal doğrulaması (9 kontrol).
- PDF yardımcı kopyası, TXT transkript ekleri, JSON işlem manifesti.

**Güvenlik ve saklama**
- Tamamen yerel ve çevrimdışı; hiçbir veri buluta gönderilmez.
- AES-256-GCM şifreli yerel vaka deposu (Windows DPAPI korumalı ana anahtar).
- Parola korumalı, bütünlük doğrulamalı yedekleme ve geri yükleme.
- ed25519 imzalı güncelleme manifesti; güncelleme öncesi yedek ve geri dönüş.

### İndirme

| Dosya | Açıklama |
|---|---|
| `DELIL-Setup-<sürüm>-x64.exe` | Windows 11 x64 kurulum dosyası (uygulama + modeller dahil) |
| `DELIL-Setup-<sürüm>-x64.exe.sha256` | Kurulum dosyasının SHA-256 checksum'ı |
| `MODEL_MANIFEST.json` | Pakete dahil edilen model bileşenlerinin kaynak/sürüm/hash kaydı |

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
| DELİL | 0.2.0 | MIT |

Ayrıntılı hash ve kaynak bilgisi için `MODEL_MANIFEST.json` dosyasına bakın.

### Uygulama imzası

**Bu sürümün yürütülebilir dosyaları İMZALANMAMIŞTIR.** Teslim ortamında
kullanılabilir bir kod imzalama sertifikası bulunmadığından, Windows
SmartScreen ilk çalıştırmada "Bilinmeyen yayımcı" uyarısı gösterebilir.
İmzalanmamış uygulama, imzalıymış gibi gösterilmez.

### Bilinen sınırlamalar

- **Gerçek UYAP Editör uyumluluk testi:** Resmî UYAP Editör Windows'a özeldir.
  UDF biçimi açık kaynak referans uygulamaları incelenerek doğrulanmıştır;
  üretilen dosyalar DELİL'in kendi doğrulayıcısından geçer. Ancak resmî UYAP
  Editör ile uyumluluk, teslim ortamında **doğrulanmamıştır**.
- **Türkçe WER/CER ölçümü:** Ölçüm altyapısı hazırdır; insan tarafından
  doğrulanmış Türkçe test setiyle gerçek ölçüm yapılması önerilir.
- **GPU hızlandırma:** Kod GPU desteğini otomatik algılar; ancak gerçek
  donanımda test edilmeden "destekleniyor" olarak bildirilmez.

### Kapsam dışı (bilinçli olarak)

Fiziksel RAW/E01 imaj üretimi, silinmiş veri kurtarma, genel adli bilişim,
cep telefonu incelemesi, konuşmacı biyometrik kimlik tespiti, ses
orijinalliği/montaj incelemesi, deepfake analizi, hukuki değerlendirme.

### Kurulum

1. `DELIL-Setup-<sürüm>-x64.exe` dosyasını indirin.
2. SHA-256 checksum'ı doğrulayın.
3. Kurulumu çalıştırın (yönetici izni gerekir).
4. Masaüstündeki DELİL kısayolundan uygulamayı açın.

Kullanıcıdan Rust, Cargo, Python, Node.js, FFmpeg, Whisper modeli veya OCR
modeli kurması **istenmez**; tüm bileşenler kurulum paketine dahildir.

---

_Bu sürüm notları bir AI ajanı (OpenHands) tarafından kullanıcı adına
hazırlanmıştır._

