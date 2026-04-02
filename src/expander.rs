use std::collections::VecDeque;

#[derive(Debug, PartialEq)]
pub struct Expansion {
    pub delete_count: usize,
    pub text: String,
}

pub struct Expander {
    buffer: VecDeque<char>,
    max_trigger_len: usize,
    expansions: Vec<(String, String)>,
}

impl Expander {
    pub fn new(expansions: Vec<(String, String)>) -> Self {
        let max_trigger_len = expansions
            .iter()
            .map(|(t, _)| t.chars().count())
            .max()
            .unwrap_or(0);
        Self {
            buffer: VecDeque::new(),
            max_trigger_len,
            expansions,
        }
    }

    pub fn update(&mut self, expansions: Vec<(String, String)>) {
        self.max_trigger_len = expansions
            .iter()
            .map(|(t, _)| t.chars().count())
            .max()
            .unwrap_or(0);
        self.expansions = expansions;
        self.buffer.clear();
    }

    pub fn push_char(&mut self, c: char) -> Option<Expansion> {
        self.buffer.push_back(c);

        // Cap the buffer at max_trigger_len
        while self.max_trigger_len > 0 && self.buffer.len() > self.max_trigger_len {
            self.buffer.pop_front();
        }

        if self.max_trigger_len == 0 {
            return None;
        }

        // Build the current buffer as a string for suffix matching
        let buf_str: String = self.buffer.iter().collect();

        for (trigger, expansion_text) in &self.expansions {
            if buf_str.ends_with(trigger.as_str()) {
                let delete_count = trigger.chars().count();
                let text = expansion_text.clone();
                self.buffer.clear();
                return Some(Expansion { delete_count, text });
            }
        }

        None
    }

    pub fn pop_char(&mut self) {
        self.buffer.pop_back();
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_expander() -> Expander {
        Expander::new(vec![
            ("/mail".to_string(), "user@example.com".to_string()),
            ("/sig".to_string(), "Best regards,\nJakub".to_string()),
        ])
    }

    #[test]
    fn test_no_match_on_partial_trigger() {
        let mut e = make_expander();
        for c in "/mai".chars() {
            let result = e.push_char(c);
            assert_eq!(result, None, "partial trigger should not match");
        }
    }

    #[test]
    fn test_match_on_complete_trigger() {
        let mut e = make_expander();
        let mut result = None;
        for c in "/mail".chars() {
            result = e.push_char(c);
        }
        assert_eq!(
            result,
            Some(Expansion {
                delete_count: 5,
                text: "user@example.com".to_string(),
            })
        );
    }

    #[test]
    fn test_backspace_prevents_match() {
        let mut e = make_expander();
        // Type /mai
        for c in "/mai".chars() {
            e.push_char(c);
        }
        // Backspace (removes 'i')
        e.pop_char();
        // Type 'l' — buffer is now "/mal", not "/mail"
        let result = e.push_char('l');
        assert_eq!(result, None, "buffer is /mal after backspace, should not match");
    }

    #[test]
    fn test_reset_prevents_match() {
        let mut e = make_expander();
        // Type /mai
        for c in "/mai".chars() {
            e.push_char(c);
        }
        // Arrow key / escape
        e.reset();
        // Type 'l' — buffer is "l", not "/mail"
        let result = e.push_char('l');
        assert_eq!(result, None, "buffer cleared by reset, single 'l' should not match");
    }

    #[test]
    fn test_reset_after_match_allows_new_match() {
        let mut e = make_expander();
        // First match
        for c in "/mail".chars() {
            e.push_char(c);
        }
        // After a match the buffer is cleared internally, but call reset explicitly too
        e.reset();
        // Retype trigger — second match should fire
        let mut result = None;
        for c in "/mail".chars() {
            result = e.push_char(c);
        }
        assert_eq!(
            result,
            Some(Expansion {
                delete_count: 5,
                text: "user@example.com".to_string(),
            }),
            "second match after reset should fire"
        );
    }

    #[test]
    fn test_multiline_expansion_text() {
        let mut e = make_expander();
        let mut result = None;
        for c in "/sig".chars() {
            result = e.push_char(c);
        }
        assert_eq!(
            result,
            Some(Expansion {
                delete_count: 4,
                text: "Best regards,\nJakub".to_string(),
            })
        );
    }

    #[test]
    fn test_delete_count_equals_trigger_char_count() {
        let mut e = make_expander();
        let mut result = None;
        for c in "/sig".chars() {
            result = e.push_char(c);
        }
        let expansion = result.expect("/sig should match");
        // "/sig" is 4 chars: '/', 's', 'i', 'g'
        assert_eq!(expansion.delete_count, 4);
    }

    #[test]
    fn test_buffer_capped_at_max_trigger_length() {
        // max trigger len is 5 ("/mail"). Type many chars before the trigger.
        let mut e = make_expander();
        // Type a bunch of unrelated chars first
        for c in "hello world this is some text ".chars() {
            e.push_char(c);
        }
        // Now type the trigger — the buffer should be capped and the suffix match should fire
        let mut result = None;
        for c in "/mail".chars() {
            result = e.push_char(c);
        }
        assert_eq!(
            result,
            Some(Expansion {
                delete_count: 5,
                text: "user@example.com".to_string(),
            }),
            "trigger should still match even after long prefix input"
        );
    }

    #[test]
    fn test_multiple_triggers() {
        let mut e = Expander::new(vec![
            ("/mail".to_string(), "user@example.com".to_string()),
            ("/phone".to_string(), "+1-555-0100".to_string()),
        ]);
        let mut result = None;
        for c in "/phone".chars() {
            result = e.push_char(c);
        }
        assert_eq!(
            result,
            Some(Expansion {
                delete_count: 6,
                text: "+1-555-0100".to_string(),
            })
        );
    }
}
