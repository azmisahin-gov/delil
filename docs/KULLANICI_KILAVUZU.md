# DELİL — Kullanıcı Kılavuzu (56.12 Bilirkişisi için)

## DELİL nedir?

DELİL, **56.12 — Ses Kayıtlarının Metin Hâline Dönüştürülmesi** uzmanlık
alanındaki bilirkişiler için geliştirilmiş bir Windows masaüstü
uygulamasıdır. Tek bir iş akışına odaklanır:

> **Dijital materyali al → içindeki sesleri bul → sesleri metne dönüştür →
> gerekli kontrolü yaptır → bilirkişi raporunu UDF olarak hazırla.**

Bütün işlemler **yerel ve çevrimdışıdır**. Hiçbir ses, transkript,
görevlendirme, taraf bilgisi, dosya numarası veya rapor buluta gönderilmez.

## Slogan

**Delili tak. Sesi çöz. UDF raporunu al.**

---

## 1. Ana ekran

Uygulama açıldığında tek bir ana eylem görürsünüz:

**YENİ BİLİRKİŞİ RAPORU OLUŞTUR**

Ayrıca:
- **Devam eden inceleme** — yarıda kalan bir iş varsa buradan sürdürülür.
- **Son oluşturulan raporlar** — tamamlanmış raporların listesi.
- **Bekleyen inceleme uyarıları** — dikkat gerektiren konular.

---

## 2. Yeni inceleme

İki temel girdi verirsiniz; her ses dosyasını tek tek seçmeniz gerekmez.

### A. Görevlendirme belgesi

Desteklenen biçimler: **PDF, UDF, DOCX, DOC, TXT** ve **görsel/taranmış belge**
(yerel OCR ile okunur).

Belge verildiğinde uygulama otomatik olarak şunları çıkarır:
- Görevlendiren merci ve mahkeme/birim adı
- Dosya numarası
- Taraf bilgileri
- Görevlendirme tarihi ve rapor teslim süresi
- İnceleme konusu
- Bilirkişiye yöneltilen sorular (orijinal sırayla, değiştirilmeden)
- İncelenecek materyaller ve özel talimatlar

> Görevlendirme belgesi yoksa uygulama yalnızca rapor için zorunlu eksik
> bilgileri ister; sizi gereksiz veri girişine zorlamaz.

### B. Delil materyali

- USB bellek, SD/microSD kart, harici HDD/SSD
- Bilgisayardan seçilen klasör
- Bilgisayardan seçilen tekil dosyalar
- ZIP veya desteklenen arşiv

Aygıt takıldığında uygulama bunu otomatik görür. Birden fazla harici aygıt
bağlıysa ve kaynak belirsizse tek bir seçim istenir.

> **Sistem diski hiçbir zaman otomatik delil kaynağı olarak seçilmez.**

### Bilirkişi bilgileri

Rapor için ad soyad, unvan ve sicil numarası girersiniz.

### İncelemeyi başlat

Butona bastığınızda uygulama kalan tüm adımları kendisi yürütür.

---

## 3. Otomatik işlem

Her adım için ayrı bir düğmeye basmanız gerekmez. Uygulama sırayla:

1. Görevlendirmeyi okur ve bilgileri çıkarır.
2. Materyali algılar ve envantere alır.
3. Dosyaları yerel vaka alanına kopyalar.
4. SHA-256 hesaplar; hedefi yeniden okuyarak doğrular.
5. Ses dosyalarını **içeriklerine göre** bulur (yalnızca uzantıya güvenmez).
6. Ses teknik envanterini çıkarır.
7. Uygun sesleri otomatik transkripsiyon kuyruğuna alır.
8. Yerel Whisper large-v3-turbo ile metne dönüştürür.
9. Kalite kontrolünü çalıştırır; riskli bölümleri işaretler.
10. Rapor taslağını oluşturur.
11. UDF dosyasını üretir ve yapısal doğrulamadan geçirir.
12. Son kontrol ekranını açar.

Teknik loglar ayrı bir **ayrıntı panelinde** tutulur; ana ekranda size
yüklenmez.

### Kaynak materyale dokunulmaz

Edinim **mantıksal dosya edinimidir**:
- Kaynak dosyaya yazılmaz, silinmez, yeniden adlandırılmaz, taşınmaz.
- Kaynak aygıt formatlanmaz.
- Veriler yalnızca yerel çalışma alanına kopyalanır.
- Dizin yapısı ve dosya adları korunur.

> Yazılımsal yazma koruması, **donanımsal yazma engelleyici ile eşdeğer
> değildir**. Yüksek güvence gereken edinimlerde donanımsal yazma engelleyici
> kullanmanız önerilir.

---

## 4. Kontrol ekranı

Bu ekranda mesleki değerlendirmenizi yaparsınız:

### Ses ve metin birlikte inceleme

**Ses tarafı:** oynat/duraklat, dalga biçimi, zaman çizelgesi, geri/ileri
sarma, hızlı/yavaş oynatma, tekrar dinleme, kanal seçimi, işaretleme.

**Metin tarafı:** transkript satırları, zaman kodları, belirsizlik işaretleri,
düzeltme, not ekleme, arama, sürüm karşılaştırma.

- Metin satırına tıkladığınızda ses o noktaya gider.
- Ses oynarken geçerli metin bölümü vurgulanır.

### Yapabilecekleriniz

- Kelimeyi veya cümleyi düzeltmek
- Zaman kodunu düzeltmek
- Anlaşılamayan bölümü işaretlemek
- Konuşmacı etiketi eklemek (ör. “Konuşmacı 1”)
- Not eklemek
- Değişikliği geri almak

> Konuşmacı etiketleri **geçici bir ayrımdır; gerçek kişi kimliği değildir.**

### Kalite kontrolü

Uygulama riskli bölümleri otomatik olarak incelenmek üzere işaretler:
uzun sessizlik, konuşma varken boş transkript, sessizlikte metin üretilmesi,
şüpheli tekrarlar, konuşma dışı kelime üretimi, çok düşük ses, bozuk kayıt
bölümü, örtüşen konuşma, anlaşılamayan bölüm, dil algılama tutarsızlığı,
zaman kodu tutarsızlığı, parça sınırında metin kaybı, özel isim ve sayı hatası
olasılığı, olumsuzluk sözcüklerinde kritik hata olasılığı.

Kalite durumları anlaşılır biçimde gösterilir:
**Normal**, **Kontrol önerilir**, **Öncelikli inceleme**, **Anlaşılamayan bölüm**.

> Model güven/log olasılık değerleri **gerçek doğruluk yüzdesi olarak
> gösterilmez.**

### Transkript statüleri

1. Makine tarafından oluşturulmuş
2. Otomatik kalite kontrolü tamamlanmış
3. Bilirkişi incelemesi bekliyor
4. Bilirkişi tarafından düzeltilmiş
5. Bilirkişi tarafından onaylanmış

> **İlk makine çıktısı hiçbir zaman kaybedilmez.** Düzeltmeler tarih ve
> kullanıcı bilgisiyle kaydedilir.

### Soru yanıtları

Uygulama her soru için ilgili ses ve transkript bölümlerini bulmanıza yardımcı
olur; ilgili transkript parçasını ve zaman kodunu yanıt alanına **dayanak**
olarak ekleyebilirsiniz.

> Yapay zekâ yalnızca kaynak metin ve zaman koduyla **desteklenen** bir yanıt
> taslağı sunabilir. Kaynakta bulunmayan bilgi veya mesleki kanaat uydurmaz.
> **Nihai yanıtı siz yazarsınız veya düzenleyip onaylarsınız.** Boş bırakılan
> veya dayanağı olmayan sorular son kontrolde gösterilir.

---

## 5. Sonuç ekranı

Başarılı sonuçta:

- **UDF raporu** (`BILIRKISI_RAPORU.udf`) — birincil çıktı
- PDF kopyası (`BILIRKISI_RAPORU.pdf`)
- Transkript ekleri (`TRANSKRIPT_EK_1.txt`, …)
- Materyal manifesti (`ISLEM_MANIFESTI.json`)
- Doğrulama raporu

UDF dosyasını doğrudan açabilir ve UYAP Editör'e gönderebilirsiniz.

> **UDF çıktısı doğrudan üretilir.** DOCX'i başka bir programda UDF'ye
> çevirmeniz **istenmez.**

---

## 6. Raporu imzalama ve UYAP'a teslim

### İmza

DELİL, imzaya hazır UDF dosyası üretir; ancak:
- Uygulama kendi içindeki onay kutusunu **e-imza gibi göstermez.**
- Resmî e-imza altyapısı olmadan **imza üretmez.**
- PIN, kart parolası veya gizli anahtar **saklamaz.**

Resmî imza, UYAP Editör / e-imza altyapısı ile yapılır. İmzadan sonra dosya
değişirse uygulama bunu bildirir.

### UYAP Portal

DELİL, teslim klasörünü hazırlar ve resmî teslim adımlarını gösterir. Ancak:
- Otomatik kullanıcı girişi yapılmaz.
- Portal parolası saklanmaz.
- E-imza atlatılmaz.
- Ekran kazıma yapılmaz.
- Teslim edilmiş gibi sahte kayıt oluşturulmaz.

**Nihai portal gönderimini kendi yetkinizle siz yaparsınız.**

---

## 7. Arşiv ve yedekleme

Vakalar şifreli olarak yerel diskte saklanır (AES-256-GCM). Ana anahtar
Windows DPAPI ile korunur.

- **Yedekleme:** şifreli, taşınabilir, parola korumalı.
- **Geri yükleme:** her dosyanın SHA-256'sı doğrulanır.
- Dosyalar uygulama güncellemesinde kaybolmaz.

Komut satırından:

```
delil yedekle ./vaka yedek.delilbackup
delil geri-yukle yedek.delilbackup ./geri-yuklenen
```

---

## 8. Sık sorulan sorular

**İnternet gerekli mi?** Hayır. İlk kurulum ve model yüklemesi dahil her şey
çevrimdışı tamamlanır.

**Modeli ayrıca indirmem gerekiyor mu?** Hayır; kurulum paketiyle gelir.

**FFmpeg kurmam gerekiyor mu?** Hayır.

**Ses dosyalarım nereye gidiyor?** Yalnızca kendi bilgisayarınızdaki yerel
vaka klasörüne kopyalanır. Hiçbir yere gönderilmez.

**UDF'yi Word'de açabilir miyim?** UDF, UYAP Editör'ün biçimidir. Word için
PDF kopyasını kullanabilirsiniz; ancak resmî teslim UDF iledir.

**Transkript %100 doğru mu?** Hayır. Otomatik transkripsiyon bir **taslaktır**;
belirsiz ve şüpheli bölümleri kontrol etmeniz gerekir. Uygulama bu bölümleri
işaretler.

**Konuşmacı kimliği tespit ediliyor mu?** Hayır. Bu ürün konuşmacı biyometrik
kimlik tespiti, ses orijinalliği/montaj incelemesi veya deepfake analizi
**yapmaz**; bunlar kapsam dışıdır.

**Silinmiş veri kurtarılıyor mu?** Hayır. DELİL mantıksal dosya edinimi
yapar; RAW/E01 imaj veya silinmiş veri kurtarma kapsam dışıdır.
