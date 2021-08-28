use std::{collections, ops};

use crate::core::error;

use super::{lexer, parser};

fn extract_id(identifier: lexer::SpanToken) -> (String, ops::Range<usize>) {
    if let lexer::SpanToken(lexer::Token::Id(string), range) = identifier {
        (string, range)
    } else {
        panic!("Expected token to be an identifier")
    }
}

struct Symbol<'a> {
    symbol: SymbolKind<'a>,
    span: &'a ops::Range<usize>,
}

enum SymbolKind<'a> {
    Function {
        parameters: Vec<&'a lexer::SpanToken>,
        return_type: &'a lexer::SpanToken,
    },
    Variable {
        data_type: &'a lexer::SpanToken,
    },
}

/*
Stores the different items in each scope of the program.
The symbol table is implemented as a list of hashmaps. The last hashmap in the list is the current scope.
*/
struct Table<'a>(Vec<collections::HashMap<&'a lexer::SpanToken, Symbol<'a>>>);

impl<'a> Table<'a> {
    // Todo: Initializing a new table should initialize all the standard functions (like add and subtract and more)
    pub fn new() -> Self {
        Self(vec![])
    }

    fn add_symbol(&mut self, id: &'a lexer::SpanToken, symbol: Symbol<'a>) {
        self.0.last_mut().unwrap().insert(id, symbol);
    }

    // Locate a symbol in the symbol table
    fn get_symbol(&self, id: &lexer::SpanToken) -> Option<&Symbol<'a>> {
        for scope in self.0.iter() {
            let result = scope.get(id);

            if result.is_some() {
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
        let mut parameters = Vec::with_capacity(function.parameters.len());
        table.enter();

        for (id, data_type) in function.parameters.iter() {
            if table.get_symbol_in_current(id).is_some() {
                panic!("Duplicate parameter");
            }

            parameters.push(data_type);
            table.add_symbol(
                id,
                Symbol {
                    symbol: SymbolKind::Variable { data_type },
                    span: &function.range,
                },
            )
        }

        let tail = self.block(&mut function.body, table, false)?;

        match &function.return_type {
            Some(return_type) => {
                if return_type != &tail {
                    panic!("Function expected different return type");
                }
            }
            None => function.return_type = Some(tail),
        }

        table.add_symbol(
            &function.id,
            Symbol {
                symbol: SymbolKind::Function {
                    parameters,
                    return_type: function.return_type.as_ref().unwrap(),
                },
                span: &function.range,
            },
        );

        Ok(lexer::SpanToken(
            lexer::Token::Id(String::from("unit")),
            ops::Range { ..function.range },
        ))
    }

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
            Ok(lexer::SpanToken(
                lexer::Token::Id(String::from("unit")),
                ops::Range { ..block.range },
            ))
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
            parser::Expression::Int(integer) => Ok(lexer::SpanToken(
                lexer::Token::Id(String::from("int")),
                ops::Range { ..integer.1 },
            )),
            parser::Expression::Bool(boolean) => Ok(lexer::SpanToken(
                lexer::Token::Id(String::from("bool")),
                ops::Range { ..boolean.1 },
            )),
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
        let variable = table.get_symbol(identifier);

        if let Some(Symbol {
            symbol: SymbolKind::Variable { data_type },
            ..
        }) = variable
        {
            Ok(lexer::SpanToken(
                data_type.0.clone(),
                ops::Range { ..identifier.1 },
            ))
        } else {
            panic!("Variable doesn't exist");
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
                    panic!("Variable is declared with different data than type hint suggests");
                }
            }
            None => {
                table.add_symbol(
                    &declaration.id,
                    Symbol {
                        symbol: SymbolKind::Variable {
                            data_type: declaration.hint.insert(inferred_type),
                        },
                        span: &declaration.range,
                    },
                );
            }
        }

        Ok(lexer::SpanToken(
            lexer::Token::Id(String::from("unit")),
            ops::Range {
                ..declaration.range
            },
        ))
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
        }) = table.get_symbol(&assign.id)
        {
            if &&inferred_type == data_type {
                Ok(lexer::SpanToken(
                    lexer::Token::Id(String::from("unit")),
                    ops::Range { ..assign.range },
                ))
            } else {
                panic!("Variable is assigned to a different type than it stores");
            }
        } else {
            panic!("Variable doesn't exist");
        }
    }

    fn while_loop<'b>(
        &self,
        while_loop: &'b mut parser::While,
        table: &mut Table<'b>,
    ) -> Result<lexer::SpanToken, error::Error<'a>> {
        if self.expression(&mut while_loop.condition, table)?.0
            != lexer::Token::Id(String::from("bool"))
        {
            panic!("Condition needs to return a boolean");
        }

        self.block(&mut while_loop.body, table, true)?;

        Ok(lexer::SpanToken(
            lexer::Token::Id(String::from("unit")),
            ops::Range { ..while_loop.range },
        ))
    }

    fn conditional<'b>(
        &self,
        conditional: &'b mut parser::Conditional,
        table: &mut Table<'b>,
    ) -> Result<lexer::SpanToken, error::Error<'a>> {
        let mut return_type = None;

        for branch in conditional.branches.iter_mut() {
            if self.expression(&mut branch.condition, table)?.0
                != lexer::Token::Id(String::from("bool"))
            {
                panic!("Condition needs to return a boolean");
            }

            let branch_tail = self.block(&mut branch.body, table, true)?;

            match &return_type {
                Some(return_type) => {
                    if return_type != &branch_tail {
                        panic!("Conditional branches return differing types")
                    }
                }
                None => return_type = Some(branch_tail),
            }
        }

        Ok(if let Some(block) = &mut conditional.tail {
            let branch_tail = self.block(block, table, true)?;

            match &return_type {
                Some(return_type) => {
                    if return_type != &branch_tail {
                        panic!("Conditional branches return differing types")
                    }
                }
                None => return_type = Some(branch_tail),
            }

            return_type.unwrap()
        } else {
            lexer::SpanToken(
                lexer::Token::Id(String::from("unit")),
                ops::Range {
                    ..conditional.range
                },
            )
        })
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
                },
            ..
        }) = table.get_symbol(&call.id)
        {
            // Hacky solution to collect call_parameters into a vector that doesn't own it's elements so we can compare it with parameters
            if parameters != &call_parameters.iter().collect::<Vec<_>>() {
                panic!("Call provides different types for function parameters")
            }

            Ok((*return_type).clone())
        } else {
            panic!("Function doesn't exist")
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
