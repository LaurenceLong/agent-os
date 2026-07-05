#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComposerState {
    pub text: String,
}

impl ComposerState {
    pub fn push(&mut self, ch: char) {
        self.text.push(ch);
    }

    pub fn backspace(&mut self) {
        self.text.pop();
    }

    pub fn clear(&mut self) {
        self.text.clear();
    }

    pub fn take_trimmed(&mut self) -> String {
        let input = self.text.trim().to_string();
        self.text.clear();
        input
    }

    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}
