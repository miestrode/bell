use std::fmt::{Display, Formatter, Result as FmtResult};
use std::iter;
use std::ops::Range;

use camino::Utf8PathBuf;

use chumsky::prelude::*;
use chumsky::recovery;
use chumsky::stream::{Flat, Stream};
use chumsky::Span as SpanTrait;

use crate::ast::WithHint;
use internment::Intern;

use crate::core::ast::{Expression, Identifier, Path, PathEnd, Type};
use crate::core::error::{Element, Error, ParseError, Pattern};
use crate::core::span::Span;
use crate::core::token::{MetaToken, Token};

impl Display for Span {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_fmt(format_args!("{}..{}", self.range.start, self.range.end))
    }
}

impl SpanTrait for Span {
    type Context = Intern<Utf8PathBuf>;
    type Offset = usize;

    fn new(context: Self::Context, range: Range<Self::Offset>) -> Self {
        Span {
            path: context,
            range,
        }
    }

    fn context(&self) -> Self::Context {
        self.path
    }

    fn start(&self) -> Self::Offset {
        self.range.start
    }

    fn end(&self) -> Self::Offset {
        self.range.end
    }
}

struct TokenIterator(Vec<(MetaToken, Span)>);

impl TokenIterator {
    fn new(mut tokens: Vec<(MetaToken, Span)>) -> Self {
        tokens.reverse();

        TokenIterator(tokens)
    }
}

impl Iterator for TokenIterator {
    type Item = (MetaToken, Span);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop()
    }
}

impl TokenIterator {
    fn get_end_span(&self) -> Span {
        self.0.first().map(|(_, span)| span).unwrap().clone()
    }
}

type MetaTokens = Box<dyn Iterator<Item = (MetaToken, Span)>>;

fn delimit_tokens(
    tokens: MetaTokens,
    start: (MetaToken, Span),
    end: (MetaToken, Span),
) -> MetaTokens {
    Box::new(iter::once(start).chain(tokens).chain(iter::once(end)))
}

impl<'a> From<TokenIterator> for Stream<'a, Token, Span, Box<dyn Iterator<Item = (Token, Span)>>> {
    fn from(
        mut tokens: TokenIterator,
    ) -> Stream<'a, Token, Span, Box<dyn Iterator<Item = (Token, Span)>>> {
        // A Token iterator must at least have an EOF token.
        let end_span = tokens.0.remove(0).1;

        Stream::from_nested(end_span, tokens, |(token, span)| match token {
            MetaToken::FormatString(elements) => Flat::Many(delimit_tokens(
                Box::new(
                    elements
                        .into_iter()
                        .map(|meta_tokens| {
                            delimit_tokens(
                                Box::new(meta_tokens.0.into_iter()),
                                (
                                    MetaToken::Token(Token::CurlyLeft),
                                    Span {
                                        path: meta_tokens.1.path,
                                        range: meta_tokens.1.range.start
                                            ..meta_tokens.1.range.start + 1,
                                    },
                                ),
                                (
                                    MetaToken::Token(Token::CurlyRight),
                                    Span {
                                        path: meta_tokens.1.path,
                                        range: meta_tokens.1.range.end..meta_tokens.1.range.end + 1,
                                    },
                                ),
                            )
                        })
                        .flatten(),
                ),
                (
                    MetaToken::Token(Token::Quote),
                    Span {
                        path: span.path,
                        range: span.range.start..span.range.start + 1,
                    },
                ),
                (
                    MetaToken::Token(Token::Quote),
                    Span {
                        path: span.path,
                        range: span.range.end..span.range.end + 1,
                    },
                ),
            )),
            MetaToken::Token(token) => Flat::Single((token, span)),
            MetaToken::Block(tokens) => Flat::Many(delimit_tokens(
                Box::new(tokens.into_iter()),
                (
                    MetaToken::Token(Token::CurlyLeft),
                    Span {
                        path: span.path,
                        range: span.range.start..span.range.start + 1,
                    },
                ),
                (
                    MetaToken::Token(Token::CurlyRight),
                    Span {
                        path: span.path,
                        range: (span.range.end - 1)..span.range.end,
                    },
                ),
            )),
        })
    }
}

macro_rules! operator {
    ($operator: expr, $mapped_function: expr) => {
        just($operator).map_with_span(move |_, span| ($mapped_function, span))
    };
}

fn binary_operation<T, O>(
    term: T,
    operator: O,
) -> impl Parser<Token, (Expression, Span), Error = ParseError> + Clone
where
    T: Parser<Token, (Expression, Span), Error = ParseError> + Clone,
    O: Parser<Token, (&'static str, Span), Error = ParseError> + Clone,
{
    term.clone()
        .then(operator.then(term).repeated())
        .foldl(|left, (operation, right)| {
            let span = Span {
                range: left.1.range.start..right.1.range.end,
                path: left.1.path,
            };

            (
                Expression::Call {
                    function: Box::new((
                        Expression::Identifier(Identifier::new(vec![Intern::new(
                            operation.0.to_string(),
                        )])),
                        operation.1,
                    )),
                    parameters: vec![left, right],
                },
                span,
            )
        })
}

fn build_parser() -> impl Parser<Token, Vec<(Expression, Span)>, Error = ParseError> {
    recursive(|expression| {
        let integer = filter_map(|span, token| match token {
            Token::Int(value) => Ok((Expression::Int(value), span)),
            _ => Err(ParseError::expected_input_found(
                span,
                [Some(Token::Int(Default::default()))],
                Some(token),
            )),
        })
        .labelled("integer");

        let boolean = filter_map(|span, token| match token {
            Token::Boolean(value) => Ok((Expression::Boolean(value), span)),
            _ => Err(ParseError::expected_input_found(
                span,
                [Some(Token::Boolean(Default::default()))],
                Some(token),
            )),
        })
        .labelled("boolean");

        let pure_string = filter_map(|span: Span, token| match token {
            Token::String(value) => Ok((Expression::String(value), span)),
            _ => Err(ParseError::expected_input_found(
                span,
                [Some(Token::String(Default::default()))],
                Some(token),
            )),
        });

        let string = pure_string
            .or(expression
                .clone()
                .delimited_by(Token::CurlyLeft, Token::CurlyRight)
                .recover_with(recovery::nested_delimiters(
                    Token::CurlyLeft,
                    Token::CurlyRight,
                    [(Token::Left, Token::Right)],
                    |span| (Default::default(), span),
                )))
            .repeated()
            .delimited_by(Token::Quote, Token::Quote)
            .map_with_span(|string, span: Span| {
                string
                    .into_iter()
                    .reduce(|accumulator, (next, next_span)| {
                        let span = Span {
                            range: accumulator.1.range.start..next_span.range.end,
                            path: accumulator.1.path,
                        };

                        (
                            Expression::Call {
                                function: Box::new((
                                    Expression::Identifier(Identifier::new(vec![Intern::new(
                                        "add".to_string(),
                                    )])),
                                    // This span shouldn't be used, ever.
                                    Default::default(),
                                )),
                                parameters: vec![
                                    accumulator,
                                    (
                                        Expression::Call {
                                            function: Box::new((
                                                Expression::Identifier(Identifier::new(vec![
                                                    Intern::new("to_string".to_string()),
                                                ])),
                                                // This span shouldn't be used, ever.
                                                Default::default(),
                                            )),
                                            parameters: vec![(next, next_span.clone())],
                                        },
                                        next_span,
                                    ),
                                ],
                            },
                            span,
                        )
                    })
                    .unwrap_or((Expression::String(Intern::new(String::new())), span))
            })
            .boxed()
            .labelled("string");

        let id = filter_map(|span: Span, token: Token| match token {
            Token::Identifier(id) => Ok((id, span)),
            _ => Err(ParseError::expected_input_found(
                span,
                [Some(Token::Identifier(Default::default()))],
                Some(token),
            )),
        })
        .labelled("identifier");

        let variable = id
            .map(|(id, _)| id)
            .separated_by(just(Token::Path))
            .at_least(1)
            .map_with_span(|id, span: Span| (Identifier::new(id), span))
            .labelled("variable");

        let data_type = just(Token::Reference)
            .or_not()
            .map(|token| token.is_some())
            .then(
                variable.map(|(structure, _)| match structure.0[0].as_str() {
                    "Int" => Type::Integer,
                    "Bool" => Type::Boolean,
                    "Str" => Type::String,
                    _ => Type::Structure(structure),
                }),
            )
            .map_with_span(|(is_reference, kind), span: Span| {
                if is_reference {
                    (Type::Reference(Box::new(kind)), span)
                } else {
                    (kind, span)
                }
            })
            .boxed()
            .labelled("type");

        let type_hint = just(Token::Specify)
            .ignore_then(data_type.clone())
            .labelled("type_hint");

        let field = id
            .then(type_hint.clone().or_not())
            .map(|(id, type_hint)| WithHint {
                data: id,
                type_hint,
            });

        let block_expression = recursive(|block_expression| {
            let block = (expression.clone().then_ignore(just(Token::Terminate)))
                .or(block_expression)
                .repeated()
                .then(expression.clone().map(Box::new).or_not())
                .delimited_by(Token::CurlyLeft, Token::CurlyRight)
                .map_with_span(|(expressions, tail), span: Span| {
                    (Expression::Block { expressions, tail }, span)
                })
                .recover_with(recovery::nested_delimiters(
                    Token::CurlyLeft,
                    Token::CurlyRight,
                    [(Token::Left, Token::Right)],
                    |span| (Default::default(), span),
                ))
                .boxed()
                .labelled("block");

            let conditional = just(Token::If)
                .ignore_then(expression.clone().then(block.clone()))
                .then(
                    just(Token::Else)
                        .then(just(Token::If))
                        .ignore_then(expression.clone().then(block.clone()))
                        .repeated(),
                )
                .then(
                    just(Token::Else)
                        .ignore_then(block.clone())
                        .map(Box::new)
                        .or_not(),
                )
                .map_with_span(|((head, mut body), tail), span: Span| {
                    body.insert(0, head);

                    (
                        Expression::Conditional {
                            branches: body,
                            tail,
                        },
                        span,
                    )
                })
                .boxed()
                .labelled("conditional");

            let basic_loop = just(Token::Loop)
                .ignore_then(block.clone())
                .map_with_span(|block, span: Span| (Expression::Loop(Box::new(block)), span))
                .labelled("loop");

            let structure = just(Token::Structure)
                .ignore_then(id)
                .then(
                    field
                        .clone()
                        .separated_by(just(Token::Separate))
                        .allow_trailing()
                        .delimited_by(Token::CurlyLeft, Token::CurlyRight)
                        .recover_with(recovery::nested_delimiters(
                            Token::CurlyLeft,
                            Token::CurlyRight,
                            [(Token::Left, Token::Right)],
                            |_| Default::default(),
                        )),
                )
                .map_with_span(|(name, fields), span: Span| {
                    (Expression::Structure { name, fields }, span)
                })
                .boxed()
                .labelled("structure");

            let object = variable
                .then(
                    id.then_ignore(just(Token::Specify))
                        .then(expression.clone())
                        .separated_by(just(Token::Separate))
                        .allow_trailing()
                        .delimited_by(Token::CurlyLeft, Token::CurlyRight)
                        .recover_with(recovery::nested_delimiters(
                            Token::CurlyLeft,
                            Token::CurlyRight,
                            [(Token::Left, Token::Right)],
                            |_| Default::default(),
                        ))
                        .map(|fields| {
                            fields
                                .into_iter()
                                .map(|(name, value)| (name, value))
                                .collect::<Vec<_>>()
                        }),
                )
                .map_with_span(|(object, fields), span: Span| {
                    (Expression::Instance { object, fields }, span)
                })
                .boxed()
                .labelled("object");

            let function = just(Token::Function)
                .ignore_then(id)
                .then(
                    field
                        .separated_by(just(Token::Separate))
                        .allow_trailing()
                        .delimited_by(Token::Left, Token::Right)
                        .recover_with(recovery::nested_delimiters(
                            Token::Left,
                            Token::Right,
                            [(Token::CurlyLeft, Token::CurlyRight)],
                            |_| Default::default(),
                        )),
                )
                .then(just(Token::Arrow).ignore_then(data_type).or_not())
                .then(block.clone())
                .map_with_span(|(((name, parameters), return_type), body), span: Span| {
                    (
                        Expression::Function {
                            name: WithHint {
                                data: name,
                                type_hint: return_type,
                            },
                            parameters,
                            body: Box::new(body),
                        },
                        span,
                    )
                })
                .boxed()
                .labelled("function");

            choice((conditional, function, basic_loop, structure, object, block))
        })
        .boxed();

        let atom = choice((
            expression
                .clone()
                .delimited_by(Token::Left, Token::Right)
                .recover_with(recovery::nested_delimiters(
                    Token::Left,
                    Token::Right,
                    [(Token::CurlyLeft, Token::CurlyRight)],
                    |span| (Default::default(), span),
                )),
            integer,
            boolean,
            pure_string,
            string,
            variable.map(|(id, span)| (Expression::Identifier(id), span)),
            block_expression.clone(),
        ))
        .recover_with(recovery::nested_delimiters(
            Token::CurlyLeft,
            Token::CurlyRight,
            [(Token::Left, Token::Right)],
            |span| (Default::default(), span),
        ))
        .boxed();

        let access = atom
            .clone()
            .then(just(Token::Of).ignore_then(id).repeated())
            .foldl(|left, right| {
                let span = Span {
                    range: left.1.range.start..right.1.range.end,
                    path: left.1.path,
                };

                (
                    Expression::Access {
                        from: Box::new(left),
                        id: right,
                    },
                    span,
                )
            });

        let call = access
            .then(
                expression
                    .clone()
                    .separated_by(just(Token::Separate))
                    .allow_trailing()
                    .delimited_by(Token::Left, Token::Right)
                    .recover_with(recovery::nested_delimiters(
                        Token::Left,
                        Token::Right,
                        [(Token::CurlyLeft, Token::CurlyRight)],
                        |_| Default::default(),
                    ))
                    .map_with_span(|expression, span| (expression, span))
                    .repeated(),
            )
            .foldl(|left: (Expression, Span), right| {
                let span = Span {
                    range: left.1.range.start..right.1.range.end,
                    path: left.1.path,
                };

                (
                    Expression::Call {
                        function: Box::new(left),
                        parameters: right.0,
                    },
                    span,
                )
            })
            .boxed();

        let product = binary_operation(
            call,
            choice((
                operator!(Token::Multiply, "multiply"),
                operator!(Token::Divide, "divide"),
                operator!(Token::Modulo, "modulo"),
            )),
        );

        let sum = binary_operation(
            product,
            operator!(Token::Add, "add").or(operator!(Token::Minus, "subtract")),
        );

        let comparison = binary_operation(
            sum,
            choice((
                operator!(Token::Lesser, "lesser"),
                operator!(Token::Greater, "greater"),
                operator!(Token::LesserOrEqual, "lesser_or_equal"),
                operator!(Token::GreaterOrEqual, "greater_or_equal"),
                operator!(Token::Equal, "equal"),
                operator!(Token::NotEqual, "not_equal"),
            )),
        )
        .boxed();

        let logic = binary_operation(
            comparison,
            operator!(Token::Or, "or").or(operator!(Token::And, "and")),
        );

        let assign = logic
            .clone()
            .then(just(Token::Assign).ignore_then(logic).repeated())
            .map(|(head, body)| {
                body.into_iter()
                    .rev()
                    .chain(iter::once(head))
                    .reduce(|last, before| {
                        let span = Span {
                            range: before.1.range.start..last.1.range.end,
                            path: before.1.path,
                        };

                        (
                            Expression::Assignment {
                                to: Box::new(before),
                                from: Box::new(last),
                            },
                            span,
                        )
                    })
                    .unwrap()
            })
            .boxed();

        let import = just(Token::Use)
            .ignore_then(
                recursive(|path| {
                    let path_end = path
                        .map(PathEnd::Path)
                        .or(just(Token::This).to(PathEnd::This))
                        .map_with_span(|path_end, span: Span| (path_end, span))
                        .separated_by(just(Token::Separate))
                        .allow_trailing()
                        .delimited_by(Token::CurlyLeft, Token::CurlyRight)
                        .recover_with(recovery::nested_delimiters(
                            Token::CurlyLeft,
                            Token::CurlyRight,
                            [(Token::Left, Token::Right)],
                            |_| Default::default(),
                        ))
                        .or(id.map(|(identifier, span)| vec![(PathEnd::End(identifier), span)]))
                        .boxed();

                    id.map(|(id, _)| id)
                        .then_ignore(just(Token::Path))
                        .repeated()
                        .map_with_span(|body, span| (Identifier(body), span))
                        .then(path_end)
                        .map(|(body, tail)| Path {
                            tail: tail
                                .into_iter()
                                .map(|(path_end, span)| {
                                    (
                                        match path_end {
                                            PathEnd::Path(mut path) => {
                                                let mut local_body = body.clone();

                                                local_body.0 .0.extend(path.body.0 .0);
                                                path.body = local_body;

                                                PathEnd::Path(path)
                                            }
                                            _ => path_end,
                                        },
                                        span,
                                    )
                                })
                                .collect(),
                            body,
                        })
                })
                .map_with_span(|path, span: Span| (Expression::Use(path), span)),
            )
            .boxed()
            .labelled("import");

        let declaration = just(Token::Variable)
            .ignore_then(id)
            .then(type_hint.or_not())
            .then_ignore(just(Token::Assign))
            .then(expression.clone())
            .map_with_span(|((name, type_hint), value), span| {
                (
                    Expression::Declaration {
                        name: WithHint {
                            data: name,
                            type_hint,
                        },
                        value: Box::new(value),
                    },
                    span,
                )
            })
            .boxed()
            .labelled("variable declaration");

        let continue_flow =
            just(Token::Continue).map_with_span(|_, span: Span| (Expression::Continue, span));

        let break_flow = just(Token::Break)
            .ignore_then(expression.clone())
            .map_with_span(|expression, span| (Expression::Break(Box::new(expression)), span));

        let return_flow = just(Token::Return)
            .ignore_then(expression)
            .map_with_span(|expression, span| (Expression::Return(Box::new(expression)), span));

        choice((
            declaration,
            import,
            block_expression,
            continue_flow,
            break_flow,
            return_flow,
            assign,
        ))
        .labelled("expression")
    })
    .boxed()
    .repeated()
    .at_least(1)
}

pub fn parse(tokens: Vec<(MetaToken, Span)>) -> (Vec<(Expression, Span)>, Vec<Error>) {
    let tokens = TokenIterator::new(tokens);
    let eof_span = tokens.get_end_span(); // In case parsing fails, it's important that we cache the EOF.

    let (ast, errors) = build_parser().parse_recovery(tokens);

    (
        ast.unwrap_or_else(|| vec![(Expression::Error, eof_span)]),
        errors
            .into_iter()
            .map(|error| Error::Unexpected {
                expected: error.expected,
                found: Element {
                    // If it's None, it must have encountered the end of file.
                    data: error.found.unwrap_or(Pattern::Construct("end of file")),
                    span: error.span,
                },
                reason: error.reason,
                while_parsing: error.label,
            })
            .collect(),
    )
}
