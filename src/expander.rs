#[derive(Debug, PartialEq)]
pub struct Expansion {
    pub delete_count: usize,
    pub text: String,
}

pub struct Expander {
    buffer: std::collections::VecDeque<char>,
    max_trigger_len: usize,
    expansions: Vec<(String, String)>,
}

impl Expander {
    pub fn new(expansions: Vec<(String, String)>) -> Self {
        let max_trigger_len = expansions.iter().map(|(t, _)| t.chars().count()).max().unwrap_or(0);
        Self { buffer: std::collections::VecDeque::new(), max_trigger_len, expansions }
    }
    pub fn update(&mut self, expansions: Vec<(String, String)>) {
        self.max_trigger_len = expansions.iter().map(|(t, _)| t.chars().count()).max().unwrap_or(0);
        self.expansions = expansions;
        self.buffer.clear();
    }
    pub fn push_char(&mut self, _c: char) -> Option<Expansion> { todo!() }
    pub fn pop_char(&mut self) { todo!() }
    pub fn reset(&mut self) { self.buffer.clear(); }
}
