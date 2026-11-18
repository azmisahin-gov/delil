# DELİL — Kurulum Kılavuzu

## Sistem gereksinimleri

Gerçek testlere dayalı gereksinimler (bkz. `docs/TEST_RAPORU.md`):

| | Minimum | Önerilen |
|---|---|---|
| İşletim sistemi | Windows 11 x64 (22H2+) | Windows 11 x64 (güncel) |
| İşlemci | 4 çekirdek x86-64 (AVX2) | 8+ çekirdek |
| RAM | 8 GB | 16 GB |
| Boş disk (uygulama) | 1 GB | 2 GB |
| Boş disk (Whisper large-v3-turbo) | ~1.6 GB | ~1.6 GB |
| Boş disk (çalışma alanı) | Materyalin ~2 katı | Materyalin ~3 katı |
| GPU | Gerekli değil (CPU yeterli) | CUDA 12+ / uyumlu GPU, 6 GB+ VRAM |
| Ses çıkışı | Gerekli (dinleme için) | — |

> Not: GPU hızlandırması CPU'ya göre belirgin hızlanma sağlar; ancak CPU
> desteği zorunludur ve GPU yoksa işlem otomatik olarak CPU'ya düşer.
> Kullanıcıdan CUDA/model/batch/beam/quantization seçimi istenmez.

## Kurulum adımları

1. **Kurulum paketini çalıştırın.**
   `DELIL-Setup-<sürüm>-x64.exe` dosyasını çift tıklayın.

2. **Kurulum konumunu seçin.** Varsayılan:
   `C:\Program Files\DELİL`

3. **Model bileşenleri.** Kurulum paketi aşağıdakileri **içerir**; ek indirme
   gerekmez:
   - Whisper large-v3-turbo model ağırlıkları (`models/`).
   - OCR motoru ve Türkçe/İngilizce tanıma modelleri (`models/ocr/`).
   - Medya çözücüleri (Symphonia; harici FFmpeg gerekmez).
   - Türkçe yazı tipi (rapor PDF'i için).

4. **Kurulumu tamamlayın.** Kısayol masaüstüne ve Başlat menüsüne eklenir.

5. **İlk çalıştırma.** Uygulama açılır; model yerel olarak yüklenir. **İnternet
   bağlantısı gerekmez.**

## Çevrimdışı doğrulama

Kurulumdan sonra ağ bağlantısını kesip şu senaryoyu çalıştırabilirsiniz:

1. Uygulamayı açın.
2. Yeni bilirkişi incelemesi başlatın.
3. Görevlendirme belgesini ve materyali verin.
4. İşlem tamamlanana kadar bekleyin.
5. UDF raporunu UYAP Editör'de açın.

Hiçbir aşamada internet gerekmez.

## Windows güvenlik uyarıları ve uygulama imzası

**Gerçek durum:** Bu sürümün yürütülebilir dosyaları, teslim ortamında
kullanılabilir bir kod imzalama sertifikası bulunmadığından **imzalanmamıştır**.
Windows SmartScreen, ilk çalıştırmada "Bilinmeyen yayımcı" uyarısı gösterebilir.

- İmzalanmamış uygulama, **imzalıymış gibi gösterilmez.**
- Kurumsal dağıtımda kod imzalama sertifikası ile imzalanması önerilir.
- Kod imzalama, SmartScreen uyarısını azaltır; ancak uygulamanın güvenliğinin
  yerine geçmez.

## Otomatik güncelleme

- Güncelleme manifesti **ed25519** ile imzalanır.
- Paket **SHA-256** ile doğrulanır.
- Güncelleme öncesi mevcut sürüm **yedeklenir**; kurulum başarısız olursa
  otomatik **geri dönüş** yapılır.
- Güncelleme, aktif transkripsiyon veya kopyalama sırasında **uygulanmaz**.
- İnternet yoksa mevcut özellikler çalışmaya devam eder.
- **Çevrimdışı güncelleme paketi** ile de güvenli güncelleme yapılabilir.
- Vaka belgeleri hiçbir koşulda güncelleme sunucusuna gönderilmez.

## Kaldırma

Denetim Masası → Programlar → DELİL → Kaldır. Kaldırma, **vaka deposunu
silmez**; vakalar varsayılan olarak `%USERPROFILE%\DELİL\vakalar` altında
kalır. Vaka verilerini ayrıca şifreli yedeklemeniz önerilir.

## Kaynak koddan derleme (geliştiriciler için)

Son kullanıcıdan Rust/Cargo/Python/Node/FFmpeg kurması **istenmez**. Yalnızca
kaynak koddan derlemek isteyen geliştiriciler için:

```powershell
rustup toolchain install stable
git clone https://github.com/azmisahin-gov/delil
cd delil
cargo build --release -p delil-app --features "candle decode"
```

Ayrıntı için `docs/MIMARI.md`.
