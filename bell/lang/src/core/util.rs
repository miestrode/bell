use std::ops;

use crate::frontend::lexer;

pub fn extract_id(identifier: lexer::SpanToken) -> (String, ops::Range<usize>) {
    if let lexer::SpanToken(lexer::Token::Id(string), range) = identifier {
        (string, range)
    } else {
        panic!("Expected token to be an identifier")
    }
}
