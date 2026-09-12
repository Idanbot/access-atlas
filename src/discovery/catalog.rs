use super::{ActionKind, CommandTemplate};

pub(super) fn command(
    id: &str,
    label: &str,
    kind: ActionKind,
    command: String,
    description: &str,
) -> CommandTemplate {
    CommandTemplate {
        id: id.to_owned(),
        label: label.to_owned(),
        kind,
        command,
        description: description.to_owned(),
    }
}

pub(super) fn shell_arg(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-._/:@".contains(character))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

pub(super) fn shell_arg_or_placeholder(value: &str, placeholder: &str) -> String {
    if value.is_empty() {
        placeholder.to_owned()
    } else {
        shell_arg(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_arg_quotes_metacharacters_and_passes_safe_tokens() {
        assert_eq!(shell_arg("prod"), "prod");
        assert_eq!(shell_arg("eu-west-1"), "eu-west-1");
        assert_eq!(shell_arg("evil;true"), "'evil;true'");
        assert_eq!(shell_arg("o'reilly"), "'o'\\''reilly'");
        assert_eq!(shell_arg_or_placeholder("", "<region>"), "<region>");
        assert_eq!(
            shell_arg_or_placeholder("us-east-1", "<region>"),
            "us-east-1"
        );
    }
}
