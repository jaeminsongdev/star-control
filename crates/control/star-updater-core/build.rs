fn main() {
    // Keep updater-owned test harnesses and one-shot binaries out of the
    // Windows elevation-name heuristic.  The updater operates only on the
    // current-user installation and never requires an elevated token.
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTUAC:level='asInvoker' uiAccess='false'");
    }
}
