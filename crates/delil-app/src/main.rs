//! DELİL Windows masaüstü arayüzü (egui/eframe).
//!
//! Arayüz yalnızca ürünün temel işine hizmet eder ve Türkçedir. Ana ekranda
//! tek ana eylem bulunur: **YENİ BİLİRKİŞİ RAPORU OLUŞTUR**. İşlem ekranında
//! adımlar sade biçimde gösterilir; teknik loglar ayrı bir ayrıntı panelinde
//! tutulur. Kontrol ekranında belirsiz bölümler dinlenebilir, transkript
//! düzeltilebilir ve soru yanıtları tamamlanabilir.

// Windows'ta konsol penceresi açılmasını engeller: bu bir GUI uygulamasıdır.
// (Konsol penceresi, kullanıcının "arka planda siyah ekran" olarak gördüğü
// şeydi.)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod screens;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([820.0, 560.0])
            .with_title("DELİL — Otomatik Ses Çözümleme ve UDF Raporlama"),
        ..Default::default()
    };
    eframe::run_native(
        "DELİL",
        options,
        Box::new(|_cc| Ok(Box::new(app::DelilApp::new()))),
    )
}
