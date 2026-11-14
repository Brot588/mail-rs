fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon.ico");
        resource.set("ProductName", "mail");
        resource.set("FileDescription", "mail - a fast, elegant desktop mail client");
        resource.set("LegalCopyright", "MIT License");
        if let Err(err) = resource.compile() {
            println!("cargo:warning=could not embed the application icon: {err}");
        }
    }
}
