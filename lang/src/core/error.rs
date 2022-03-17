use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::io::Error as IOError;
use std::iter;

use camino::Utf8PathBuf;
use chumsky::error::Error as ErrorTrait;
use internment::Intern;

use crate::ast::Id;

use crate::core::span::Span;
use crate::core::token::Token;
use crate::core::types::{LinkReason, Type};

use super::{span::SourceMap, Name};

// An abstraction that saves ErrorKind fields. Because sometimes elements will be directly linked with their range.
#[derive(Debug)]
pub struct Element<T> {
    pub value: T,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct OptElement<T> {
    pub value: T,
    pub span: Option<Span>,
}

pub struct Errors {
    pub errors: Vec<Error>,
    pub sources: SourceMap,
}

impl Errors {
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn insert_source(&mut self, path: Intern<Utf8PathBuf>, contents: String) {
        self.sources.insert(path, contents)
    }

    pub fn extend(&mut self, iterator: impl Iterator<Item = Error>) {
        self.errors.extend(iterator);
    }

    pub fn insert_error(&mut self, error: Error) {
        self.errors.push(error);
    }
}

#[derive(Debug)]
pub enum Reason {
    UnclosedDelimiter(Element<Token>),
    Unexpected,
}

// Used to represent a token, or a language construct, such as a while loop.
#[derive(Eq, PartialEq, Hash, Debug)]
pub enum Pattern {
    Token(Token),
    Construct(&'static str),
    Character(char),
}

impl Display for Pattern {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Pattern::Token(token) => Display::fmt(&token, f),
            other => match other {
                Pattern::Construct(construct) => f.write_fmt(format_args!("{}", construct)),
                Pattern::Character(character) => f.write_fmt(format_args!("'{}'", character)),
                _ => unreachable!(),
            },
        }
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub span: Span,
    pub expected: HashSet<Pattern>,
    pub found: Option<Pattern>,
    pub reason: Reason,
    pub label: Option<&'static str>,
}

impl ParseError {
    pub fn unify(mut self, other: Self) -> Self {
        assert_eq!(self.span, other.span);

        self.reason = match (&self.reason, &other.reason) {
            (Reason::UnclosedDelimiter(..), _) => self.reason,
            (_, Reason::UnclosedDelimiter(..)) => other.reason,
            _ => self.reason,
        };
        self.expected = self.expected.into_iter().chain(other.expected).collect();

        self
    }
}

impl ErrorTrait<Token> for ParseError {
    type Span = Span;
    type Label = &'static str;

    fn expected_input_found<Iter: IntoIterator<Item = Option<Token>>>(
        span: Self::Span,
        expected: Iter,
        found: Option<Token>,
    ) -> Self {
        Self {
            span,
            expected: expected
                .into_iter()
                .map(|token| {
                    token
                        .map(Pattern::Token)
                        .unwrap_or_else(|| Pattern::Construct("end of file"))
                })
                .collect(),
            found: found.map(Pattern::Token),
            reason: Reason::Unexpected,
            label: None,
        }
    }

    fn unclosed_delimiter(
        start_span: Self::Span,
        start: Token,
        span: Self::Span,
        expected: Token,
        found: Option<Token>,
    ) -> Self {
        Self {
            span,
            expected: iter::once(expected).map(Pattern::Token).collect(),
            found: found.map(Pattern::Token),
            reason: Reason::UnclosedDelimiter(Element {
                value: start,
                span: start_span,
            }),
            label: None,
        }
    }

    fn with_label(mut self, label: Self::Label) -> Self {
        self.label = Some(label);
        self
    }

    fn merge(self, other: Self) -> Self {
        self.unify(other)
    }
}

impl ErrorTrait<char> for ParseError {
    type Span = Span;
    type Label = &'static str;

    fn expected_input_found<Iter: IntoIterator<Item = Option<char>>>(
        span: Self::Span,
        expected: Iter,
        found: Option<char>,
    ) -> Self {
        Self {
            span,
            expected: expected
                .into_iter()
                .map(|token| {
                    token
                        .map(Pattern::Character)
                        .unwrap_or_else(|| Pattern::Construct("end of file"))
                })
                .collect(),
            found: found.map(Pattern::Character),
            reason: Reason::Unexpected,
            label: None,
        }
    }

    fn with_label(mut self, label: Self::Label) -> Self {
        self.label = Some(label);
        self
    }

    fn merge(self, other: Self) -> Self {
        self.unify(other)
    }
}

#[derive(Debug)]
pub struct TraceElement {
    pub reason: LinkReason,
    pub data_type: OptElement<Type>,
}

#[derive(Debug)]
pub struct Backtrace(pub Vec<TraceElement>);

impl Backtrace {
    pub fn new() -> Self {
        Self(Vec::new())
    }
}

#[derive(Debug)]
pub enum Error {
    Basic(String),
    IO {
        error: IOError,
        action: Cow<'static, str>,
    },
    UnterminatedBlockComment {
        span: Span,
    },
    UnterminatedString {
        span: Span,
    },
    Unexpected {
        expected: HashSet<Pattern>,
        found: Element<Pattern>,
        reason: Reason,
        while_parsing: Option<&'static str>,
    },
    ConflictingModuleNames {
        parent: Id,
        name: Name,
    },
    InvalidAssign(Element<&'static str>),
    MissingId {
        id: Element<Id>,
    },
    ConflictingIds {
        first: Span,
        second: Span,
        id: Id,
    },
    TypeMismatch {
        a: Backtrace,
        b: Backtrace,
        reason: LinkReason,
    },
    MissingField {
        structure: Element<Type>,
        field_name: Id,
    },
    InvalidFlow {
        span: Span,
        construct: &'static str,
    },
}
