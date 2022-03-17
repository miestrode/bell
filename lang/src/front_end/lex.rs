use camino::Utf8PathBuf;

use chumsky::prelude::*;
use chumsky::recovery;
use chumsky::{Parser, Stream};

use internment::Intern;

use crate::core::error::{Element, Error, Errors, ParseError, Pattern};
use crate::core::span::Span;
use crate::core::token::{MetaToken, Token};

pub fn lexer() -> impl Parser<char, Vec<(MetaToken, Span)>, Error = ParseError> {
    recursive(|meta_tokens| {
        let identifier = text::ident::<char, _>()
            .map(|identifier| match identifier.as_str() {
                "var" => Token::Variable,
                "loop" => Token::Loop,
                "break" => Token::Break,
                "continue" => Token::Continue,
                "return" => Token::Return,
                "func" => Token::Function,
                "struct" => Token::Structure,
                "if" => Token::If,
                "else" => Token::Else,
                "use" => Token::Use,
                "true" => Token::Boolean(true),
                "false" => Token::Boolean(false),
                _ => Token::Name(Intern::new(identifier)),
            })
            .labelled("identifier");

        let integer = text::int(10)
            // The input must be a valid number due to how it's defined.
            .map(|characters: String| Token::Int(characters.parse::<i32>().unwrap()))
            .labelled("integer");

        let symbol = choice((
            just("+").to(Token::Add),
            just("-").to(Token::Minus),
            just("*").to(Token::Multiply),
            just("/").to(Token::Divide),
            just("%").to(Token::Modulo),
            just("<=").to(Token::LesserOrEqual),
            just(">=").to(Token::GreaterOrEqual),
            just("==").to(Token::Equal),
            just("!=").to(Token::NotEqual),
            just("&").to(Token::Reference),
            just("||").to(Token::Or),
            just("&&").to(Token::And),
            just("=").to(Token::Assign),
            just("::").to(Token::ModuleAcess),
            just(":").to(Token::Specify),
            just(".").to(Token::Of),
            just("->").to(Token::Arrow),
            just(";").to(Token::Terminate),
            just("<").to(Token::Lesser),
            just(">").to(Token::Greater),
            just("(").to(Token::Left),
            just(")").to(Token::Right),
            just(",").to(Token::Separate),
        ));

        let token = identifier.or(integer).or(symbol).map(MetaToken::Token);

        let block = meta_tokens
            .clone()
            .delimited_by(just('{'), just('}'))
            .recover_with(recovery::nested_delimiters('{', '}', [], |_| Vec::new()))
            .map(MetaToken::Block);

        let string = meta_tokens
            .delimited_by(just('{'), just('}'))
            .recover_with(recovery::nested_delimiters('{', '}', [], |_| Vec::new()))
            .map_with_span(|tokens, span: Span| (tokens, span))
            .or(none_of("{\"")
                .repeated()
                .at_least(1)
                .map_with_span(|characters, span: Span| {
                    (
                        vec![(
                            MetaToken::Token(Token::String(Intern::new(
                                characters.into_iter().collect::<String>(),
                            ))),
                            span.clone(),
                        )],
                        span,
                    )
                }))
            .repeated()
            .delimited_by(just('"'), just('"'))
            .map(MetaToken::FormatString)
            .boxed()
            .labelled("string");

        let meta_token = token
            .or(block)
            .or(string)
            .map_with_span(|token, span: Span| (token, span))
            .padded();

        let block_comment = just("/*").then(any().repeated()).then(just("*/")).ignored();

        let comment = just("//")
            .then(any().repeated().then(text::newline().or(end())))
            .ignored();

        let comments = block_comment.or(comment).padded().repeated();

        meta_token.repeated().padded_by(comments)
    })
    .then(Parser::<char, _>::map_with_span(end(), |_, span: Span| {
        (MetaToken::Token(Token::EndOfFile), span)
    }))
    .map(|(mut head, eof)| {
        head.push(eof);
        head
    })
}

pub fn lex(
    path: Intern<Utf8PathBuf>,
    text: &str,
    global_errors: &mut Errors,
) -> Vec<(MetaToken, Span)> {
    let (tokens, errors) = lexer().parse_recovery(Stream::from_iter(
        Span {
            path,
            // Using a saturating subtraction since the file may be empty.
            range: (text.len().saturating_sub(1))..text.len().max(1),
        },
        text.char_indices().map(|(index, character)| {
            (
                character,
                Span {
                    path,
                    range: index..index + 1,
                },
            )
        }),
    ));

    global_errors.extend(errors.into_iter().map(|error| Error::Unexpected {
        expected: error.expected,
        found: Element {
            // If it's None, it must have encountered the end of file.
            value: error.found.unwrap_or(Pattern::Construct("end of file")),
            span: error.span,
        },
        reason: error.reason,
        while_parsing: error.label,
    }));

    tokens.unwrap_or_else(|| Vec::new())
}
