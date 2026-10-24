fn main() {
    println!("cargo:rerun-if-changed=assets/icons");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let target = out.ancestors().nth(3).unwrap().join("icons");
    std::fs::create_dir_all(&target).expect("create runtime icon directory");
    for entry in std::fs::read_dir("assets/icons").unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().is_some_and(|ext| ext == "svg") {
            std::fs::copy(entry.path(), target.join(entry.file_name())).expect("copy runtime SVG");
        }
    }

    println!("cargo:rerun-if-changed=assets/pixloom.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/pixloom.ico")
            .set("ProductName", "Pixloom")
            .compile()
            .expect("compile Pixloom Windows icon resource");
    }
}
