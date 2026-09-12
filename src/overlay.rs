use crate::modal::{ConfirmChoice, ConfirmState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overlay {
    #[default]
    None,
    LoadPrompt(ConfirmState),
    CommandLibrary,
    ConnectionBrowser,
}

impl Overlay {
    pub fn is_command_library(self) -> bool {
        matches!(self, Self::CommandLibrary)
    }

    pub fn is_connection_browser(self) -> bool {
        matches!(self, Self::ConnectionBrowser)
    }

    pub fn load_prompt(self) -> Option<ConfirmState> {
        match self {
            Self::LoadPrompt(state) => Some(state),
            _ => None,
        }
    }

    pub fn load_prompt_choice(self) -> Option<ConfirmChoice> {
        self.load_prompt().map(|prompt| prompt.selected)
    }

    pub fn load_prompt_mut(&mut self) -> Option<&mut ConfirmState> {
        match self {
            Self::LoadPrompt(state) => Some(state),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_modes_are_exclusive() {
        let overlay = Overlay::CommandLibrary;
        assert!(overlay.is_command_library());
        assert!(!overlay.is_connection_browser());
        assert!(overlay.load_prompt().is_none());
    }
}
