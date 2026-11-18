# DELİL — Desteklenen Dosya Biçimleri Matrisi

Algılama **dosya uzantısına güvenmez**: dosyanın ilk baytlarındaki imza
(magic bytes) incelenir. Bu nedenle yanlış uzantılı bir dosya sessizce
atlanmaz; içeriğine göre sınıflandırılır.

## Ses biçimleri

| Biçim | Algılama imzası | Envanter | Çözme | Notlar |
|---|---|---|---|---|
| WAV | `RIFF....WAVE` | Tam (PCM) | Yerleşik | 8/16/32-bit PCM, IEEE float |
| BWF | `RIFF....WAVE` + `bext` | Tam | Yerleşik | Yayın dalga biçimi |
| MP3 | `ID3` veya `0xFFEx` | Kısmi | Symphonia¹ | Süre için tam çözme |
| M4A | `ftyp` + `M4A `/`M4B ` | Kısmi | Symphonia¹ | ISO-BMFF |
| AAC | ADTS `0xFFF1`/`0xFFF9` | Kısmi | Symphonia¹ | |
| FLAC | `fLaC` | Tam (STREAMINFO) | Symphonia¹ | |
| OGG | `OggS` | Kısmi | Symphonia¹ | Vorbis |
| Opus | `OggS` + `OpusHead` | Kısmi | Symphonia¹ | |
| WMA | ASF GUID | Kısmi | Symphonia¹ | |
| AMR | `#!AMR` | Kısmi | Symphonia¹ | |
| AIFF | `FORM....AIFF` | Kısmi | Symphonia¹ | |
| CAF | `caff` | Kısmi | Symphonia¹ | |

¹ Symphonia desteği `decode` özelliğiyle derlenir. Kurulum paketi bu özelliği
içerir.

## Video kapsayıcıları (yalnızca ses akışı)

| Biçim | Algılama imzası | Davranış |
|---|---|---|
| MP4 | `ftyp` (genel markalar) | Yalnızca ses akışı çıkarılır |
| MOV | `ftypqt  ` | Yalnızca ses akışı çıkarılır |
| MKV | EBML `1A45DFA3` | Yalnızca ses akışı çıkarılır |
| WebM | EBML + `webm` | Yalnızca ses akışı çıkarılır |

> Video görüntüsü analiz edilmez; görüntüden delil incelemesi yapılmaz.
> Yalnızca ses akışı metne dönüştürme amacıyla işlenir.

## Görevlendirme belgesi biçimleri

| Biçim | Okuma yöntemi | Notlar |
|---|---|---|
| PDF | Metin katmanı | Taranmışsa OCR'a yönlendirilir |
| UDF | Kendi UDF okuyucu | `content.xml` metin havuzu |
| DOCX | `word/document.xml` | Yerel çözümleme |
| DOC | Yerel dönüştürme yolu | Bulunamazsa açıkça bildirilir |
| TXT | Düz metin | UTF-8 |
| PNG/JPG/TIFF/BMP/WebP | Yerel OCR | ocrs + rten, çevrimdışı |

## Arşivler

| Biçim | Davranış |
|---|---|
| ZIP | Açılır, içeriği taranır |

## Desteklenmeyen codec davranışı

Desteklenmeyen codec kullanıcıdan manuel dönüştürme istemeden **açıkça
bildirilir**; dosya sessizce atlanmaz. Arayüzde ve sonuç listesinde
`unsupported_reason` alanı gösterilir.
