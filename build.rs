fn main() {
    println!("cargo:rerun-if-changed=assets/ateicon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/ateicon.ico")
            .set("ProductName", "Aurora TLK Explorer")
            .set("FileDescription", "Aurora TLK, 2DA and ITP editor")
            .set("CompanyName", "Aurora Tools contributors")
            .set("OriginalFilename", "Aurora-TLK-Explorer.exe")
            .set(
                "LegalCopyright",
                "Copyright Aurora TLK Explorer contributors; GPL-3.0-or-later",
            );
        resource
            .compile()
            .expect("failed to embed Windows resources");
    }
}
