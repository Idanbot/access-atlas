use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmChoice {
    Approve,
    Decline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmOutcome {
    Pending,
    Approved,
    Declined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfirmState {
    pub selected: ConfirmChoice,
}

impl Default for ConfirmState {
    fn default() -> Self {
        Self {
            selected: ConfirmChoice::Approve,
        }
    }
}

impl ConfirmState {
    pub fn handle_key(&mut self, key: KeyEvent) -> ConfirmOutcome {
        if !is_actionable_key(key) {
            return ConfirmOutcome::Pending;
        }
        match key.code {
            KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Tab
            | KeyCode::BackTab => {
                self.toggle();
                ConfirmOutcome::Pending
            }
            KeyCode::Enter => self.confirm_selected(),
            KeyCode::Esc => ConfirmOutcome::Declined,
            KeyCode::Char('y' | 'Y') => {
                self.selected = ConfirmChoice::Approve;
                ConfirmOutcome::Approved
            }
            KeyCode::Char('n' | 'N') => {
                self.selected = ConfirmChoice::Decline;
                ConfirmOutcome::Declined
            }
            _ => ConfirmOutcome::Pending,
        }
    }

    pub fn toggle(&mut self) {
        self.selected = match self.selected {
            ConfirmChoice::Approve => ConfirmChoice::Decline,
            ConfirmChoice::Decline => ConfirmChoice::Approve,
        };
    }

    fn confirm_selected(self) -> ConfirmOutcome {
        match self.selected {
            ConfirmChoice::Approve => ConfirmOutcome::Approved,
            ConfirmChoice::Decline => ConfirmOutcome::Declined,
        }
    }
}

pub fn is_actionable_key(key: KeyEvent) -> bool {
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

pub fn wrap_index(index: usize, count: usize, direction: isize) -> usize {
    if count == 0 {
        0
    } else if direction < 0 {
        (index + count - 1) % count
    } else {
        (index + 1) % count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_kind(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn arrows_and_tab_toggle_the_selected_choice() {
        let mut prompt = ConfirmState::default();
        assert_eq!(prompt.selected, ConfirmChoice::Approve);

        assert_eq!(
            prompt.handle_key(key(KeyCode::Tab)),
            ConfirmOutcome::Pending
        );
        assert_eq!(prompt.selected, ConfirmChoice::Decline);

        assert_eq!(
            prompt.handle_key(key(KeyCode::Down)),
            ConfirmOutcome::Pending
        );
        assert_eq!(prompt.selected, ConfirmChoice::Approve);
    }

    #[test]
    fn enter_confirms_the_highlighted_choice() {
        let mut prompt = ConfirmState::default();
        assert_eq!(
            prompt.handle_key(key(KeyCode::Enter)),
            ConfirmOutcome::Approved
        );

        prompt.selected = ConfirmChoice::Decline;
        assert_eq!(
            prompt.handle_key(key(KeyCode::Enter)),
            ConfirmOutcome::Declined
        );
    }

    #[test]
    fn y_n_and_escape_resolve_without_moving_through_the_list() {
        let mut prompt = ConfirmState {
            selected: ConfirmChoice::Decline,
        };
        assert_eq!(
            prompt.handle_key(key(KeyCode::Char('y'))),
            ConfirmOutcome::Approved
        );
        assert_eq!(prompt.selected, ConfirmChoice::Approve);

        prompt.selected = ConfirmChoice::Approve;
        assert_eq!(
            prompt.handle_key(key(KeyCode::Char('n'))),
            ConfirmOutcome::Declined
        );
        assert_eq!(
            prompt.handle_key(key(KeyCode::Esc)),
            ConfirmOutcome::Declined
        );
    }

    #[test]
    fn key_release_does_not_control_the_prompt() {
        let mut prompt = ConfirmState::default();
        assert_eq!(
            prompt.handle_key(key_kind(KeyCode::Tab, KeyEventKind::Release)),
            ConfirmOutcome::Pending
        );
        assert_eq!(prompt.selected, ConfirmChoice::Approve);
        assert!(!is_actionable_key(key_kind(
            KeyCode::Enter,
            KeyEventKind::Release
        )));
    }

    #[test]
    fn wrap_index_cycles_and_ignores_empty_lists() {
        assert_eq!(wrap_index(0, 3, -1), 2);
        assert_eq!(wrap_index(2, 3, 1), 0);
        assert_eq!(wrap_index(0, 0, 1), 0);
    }
}
