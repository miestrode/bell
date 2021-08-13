use std::ops;

pub struct Source<'a> {
    pub file: &'a str,
    pub text: &'a str,
}

pub enum Error<'a> {
    InvalidCharacter { filename: &'a str, text: &'a str, range: ops::Range<usize>, character: char },
    UnterminatedBlockComment { filename: &'a str, text: &'a str, range: ops::Range<usize> },
    Expected { filename: &'a str, text: &'a str, range: ops::Range<usize>, expected: &'static [&'static str], found: &'a str },
}
