fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icon.ico");
        res.set("ProductName", "Borderline no optimization 4 resigner");
        res.set("FileDescription", "Resigns/Decrypts/Encrypts BL4 Saves");
        res.set("CompanyName", "Anti Denuvo Sanctuary");
        res.set("LegalCopyright", "Copyright (C) 2025 - PRS");
        res.set("FileVersion", "1.3.0.0");
        res.set("ProductVersion", "1.3.0.0");

        if let Err(e) = res.compile() {
            eprintln!("Failed to compile Windows resources: {}", e);
        }
    }
}

#[cfg(not(windows))]
fn main() {}