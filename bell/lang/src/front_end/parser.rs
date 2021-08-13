use core::ops;

use crate::core::{alias, error};
use crate::front_end::lexer::Token;

use super::lexer;

#[derive(Debug)]
enum Expression {
    Int(alias::SpanToken),
    Bool(alias::SpanToken),
    Id(alias::SpanToken),
    Block(Box<Block>),
    Assign(Box<Assign>),
    Variable(Box<Variable>),
    While(Box<While>),
    Conditional(Box<Conditional>),
    Function(Box<Function>),
    Call(Call),
}

#[derive(Debug)]
struct Assign {
    id: alias::SpanToken,
    value: Expression,
}

#[derive(Debug)]
struct Variable {
    id: alias::SpanToken,
    hint: Option<alias::SpanToken>,
    value: Expression,
}

#[derive(Debug)]
struct Block {
    expressions: Vec<Expression>,
    tail: Option<Expression>,
}

#[derive(Debug)]
struct Function {
    id: alias::SpanToken,
    parameters: Vec<(alias::SpanToken, alias::SpanToken)>,
    return_type: Option<alias::SpanToken>,
    body: Block,
}

#[derive(Debug)]
struct Branch {
    condition: Expression,
    body: Block,
}

#[derive(Debug)]
struct Call {
    id: alias::SpanToken,
    parameters: Vec<Expression>,
}

#[derive(Debug)]
struct Conditional {
    branches: Vec<Branch>,
    tail: Option<Block>,
}

#[derive(Debug)]
struct While {
    condition: Expression,
    body: Block,
}

#[derive(Debug)]
pub struct Program(Vec<Function>);

// A struct to do parsing
struct Parser<'a> {
    tokens: Vec<alias::SpanToken>,
    current: Option<alias::SpanToken>,
    filename: &'a str,
    text: &'a str,
}

impl<'a> Parser<'a> {
    // Advance in the text
    fn next(&mut self) {
        self.current = if !self.tokens.is_empty() {
            Some(self.tokens.remove(0))
        } else {
            None
        };
    }

    // Generate some data, to be used for error reporting
    fn token_report_data(&mut self) -> (&'static str, ops::Range<usize>) {
        if let Some((token, range)) = self.current.take() {
            (match token {
                Token::Function => "`fn`",
                Token::Variable => "`let`",
                Token::While => "`while`",
                Token::If => "`if`",
                Token::Else => "`else`",
                Token::Add => "`+`",
                Token::Subtract => "`-`",
                Token::Multiply => "`*`",
                Token::Divide => "`/`",
                Token::Modulo => "`%`",
                Token::Assign => "`=`",
                Token::Equal => "`==`",
                Token::NotEqual => "`!=`",
                Token::Larger => "`>`",
                Token::LargerEqual => "`>=`",
                Token::Smaller => "`<`",
                Token::SmallerEqual => "`<=`",
                Token::Or => "`||`",
                Token::And => "`&&`",
                Token::Xor => "`^`",
                Token::Not => "`!`",
                Token::LeftBracket => "`(`",
                Token::RightBracket => "`)`",
                Token::LeftCurly => "`{`",
                Token::RightCurly => "`}`",
                Token::Terminator => "`;`",
                Token::Separate => "`,`",
                Token::Arrow => "`->`",
                Token::Hint => "`:`",
                Token::Int(_) => "integer",
                Token::Bool(_) => "boolean",
                Token::Id(_) => "identifier",
                _ => panic!("Found error tokens on parsing tokens.")
            }, range)
        } else {
            let length = self.text.len();

            ("nothing", length..(length + 1))
        }
    }

    // Again, since the grammar is LL(1), no need to lookahead more than 1 token
    fn lookahead(&self) -> Option<&alias::SpanToken> {
        self.tokens.get(0)
    }

    // Parse a program
    fn parse(&mut self) -> Result<Program, error::Error<'a>> {
        self.program()
    }

    fn program(&mut self) -> Result<Program, error::Error<'a>> {
        self.next();

        let mut functions: Vec<Function> = Vec::new();

        Ok(Program({
            loop {
                if self.current == None {
                    break functions;
                }

                functions.push(self.function()?)
            }
        }))
    }

    fn conditional(&mut self) -> Result<Conditional, error::Error<'a>> {
        let mut branches = Vec::new();

        // A conditional is matched with a `if` at the start, so you can use a branch here
        branches.push(self.branch()?);

        Ok(loop {
            branches.push(match self.current {
                Some((lexer::Token::Else, _)) if matches!(self.lookahead(), Some((lexer::Token::If, _))) => self.branch()?,
                Some((lexer::Token::Else, _)) => {
                    self.next();

                    break Conditional {
                        branches,
                        tail: Some(self.block()?),
                    };
                }
                _ => break Conditional {
                    branches,
                    tail: None,
                }
            })
        })
    }

    fn branch(&mut self) -> Result<Branch, error::Error<'a>> {
        if matches!(self.current, Some((lexer::Token::Else, _))) {
            self.next();
        }

        if matches!(self.current, Some((lexer::Token::If, _))) {
            self.next();

            Ok(Branch {
                condition: self.expression()?,
                body: self.block()?,
            })
        } else {
            let (name, range) = self.token_report_data();

            Err(error::Error::Expected {
                filename: self.filename,
                text: self.text,
                range,
                expected: &["`if`"],
                found: name,
            })
        }
    }

    fn while_loop(&mut self) -> Result<While, error::Error<'a>> {
        if !matches!(self.current, Some((lexer::Token::While, _))) {
            let (name, range) = self.token_report_data();

            return Err(error::Error::Expected {
                filename: self.filename,
                text: self.text,
                range,
                expected: &["`while`"],
                found: name,
            });
        }

        self.next();

        Ok(While {
            condition: self.expression()?,
            body: self.block()?,
        })
    }

    fn function(&mut self) -> Result<Function, error::Error<'a>> {
        if !matches!(self.current, Some((lexer::Token::Function, _))) {
            let (name, range) = self.token_report_data();

            return Err(error::Error::Expected {
                filename: self.filename,
                text: self.text,
                range,
                expected: &["`fn`"],
                found: name,
            });
        }

        self.next();

        if matches!(self.current, Some((lexer::Token::Id(_), _))) {
            let id = self.current.take().unwrap();

            self.next();

            if matches!(self.current, Some((lexer::Token::LeftBracket, _))) {
                self.next();

                let mut parameters = Vec::new();

                loop {
                    match self.current.take() {
                        Some(id @ (lexer::Token::Id(_), _)) => {
                            self.next();

                            if matches!(self.current, Some((lexer::Token::Hint, _))) {
                                self.next();

                                if matches!(self.current, Some((lexer::Token::Id(_), _))) {
                                    let hint = self.current.take().unwrap();

                                    self.next();

                                    parameters.push((id, hint));
                                } else {
                                    let (name, range) = self.token_report_data();

                                    return Err(error::Error::Expected {
                                        filename: self.filename,
                                        text: self.text,
                                        range,
                                        expected: &["identifier"],
                                        found: name,
                                    });
                                }
                            } else {
                                let (name, range) = self.token_report_data();

                                return Err(error::Error::Expected {
                                    filename: self.filename,
                                    text: self.text,
                                    range,
                                    expected: &["type specifier"],
                                    found: name,
                                });
                            }

                            if matches!(self.current, Some((lexer::Token::Separate, _))) {
                                self.next();
                            } else if !matches!(self.current, Some((lexer::Token::RightBracket, _))) {
                                let (name, range) = self.token_report_data();

                                return Err(error::Error::Expected {
                                    filename: self.filename,
                                    text: self.text,
                                    range,
                                    expected: &["`,`", "`)`"],
                                    found: name,
                                });
                            }
                        }
                        Some((lexer::Token::RightBracket, _)) => break,
                        token @ _ => {
                            self.current = token;

                            let (name, range) = self.token_report_data();

                            return Err(error::Error::Expected {
                                filename: self.filename,
                                text: self.text,
                                range,
                                expected: &["parameter", "`)`"],
                                found: name,
                            });
                        }
                    }
                }

                self.next();

                let return_type = if matches!(self.current, Some((lexer::Token::Arrow, _))) {
                    self.next();

                    self.next();

                    if matches!(self.current, Some((lexer::Token::Id(_), _))) {
                        self.current.take()
                    } else {
                        let (name, range) = self.token_report_data();

                        return Err(error::Error::Expected {
                            filename: self.filename,
                            text: self.text,
                            range,
                            expected: &["identifier"],
                            found: name,
                        });
                    }
                } else {
                    None
                };

                Ok(Function {
                    id,
                    parameters,
                    return_type,
                    body: self.block()?,
                })
            } else {
                let (name, range) = self.token_report_data();

                return Err(error::Error::Expected {
                    filename: self.filename,
                    text: self.text,
                    range,
                    expected: &["`(`"],
                    found: name,
                });
            }
        } else {
            let (name, range) = self.token_report_data();

            Err(error::Error::Expected {
                filename: self.filename,
                text: self.text,
                range,
                expected: &["identifier"],
                found: name,
            })
        }
    }

    fn block(&mut self) -> Result<Block, error::Error<'a>> {
        if !matches!(self.current, Some((lexer::Token::LeftCurly, _))) {
            let (name, range) = self.token_report_data();

            return Err(error::Error::Expected {
                filename: self.filename,
                text: self.text,
                range,
                expected: &["`{`"],
                found: name,
            });
        }

        self.next();

        let mut expressions = Vec::new();

        let mut has_tail = false;
        let mut needs_termination; // A flag to check if the last node was a block node. If it was, then no terminator is necessary

        loop {
            match self.current {
                Some((lexer::Token::RightCurly, _)) => break,
                None => {
                    let (name, range) = self.token_report_data();

                    return Err(error::Error::Expected {
                        filename: self.filename,
                        text: self.text,
                        range,
                        expected: &["expression", "`}`"],
                        found: name,
                    });
                }
                _ => expressions.push({
                    let expression = self.expression()?;

                    needs_termination = !matches!(expression, Expression::Block(..)
                        | Expression::While(..)
                        | Expression::Conditional(..)
                        | Expression::Function(..));

                    expression
                })
            }

            has_tail = true;

            if matches!(self.current, Some((lexer::Token::Terminator, _))) {
                has_tail = false;

                self.next();
            } else if needs_termination && !matches!(self.current, Some((lexer::Token::RightCurly, _))) {
                let (name, range) = self.token_report_data();

                return Err(error::Error::Expected {
                    filename: self.filename,
                    text: self.text,
                    range,
                    expected: &["`}`", "`;`"],
                    found: name,
                });
            }
        }

        if self.current == None {
            let (name, range) = self.token_report_data();

            return Err(error::Error::Expected {
                filename: self.filename,
                text: self.text,
                range,
                expected: &["`}`"],
                found: name,
            });
        }

        let tail = if has_tail {
            expressions.pop()
        } else {
            None
        };

        self.next();

        Ok(Block {
            expressions,
            tail,
        })
    }

    fn assign(&mut self) -> Result<Assign, error::Error<'a>> {
        if matches!(self.current, Some((lexer::Token::Id(_), _))) {
            let id = self.current.take().unwrap();

            self.next();

            if matches!(self.current, Some((lexer::Token::Assign, _))) {
                self.next();

                Ok(Assign {
                    id,
                    value: self.expression()?,
                })
            } else {
                let (name, range) = self.token_report_data();

                Err(error::Error::Expected {
                    filename: self.filename,
                    text: self.text,
                    range,
                    expected: &["`=`"],
                    found: name,
                })
            }
        } else {
            let (name, range) = self.token_report_data();

            Err(error::Error::Expected {
                filename: self.filename,
                text: self.text,
                range,
                expected: &["identifier"],
                found: name,
            })
        }
    }

    fn variable(&mut self) -> Result<Variable, error::Error<'a>> {
        if !matches!(self.current, Some((lexer::Token::Variable, _))) {
            let (name, range) = self.token_report_data();

            return Err(error::Error::Expected {
                filename: self.filename,
                text: self.text,
                range,
                expected: &["`var`"],
                found: name,
            });
        }

        self.next();

        if matches!(self.current, Some((lexer::Token::Id(_), _))) {
            let id = self.current.take().unwrap();

            self.next();

            let mut hint = None;

            if matches!(self.current, Some((lexer::Token::Hint, _))) {
                self.next();

                if matches!(self.current, Some((lexer::Token::Id(_), _))) {
                    hint = self.current.take();

                    self.next();
                } else {
                    let (name, range) = self.token_report_data();

                    return Err(error::Error::Expected {
                        filename: self.filename,
                        text: self.text,
                        range,
                        expected: &["identifier"],
                        found: name,
                    });
                }
            }

            if matches!(self.current, Some((lexer::Token::Assign, _))) {
                self.next();

                Ok(Variable {
                    id,
                    hint,
                    value: self.expression()?,
                })
            } else {
                let (name, range) = self.token_report_data();

                Err(error::Error::Expected {
                    filename: self.filename,
                    text: self.text,
                    range,
                    expected: &["`=`"],
                    found: name,
                })
            }
        } else {
            let (name, range) = self.token_report_data();

            Err(error::Error::Expected {
                filename: self.filename,
                text: self.text,
                range,
                expected: &["identifier"],
                found: name,
            })
        }
    }

    // Expressions are anything that returns a value
    fn expression(&mut self) -> Result<Expression, error::Error<'a>> {
        Ok(match self.current {
            Some((lexer::Token::Variable, _)) => Expression::Variable(Box::from(self.variable()?)),
            Some((lexer::Token::While, _)) => Expression::While(Box::from(self.while_loop()?)),
            Some((lexer::Token::If, _)) => Expression::Conditional(Box::from(self.conditional()?)),
            Some((lexer::Token::LeftCurly, _)) => Expression::Block(Box::from(self.block()?)),
            Some((lexer::Token::Function, _)) => Expression::Function(Box::from(self.function()?)),
            Some((lexer::Token::Id(_), _)) if matches!(self.lookahead(), Some((lexer::Token::Assign, _))) => Expression::Assign(Box::from(self.assign()?)),
            _ => self.logic()?
        })
    }

    // Binary operations that act on booleans
    fn logic(&mut self) -> Result<Expression, error::Error<'a>> {
        self.binary_operation(&Self::comparison, vec![lexer::Token::Or, lexer::Token::And, lexer::Token::Xor])
    }

    // Binary operations that compare objects
    fn comparison(&mut self) -> Result<Expression, error::Error<'a>> {
        self.binary_operation(&Self::sum, vec![
            lexer::Token::Equal,
            lexer::Token::NotEqual,
            lexer::Token::Smaller,
            lexer::Token::SmallerEqual,
            lexer::Token::Larger,
            lexer::Token::LargerEqual,
        ])
    }

    fn sum(&mut self) -> Result<Expression, error::Error<'a>> {
        self.binary_operation(&Self::product, vec![lexer::Token::Add, lexer::Token::Subtract])
    }

    fn product(&mut self) -> Result<Expression, error::Error<'a>> {
        self.binary_operation(&Self::value, vec![lexer::Token::Multiply, lexer::Token::Divide, lexer::Token::Modulo])
    }

    // A "generic" function to parse a binary operation. Is used primarily by expression-related nodes
    fn binary_operation<F>(&mut self, operand: &F, operators: Vec<lexer::Token>) -> Result<Expression, error::Error<'a>>
        where F: Fn(&mut Self) -> Result<Expression, error::Error<'a>>
    {
        let mut left = operand(self)?;

        loop {
            match self.current.to_owned() {
                Some((operator, range)) if operators.contains(&operator) => left = {
                    self.next();
                    let right = operand(self)?;

                    Expression::Call(match operator {
                        lexer::Token::Add => Call { id: (lexer::Token::Id(String::from("add")), range.clone()), parameters: vec![left, right] },
                        lexer::Token::Subtract => Call { id: (lexer::Token::Id(String::from("subtract")), range), parameters: vec![left, right] },
                        lexer::Token::Multiply => Call { id: (lexer::Token::Id(String::from("multiply")), range), parameters: vec![left, right] },
                        lexer::Token::Divide => Call { id: (lexer::Token::Id(String::from("divide")), range), parameters: vec![left, right] },
                        lexer::Token::Modulo => Call { id: (lexer::Token::Id(String::from("modulo")), range), parameters: vec![left, right] },
                        lexer::Token::Equal => Call { id: (lexer::Token::Id(String::from("equal")), range), parameters: vec![left, right] },
                        lexer::Token::NotEqual => Call { id: (lexer::Token::Id(String::from("not_equal")), range), parameters: vec![left, right] },
                        lexer::Token::Smaller => Call { id: (lexer::Token::Id(String::from("smaller")), range), parameters: vec![left, right] },
                        lexer::Token::SmallerEqual => Call { id: (lexer::Token::Id(String::from("smaller_equal")), range), parameters: vec![left, right] },
                        lexer::Token::Larger => Call { id: (lexer::Token::Id(String::from("larger")), range), parameters: vec![left, right] },
                        lexer::Token::LargerEqual => Call { id: (lexer::Token::Id(String::from("larger_equal")), range), parameters: vec![left, right] },
                        lexer::Token::Or => Call { id: (lexer::Token::Id(String::from("or")), range), parameters: vec![left, right] },
                        lexer::Token::And => Call { id: (lexer::Token::Id(String::from("and")), range), parameters: vec![left, right] },
                        lexer::Token::Xor => Call { id: (lexer::Token::Id(String::from("xor")), range), parameters: vec![left, right] },
                        _ => panic!("Invalid token as operator")
                    })
                },
                _ => break Ok(left)
            }
        }
    }

    // A value is an atom, a fundamental unit that cannot be split (generally)
    fn value(&mut self) -> Result<Expression, error::Error<'a>> {
        let current = self.current.take();
        self.next();

        Ok(match current {
            Some(value @ (lexer::Token::Int(_), _)) => Expression::Int(value),
            Some(id @ (lexer::Token::Id(_), _)) if matches!(self.current, Some((lexer::Token::LeftBracket, _))) => {
                self.next();

                let mut parameters = Vec::new();

                loop {
                    match self.current {
                        Some((lexer::Token::RightBracket, _)) => break,
                        None => {
                            let (name, range) = self.token_report_data();

                            return Err(error::Error::Expected {
                                filename: self.filename,
                                text: self.text,
                                range,
                                expected: &["expression", "`)`"],
                                found: name,
                            });
                        }
                        _ => parameters.push(self.expression()?)
                    }

                    if matches!(self.current, Some((lexer::Token::Separate, _))) {
                        self.next();
                    } else if !matches!(self.current, Some((lexer::Token::RightBracket, _))) {
                        let (name, range) = self.token_report_data();

                        return Err(error::Error::Expected {
                            filename: self.filename,
                            text: self.text,
                            range,
                            expected: &["`,`", "`)`"],
                            found: name,
                        });
                    }
                }

                self.next();

                Expression::Call(Call {
                    id,
                    parameters,
                })
            }
            Some(value @ (lexer::Token::Id(_), _)) => Expression::Id(value),
            Some(value @ (lexer::Token::Bool(_), _)) => Expression::Bool(value),
            Some((lexer::Token::Not, range)) => Expression::Call(Call {
                id: (lexer::Token::Id(String::from("not")), range),
                parameters: vec![self.value()?],
            }),
            Some((lexer::Token::LeftBracket, _)) => {
                let result = self.expression()?;

                if matches!(self.current, Some((lexer::Token::RightBracket, _))) {
                    self.next();

                    result
                } else {
                    let (name, range) = self.token_report_data();

                    return Err(error::Error::Expected {
                        filename: self.filename,
                        text: self.text,
                        range,
                        expected: &["`)`"],
                        found: name,
                    });
                }
            }
            _ => {
                self.current = current;

                let (name, range) = self.token_report_data();

                return Err(error::Error::Expected {
                    filename: self.filename,
                    text: self.text,
                    range,
                    expected: &["integer", "boolean", "identifier", "`(`"],
                    found: name,
                });
            }
        })
    }
}

pub fn parse<'a>(filename: &'a str, text: &'a str, tokens: Vec<alias::SpanToken>) -> Result<Program, error::Error<'a>> {
    Parser {
        filename,
        text,
        tokens,
        current: None,
    }.parse()
}
