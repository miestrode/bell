use std::{borrow, collections, ops};

use crate::core::error;

use super::{lexer, parser};

fn extract_id(identifier: lexer::SpanToken) -> (String, ops::Range<usize>) {
    if let lexer::SpanToken(lexer::Token::Id(string), range) = identifier {
        (string, range)
    } else {
        panic!("Expected token to be an identifier")
    }
}

fn create_identifier(name: &str, range: ops::Range<usize>) -> lexer::SpanToken {
    lexer::SpanToken(lexer::Token::Id(String::from(name)), range)
}

struct Symbol<'a> {
    symbol: SymbolKind<'a>,
    span: borrow::Cow<'a, ops::Range<usize>>,
}

enum SymbolKind<'a> {
    Function {
        parameters: Vec<borrow::Cow<'a, lexer::SpanToken>>,
        return_type: borrow::Cow<'a, lexer::SpanToken>,
        standard: bool,
    },
    Variable {
        data_type: &'a lexer::SpanToken,
    },
}

/*
Stores the different items in each scope of the program.
The symbol table is implemented as a list of hashmaps. The last hashmap in the list is the current scope.
*/
struct Table<'a>(Vec<collections::HashMap<borrow::Cow<'a, lexer::SpanToken>, Symbol<'a>>>);

impl<'a> Table<'a> {
    // Todo: Make standard functions be included in the standard library using functions that insert MCfunction directly,
    pub fn new() -> Self {
        // These symbols are implemented by the compiler, so they are just added in for verification
        let mut standards = collections::HashMap::new();

        standards.insert(
            borrow::Cow::Owned(create_identifier("negate", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![borrow::Cow::Owned(create_identifier("int", 0..0))],
                    return_type: borrow::Cow::Owned(create_identifier("int", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        standards.insert(
            borrow::Cow::Owned(create_identifier("add", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                    ],
                    return_type: borrow::Cow::Owned(create_identifier("int", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        standards.insert(
            borrow::Cow::Owned(create_identifier("subtract", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                    ],
                    return_type: borrow::Cow::Owned(create_identifier("int", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        standards.insert(
            borrow::Cow::Owned(create_identifier("multiply", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                    ],
                    return_type: borrow::Cow::Owned(create_identifier("int", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        standards.insert(
            borrow::Cow::Owned(create_identifier("divide", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                    ],
                    return_type: borrow::Cow::Owned(create_identifier("int", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        standards.insert(
            borrow::Cow::Owned(create_identifier("modulo", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                    ],
                    return_type: borrow::Cow::Owned(create_identifier("int", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        standards.insert(
            borrow::Cow::Owned(create_identifier("equal", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                    ],
                    return_type: borrow::Cow::Owned(create_identifier("bool", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        standards.insert(
            borrow::Cow::Owned(create_identifier("not_equal", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                    ],
                    return_type: borrow::Cow::Owned(create_identifier("bool", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        standards.insert(
            borrow::Cow::Owned(create_identifier("lesser", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                    ],
                    return_type: borrow::Cow::Owned(create_identifier("bool", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        standards.insert(
            borrow::Cow::Owned(create_identifier("greater", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                    ],
                    return_type: borrow::Cow::Owned(create_identifier("bool", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        standards.insert(
            borrow::Cow::Owned(create_identifier("lesser_equal", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                    ],
                    return_type: borrow::Cow::Owned(create_identifier("bool", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        standards.insert(
            borrow::Cow::Owned(create_identifier("greater_equal", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                        borrow::Cow::Owned(create_identifier("int", 0..0)),
                    ],
                    return_type: borrow::Cow::Owned(create_identifier("bool", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        standards.insert(
            borrow::Cow::Owned(create_identifier("or", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![
                        borrow::Cow::Owned(create_identifier("bool", 0..0)),
                        borrow::Cow::Owned(create_identifier("bool", 0..0)),
                    ],
                    return_type: borrow::Cow::Owned(create_identifier("bool", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        standards.insert(
            borrow::Cow::Owned(create_identifier("and", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![
                        borrow::Cow::Owned(create_identifier("bool", 0..0)),
                        borrow::Cow::Owned(create_identifier("bool", 0..0)),
                    ],
                    return_type: borrow::Cow::Owned(create_identifier("bool", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        standards.insert(
            borrow::Cow::Owned(create_identifier("not", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![borrow::Cow::Owned(create_identifier("bool", 0..0))],
                    return_type: borrow::Cow::Owned(create_identifier("bool", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        standards.insert(
            borrow::Cow::Owned(create_identifier("println", 0..0)),
            Symbol {
                symbol: SymbolKind::Function {
                    parameters: vec![borrow::Cow::Owned(create_identifier("int", 0..0))],
                    return_type: borrow::Cow::Owned(create_identifier("unit", 0..0)),
                    standard: true,
                },
                span: borrow::Cow::Owned(0..0),
            },
        );

        Self(vec![standards])
    }

    fn add_symbol(&mut self, id: &'a lexer::SpanToken, symbol: Symbol<'a>) {
        self.0
            .last_mut()
            .unwrap()
            .insert(borrow::Cow::Borrowed(id), symbol);
    }

    // Locate a symbol in the symbol table with a condition
    fn get_symbol<F: Fn(&Symbol) -> bool>(
        &self,
        id: &lexer::SpanToken,
        looking_for: F,
    ) -> Option<&Symbol<'a>> {
        for scope in self.0.iter() {
            let result = scope.get(id);

            if result.is_some() && looking_for(result.unwrap()) {
                return result;
            }
        }

        None
    }

    fn get_symbol_in_current(&self, id: &lexer::SpanToken) -> Option<&Symbol<'a>> {
        self.0.last().unwrap().get(id)
    }

    fn enter(&mut self) {
        self.0.push(collections::HashMap::new());
    }

    fn exit(&mut self) {
        self.0.pop();
    }
}

/*
The type checker has two goals:
Firstly, to make sure the program is correct with the Bell type system
And secondly Infer types for variables and functions
Because of these goals, the type checker will modify the Program node it stores in order
to specify things like concrete types.
*/
struct TypeChecker<'a> {
    filename: &'a str,
    text: &'a str,
}

impl<'a> TypeChecker<'a> {
    fn check(&mut self, mut program: parser::Program) -> Result<parser::Program, error::Error<'a>> {
        let mut table = Table::new();

        table.enter();

        for function in program.0.iter_mut() {
            self.function(function, &mut table)?;
        }

        table.exit();

        Ok(program)
    }

    fn function<'b>(
        &self,
        function: &'b mut parser::Function,
        table: &mut Table<'b>,
    ) -> Result<lexer::SpanToken, error::Error<'a>> {
        if let Some(Symbol {
            symbol: SymbolKind::Function { .. },
            span,
        }) = table.get_symbol(&function.id, |symbol| {
            matches!(symbol.symbol, SymbolKind::Function { .. })
        }) {
            return Err(error::Error {
                filename: self.filename,
                text: self.text,
                error: error::ErrorKind::ShadowedSymbol {
                    old: (**span).clone(),
                    new: function.range.clone(),
                    name: extract_id(function.id.clone()).0,
                    symbol: String::from("function"),
                },
            });
        }

        let mut parameters = Vec::with_capacity(function.parameters.len());
        table.enter();

        for (id, data_type) in function.parameters.iter() {
            let parameter_span = ops::Range {
                start: id.1.start,
                end: data_type.1.end,
            };

            if let Some(Symbol {
                symbol: SymbolKind::Variable { .. },
                span,
            }) = table.get_symbol_in_current(id)
            {
                return Err(error::Error {
                    filename: self.filename,
                    text: self.text,
                    error: error::ErrorKind::ShadowedSymbol {
                        old: (**span).clone(),
                        new: parameter_span,
                        name: extract_id(id.clone()).0,
                        symbol: String::from("parameter"),
                    },
                });
            }

            parameters.push(borrow::Cow::Borrowed(data_type));
            table.add_symbol(
                id,
                Symbol {
                    symbol: SymbolKind::Variable { data_type },
                    span: borrow::Cow::Owned(parameter_span),
                },
            )
        }

        let tail = self.block(&mut function.body, table, false)?;

        match &function.return_type {
            Some(return_type) => {
                if return_type != &tail {
                    let tail = extract_id(tail);
                    let return_type = extract_id(function.return_type.take().unwrap());

                    return Err(error::Error {
                        filename: self.filename,
                        text: self.text,
                        error: error::ErrorKind::DataTypeMismatch {
                            expected: return_type.0,
                            got: tail.0,
                            because: Some(return_type.1),
                            got_location: tail.1,
                        },
                    });
                }
            }
            None => function.return_type = Some(tail),
        }

        table.add_symbol(
            &function.id,
            Symbol {
                symbol: SymbolKind::Function {
                    parameters,
                    return_type: borrow::Cow::Borrowed(function.return_type.as_ref().unwrap()),
                    standard: false,
                },
                span: borrow::Cow::Borrowed(&function.range),
            },
        );

        Ok(create_identifier("unit", function.range.clone()))
    }

    // Since blocks don't have early returns, it's really easy to infer types for them
    fn block<'b>(
        &self,
        block: &'b mut parser::Block,
        table: &mut Table<'b>,
        new_scope: bool, // A special case added because of functions. You want to check their parameters but also have them in the same scope as other things
    ) -> Result<lexer::SpanToken, error::Error<'a>> {
        if new_scope {
            table.enter();
        }

        for expression in block.expressions.iter_mut() {
            self.expression(expression, table)?;
        }

        let returned_type = if let Some(tail) = &mut block.tail {
            self.expression(tail, table)
        } else {
            Ok(create_identifier("unit", block.range.clone()))
        };

        table.exit();

        returned_type
    }

    fn expression<'b>(
        &self,
        expression: &'b mut parser::Expression,
        table: &mut Table<'b>,
    ) -> Result<lexer::SpanToken, error::Error<'a>> {
        match expression {
            parser::Expression::Int(integer) => Ok(create_identifier("int", integer.1.clone())),
            parser::Expression::Bool(boolean) => Ok(create_identifier("bool", boolean.1.clone())),
            parser::Expression::Id(identifier) => self.variable(identifier, table),
            parser::Expression::Function(function) => self.function(function, table),
            parser::Expression::Block(block) => self.block(block, table, true),
            parser::Expression::Declaration(declaration) => self.declaration(declaration, table),
            parser::Expression::Assign(assign) => self.assign(assign, table),
            parser::Expression::While(while_loop) => self.while_loop(while_loop, table),
            parser::Expression::Conditional(conditional) => self.conditional(conditional, table),
            parser::Expression::Call(call) => self.call(call, table),
        }
    }

    fn variable<'b>(
        &self,
        identifier: &'b lexer::SpanToken,
        table: &mut Table<'b>,
    ) -> Result<lexer::SpanToken, error::Error<'a>> {
        let variable = table.get_symbol(identifier, |symbol| {
            matches!(symbol.symbol, SymbolKind::Variable { .. })
        });

        if let Some(Symbol {
            symbol: SymbolKind::Variable { data_type },
            ..
        }) = variable
        {
            Ok(lexer::SpanToken(data_type.0.clone(), identifier.1.clone()))
        } else {
            let variable = extract_id(identifier.clone());

            Err(error::Error {
                filename: self.filename,
                text: self.text,
                error: error::ErrorKind::UndeclaredSymbol {
                    name: variable.0,
                    usage: variable.1,
                },
            })
        }
    }

    fn declaration<'b>(
        &self,
        declaration: &'b mut parser::Declaration,
        table: &mut Table<'b>,
    ) -> Result<lexer::SpanToken, error::Error<'a>> {
        let inferred_type = self.expression(&mut declaration.value, table)?;

        match &declaration.hint {
            Some(return_type) => {
                if return_type != &inferred_type {
                    let hint = extract_id(declaration.hint.take().unwrap());
                    let inferred_type = extract_id(inferred_type);

                    return Err(error::Error {
                        filename: self.filename,
                        text: self.text,
                        error: error::ErrorKind::DataTypeMismatch {
                            expected: hint.0,
                            got: inferred_type.0,
                            because: Some(hint.1),
                            got_location: inferred_type.1,
                        },
                    });
                }
            }
            None => declaration.hint = Some(inferred_type),
        }

        table.add_symbol(
            &declaration.id,
            Symbol {
                symbol: SymbolKind::Variable {
                    data_type: declaration.hint.as_ref().unwrap(),
                },
                span: borrow::Cow::Borrowed(&declaration.range),
            },
        );

        Ok(create_identifier("unit", declaration.range.clone()))
    }

    fn assign<'b>(
        &self,
        assign: &'b mut parser::Assign,
        table: &mut Table<'b>,
    ) -> Result<lexer::SpanToken, error::Error<'a>> {
        let inferred_type = self.expression(&mut assign.value, table)?;

        if let Some(Symbol {
            symbol: SymbolKind::Variable { data_type },
            ..
        }) = table.get_symbol(&assign.id, |symbol| {
            matches!(symbol.symbol, SymbolKind::Variable { .. })
        }) {
            if &&inferred_type == data_type {
                Ok(create_identifier("unit", assign.range.clone()))
            } else {
                let data_type = extract_id(data_type.clone().clone());
                let inferred_type = extract_id(inferred_type);

                Err(error::Error {
                    filename: self.filename,
                    text: self.text,
                    error: error::ErrorKind::DataTypeMismatch {
                        expected: data_type.0,
                        got: inferred_type.0,
                        because: Some(data_type.1),
                        got_location: inferred_type.1,
                    },
                })
            }
        } else {
            let variable = extract_id(assign.id.clone());

            Err(error::Error {
                filename: self.filename,
                text: self.text,
                error: error::ErrorKind::UndeclaredSymbol {
                    name: variable.0,
                    usage: variable.1,
                },
            })
        }
    }

    fn while_loop<'b>(
        &self,
        while_loop: &'b mut parser::While,
        table: &mut Table<'b>,
    ) -> Result<lexer::SpanToken, error::Error<'a>> {
        let condition_type = extract_id(self.expression(&mut while_loop.condition, table)?);
        let boolean = String::from("bool");

        if condition_type.0 != boolean {
            return Err(error::Error {
                filename: self.filename,
                text: self.text,
                error: error::ErrorKind::DataTypeMismatch {
                    expected: boolean,
                    got: condition_type.0,
                    because: None,
                    got_location: condition_type.1,
                },
            });
        }

        self.block(&mut while_loop.body, table, true)?;

        Ok(create_identifier("unit", while_loop.range.clone()))
    }

    fn conditional<'b>(
        &self,
        conditional: &'b mut parser::Conditional,
        table: &mut Table<'b>,
    ) -> Result<lexer::SpanToken, error::Error<'a>> {
        let mut conditional_type = None;

        for branch in conditional.branches.iter_mut() {
            let condition_type = extract_id(self.expression(&mut branch.condition, table)?);
            let boolean = String::from("bool");

            if condition_type.0 != boolean {
                return Err(error::Error {
                    filename: self.filename,
                    text: self.text,
                    error: error::ErrorKind::DataTypeMismatch {
                        expected: boolean,
                        got: condition_type.0,
                        because: None,
                        got_location: condition_type.1,
                    },
                });
            }

            let branch_type = self.block(&mut branch.body, table, true)?;

            match conditional_type {
                Some(_) if conditional_type.as_ref().unwrap() != &branch_type => {
                    let branch_type = extract_id(branch_type);
                    let conditional_type = extract_id(conditional_type.unwrap());

                    return Err(error::Error {
                        filename: self.filename,
                        text: self.text,
                        error: error::ErrorKind::DataTypeMismatch {
                            expected: conditional_type.0,
                            got: branch_type.0,
                            because: Some(conditional_type.1),
                            got_location: branch_type.1,
                        },
                    });
                }
                _ => conditional_type = Some(branch_type),
            }
        }

        if let Some(block) = &mut conditional.tail {
            let branch_type = self.block(block, table, true)?;

            match conditional_type {
                Some(_) if conditional_type.as_ref().unwrap() != &branch_type => {
                    let branch_type = extract_id(branch_type);
                    let conditional_type = extract_id(conditional_type.unwrap());

                    return Err(error::Error {
                        filename: self.filename,
                        text: self.text,
                        error: error::ErrorKind::DataTypeMismatch {
                            expected: conditional_type.0,
                            got: branch_type.0,
                            because: Some(conditional_type.1),
                            got_location: branch_type.1,
                        },
                    });
                }
                _ => conditional_type = Some(branch_type),
            }

            // Set the range of the conditional value to be the entire conditional, since technically it's a single value
            conditional_type.as_mut().unwrap().1 = conditional.range.clone();
        } else if conditional_type.as_ref().unwrap().0 != lexer::Token::Id(String::from("unit")) {
            // When a conditional doesn't return a unit on all branches, it is considered an expression, so it must have an else branch
            return Err(error::Error {
                filename: self.filename,
                text: self.text,
                error: error::ErrorKind::NoElseBranch {
                    location: conditional.range.clone(),
                },
            });
        }

        Ok(conditional_type.unwrap())
    }

    fn call<'b>(
        &self,
        call: &'b mut parser::Call,
        table: &mut Table<'b>,
    ) -> Result<lexer::SpanToken, error::Error<'a>> {
        let mut call_parameters = Vec::with_capacity(call.parameters.len());

        for expression in call.parameters.iter_mut() {
            call_parameters.push(self.expression(expression, table)?);
        }

        if let Some(Symbol {
            symbol:
                SymbolKind::Function {
                    parameters,
                    return_type,
                    standard,
                },
            span,
        }) = table.get_symbol(&call.id, |symbol| {
            matches!(symbol.symbol, SymbolKind::Function { .. })
        }) {
            if call_parameters.len() != parameters.len() {
                return Err(error::Error {
                    filename: self.filename,
                    text: self.text,
                    error: error::ErrorKind::ParameterCountMismatch {
                        expected_count: parameters.len(),
                        got_count: call_parameters.len(),
                        because: if !*standard {
                            Some((**span).clone())
                        } else {
                            None
                        },
                        got_location: call.range.clone(),
                        function: extract_id(call.id.clone()).0,
                    },
                });
            }

            for (call_parameter, expected_parameter) in call_parameters
                .iter()
                .map(|parameter| borrow::Cow::Borrowed(parameter))
                .zip(parameters.iter())
            {
                if expected_parameter != &call_parameter {
                    let expected_parameter = extract_id((**expected_parameter).clone());
                    let call_parameter = extract_id((*call_parameter).clone());

                    return Err(error::Error {
                        filename: self.filename,
                        text: self.text,
                        error: error::ErrorKind::DataTypeMismatch {
                            expected: expected_parameter.0,
                            got: call_parameter.0,
                            because: if !*standard {
                                Some(expected_parameter.1)
                            } else {
                                None
                            },
                            got_location: call_parameter.1,
                        },
                    });
                }
            }

            Ok(lexer::SpanToken(
                return_type.clone().into_owned().0,
                call.range.clone(),
            ))
        } else {
            let id = extract_id(call.id.clone());

            Err(error::Error {
                filename: self.filename,
                text: self.text,
                error: error::ErrorKind::UndeclaredSymbol {
                    name: id.0,
                    usage: id.1,
                },
            })
        }
    }
}

pub fn check<'a>(
    filename: &'a str,
    text: &'a str,
    program: parser::Program,
) -> Result<parser::Program, error::Error<'a>> {
    TypeChecker { filename, text }.check(program)
}
