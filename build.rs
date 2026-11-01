fn main() {
    // Build-скрипт всегда компилируется под хост-платформу, поэтому
    // `#[cfg(windows)]` проверяет машину сборки, а не целевую.
    // Ресурсы Windows нужны только когда собираем ПОД Windows.
    if std::env::var("CARGO_CFG_TARGET_FAMILY").as_deref() != Ok("windows") {
        return;
    }

    #[cfg(windows)]
    {
        let icon_path = "assets/icon.ico";
        let mut res = winresource::WindowsResource::new();

        if std::path::Path::new(icon_path).exists() {
            res.set_icon(icon_path);
        } else {
            println!("cargo:warning=Иконка {} не найдена", icon_path);
        }

        res.set("ProductName", "WOL Listener");
        res.set(
            "FileDescription",
            "Listens for Wake-on-LAN magic packets and executes configured command.",
        );
        res.set("CompanyName", env!("CARGO_PKG_AUTHORS"));
        res.set("LegalCopyright", env!("CARGO_PKG_AUTHORS"));
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));

        if let Err(e) = res.compile() {
            eprintln!("Ошибка компиляции ресурсов Windows: {}", e);
            std::process::exit(1);
        }
    }
}
