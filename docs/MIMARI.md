# DELİL — Teknik Mimari

## Genel bakış

DELİL, Rust ile yazılmış, Windows 11 x64 üzerinde çalışan yerel bir masaüstü
uygulamasıdır. Ana ve desteklenen platform Windows 11 x64'tür. Uygulama
tamamen çevrimdışıdır; bulut ASR, bulut OCR veya çevrimiçi yapay zekâ API'si
kullanılmaz.

## Teknoloji seçimleri

| Bileşen | Seçim | Gerekçe |
|---|---|---|
| Dil | Rust (edition 2021) | Bellek güvenliği, tek ikili, hız |
| Arayüz | egui / eframe 0.28 | Yerel, hafif, GPU hızlandırmalı |
| Transkripsiyon | Candle 0.8 + Whisper large-v3-turbo | Yerel, saf Rust çıkarım |
| Ses çözme | Yerleşik WAV PCM çözücü + Symphonia | Harici FFmpeg gerekmez |
| OCR | ocrs 0.14 + rten | Yerel, çevrimdışı |
| Veritabanı/meta | JSON + şifreli zarf | Basit, denetlenebilir |
| Şifreleme | AES-256-GCM + Argon2id + DPAPI | Doğrulanmış AEAD |
| UDF | zip + quick-xml | Gerçek UDF biçimi |
| PDF | printpdf + gömülü DejaVu | Türkçe karakter desteği |
| Güncelleme | ed25519-dalek + SHA-256 | İmzalı, doğrulanabilir |

**Kullanılmayan teknolojiler:** Electron, Tauri, Node.js, Python çalışma
zamanı, bulut ASR/OCR, kullanıcının kuracağı FFmpeg, kullanıcının indireceği
Whisper modeli, kullanıcının Cargo/Rust kurmasını gerektiren teslim.

## Crate bağımlılık grafiği

```mermaid
graph TD
  core[delil-core]
  hash[delil-hash] --> core
  udf[delil-udf] --> core
  audio[delil-audio] --> core
  evidence[delil-evidence] --> core
  evidence --> hash
  evidence --> audio
  assignment[delil-assignment] --> core
  assignment --> udf
  transcription[delil-transcription] --> core
  transcription --> audio
  quality[delil-quality] --> core
  report[delil-report] --> core
  store[delil-case-store] --> core
  store --> hash
  export[delil-export] --> core
  export --> report
  export --> udf
  update[delil-update] --> core
  pipeline[delil-pipeline] --> core
  pipeline --> hash
  pipeline --> store
  pipeline --> evidence
  pipeline --> audio
  pipeline --> assignment
  pipeline --> transcription
  pipeline --> quality
  pipeline --> report
  pipeline --> udf
  pipeline --> export
  cli[delil-cli] --> pipeline
  cli --> store
  cli --> udf
  cli --> audio
  cli --> evidence
  cli --> transcription
  app[delil-app] --> pipeline
  app --> store
  app --> evidence
```

Modüller gereksiz bağımlılıktan uzak tutulur; `delil-core` yalnızca serde,
chrono, uuid ve thiserror kullanır.

## İş akışı (şartname 4.2)

```mermaid
sequenceDiagram
  participant U as Bilirkişi
  participant P as delil-pipeline
  participant E as delil-evidence
  participant T as delil-transcription
  participant Q as delil-quality
  participant R as delil-report
  participant D as delil-udf
  U->>P: Görevlendirme + materyal
  P->>P: Görevlendirmeyi oku (assignment)
  P->>E: Materyali algıla ve al
  E->>E: Kopyala + SHA-256 doğrula
  P->>E: Sesleri içerikten bul
  P->>T: Kuyruğa al ve transkribe et
  T->>T: Whisper (Candle, yerel)
  P->>Q: Kalite kontrolü
  P->>R: Rapor taslağı
  P->>D: UDF üret
  D->>D: Yapısal doğrulama
  P->>U: UDF + PDF + ekler
```

## Güvenlik ve veri akışı

```mermaid
graph LR
  src[Kaynak aygıt / klasör] -->|salt okunur, yalnızca kopya| case[Yerel vaka deposu]
  case -->|AES-256-GCM| disk[(Şifreli disk)]
  key[DPAPI korumalı ana anahtar] --> disk
  case --> udf[UDF raporu]
  case --> pdf[PDF kopyası]
  case --> backup[Parola korumalı yedek]
  cloud[Bulut] -.->|veri gönderilmez| case
```

Hiçbir ses, transkript, görevlendirme, taraf bilgisi, dosya numarası, rapor
veya materyal manifesti bulut sistemine gönderilmez.

## UDF biçimi

UDF, Deflate sıkıştırmalı bir ZIP arşividir ve içinde `content.xml` barındırır.
Metin havuzu (CDATA) + karakter tabanlı ofsetler yaklaşımı kullanılır. Ayrıntı
için `AGENTS.md` ve `docs/UDF_UYUMLULUK.md`.

## İş parçacığı modeli

- Arayüz ana iş parçacığında çalışır (egui).
- İş akışı ayrı bir iş parçacığında yürütülür; ilerleme bir kanal (mpsc)
  üzerinden arayüze aktarılır.
- Transkripsiyon kuyruğu tek iş parçacığında sıralı çalışır; uzun işler
  duraklatma/iptal bayrağını düzenli aralıklarla kontrol eder.
- Kuyruk durumu JSON olarak diske yazılır; kesintiden devam eder.

## Performans ve bellek

- Hash hesaplama 1 MiB tamponla akış hâlinde yapılır; büyük dosyalarda bellek
  sabit kalır.
- Ses çözme 16 kHz mono f32'ye indirger; Whisper girişi budur.
- Uzun kayıtlar 30 saniyelik parçalara ayrılır; parça sınırında metin kaybı
  kalite kontrolünde ayrıca tespit edilir.

## Hata ve istisna yönetimi

Uygulama yalnızca gerçekten karar gerektiren durumlarda kullanıcıya sorar:
birden fazla harici aygıt, okunamayan dosya numarası, bozuk ses, desteklenmeyen
codec, çok düşük ses, anlaşılmayan bölüm, örtüşen konuşma, şüpheli model
çıktısı, dayanağı bulunamayan soru, doldurulmamış mesleki kanaat. İstisna
ekranı teknik hata kodları yığmaz; sorunu ve gereken kararı sade biçimde
açıklar.
