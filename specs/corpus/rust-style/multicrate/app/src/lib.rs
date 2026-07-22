use rust_style_macros::passthrough;

#[passthrough]
pub fn answer() -> u32 {
    42
}

#[cfg(feature = "cli")]
pub fn cli_enabled() -> bool {
    true
}

#[cfg(target_arch = "aarch64")]
pub fn architecture_label() -> &'static str {
    "arm64"
}

#[cfg(not(target_arch = "aarch64"))]
pub fn architecture_label() -> &'static str {
    "non-arm64"
}

#[cfg(test)]
mod tests {
    #[test]
    fn proc_macro_and_cfg_paths_compile() {
        assert_eq!(super::answer(), 42);
        assert!(!super::architecture_label().is_empty());
    }
}
