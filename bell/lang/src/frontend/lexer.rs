use std::hash::Hasher;
use std::{hash, ops};

use logos::Logos;

use crate::core::error;

#[derive(Logos, Debug, Clone, Eq, PartialEq, Hash)]
pub enum Token {
    #[token("fn")]
    Fn,
    #[token("let")]
    Let,
    #[token("while")]
    While,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("+")]
    Add,
    #[token("-")]
    Subtract,
    #[token("*")]
    Multiply,
    #[token("/")]
    Divide,
    #[token("%")]
    Modulo,
    #[token("=")]
    Assign,
    #[token("==")]
    Equal,
    #[token("!=")]
    NotEqual,
    #[token(">")]
    Larger,
    #[token(">=")]
    LargerEqual,
    #[token("<")]
    Smaller,
    #[token("<=")]
    SmallerEqual,
    #[token("||")]
    Or,
    #[token("&&")]
    And,
    #[token("!")]
    Not,
    #[token("(")]
    LeftBracket,
    #[token(")")]
    RightBracket,
    #[token("{")]
    LeftCurly,
    #[token("}")]
    RightCurly,
    #[token(";")]
    Terminator,
    #[token(",")]
    Separate,
    #[token("->")]
    Arrow,
    #[token(":")]
    Hint,
    #[regex(r"\d+", | number | number.slice().parse())]
    Int(i32),
    #[regex("true|false", | boolean | boolean.slice().parse())]
    Bool(bool),
    #[regex(r"\w*", | id | String::from(id.slice()))]
    Id(String),
    #[regex(r"/\*", | lexer | skip_block_comment(lexer, 1))]
    UnterminatedBlockComment,
    // Error can also skip things. In this case it's instructed to skip comments and whitespaces
    #[regex(r"\s+", logos::skip)]
    #[regex("//.*", logos::skip)]
    #[error]
    InvalidCharacter,
}

// Lookaheads are not implemented in Logos regex parser, so we have to use a custom callback.
// It only looks if a block comment starts, and does the rest here.
// Block comments in Bell can be nested
fn skip_block_comment(lexer: &mut logos::Lexer<Token>, mut depth: i32) -> logos::Filter<()> {
    let remainder: &str = lexer.remainder();
    let mut index = 0;

    // Iterate over the indices so we can get multiple characters from the text
    while index < remainder.len() {
        match remainder.get(index..(index + 2)) {
            Some("*/") => {
                depth -= 1;

                // Notice that the depth can only be zero when this arm is visited. So we save some checks
                if depth == 0 {
                    // Update the lexers position in the text. We add two to it since we aren't actually visiting the part of the text we just checked
                    // Rather we are "peeking at it"
                    lexer.bump(index + 2);

                    return logos::Filter::Skip;
                }
            }
            Some("/*") => depth += 1,
            _ => (),
        }

        index += 1;
    }

    // Update the lexers position in the text
    lexer.bump(index);

    // If we reach this point some block comment has not been terminated
    logos::Filter::Emit(())
}

#[derive(Debug, Clone, Eq)]
pub struct SpanToken(pub Token, pub ops::Range<usize>);

// In other parts of the codebase, span tokens are compared (Mainly in the type checker).
// It isn't worth the hassle to extract the actual token values every time, so this was introduced.
impl PartialEq<Self> for SpanToken {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

// Just like the PartialEq implementation, we don't care about the SpanTokens span
impl hash::Hash for SpanToken {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

pub fn tokenize<'a>(filename: &'a str, text: &'a str) -> Result<Vec<SpanToken>, error::Error<'a>> {
    let tokens: Vec<SpanToken> = Token::lexer(text)
        .spanned()
        .map(|(token, span)| SpanToken(token, span))
        .collect();

    // Create the error enums from the error tokens Logos generated
    for SpanToken(token, span) in &tokens {
        match token {
            Token::InvalidCharacter => {
                return Err(error::Error {
                    filename,
                    text,
                    error: error::ErrorKind::InvalidCharacter {
                        range: span.clone(),
                        character: text[span.clone()].parse().unwrap(),
                    },
                });
            }
            Token::UnterminatedBlockComment => {
                return Err(error::Error {
                    filename,
                    text,
                    error: error::ErrorKind::UnterminatedBlockComment {
                        range: span.to_owned(),
                    },
                });
            }
            _ => continue,
        };
    }

    Ok(tokens)
}