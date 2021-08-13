use logos::Logos;

use crate::core::{alias, error};

#[derive(Logos, Debug, Clone, PartialEq)]
pub enum Token {
    #[token("fn")]
    Function,
    #[token("let")]
    Variable,
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
    #[token("^")]
    Xor,
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
    #[regex(r"/\*", | lexer | skip_block_comment(lexer))]
    UnterminatedBlockComment,
    // Error can also skip things. In this case it's instructed to skip comments and whitespaces
    #[regex(r"\s+", logos::skip)]
    #[regex("//.*", logos::skip)]
    #[error]
    InvalidCharacter,
}

// Lookaheads are not implemented in Logos regex parser, so we have to use a custom callback.
// It only looks if a block comment starts, and does the rest here
fn skip_block_comment(lexer: &mut logos::Lexer<Token>) -> logos::Filter<()> {
    let remainder: &str = lexer.remainder();
    let mut index = 0;

    // Iterate over the indices so we can get multiple characters from the text
    while index < remainder.len() {
        // It looks ahead to see if a block comment terminator is present
        if Some("*/") == remainder.get(index..(index + 2)) {
            // Update the lexers position in the text
            lexer.bump(index + 2);

            return logos::Filter::Skip;
        }

        index += 1;
    }

    // Update the lexers position in the text
    lexer.bump(index);

    logos::Filter::Emit(())
}

pub fn tokenize<'a>(filename: &'a str, text: &'a str) -> Result<Vec<alias::SpanToken>, error::Error<'a>> {
    let tokens: Vec<alias::SpanToken> = Token::lexer(text)
        .spanned()
        .collect();

    for (token, range) in &tokens {
        match token {
            Token::InvalidCharacter => return Err(error::Error::InvalidCharacter { filename, text, range: range.clone(), character: text[range.clone()].parse().unwrap() }),
            Token::UnterminatedBlockComment => return Err(error::Error::UnterminatedBlockComment { filename, text, range: range.to_owned() }),
            _ => continue
        };
    };

    Ok(tokens)
}