mod commands;
mod errors;

#[test]
fn sentinel_commands_wrap_star_sentinel_artifacts() {
    commands::sentinel_commands_wrap_star_sentinel_artifacts();
}

#[test]
fn sentinel_rejects_missing_inputs_and_reserved_options() {
    errors::sentinel_rejects_missing_inputs_and_reserved_options();
}
