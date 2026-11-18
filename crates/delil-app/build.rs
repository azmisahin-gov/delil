//! Windows yürütülebilir dosyası için kaynak bilgisi (sürüm, uygulama adı).
//!
//! Yalnızca Windows hedefinde derlenir; diğer platformlarda boş geçer.

fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set("ProductName", "DELİL");
        res.set(
            "FileDescription",
            "DELİL — Otomatik Ses Çözümleme ve UDF Raporlama",
        );
        res.set("CompanyName", "DELİL");
        res.set("LegalCopyright", "MIT Lisansı");
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        if let Err(e) = res.compile() {
            eprintln!("Uyarı: Windows kaynak bilgisi derlenemedi: {e}");
        }
    }
}
