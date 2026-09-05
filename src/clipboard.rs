use base64::{Engine, engine::general_purpose::STANDARD};
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyResult {
    pub osc52: String,
    pub native: bool,
}

pub fn osc52_sequence(text: &str) -> String {
    format!("\u{1b}]52;c;{}\u{7}", STANDARD.encode(text))
}

pub fn copy_text(text: &str) -> CopyResult {
    CopyResult {
        osc52: osc52_sequence(text),
        native: copy_native(text),
    }
}

fn copy_native(text: &str) -> bool {
    for (program, args) in [
        ("wl-copy", vec![] as Vec<&str>),
        ("pbcopy", vec![]),
        ("xclip", vec!["-selection", "clipboard"]),
    ] {
        if pipe_to(program, &args, text) {
            return true;
        }
    }
    false
}

fn pipe_to(program: &str, args: &[&str], text: &str) -> bool {
    let Ok(mut child) = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let Some(mut stdin) = child.stdin.take() else {
        let _ = child.kill();
        return false;
    };
    if stdin.write_all(text.as_bytes()).is_err() {
        let _ = child.kill();
        return false;
    }
    drop(stdin);
    child.wait().is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc52_copy_sequence_is_base64_encoded_and_terminal_safe() {
        assert_eq!(
            osc52_sequence("kubectl get pods"),
            "\u{1b}]52;c;a3ViZWN0bCBnZXQgcG9kcw==\u{7}"
        );
    }

    #[test]
    fn copy_text_always_includes_osc52() {
        let copied = copy_text("echo hi");
        assert_eq!(copied.osc52, osc52_sequence("echo hi"));
    }
}
