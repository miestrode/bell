use std::ops;

use crate::front_end::lexer;

pub type SpanToken = (lexer::Token, ops::Range<usize>);