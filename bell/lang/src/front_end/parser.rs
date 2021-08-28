use std::ops;

use crate::core::error;

use super::lexer;

#[derive(Debug)]
pub enum Expression {
    Int(lexer::SpanToken),
    Bool(lexer::SpanToken),
    Id(lexer::SpanToken),
    Block(Box<Block>),
    Assign(Box<Assign>),
    Declaration(Box<Declaration>),
    While(Box<While>),
    Conditional(Box<Conditional>),
    Function(Box<Function>),
    Call(Call),
}

#[derive(Debug)]
pub struct Assign {
    pub id: lexer::SpanToken,
    pub value: Expression,
    pub range: ops::Range<usize>,
}

#[derive(Debug)]
pub struct Declaration {
    pub id: lexer::SpanToken,
    pub hint: Option<lexer::SpanToken>,
    pub value: Expression,
    pub range: ops::Range<usize>,
}

#[derive(Debug)]
pub struct Block {
    pub expressions: Vec<Expression>,
    pub tail: Option<Expression>,
    pub range: ops::Range<usize>,
}

#[derive(Debug)]
pub struct Function {
    pub id: lexer::SpanToken,
    pub parameters: Vec<(lexer::SpanToken, lexer::SpanToken)>,
    pub return_type: Option<lexer::SpanToken>,
    pub body: Block,
    pub range: ops::Range<usize>,
}

#[derive(Debug)]
pub struct Branch {
    pub condition: Expression,
    pub body: Block,
    pub range: ops::Range<usize>,
}

#[derive(Debug)]
pub struct Call {
    pub id: lexer::SpanToken,
    pub parameters: Vec<Expression>,
    pub range: ops::Range<usize>,
}

#[derive(Debug)]
pub struct Conditional {
    pub branches: Vec<Branch>,
    pub tail: Option<Block>,
    pub range: ops::Range<usize>,
}

#[derive(Debug)]
pub struct While {
    pub condition: Expression,
    pub body: Block,
    pub range: ops::Range<usize>,
}

#[derive(Debug)]
pub struct Program(pub Vec<Function>);

struct Parser<'a> {
    tokens: Vec<lexer::SpanToken>,
    current: Option<lexer::SpanToken>,
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

    // Get the current tokens range. Used for future error reporting
    fn get_token_range(&self) -> ops::Range<usize> {
        if let Some(lexer::SpanToken(_, ops::Range { start, end })) = self.current {
            start..end
        } else {
            let length = self.text.len();

            length..(length + 1)
        }
    }

    // Generate some data to be used for error reporting
    fn token_report_data(&mut self) -> (&'static str, ops::Range<usize>) {
        if let Some(lexer::SpanToken(token, range)) = self.current.take() {
            (
                match token {
                    lexer::Token::Fn => "`fn`",
                    lexer::Token::Var => "`let`",
                    lexer::Token::While => "`while`",
                    lexer::Token::If => "`if`",
                    lexer::Token::Else => "`else`",
                    lexer::Token::Add => "`+`",
                    lexer::Token::Subtract => "`-`",
                    lexer::Token::Multiply => "`*`",
                    lexer::Token::Divide => "`/`",
                    lexer::Token::Modulo => "`%`",
                    lexer::Token::Assign => "`=`",
                    lexer::Token::Equal => "`==`",
                    lexer::Token::NotEqual => "`!=`",
                    lexer::Token::Larger => "`>`",
                    lexer::Token::LargerEqual => "`>=`",
                    lexer::Token::Smaller => "`<`",
                    lexer::Token::SmallerEqual => "`<=`",
                    lexer::Token::Or => "`||`",
                    lexer::Token::And => "`&&`",
                    lexer::Token::Not => "`!`",
                    lexer::Token::LeftBracket => "`(`",
                    lexer::Token::RightBracket => "`)`",
                    lexer::Token::LeftCurly => "`{`",
                    lexer::Token::RightCurly => "`}`",
                    lexer::Token::Terminator => "`;`",
                    lexer::Token::Separate => "`,`",
                    lexer::Token::Arrow => "`->`",
                    lexer::Token::Hint => "`:`",
                    lexer::Token::Int(_) => "integer",
                    lexer::Token::Bool(_) => "boolean",
                    lexer::Token::Id(_) => "identifier",
                    _ => panic!("Found error tokens on parsing tokens."),
                },
                range,
            )
        } else {
            let length = self.text.len();

            ("nothing", length..(length + 1))
        }
    }

    // Since the grammar is LL(1), no need to lookahead more than 1 token
    fn lookahead(&self) -> Option<&lexer::SpanToken> {
        self.tokens.get(0)
    }

    fn program(&mut self) -> Result<Program, error::Error<'a>> {
        self.next();

        let mut functions: Vec<Function> = Vec::new();

        Ok(Program(loop {
            if self.current == None {
                break functions;
            }

            functions.push(self.function()?)
        }))
    }

    // A conditional is made as a series of branches. It calls the last "else" branch the tail
    fn conditional(&mut self) -> Result<Conditional, error::Error<'a>> {
        let mut branches = Vec::new();
        let start = self.get_token_range().start;

        // A conditional is matched with a `if` at the start, so you can use a branch here
        branches.push(self.branch()?);

        Ok(loop {
            branches.push(match self.current {
                Some(lexer::SpanToken(lexer::Token::Else, _))
                    if matches!(
                        self.lookahead(),
                        Some(lexer::SpanToken(lexer::Token::If, _))
                    ) =>
                {
                    self.branch()?
                }
                Some(lexer::SpanToken(lexer::Token::Else, _)) => {
                    self.next();

                    break Conditional {
                        branches,
                        tail: Some(self.block()?),
                        range: start..self.get_token_range().end,
                    };
                }
                _ => {
                    break Conditional {
                        branches,
                        tail: None,
                        range: start..self.get_token_range().end,
                    }
                }
            })
        })
    }

    // Branches can be of the form `else if` or `if`. We can manually parse `else` branches
    fn branch(&mut self) -> Result<Branch, error::Error<'a>> {
        let start = self.get_token_range().start;

        if matches!(self.current, Some(lexer::SpanToken(lexer::Token::Else, _))) {
            self.next();
        }

        if matches!(self.current, Some(lexer::SpanToken(lexer::Token::If, _))) {
            self.next();

            Ok(Branch {
                condition: self.expression()?,
                body: self.block()?,
                range: start..self.get_token_range().end,
            })
        } else {
            let (name, range) = self.token_report_data();

            Err(error::Error {
                filename: self.filename,
                text: self.text,
                error: error::ErrorKind::Expected {
                    range,
                    expected: &["`if`"],
                    found: name,
                },
            })
        }
    }

    fn while_loop(&mut self) -> Result<While, error::Error<'a>> {
        let start = self.get_token_range().start;

        if !matches!(self.current, Some(lexer::SpanToken(lexer::Token::While, _))) {
            let (name, range) = self.token_report_data();

            return Err(error::Error {
                filename: self.filename,
                text: self.text,
                error: error::ErrorKind::Expected {
                    range,
                    expected: &["`while`"],
                    found: name,
                },
            });
        }

        self.next();

        Ok(While {
            condition: self.expression()?,
            body: self.block()?,
            range: start..self.get_token_range().end,
        })
    }

    fn function(&mut self) -> Result<Function, error::Error<'a>> {
        let start = self.get_token_range().start;

        if !matches!(self.current, Some(lexer::SpanToken(lexer::Token::Fn, _))) {
            let (name, range) = self.token_report_data();

            return Err(error::Error {
                filename: self.filename,
                text: self.text,
                error: error::ErrorKind::Expected {
                    range,
                    expected: &["`fn`"],
                    found: name,
                },
            });
        }

        self.next();

        if matches!(self.current, Some(lexer::SpanToken(lexer::Token::Id(_), _))) {
            let id = self.current.take().unwrap();

            self.next();

            // Get the list of parameters this function takes. This is easily the largest part of this function.
            // It is done this way in order to allow for oxford commas
            if matches!(
                self.current,
                Some(lexer::SpanToken(lexer::Token::LeftBracket, _))
            ) {
                self.next();

                let mut parameters = Vec::new();

                loop {
                    match self.current.take() {
                        Some(id @ lexer::SpanToken(lexer::Token::Id(_), _)) => {
                            self.next();

                            if matches!(self.current, Some(lexer::SpanToken(lexer::Token::Hint, _)))
                            {
                                self.next();

                                if matches!(
                                    self.current,
                                    Some(lexer::SpanToken(lexer::Token::Id(_), _))
                                ) {
                                    parameters.push((id, self.current.take().unwrap()));

                                    self.next();
                                } else {
                                    let (name, range) = self.token_report_data();

                                    return Err(error::Error {
                                        filename: self.filename,
                                        text: self.text,
                                        error: error::ErrorKind::Expected {
                                            range,
                                            expected: &["identifier"],
                                            found: name,
                                        },
                                    });
                                }
                            } else {
                                let (name, range) = self.token_report_data();

                                return Err(error::Error {
                                    filename: self.filename,
                                    text: self.text,
                                    error: error::ErrorKind::Expected {
                                        range,
                                        expected: &["type specifier"],
                                        found: name,
                                    },
                                });
                            }

                            if matches!(
                                self.current,
                                Some(lexer::SpanToken(lexer::Token::Separate, _))
                            ) {
                                self.next();
                            } else if !matches!(
                                self.current,
                                Some(lexer::SpanToken(lexer::Token::RightBracket, _))
                            ) {
                                let (name, range) = self.token_report_data();

                                return Err(error::Error {
                                    filename: self.filename,
                                    text: self.text,
                                    error: error::ErrorKind::Expected {
                                        range,
                                        expected: &["`,`", "`)`"],
                                        found: name,
                                    },
                                });
                            }
                        }
                        Some(lexer::SpanToken(lexer::Token::RightBracket, _)) => break,
                        token => {
                            self.current = token;

                            let (name, range) = self.token_report_data();

                            return Err(error::Error {
                                filename: self.filename,
                                text: self.text,
                                error: error::ErrorKind::Expected {
                                    range,
                                    expected: &["parameter", "`)`"],
                                    found: name,
                                },
                            });
                        }
                    }
                }

                self.next();

                let return_type =
                    if matches!(self.current, Some(lexer::SpanToken(lexer::Token::Arrow, _))) {
                        self.next();

                        if matches!(self.current, Some(lexer::SpanToken(lexer::Token::Id(_), _))) {
                            let result = self.current.take();

                            self.next();

                            result
                        } else {
                            let (name, range) = self.token_report_data();

                            return Err(error::Error {
                                filename: self.filename,
                                text: self.text,
                                error: error::ErrorKind::Expected {
                                    range,
                                    expected: &["identifier"],
                                    found: name,
                                },
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
                    range: start..self.get_token_range().end,
                })
            } else {
                let (name, range) = self.token_report_data();

                Err(error::Error {
                    filename: self.filename,
                    text: self.text,
                    error: error::ErrorKind::Expected {
                        range,
                        expected: &["`(`"],
                        found: name,
                    },
                })
            }
        } else {
            let (name, range) = self.token_report_data();

            Err(error::Error {
                filename: self.filename,
                text: self.text,
                error: error::ErrorKind::Expected {
                    range,
                    expected: &["identifier"],
                    found: name,
                },
            })
        }
    }

    // The tail of the block is the final expression that is returned by the block. A block may not have one.
    // Currently no named blocks or named returns exist, however this node will be redone once they are implemented
    fn block(&mut self) -> Result<Block, error::Error<'a>> {
        let start = self.get_token_range().start;

        if !matches!(
            self.current,
            Some(lexer::SpanToken(lexer::Token::LeftCurly, _))
        ) {
            let (name, range) = self.token_report_data();

            return Err(error::Error {
                filename: self.filename,
                text: self.text,
                error: error::ErrorKind::Expected {
                    range,
                    expected: &["`{`"],
                    found: name,
                },
            });
        }

        self.next();

        let mut expressions = Vec::new();

        let mut has_tail = false;
        let mut needs_termination; // A flag to check if the last node was a block node. If it was, then no terminator is necessary

        loop {
            match self.current {
                Some(lexer::SpanToken(lexer::Token::RightCurly, _)) => break,
                None => {
                    let (name, range) = self.token_report_data();

                    return Err(error::Error {
                        filename: self.filename,
                        text: self.text,
                        error: error::ErrorKind::Expected {
                            range,
                            expected: &["expression", "`}`"],
                            found: name,
                        },
                    });
                }
                _ => expressions.push({
                    let expression = self.expression()?;

                    /*
                    Some types of expressions don't need a termination since it can be inferred where they end.
                    Despite the fact you could initialize a variable by using a block, you won't always do that.
                    Thus things that don't always have the block as their final part aren't included here. Note
                    that if these expressions are at the end of the block, having a semicolon WILL effect
                    whether or not the function returns a value
                    */
                    needs_termination = !matches!(
                        expression,
                        Expression::Block(..)
                            | Expression::While(..)
                            | Expression::Conditional(..)
                            | Expression::Function(..)
                    );

                    expression
                }),
            }

            has_tail = true;

            if matches!(
                self.current,
                Some(lexer::SpanToken(lexer::Token::Terminator, _))
            ) {
                has_tail = false;

                self.next();
            } else if needs_termination
                && !matches!(
                    self.current,
                    Some(lexer::SpanToken(lexer::Token::RightCurly, _))
                )
            {
                let (name, range) = self.token_report_data();

                return Err(error::Error {
                    filename: self.filename,
                    text: self.text,
                    error: error::ErrorKind::Expected {
                        range,
                        expected: &["`;`", "`}`"],
                        found: name,
                    },
                });
            }
        }

        if self.current == None {
            let (name, range) = self.token_report_data();

            return Err(error::Error {
                filename: self.filename,
                text: self.text,
                error: error::ErrorKind::Expected {
                    range,
                    expected: &["`}`"],
                    found: name,
                },
            });
        }

        let tail = if has_tail { expressions.pop() } else { None };

        self.next();

        Ok(Block {
            expressions,
            tail,
            range: start..self.get_token_range().end,
        })
    }

    fn assign(&mut self) -> Result<Assign, error::Error<'a>> {
        let start = self.get_token_range().start;

        if matches!(self.current, Some(lexer::SpanToken(lexer::Token::Id(_), _))) {
            let id = self.current.take().unwrap();

            self.next();

            if matches!(
                self.current,
                Some(lexer::SpanToken(lexer::Token::Assign, _))
            ) {
                self.next();

                Ok(Assign {
                    id,
                    value: self.expression()?,
                    range: start..self.get_token_range().end,
                })
            } else {
                let (name, range) = self.token_report_data();

                Err(error::Error {
                    filename: self.filename,
                    text: self.text,
                    error: error::ErrorKind::Expected {
                        range,
                        expected: &["`=`"],
                        found: name,
                    },
                })
            }
        } else {
            let (name, range) = self.token_report_data();

            Err(error::Error {
                filename: self.filename,
                text: self.text,
                error: error::ErrorKind::Expected {
                    range,
                    expected: &["identifier"],
                    found: name,
                },
            })
        }
    }

    // Constants may be added back in, although they currently are not.
    // There is no reason not to add them aside from making the language more complex
    fn declaration(&mut self) -> Result<Declaration, error::Error<'a>> {
        let start = self.get_token_range().start;

        if !matches!(self.current, Some(lexer::SpanToken(lexer::Token::Var, _))) {
            let (name, range) = self.token_report_data();

            return Err(error::Error {
                filename: self.filename,
                text: self.text,
                error: error::ErrorKind::Expected {
                    range,
                    expected: &["`var`"],
                    found: name,
                },
            });
        }

        self.next();

        if matches!(self.current, Some(lexer::SpanToken(lexer::Token::Id(_), _))) {
            let id = self.current.take().unwrap();

            self.next();

            let mut hint = None;

            if matches!(self.current, Some(lexer::SpanToken(lexer::Token::Hint, _))) {
                self.next();

                if matches!(self.current, Some(lexer::SpanToken(lexer::Token::Id(_), _))) {
                    hint = self.current.take();

                    self.next();
                } else {
                    let (name, range) = self.token_report_data();

                    return Err(error::Error {
                        filename: self.filename,
                        text: self.text,
                        error: error::ErrorKind::Expected {
                            range,
                            expected: &["identifier"],
                            found: name,
                        },
                    });
                }
            }

            if matches!(
                self.current,
                Some(lexer::SpanToken(lexer::Token::Assign, _))
            ) {
                self.next();

                Ok(Declaration {
                    id,
                    hint,
                    value: self.expression()?,
                    range: start..self.get_token_range().end,
                })
            } else {
                let (name, range) = self.token_report_data();

                Err(error::Error {
                    filename: self.filename,
                    text: self.text,
                    error: error::ErrorKind::Expected {
                        range,
                        expected: &["`=`"],
                        found: name,
                    },
                })
            }
        } else {
            let (name, range) = self.token_report_data();

            Err(error::Error {
                filename: self.filename,
                text: self.text,
                error: error::ErrorKind::Expected {
                    range,
                    expected: &["identifier"],
                    found: name,
                },
            })
        }
    }

    // Expressions are anything that returns a value. It thus follows that almost everything in Bell is an expression.
    // In order to be in a block comment, you must be an expression.
    // Some things are expressions, but just return the unit type such as assignments
    fn expression(&mut self) -> Result<Expression, error::Error<'a>> {
        Ok(match self.current {
            Some(lexer::SpanToken(lexer::Token::Var, _)) => {
                Expression::Declaration(Box::from(self.declaration()?))
            }
            Some(lexer::SpanToken(lexer::Token::While, _)) => {
                Expression::While(Box::from(self.while_loop()?))
            }
            Some(lexer::SpanToken(lexer::Token::If, _)) => {
                Expression::Conditional(Box::from(self.conditional()?))
            }
            Some(lexer::SpanToken(lexer::Token::LeftCurly, _)) => {
                Expression::Block(Box::from(self.block()?))
            }
            Some(lexer::SpanToken(lexer::Token::Fn, _)) => {
                Expression::Function(Box::from(self.function()?))
            }
            Some(lexer::SpanToken(lexer::Token::Id(_), _))
                if matches!(
                    self.lookahead(),
                    Some(lexer::SpanToken(lexer::Token::Assign, _))
                ) =>
            {
                Expression::Assign(Box::from(self.assign()?))
            }
            _ => self.logic()?,
        })
    }

    // Logic operations act on booleans
    fn logic(&mut self) -> Result<Expression, error::Error<'a>> {
        self.binary_operation(&Self::comparison, vec![lexer::Token::Or, lexer::Token::And])
    }

    // Comparisons compare objects
    fn comparison(&mut self) -> Result<Expression, error::Error<'a>> {
        self.binary_operation(
            &Self::sum,
            vec![
                lexer::Token::Equal,
                lexer::Token::NotEqual,
                lexer::Token::Smaller,
                lexer::Token::SmallerEqual,
                lexer::Token::Larger,
                lexer::Token::LargerEqual,
            ],
        )
    }

    fn sum(&mut self) -> Result<Expression, error::Error<'a>> {
        self.binary_operation(
            &Self::product,
            vec![lexer::Token::Add, lexer::Token::Subtract],
        )
    }

    fn product(&mut self) -> Result<Expression, error::Error<'a>> {
        self.binary_operation(
            &Self::value,
            vec![
                lexer::Token::Multiply,
                lexer::Token::Divide,
                lexer::Token::Modulo,
            ],
        )
    }

    // A "generic" function to parse a binary operation. Is used by math expression-related nodes.
    // Each group of operations are implemented in their own function, so operator precedence will work
    #[inline]
    fn binary_operation<F>(
        &mut self,
        operand: &F,
        operators: Vec<lexer::Token>,
    ) -> Result<Expression, error::Error<'a>>
    where
        F: Fn(&mut Self) -> Result<Expression, error::Error<'a>>,
    {
        let start = self.get_token_range().start;
        let mut left = operand(self)?;

        loop {
            match self.current.to_owned() {
                Some(lexer::SpanToken(operator, range)) if operators.contains(&operator) => {
                    left = {
                        self.next();
                        let right = operand(self)?;

                        Expression::Call(match operator {
                            lexer::Token::Add => Call {
                                id: lexer::SpanToken(lexer::Token::Id(String::from("add")), range),
                                parameters: vec![left, right],
                                range: start..self.get_token_range().end,
                            },
                            lexer::Token::Subtract => Call {
                                id: lexer::SpanToken(
                                    lexer::Token::Id(String::from("subtract")),
                                    range,
                                ),
                                parameters: vec![left, right],
                                range: start..self.get_token_range().end,
                            },
                            lexer::Token::Multiply => Call {
                                id: lexer::SpanToken(
                                    lexer::Token::Id(String::from("multiply")),
                                    range,
                                ),
                                parameters: vec![left, right],
                                range: start..self.get_token_range().end,
                            },
                            lexer::Token::Divide => Call {
                                id: lexer::SpanToken(
                                    lexer::Token::Id(String::from("divide")),
                                    range,
                                ),
                                parameters: vec![left, right],
                                range: start..self.get_token_range().end,
                            },
                            lexer::Token::Modulo => Call {
                                id: lexer::SpanToken(
                                    lexer::Token::Id(String::from("modulo")),
                                    range,
                                ),
                                parameters: vec![left, right],
                                range: start..self.get_token_range().end,
                            },
                            lexer::Token::Equal => Call {
                                id: lexer::SpanToken(
                                    lexer::Token::Id(String::from("equal")),
                                    range,
                                ),
                                parameters: vec![left, right],
                                range: start..self.get_token_range().end,
                            },
                            lexer::Token::NotEqual => Call {
                                id: lexer::SpanToken(
                                    lexer::Token::Id(String::from("not_equal")),
                                    range,
                                ),
                                parameters: vec![left, right],
                                range: start..self.get_token_range().end,
                            },
                            lexer::Token::Larger => Call {
                                id: lexer::SpanToken(
                                    lexer::Token::Id(String::from("larger")),
                                    range,
                                ),
                                parameters: vec![left, right],
                                range: start..self.get_token_range().end,
                            },
                            lexer::Token::LargerEqual => Call {
                                id: lexer::SpanToken(
                                    lexer::Token::Id(String::from("larger_equal")),
                                    range,
                                ),
                                parameters: vec![left, right],
                                range: start..self.get_token_range().end,
                            },
                            lexer::Token::Smaller => Call {
                                id: lexer::SpanToken(
                                    lexer::Token::Id(String::from("smaller")),
                                    range,
                                ),
                                parameters: vec![left, right],
                                range: start..self.get_token_range().end,
                            },
                            lexer::Token::SmallerEqual => Call {
                                id: lexer::SpanToken(
                                    lexer::Token::Id(String::from("smaller_equal")),
                                    range,
                                ),
                                parameters: vec![left, right],
                                range: start..self.get_token_range().end,
                            },
                            lexer::Token::Or => Call {
                                id: lexer::SpanToken(lexer::Token::Id(String::from("or")), range),
                                parameters: vec![left, right],
                                range: start..self.get_token_range().end,
                            },
                            lexer::Token::And => Call {
                                id: lexer::SpanToken(lexer::Token::Id(String::from("and")), range),
                                parameters: vec![left, right],
                                range: start..self.get_token_range().end,
                            },
                            _ => panic!("Invalid token as operator"),
                        })
                    }
                }
                _ => break Ok(left),
            }
        }
    }

    // Values represent a single value which can be manipulated by expressions.
    // When used in expressions, it's impossible to get any lower than them. They make up expressions
    fn value(&mut self) -> Result<Expression, error::Error<'a>> {
        let start = self.get_token_range().start;
        let current = self.current.take();

        self.next();

        Ok(match current {
            Some(int @ lexer::SpanToken(lexer::Token::Int(_), _)) => Expression::Int(int),
            Some(id @ lexer::SpanToken(lexer::Token::Id(_), _))
                if matches!(
                    self.current,
                    Some(lexer::SpanToken(lexer::Token::LeftBracket, _))
                ) =>
            {
                self.next();

                let mut parameters = Vec::new();

                loop {
                    match self.current {
                        Some(lexer::SpanToken(lexer::Token::RightBracket, _)) => break,
                        None => {
                            let (name, range) = self.token_report_data();

                            return Err(error::Error {
                                filename: self.filename,
                                text: self.text,
                                error: error::ErrorKind::Expected {
                                    range,
                                    expected: &["expression", "`)`"],
                                    found: name,
                                },
                            });
                        }
                        _ => parameters.push(self.expression()?),
                    }

                    if matches!(
                        self.current,
                        Some(lexer::SpanToken(lexer::Token::Separate, _))
                    ) {
                        self.next();
                    } else if !matches!(
                        self.current,
                        Some(lexer::SpanToken(lexer::Token::RightBracket, _))
                    ) {
                        let (name, range) = self.token_report_data();

                        return Err(error::Error {
                            filename: self.filename,
                            text: self.text,
                            error: error::ErrorKind::Expected {
                                range,
                                expected: &["`,`", "`)`"],
                                found: name,
                            },
                        });
                    }
                }

                self.next();

                Expression::Call(Call {
                    id,
                    parameters,
                    range: start..self.get_token_range().end,
                })
            }
            Some(id @ lexer::SpanToken(lexer::Token::Id(_), _)) => Expression::Id(id),
            Some(bool @ lexer::SpanToken(lexer::Token::Bool(_), _)) => Expression::Bool(bool),
            Some(lexer::SpanToken(lexer::Token::Not, range)) => Expression::Call(Call {
                id: lexer::SpanToken(lexer::Token::Id(String::from("not")), range),
                parameters: vec![self.value()?],
                range: start..self.get_token_range().end,
            }),
            Some(lexer::SpanToken(lexer::Token::LeftBracket, _)) => {
                let result = self.expression()?;

                if matches!(
                    self.current,
                    Some(lexer::SpanToken(lexer::Token::RightBracket, _))
                ) {
                    self.next();

                    result
                } else {
                    let (name, range) = self.token_report_data();

                    return Err(error::Error {
                        filename: self.filename,
                        text: self.text,
                        error: error::ErrorKind::Expected {
                            range,
                            expected: &["`)`"],
                            found: name,
                        },
                    });
                }
            }
            _ => {
                self.current = current;

                let (name, range) = self.token_report_data();

                return Err(error::Error {
                    filename: self.filename,
                    text: self.text,
                    error: error::ErrorKind::Expected {
                        range,
                        expected: &["integer", "boolean", "identifier", "`(`"],
                        found: name,
                    },
                });
            }
        })
    }
}

pub fn parse<'a>(
    filename: &'a str,
    text: &'a str,
    tokens: Vec<lexer::SpanToken>,
) -> Result<Program, error::Error<'a>> {
    Parser {
        filename,
        text,
        tokens,
        current: None,
    }
    .program()
}
