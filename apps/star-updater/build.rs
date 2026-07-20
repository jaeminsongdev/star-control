fn main() {
    // Windows may apply an elevation heuristic to executables named *updater*.
    // This updater only modifies the per-user Star-Control installation, so it
    // must always remain an ordinary-user background process.
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTUAC:level='asInvoker' uiAccess='false'");
    }
}
