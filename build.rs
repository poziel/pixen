fn main() {
    println!("cargo:rerun-if-changed=assets/pixloom.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/pixloom.ico")
            .set("ProductName", "Pixloom")
            .compile()
            .expect("compile Pixloom Windows icon resource");
    }
}
