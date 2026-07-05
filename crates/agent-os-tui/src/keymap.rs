#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBinding {
    pub key: &'static str,
    pub command_id: &'static str,
    pub description: &'static str,
}

pub fn default_keymap() -> &'static [KeyBinding] {
    KEYMAP
}

const KEYMAP: &[KeyBinding] = &[
    KeyBinding {
        key: "Enter",
        command_id: "run",
        description: "Submit composer",
    },
    KeyBinding {
        key: "Esc",
        command_id: "close_mode",
        description: "Close overlay or pane",
    },
    KeyBinding {
        key: "Ctrl-C",
        command_id: "interrupt_or_exit",
        description: "Interrupt running turn or exit",
    },
    KeyBinding {
        key: "F1",
        command_id: "help",
        description: "Open help",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keymap_has_unique_keys() {
        let mut keys = std::collections::BTreeSet::new();
        for binding in default_keymap() {
            assert!(keys.insert(binding.key), "duplicate key {}", binding.key);
        }
    }
}
