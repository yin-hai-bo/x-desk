fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=src/win/resources.rc");
        embed_resource::compile("src/win/resources.rc", embed_resource::NONE);
    }
}
