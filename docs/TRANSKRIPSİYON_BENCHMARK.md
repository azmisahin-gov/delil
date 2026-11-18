# DELİL — Türkçe Transkripsiyon Benchmark Raporu

## Amaç

Whisper large-v3-turbo modelinin Türkçe transkripsiyon niteliğini gerçek
kullanım koşullarında ölçmek ve model güncellemelerinde aynı ölçümü
tekrarlayabilmek.

## Ölçüm altyapısı

Ölçüm altyapısı `delil-transcription::benchmark` modülünde uygulanmış ve
birim testleriyle doğrulanmıştır:

| Ölçüm | İşlev |
|---|---|
| WER (kelime hatası oranı) | `compute_wer(reference, hypothesis)` |
| CER (karakter hatası oranı) | `compute_cer(reference, hypothesis)` |
| İkame/silme/ekleme ayrıştırması | Hizalama geri izlemesi |
| Hata sınıfları | `evaluate_classes(...)` |

Türkçe normalizasyon: büyük/küçük harf, noktalama ve Türkçe **İ/I**
dönüşümü dikkate alınır (`normalize_words`, `normalize_chars`).

### Ayrı değerlendirilen hata sınıfları (şartname 19.1)

- Özel isim hataları
- Sayı, tarih ve tutar hataları
- Olumsuzluk sözcüğü hataları
- Metin atlama (silme)
- Sessizlikte metin üretme (halüsinasyon)
- Zaman kodu hataları

## Test seti planı

İnsan tarafından doğrulanmış Türkçe test seti şu kayıt türlerini içermelidir:

| # | Kayıt türü | Durum |
|---|---|---|
| 1 | Temiz konuşma | Bekliyor¹ |
| 2 | Telefon görüşmesi | Bekliyor¹ |
| 3 | Gürültülü kayıt | Bekliyor¹ |
| 4 | Düşük ses | Bekliyor¹ |
| 5 | Çok konuşmacı | Bekliyor¹ |
| 6 | Örtüşen konuşma | Bekliyor¹ |
| 7 | Hızlı konuşma | Bekliyor¹ |
| 8 | Farklı aksanlar | Bekliyor¹ |
| 9 | Uzak mikrofon | Bekliyor¹ |
| 10 | Uzun kayıt | Bekliyor¹ |
| 11 | Kesintili kayıt | Bekliyor¹ |
| 12 | Özel isimler | Bekliyor¹ |
| 13 | Yer ve kurum adları | Bekliyor¹ |
| 14 | Sayı, tarih ve tutar | Bekliyor¹ |
| 15 | Olumsuzluk ifadeleri | Bekliyor¹ |

¹ **Sınırlama:** İnsan tarafından doğrulanmış gerçek Türkçe test seti ve
Whisper model ağırlıkları (~1.6 GB) bu teslim ortamında bulunmamaktadır. Bu
nedenle gerçek WER/CER **ölçümü** bu ortamda yapılamamıştır. Ölçüm altyapısı
hazır ve test edilmiştir.

## Ölçümün çalıştırılması (model ve test seti sağlandığında)

```rust
use delil_transcription::benchmark::*;

let reference = "Ahmet Yılmaz 12.05.2024 tarihinde 5000 TL ödedi.";
let hypothesis = motor_cikti;
let wer = compute_wer(reference, hypothesis);
let classes = evaluate_classes(reference, hypothesis, false);

println!("WER: {:.3}, CER: {:.3}", wer.wer, wer.cer);
println!("Özel isim hatası: {}", classes.proper_noun_errors);
println!("Sayı hatası: {}", classes.number_errors);
println!("Olumsuzluk hatası: {}", classes.negation_errors);
println!("Halüsinasyon: {}", classes.hallucinated_words);
```

Toplu ölçüm için `BenchmarkReport::from_samples(...)` kullanılır.

## Ölçüm altyapısının doğrulanması (bu ortamda çalıştırıldı)

Aşağıdaki davranışlar birim testleriyle doğrulanmıştır:

| Test | Sonuç |
|---|---|
| Birebir eşleşme → WER 0.0, CER 0.0 | ✅ |
| İkame tespiti (2 kelimede 1 ikame → WER 0.5) | ✅ |
| Silme tespiti | ✅ |
| Ekleme tespiti | ✅ |
| Türkçe İ/I normalizasyonu ("İstanbul" ≈ "istanbul") | ✅ |
| CER hesabı | ✅ |
| Özel isim hatası sayımı | ✅ |
| Sayı/tarih hatası sayımı | ✅ |
| Olumsuzluk hatası sayımı (Türkçe ek toleransı) | ✅ |
| Halüsinasyon sayımı | ✅ |
| Sessizlikte metin sayımı | ✅ |
| Toplu özet (ortalama WER/CER) | ✅ |
| Boş referans güvenliği | ✅ |

## Sonuç ve uyarı

> **Model başarısı kesin doğruluk garantisi olarak ifade edilmez.** WER/CER
> ölçümleri yalnızca izleme ve karşılaştırma amaçlıdır. Model güncellemeleri
> aynı test setiyle yeniden sınanmalıdır. Otomatik transkripsiyon çıktısı
> bilirkişi incelemesine uygun bir **taslaktır**; nihai değerlendirme
> bilirkişiye aittir.

## Donanım ve süre beklentileri

Bu ortamda gerçek ölçüm yapılamadığından, işlem süresi ve donanım
gereksinimleri için **test edilmemiş** kesin performans vaadi verilmez.
Beklenen davranış:

- **CPU:** 4 çekirdek + AVX2 ile gerçek zaman faktörü (RTF) > 1 (yani sesten
  yavaş). Uzun kayıtlarda belirgin süre gerekir.
- **GPU (CUDA/Metal):** Belirgin hızlanma; gerçek donanımda doğrulanmalıdır.
- GPU yoksa işlem otomatik olarak CPU'ya düşer.

Gerçek ölçümler Windows 11 x64 üzerinde, hedef donanımda yapılmalı ve bu
rapora işlenmelidir.
