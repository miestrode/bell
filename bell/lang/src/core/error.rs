use std::ops;

pub enum Error<'a> {
    InvalidCharacter { filename: &'a str, text: &'a str, range: ops::Range<usize>, character: char },
    UnterminatedBlockComment { filename: &'a str, text: &'a str, range: ops::Range<usize> },
    Expected { filename: &'a str, text: &'a str, range: ops::Range<usize>, expected: &'static [&'static str], found: &'a str },
    DuplicateParameter { filename: &'a str, text: &'a str, existing: ops::Range<usize>, found: ops::Range<usize>, name: String },
}
