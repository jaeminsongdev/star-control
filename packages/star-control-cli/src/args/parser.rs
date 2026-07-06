use super::model::ParsedArgs;
use super::options::parse_next_argument;
use crate::CliError;

pub(crate) fn parse_args(args: &[String]) -> Result<ParsedArgs, CliError> {
    let Some(command) = args.first().cloned() else {
        return Err(CliError::InvalidInput {
            command: "unknown".to_string(),
            message: "missing command".to_string(),
        });
    };

    let mut parsed = ParsedArgs::new(command);
    let mut index = 1;
    while index < args.len() {
        parse_next_argument(args, &mut index, &mut parsed)?;
        index += 1;
    }

    Ok(parsed)
}
