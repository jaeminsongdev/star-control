pub(super) fn role_for_stage(stage: &str) -> &'static str {
    match stage {
        "design" => "worker-design",
        "implement" => "worker-impl",
        "validate" => "worker-impl",
        "review" => "worker-review",
        "polish" => "worker-polish",
        "report" => "worker-docs",
        _ => "worker-impl",
    }
}
