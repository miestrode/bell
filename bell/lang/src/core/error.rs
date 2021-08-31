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
    ExpectedDifferentToken {
        range: ops::Range<usize>,
        expected: &'static [&'static str],
        found: &'a str,
    },
    ShadowedSymbol {
        old: ops::Range<usize>,
        new: ops::Range<usize>,
        name: String,
        symbol: String,
    },
    DataTypeMismatch {
        expected: String,
        got: String,
        because: Option<ops::Range<usize>>,
        got_location: ops::Range<usize>,
    },
    // todo: Provide suggestions for valid variables to use
    UndeclaredSymbol {
        name: String,
        usage: ops::Range<usize>,
    },
    NoElseBranch {
        location: ops::Range<usize>,
    },
    ParameterCountMismatch {
        expected_count: usize,
        got_count: usize,
        because: Option<ops::Range<usize>>,
        got_location: ops::Range<usize>,
        function: String,
    },
}
