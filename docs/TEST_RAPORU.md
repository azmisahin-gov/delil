# DELİL — Test Raporu

Bu rapor, teslim edilen kod tabanında çalıştırılan **gerçek** testlerin
sonuçlarını içerir. Sonuçlar `cargo test --workspace --no-fail-fast` çıktısına
dayanır.

## Özet

| Ölçüm | Değer |
|---|---|
| Test ikilisi (binary) | 34 |
| **Geçen test** | **215** |
| Başarısız test | 0 |
| Sonuç | `EXIT:0` |

## Crate bazında kapsam

| Crate | Kapsanan davranış |
|---|---|
| `delil-core` | Alan modeli, kimlikler, durum türleri, transkript sürümleme/düzenleme |
| `delil-hash` | Bilinen SHA-256 vektörleri, dosya akışı, kaynak-hedef karşılaştırma |
| `delil-udf` | UDF yazıcı/okuyucu/doğrulayıcı, karakter tabanlı ofsetler, Türkçe karakter |
| `delil-audio` | İçerik tabanlı algılama (11 biçim), WAV/FLAC envanteri, dalga biçimi, oynatma |
| `delil-evidence` | Edinim, dizin koruma, yinelenen dosya, sistem diski dışlama, belirsizlik |
| `delil-assignment` | PDF/DOCX/UDF/TXT okuma, alan/soru çıkarımı, Türkçe tarih, kaynak ilişkisi |
| `delil-transcription` | Kuyruk, otomatik yeniden deneme, kesintiden devam, iptal, WER/CER |
| `delil-quality` | 14 kalite tespiti, bayrak sıralaması, inceleme kuyruğu |
| `delil-report` | 24 bölümlü rapor, eksik alan, dayanak doğrulama, uydurma reddi |
| `delil-export` | PDF (Türkçe yazı tipi), TXT transkript, JSON manifest |
| `delil-case-store` | AES-256-GCM, yanlış anahtar, kurcalama, yedek/geri yükleme bütünlüğü |
| `delil-update` | İmza doğrulama, hash, geriye alma reddi, geri dönüş, çevrimdışı paket |
| `delil-pipeline` | 8 adımlı akış, rapor→UDF, simülasyon yapmama, eksik alan |
| `delil-app` | İş akışı durumu, ilerleme, hata/karar kaydı |
| `delil-integration-tests` | 38 adımlı kabul senaryosu, kesinti, biçim, kalite |

## Şartname 19'a göre test sınıfları

### 19.1 Türkçe transkripsiyon testleri

WER/CER altyapısı `delil-transcription::benchmark` içinde uygulanmış ve test
edilmiştir (ikame, silme, ekleme, Türkçe İ/I normalizasyonu, özel isim, sayı,
olumsuzluk, halüsinasyon, sessizlikte metin sınıfları).

> **Sınırlama:** İnsan tarafından doğrulanmış gerçek Türkçe test setiyle WER/CER
> **ölçümü**, Whisper large-v3-turbo modelinin ve ses kayıtlarının bu teslim
> ortamında bulunmaması nedeniyle **bu ortamda yapılamamıştır.** Ölçüm
> altyapısı hazırdır; model ve test seti sağlandığında çalıştırılabilir (bkz.
> `docs/TRANSKRIPSİYON_BENCHMARK.md`).

### 19.2 Ses format testleri

5 farklı örnekleme frekansı/kanal yapılandırmasında WAV: açma, süre, kanal,
örnekleme frekansı, çözme ve transkripsiyon girdisi doğrulanmıştır. İmza
tabanlı algılama 7 biçim için test edilmiştir. Video kapsayıcı işaretlemesi
doğrulanmıştır.

### 19.3 UDF testleri

- Yazma → yapısal doğrulama → kapatma → tekrar açma döngüsü.
- Türkçe karakter korunması (ı, İ, ş, Ş, ğ, Ğ, ü, Ü, ö, Ö, ç, Ç).
- Tablo yapısı ve sütun sayısı korunması.
- Sayfa düzeni (A4) ve sayfa sonu korunması.
- Karakter tabanlı ofset doğruluğu.
- Bozuk paket reddi.

> **Sınırlama:** Gerçek UYAP Editör testi bu Linux ortamında yapılamamıştır
> (bkz. `docs/UDF_UYUMLULUK.md` §4).

### 19.4 Delil edinim testleri

- Aygıt algılama (birim testleri ile model düzeyinde).
- Dahili sistem diskini dışlama.
- Kaynak dosyaya yazmama (kaynak içerik önce/sonra karşılaştırması).
- Dosya kopyalama ve dizin yapısı koruma.
- SHA-256 karşılaştırma.
- Büyük dosya (8 MiB) akışlı hash tutarlılığı.
- Kopyalama kesintisi (geçici dosya kalıntısı yok, yeniden çalıştırma başarılı).
- Disk dolması koşulu.
- Aynı hash'e sahip yinelenen dosyalar (silinmez, işaretlenir).

### 19.5 Kesinti testleri

- Uygulama çökmesi / yeniden başlatma → kuyruk kaldığı yerden devam eder.
- Bozuk kuyruk dosyası → panik yok, hata döner.
- İptal → kuyruk durur, hiçbir iş tamamlanmaz.
- Yedekleme kesintisi → orijinal vaka bozulmaz.
- Güncelleme hatası → yedekten geri dönüş çalışır.
- Model dosyaları eksik → açık hata (simülasyon yok).

## Uçtan uca kabul testi (şartname 20)

`delil-integration-tests/tests/acceptance.rs` içindeki
`full_acceptance_scenario`, şartnamedeki 38 adımı **gerçek verilerle** yürütür
ve doğrular:

| Adım | Doğrulanan |
|---|---|
| 1-4 | Vaka oluşturma, yapılandırma |
| 5-6 | Görevlendirme: dosya no, mahkeme, taraflar, 3 soru |
| 7-12 | 4 dosya edinildi, **4/4 SHA-256 doğrulandı** |
| 13-14 | **3 ses** içerikten bulundu (yanlış uzantılı `.dat` dahil), envanter |
| 15-23 | (Transkripsiyon adımı model gerektirir; ayrı test edilir) |
| 24-26 | Rapor taslağı ve UDF; **yapısal doğrulama geçti** |
| 27-29 | UDF okunabilir; beklenen bölümler mevcut |
| 31 | PDF yardımcı kopyası üretildi (`%PDF` imzası) |
| 32-35 | Vaka şifreli kaydedildi; yeniden açıldı, içerik korundu |
| 36-38 | Şifreli yedek; başka konuma geri yüklendi; **hash/bütünlük doğrulandı** |

## Bilinen sınırlamalar (gizlenmemiştir)

1. **Gerçek Whisper transkripsiyonu** bu teslim ortamında çalıştırılmamıştır;
   model ağırlıkları (~1.6 GB) bu konteynerde yoktur. Motor kodu (Candle,
   large-v3-turbo) derlenir ve `--features candle` ile etkinleşir; kuyruk,
   kalite kontrolü ve rapor akışı model olmadan da gerçek verilerle test
   edilmiştir. Yer tutucu motor **transkripsiyon üretmez**; açık hata verir.
2. **Gerçek UYAP Editör testi** Windows gerektirir; bu ortamda yapılamamıştır.
3. **Türkçe transkripsiyon WER/CER ölçümü** model + insan doğrulamalı test seti
   gerektirir; altyapı hazırdır.
4. **OCR** model dosyaları gerektirir; kod `--features ocr` ile derlenir ve
   model dizini yokluğunda açık hata verir.
5. **Windows kurulum paketi** (Inno Setup) üretimi Windows'ta veya çapraz
   derleme ortamında yapılır; betik ve yapılandırma hazırdır
   (`scripts/package-windows.sh`, `packaging/windows/delil.iss`).

Bu sınırlamalar, şartnamenin “gerçekleştirilemeyen bir özellik gizlenmez veya
simüle edilmez” ilkesine uygun olarak açıkça bildirilmiştir.
