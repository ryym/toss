use regex::Regex;

#[derive(Debug)]
pub(crate) struct Query {
    regex: Regex,
}

impl Query {
    pub fn new(regex: Regex) -> Self {
        Self { regex }
    }

    #[inline]
    pub fn regex(&self) -> &Regex {
        &self.regex
    }
}
