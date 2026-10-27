fn main() {
    println!("cargo:rerun-if-changed=assets/pixen.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/pixen.ico")
            .set("ProductName", "Pixen")
            .compile()
            .expect("compile Pixen Windows icon resource");
    }
}
