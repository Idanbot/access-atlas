use base64::{Engine, engine::general_purpose::STANDARD};

pub fn osc52_sequence(text: &str) -> String {
    format!("\u{1b}]52;c;{}\u{7}", STANDARD.encode(text))
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
}
