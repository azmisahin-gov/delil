# DELİL — UDF Uyumluluk Raporu

## 1. UDF biçimi (uyguladığımız gerçek yapı)

UDF (UYAP Döküman Formatı), **Deflate sıkıştırmalı bir ZIP arşividir** ve
içinde **tek bir dosya** barındırır: `content.xml` (UTF-8).

```xml
<?xml version="1.0" encoding="UTF-8" ?>
<template format_id="1.8">
<content><![CDATA[metin havuzu]]></content>
<properties><pageFormat mediaSizeName="1" leftMargin="42.51968479156494"
  rightMargin="28.34645652770996" topMargin="14.17322826385498"
  bottomMargin="14.17322826385498" paperOrientation="1"
  headerFOffset="20.0" footerFOffset="20.0" /></properties>
<elements resolver="hvl-default">
  <paragraph Alignment="0" LeftIndent="0.0" RightIndent="0.0">
    <content startOffset="0" length="5" family="Times New Roman" size="11" />
  </paragraph>
</elements>
<style name="default" ... /><style name="hvl-default" ... />
</template>
```

### Doğrulanan kurallar

- `format_id` her zaman `"1.8"`.
- Tüm metin tek CDATA havuzunda: önce üstbilgi, sonra gövde, en son altbilgi.
- `startOffset`/`length` **karakter (rune)** cinsindendir, bayt değil.
  Türkçe karakterler ve emoji dahil her karakter 1 sayılır.
- Paragraflar arası `\n`. Resim yer tutucusu `\uFFFC`. Sekme `\t`.
- Boş paragraf `\u200B` (sıfır genişlikli boşluk).
- Renkler işaretli 32-bit ARGB tam sayı (siyah = `-16777216`, beyaz = `-1`).
- A4: 595.28 × 841.89 punto.
- Tablolar `columnCount`/`columnSpans` ve satır/hücre yapısıyla temsil edilir.
- Üstbilgi/altbilgi `<header>`/`<footer>` öğeleriyle; sayfa numarası
  `pageNumber-spec` ile.

## 2. Uyguladığımız yazıcı özellikleri

- [x] Metin ve paragraflar
- [x] Başlıklar (kalın, farklı punto)
- [x] Biçimlendirme (kalın, italik, altı çizili, yazı tipi, punto, renk)
- [x] Tablolar (sütun genişlikleri, satır/hücre, kenarlık)
- [x] Resimler (base64 gömülü, punto cinsinden boyut)
- [x] Sayfa düzeni (A4, kenar boşlukları, yön)
- [x] Üstbilgi/altbilgi ve sayfa numarası
- [x] Listeler (madde işaretli ve numaralı, tüm türler)
- [x] Sayfa sonları
- [x] Ek düzeni (her transkript ayrı ek)

Desteklenmeyen biçimlendirme sessizce kaybolmaz; yazıcı yalnızca UDF'nin
desteklediği öznitelikleri üretir ve kaybedeceği bir şey bırakmaz.

## 3. Kendi doğrulayıcımızın kontrolleri

`delil udf-dogrula <dosya>` komutu şunları kontrol eder:

1. UTF-8 kodlama geçerliliği
2. XML bildirimi varlığı
3. `format_id="1.8"` varlığı
4. CDATA metin havuzu varlığı
5. XML yapısal çözümlenebilirliği
6. **Metin/ofset tutarlılığı** — her `startOffset`/`length` havuz sınırları
   içinde mi?
7. Sayfa özellikleri (A4, kenar boşlukları)
8. Tablo sütun sayısı tutarlılığı
9. Karakter geçerliliği ve Türkçe karakter korunması

Örnek çıktı:

```
[OK] UTF-8 kodlama — content.xml geçerli UTF-8
[OK] XML bildirimi — XML bildirimi mevcut
[OK] format_id — format_id="1.8"
[OK] Metin havuzu (CDATA) — CDATA bloğu mevcut
[OK] XML yapısı — content.xml başarıyla çözümlendi
[OK] Metin/ofset tutarlılığı — 412 ofset aralığı geçerli (havuz 1834 karakter)
[OK] Sayfa özellikleri — A4=true, kenar boşlukları=42.52/28.35
[OK] Tablo yapısı — 2 tablo, sütun sayıları tutarlı
[OK] Karakter geçerliliği — Havuzda Türkçe karakter: true, kontrol karakteri: yok

UDF doğrulaması başarılı (9/9 kontrol geçti).
```

## 4. ÖNEMLİ SINIRLAMA: Gerçek UYAP Editör testi

> **DELİL'in kendi doğrulayıcısı, resmî UYAP Editör testinin yerine geçmez.**

Bu teslim ortamı bir **Linux geliştirme konteyneridir**; resmî UYAP Editör
yalnızca Windows üzerinde çalışır ve bu ortamda çalıştırılamaz. Bu nedenle
aşağıdaki test adımları **bu ortamda gerçekleştirilememiştir** ve
**gizlenmemektedir**:

- [ ] UDF'nin güncel resmî UYAP Editör'de açılması
- [ ] Editör'de kaydedilmesi ve kapatılması
- [ ] Tekrar açılması ve içerik/sayfa görünümünün kontrolü
- [ ] Yazdırma/önizleme kontrolü
- [ ] Resmî imza akışı için kullanılabilirliğin sınanması

### Teslim öncesi yapılması zorunlu test (Windows 11 x64)

1. Güncel resmî Windows UYAP Editör sürümünü
   <https://uyap.gov.tr> adresinden edinin.
2. `delil inceleme …` ile bir `BILIRKISI_RAPORU.udf` üretin.
3. Dosyayı UYAP Editör'de açın.
4. İçeriği kontrol edin (metin, tablolar, sayfa düzeni).
5. Türkçe karakterleri kontrol edin (ı, İ, ş, Ş, ğ, Ğ, ü, Ü, ö, Ö, ç, Ç).
6. Yazdırma/önizleme görünümünü kontrol edin.
7. Editör'de kaydedin ve kapatın.
8. Kaydedilen dosyayı tekrar açın; içerik ve sayfa görünümünü yeniden
   kontrol edin.
9. Resmî imza akışı için kullanılabilirliği sınayın.
10. Sonuçları bu rapora işleyin.

Araştırma sırasında resmî sayfada Windows için **v5.4.20** yayımlanmış olarak
görülmüştür; teslim tarihindeki güncel sürüm yeniden kontrol edilmelidir.

## 5. Kaynakça

UDF biçimi, açık kaynak referans uygulamaları incelenerek doğrulanmıştır:

- `saidsurucu/UDF-Toolkit` — `Docs.md` (biçim dokümantasyonu) ve
  `docx_to_udf.py`, `paragraph_processor.py`, `table_processor.py` (yazıcı).
- `mistih/UdfParser` — UDF ayrıştırıcı.
- `mertcelen/udf-okuyucu` — basit UDF okuyucu.
