use std::ops::Range;

/// Tracks IME composition state for the editor.
/// The Editor struct implements EntityInputHandler and uses this to manage
/// preedit text (marked range) display.
#[derive(Debug)]
pub struct ImeState {
    /// Currently marked (preedit) range in the buffer, if composing.
    pub marked_range: Option<Range<usize>>,
}

impl ImeState {
    pub fn new() -> Self {
        Self { marked_range: None }
    }

    #[allow(dead_code)]
    pub fn is_composing(&self) -> bool {
        self.marked_range.is_some()
    }

    /// Start or update a composition with the given marked range.
    pub fn set_marked(&mut self, range: Range<usize>) {
        self.marked_range = Some(range);
    }

    /// Clear composition state (text was committed or cancelled).
    pub fn clear(&mut self) {
        self.marked_range = None;
    }

    /// Take the current marked range, clearing it.
    #[allow(dead_code)]
    pub fn take_marked(&mut self) -> Option<Range<usize>> {
        self.marked_range.take()
    }
}

impl Default for ImeState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_is_idle() {
        let state = ImeState::new();
        assert!(!state.is_composing());
        assert!(state.marked_range.is_none());
    }

    #[test]
    fn set_and_clear_marked() {
        let mut state = ImeState::new();
        state.set_marked(5..10);
        assert!(state.is_composing());
        assert_eq!(state.marked_range, Some(5..10));
        state.clear();
        assert!(!state.is_composing());
    }

    #[test]
    fn take_marked() {
        let mut state = ImeState::new();
        state.set_marked(0..3);
        let range = state.take_marked();
        assert_eq!(range, Some(0..3));
        assert!(!state.is_composing());
    }
}
