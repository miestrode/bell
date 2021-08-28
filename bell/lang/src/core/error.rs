use std::ops;

pub struct Error<'a> {
    pub filename: &'a str,
    pub text: &'a str,
    pub error: ErrorKind<'a>,
}

pub enum ErrorKind<'a> {
    InvalidCharacter {
        range: ops::Range<usize>,
        character: char,
    },
    UnterminatedBlockComment {
        range: ops::Range<usize>,
    },
    Expected {
        range: ops::Range<usize>,
        expected: &'static [&'static str],
        found: &'a str,
    },
    DuplicateParameter {
        existing: ops::Range<usize>,
        found: ops::Range<usize>,
        name: String,
    },
    DataTypeMismatch {
        expected: String,
        found: String,
        because: Option<ops::Range<usize>>,
        location: ops::Range<usize>,
    },
    // todo: Provide suggestions for valid variables to use
    UndeclaredSymbol {
        name: String,
        usage: ops::Range<usize>,
    },
}
