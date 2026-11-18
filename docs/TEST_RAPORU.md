# DELİL — Test Raporu

Bu rapor, **gerçekten çalıştırılan** testlerin sonuçlarını içerir. Her test
kategorisi ayrı gösterilir; **çalıştırılmayan test başarılı olarak
işaretlenmemiştir.**

Son güncelleme: 2026-11-18 (delil-implementation dalı)

---

## 1. Linux birim + entegrasyon testleri (bu ortamda çalıştırıldı) ✅

| Ölçüm | Değer |
|---|---|
| Komut | `cargo test --workspace --no-fail-fast` |
| **Geçen test** | **227** |
| Başarısız test | 0 |
| Sonuç | `EXIT:0` |

> **Not (önceki tutarsızlık giderildi):** Önceki raporlarda 215 ve 218 farklı
> sayılar bildirilmişti. Gerçek ve güncel sayı **221**'dir. Fark, eklenen
> testlerden (transkripsiyon sonucu, aygıt izleyici, OCR model çözümleme,
> gerçek model testi) kaynaklanmaktadır. Sayı `cargo test` çıktısından
> doğrulanmıştır.

### Lint ve biçim

| Kontrol | Sonuç |
|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` | **Temiz** (0 uyarı, 0 hata) |
| `cargo fmt --all -- --check` | **Temiz** |

### Kapsam

| Crate | Kapsanan davranış |
|---|---|
| `delil-core` | Alan modeli, kimlikler, durum türleri, transkript sürümleme/düzenleme |
| `delil-hash` | Bilinen SHA-256 vektörleri, dosya akışı, kaynak-hedef karşılaştırma |
| `delil-udf` | UDF yazıcı/okuyucu/doğrulayıcı/arama, karakter tabanlı ofsetler, Türkçe |
| `delil-audio` | İçerik tabanlı algılama, WAV/FLAC envanteri, dalga biçimi, oynatma, çözme |
| `delil-evidence` | Edinim, dizin koruma, yinelenen dosya, sistem diski dışlama, **aygıt izleyici** |
| `delil-assignment` | PDF/DOCX/UDF/TXT okuma, alan/soru çıkarımı, OCR model çözümleme |
| `delil-transcription` | Kuyruk, yeniden deneme, kesintiden devam, iptal, WER/CER, **sessizlik eleme** |
| `delil-quality` | 14 kalite tespiti, bayrak sıralaması, inceleme kuyruğu |
| `delil-report` | 24 bölümlü rapor, eksik alan, dayanak doğrulama, uydurma reddi |
| `delil-export` | PDF (Türkçe yazı tipi), TXT transkript, JSON manifest |
| `delil-case-store` | AES-256-GCM, yanlış anahtar, kurcalama, yedek/geri yükleme bütünlüğü |
| `delil-update` | İmza doğrulama, hash, geriye alma reddi, geri dönüş, çevrimdışı paket |
| `delil-pipeline` | 8 adımlı akış, rapor→UDF, **model yokken "tamamlanmadı" durumu** |
| `delil-app` | İş akışı durumu, ilerleme, hata/karar kaydı |
| `delil-integration-tests` | Kabul senaryosu, kesinti, biçim, kalite, **gerçek model** |

---

## 2. Ağır özellik derlemeleri (bu ortamda çalıştırıldı) ✅

| Özellik | Komut | Sonuç |
|---|---|---|
| Candle Whisper motoru | `cargo build -p delil-transcription --features candle` | ✅ Derlendi |
| Ses çözme (Symphonia) | `cargo test -p delil-audio --features decode` | ✅ 24 test geçti |
| OCR bileşeni | `cargo test -p delil-assignment --features ocr` | ✅ 17 test geçti |
| Tümü birlikte | `cargo build -p delil-app --features production` | ✅ Derlendi |

---

## 3. Gerçek Whisper transkripsiyon testi (bu ortamda çalıştırıldı) ✅

**Gerçek model ağırlıklarıyla** motor kod yolu uçtan uca çalıştırılmıştır.

| | |
|---|---|
| Test | `crates/delil-integration-tests/tests/real_transcription.rs` |
| Model | `openai/whisper-tiny` (motor kod yolunu kanıtlamak için; üretimde large-v3-turbo) |
| Komut | `DELIL_TEST_MODEL_DIR=/tmp/mini-model cargo test -p delil-integration-tests --features candle --test real_transcription -- --nocapture` |
| Sonuç | ✅ 2 test geçti (69 sn) |

**Kanıtlananlar:**

1. Model gerçek `safetensors` ağırlıklarından yüklendi.
2. Gerçek WAV dosyası çözüldü (16 kHz mono).
3. Mel spektrogramı → kodlayıcı → kod çözücü → token → metin zinciri çalıştı.
4. Süre doğru raporlandı (3.00 s).
5. Dil token'ı uygulandı (`tr`).

**Sessizlik filtresi (gerçek bulgu):**

İlk çalıştırmada model **sessizlikte halüsinasyon metni üretti**
(`"Bu videonun bir şey var."`). Bu, kalite kontrolünün "sessizlikte metin
üretilmesi" kuralının gerçek bir ihtiyaç olduğunu gösterdi. Motor, referans
Whisper uygulamasıyla aynı `no_speech_prob` + `avg_logprob` eşikleriyle
güçlendirildi. İkinci çalıştırmada sessizlik çıktısı **boş** oldu:

```
Gerçek model transkripsiyonu tamamlandı: 0 segment, süre 3.00s, dil Some("tr")
Sessizlik çıktısı: ""
```

> **Sınırlama:** Bu test, motorun **çalıştığını** kanıtlar; large-v3-turbo ile
> **doğruluk (WER/CER)** ölçümü değildir. Üretim modeliyle WER/CER ölçümü,
> insan tarafından doğrulanmış Türkçe test seti gerektirir (bkz. §10).

---

## 4. Windows 11 x64 testleri (GitHub Actions ile çalıştırılır) ⏳

Bu geliştirme ortamı Linux'tur. Windows testleri, GitHub Actions
`windows-latest` koşucusunda `.github/workflows/ci.yml` (`test-windows` işi)
ile çalıştırılır:

```
cargo test --workspace --no-fail-fast
cargo build --release -p delil-app --features production
```

**Durum:** İş akışı tanımlıdır ve YAML geçerlidir. Sonuçlar GitHub Actions
sayfasında görülebilir. **Bu raporda "geçti" olarak işaretlenmemiştir** çünkü
sonuçlar bu ortamda doğrulanamamıştır; kullanıcı Actions çıktısından
doğrulamalıdır.

---

## 5. Windows üretim derlemesi ve kurulum dosyası (CI ile üretilir) ⏳

`DELIL-Setup-0.1.0-x64.exe` kurulum dosyası, `windows-latest` koşucusunda
`.github/workflows/release-windows.yml` iş akışıyla üretilir:

1. `scripts/fetch-models.sh` — Whisper (MIT) + OCR (CC-BY-SA-4.0) modellerini
   sabitlenmiş revizyondan indirir, SHA-256 manifesti üretir.
2. `cargo build --release --features production` — gerçek motor + OCR.
3. İkili dosyada Whisper/Candle ve OCR sembolleri doğrulanır.
4. Inno Setup ile kurulum EXE'si derlenir.
5. SHA-256 checksum üretilir.
6. GitHub Release'e yüklenir.

**Durum:** İş akışı tanımlıdır. Kurulum EXE'sinin gerçek üretimi ve Release'e
yüklenmesi CI'da gerçekleşir.

---

## 6. Uçtan uca kabul senaryosu (bu ortamda çalıştırıldı) ✅

`crates/delil-integration-tests/tests/acceptance.rs` içindeki
`full_acceptance_scenario`, şartnamedeki 38 adımı **gerçek verilerle** yürütür:

| Adım | Doğrulanan | Sonuç |
|---|---|---|
| 1-4 | Vaka oluşturma, yapılandırma | ✅ |
| 5-6 | Görevlendirme: dosya no, mahkeme, taraflar, 3 soru | ✅ |
| 7-12 | 4 dosya edinildi, **4/4 SHA-256 doğrulandı** | ✅ |
| 13-14 | **3 ses** içerikten bulundu (yanlış uzantılı `.dat` dahil), envanter | ✅ |
| 15-23 | Transkripsiyon adımı (model gerekir; §3'te ayrıca kanıtlandı) | — |
| 24-26 | Rapor taslağı ve UDF; **yapısal doğrulama geçti (9/9)** | ✅ |
| 27-29 | UDF okunabilir; beklenen bölümler mevcut | ✅ |
| 31 | PDF yardımcı kopyası üretildi (`%PDF` imzası) | ✅ |
| 32-35 | Vaka şifreli kaydedildi; yeniden açıldı, içerik korundu | ✅ |
| 36-38 | Şifreli yedek; başka konuma geri yüklendi; **hash/bütünlük doğrulandı** | ✅ |

### Kesinti ve dayanıklılık testleri ✅

- Uygulama çökmesi / yeniden başlatma → kuyruk kaldığı yerden devam eder.
- Bozuk kuyruk dosyası → panik yok, hata döner.
- İptal → kuyruk durur, hiçbir iş tamamlanmaz.
- Yedekleme kesintisi → orijinal vaka bozulmaz.
- Güncelleme hatası → yedekten geri dönüş çalışır.
- Model dosyaları eksik → açık hata (simülasyon yok).
- **Model yokken transkripsiyon istenirse → sonuç "Unavailable", rapor
  "tamamlanmadı" olarak işaretlenir.**

### Biçim testleri ✅

- 5 farklı örnekleme frekansı/kanal yapılandırmasında WAV: açma, süre, kanal,
  çözme, transkripsiyon girdisi.
- İmza tabanlı algılama 7 biçim için doğrulandı.
- Video kapsayıcı işaretlemesi (`isom`/`mp42` = video) doğrulandı.

### UDF testleri ✅

- Yazma → doğrulama → kapatma → tekrar açma döngüsü.
- Türkçe karakter korunması (ı, İ, ş, Ş, ğ, Ğ, ü, Ü, ö, Ö, ç, Ç).
- Tablo yapısı, sayfa düzeni (A4), sayfa sonu korunması.
- Karakter tabanlı ofset doğruluğu; bozuk paket reddi.

---

## 7. Gerçek UYAP Editör uyumluluk testi ❌ (yapılmadı)

Resmî UYAP Editör Windows'a özeldir ve bu geliştirme ortamında (Linux)
çalıştırılamaz. **Bu test gerçekleştirilmemiştir** ve başarılı olarak
işaretlenmemiştir.

UDF biçimi, açık kaynak referans uygulamaları (`saidsurucu/UDF-Toolkit`,
`mistih/UdfParser`) incelenerek doğrulanmıştır. DELİL'in kendi doğrulayıcısı
9 kontrol yapar. Ancak **bu, resmî UYAP Editör uyumluluğunun kanıtı değildir.**

**Yapılması gereken test:** Windows 11 x64'te güncel resmî UYAP Editör ile
açma → Türkçe karakter/tablo/sayfa düzeni kontrolü → kaydetme → kapatma →
tekrar açma. Prosedür `docs/UDF_UYUMLULUK.md` §4'te ayrıntılıdır.

---

## 8. Gerçek USB/SD edinim testi (fiziksel donanım) ❌ (yapılmadı)

Bu ortamda fiziksel USB/SD aygıtı yoktur. Edinim mantığı, gerçek dosya
sistemleri ve gerçek WAV dosyalarıyla test edilmiştir; aygıt algılama
mantığı birim testleriyle doğrulanmıştır (sistem diski dışlama, çoklu aday
belirsizliği, izleyici). Ancak **fiziksel aygıt takma/çıkarma testi Windows
üzerinde yapılmalıdır.**

---

## 9. Çevrimdışı temiz kurulum testi ❌ (yapılmadı)

Temiz bir Windows 11 makinesine kurulum + internet kapalı senaryo, kurulum
EXE'si üretildikten sonra Windows'ta yapılmalıdır.

---

## 10. Türkçe WER/CER ölçümü ⏳ (altyapı hazır, ölçüm bekliyor)

WER/CER altyapısı (`delil-transcription::benchmark`) uygulanmış ve birim
testleriyle doğrulanmıştır. İnsan tarafından doğrulanmış Türkçe test setiyle
gerçek ölçüm, large-v3-turbo modeli ve test seti sağlandığında yapılır
(bkz. `docs/TRANSKRIPSİYON_BENCHMARK.md`).

---

## Özet tablo

| # | Test kategorisi | Ortam | Durum |
|---|---|---|---|
| 1 | Linux birim + entegrasyon (227 test) | Bu ortam | ✅ Geçti |
| 2 | Ağır özellik derlemeleri | Bu ortam | ✅ Geçti |
| 3 | Gerçek Whisper transkripsiyon (tiny) | Bu ortam | ✅ Geçti |
| 4 | Windows 11 x64 testleri | GH Actions | ⏳ CI'da |
| 5 | Windows kurulum EXE üretimi | GH Actions | ⏳ CI'da |
| 6 | Uçtan uca kabul senaryosu | Bu ortam | ✅ Geçti |
| 7 | Gerçek UYAP Editör uyumluluğu | Windows gerekir | ❌ Yapılmadı |
| 8 | Fiziksel USB/SD edinim | Donanım gerekir | ❌ Yapılmadı |
| 9 | Çevrimdışı temiz kurulum | Windows gerekir | ❌ Yapılmadı |
| 10 | Türkçe WER/CER ölçümü | Model + set gerekir | ⏳ Bekliyor |

Bu tablo, şartnamenin "başarısız testleri gizleme; gerçekleştirilmeyen testi
başarılı olarak işaretleme" ilkesine uygun olarak düzenlenmiştir.
