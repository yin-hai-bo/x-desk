fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=src/win/resources.rc");
        println!("cargo:rerun-if-changed=src/win/app.manifest");
        println!("cargo:rerun-if-changed=assets/icon.ico");
        embed_resource::compile("src/win/resources.rc", embed_resource::NONE);
    }
}
