use std::collections;

use crate::core::error;

use super::{lexer, parser};

enum Symbol {
    Function { parameters: Vec<lexer::SpanToken>, return_type: lexer::SpanToken },
    Variable { data_type: lexer::SpanToken },
}

/*
Stores the different items in each scope of the program.
The symbol table is implemented as a list of hashmaps. The last hashmap in the list is the current scope.
*/
struct Table (Vec<collections::HashMap<lexer::SpanToken, Symbol>>);

impl Table {
    fn add_symbol(&mut self, id: lexer::SpanToken, symbol: Symbol) {
        self.0.last_mut().unwrap().insert(id, symbol);
    }

    // Get a symbol in the symbol table, with some constraint (`looking_for`)
    fn get_symbol<F: Fn(&Symbol) -> bool>(&self, id: &lexer::SpanToken, looking_for: F) -> Option<&Symbol> {
        for scope in self.0.iter() {
            let result = scope.get(id);
            if result.is_some() && looking_for(result.unwrap()) {
                return result;
            }
        }

        None
    }

    fn enter(&mut self) {
        self.0.push(collections::HashMap::new());
    }

    fn exit(&mut self) {
        self.0.pop();
    }
}

struct TypeChecker<'a> {
    table: Table,
    filename: &'a str,
    text: &'a str
}

type TypeResult<'a> = Result<lexer::SpanToken, error::Error<'a>>;

/*
The type checker has two goals:
Firstly, to make sure the program is correct with the Bell type system
And secondly Infer types for variables and functions
Because of these goals, the type checker will modify the Program node it stores in order
to specify things like concrete types.

todo: Change 0..0 ranges to actual values,
todo: replace clones with references
*/
impl<'a> TypeChecker<'a> {
    fn check(&mut self, mut program: parser::Program) -> Result<parser::Program, error::Error<'a>> {
        self.table.enter();

        for function in program.0.iter_mut() {
            self.function(function)?;
        }

        self.table.exit();

        Ok(program)
    }

    fn function(&mut self, function: &mut parser::Function) -> TypeResult<'a> {
        // The parameters need to be in the same scope as the functions block
        self.table.enter();

        let mut parameters = Vec::with_capacity(function.parameters.len());

        // Load in all the parameters of the function into the symbol table
        for (id, data_type) in function.parameters.iter() {
            if self.table.get_symbol(id, |symbol| matches!(symbol, Symbol::Variable {..})).is_some() {
                panic!("Parameters must all be unique variables");
            }

            parameters.push(data_type.clone());
            self.table.add_symbol(id.clone(), Symbol::Variable { data_type: data_type.clone() });
        }

        let tail = self.block(&mut function.body, false)?;

        // Replace the tail if it's none or check if it's equal to the actual type inferred
        if &&tail != &function.return_type.get_or_insert(tail.clone()) {
            panic!("Function returns different type than specified");
        } else {
            self.table.add_symbol(function.id.clone(), Symbol::Function { parameters, return_type: function.return_type.clone().unwrap() });

            Ok(lexer::SpanToken(lexer::Token::Id(String::from("unit")), 0..0))
        }
    }

    // Currently inferring return types for block comments is simple, since you cannot early-return from a block currently.
    // In later editions, this is bound to change
    fn block(&mut self, block: &mut parser::Block, new_scope: bool) -> TypeResult<'a> {
        if new_scope {
            self.table.enter();
        }

        for expression in block.expressions.iter_mut() {
            self.expression(expression)?;
        }

        let tail = if let Some(tail) = &mut block.tail {
            self.expression(tail)?
        } else {
            lexer::SpanToken(lexer::Token::Id(String::from("unit")), 0..0)
        };

        self.table.exit();

        Ok(tail)
    }

    fn expression(&mut self, expression: &mut parser::Expression) -> TypeResult<'a> {
        match expression {
            parser::Expression::Int(_) => Ok(lexer::SpanToken(lexer::Token::Id(String::from("int")), 0..0)),
            parser::Expression::Bool(_) => Ok(lexer::SpanToken(lexer::Token::Id(String::from("bool")), 0..0)),
            parser::Expression::Id(identifier) => self.identifier(identifier),
            parser::Expression::Block(block) => self.block(block, true),
            parser::Expression::Assign(assign) => self.assign(assign),
            parser::Expression::Declaration(declaration) => self.declaration(declaration),
            parser::Expression::While(while_loop) => self.while_loop(while_loop),
            parser::Expression::Conditional(conditional) => self.conditional(conditional),
            parser::Expression::Function(function) => self.function(function),
            parser::Expression::Call(call) => self.call(call)
        }
    }

    fn identifier(&self, identifier: &lexer::SpanToken) -> TypeResult<'a> {
        if let Some(Symbol::Variable {data_type, ..}) = self.table.get_symbol(&identifier, |_| true) {
            Ok(data_type.clone().clone())
        } else {
            panic!("Use of undeclared variable");
        }
    }

    fn assign(&mut self, assign: &mut parser::Assign) -> TypeResult<'a> {
        let data_type = self.identifier(&assign.id)?;

        if data_type != self.expression(&mut assign.value)? {
            panic!("Variable is being assigned a different type than it stores");
        } else {
            Ok(lexer::SpanToken(lexer::Token::Id(String::from("unit")), 0..0))
        }
    }

    fn declaration(&mut self, declaration: &mut parser::Declaration) -> TypeResult<'a> {
        let hint = self.expression(&mut declaration.value)?;

        // Replace the data type if it's none or check if it's equal to the actual type inferred
        if &hint != declaration.hint.get_or_insert(hint.clone()) {
            panic!("Variable is being declared with different type than is hinted");
        } else {
            self.table.add_symbol(declaration.id.clone(), Symbol::Variable { data_type: declaration.hint.clone().unwrap() });

            Ok(lexer::SpanToken(lexer::Token::Id(String::from("unit")), 0..0))
        }
    }

    fn while_loop(&mut self, while_loop: &mut parser::While) -> TypeResult<'a> {
        if self.expression(&mut while_loop.condition)?.0 != lexer::Token::Id(String::from("bool")) {
            panic!("While loop condition needs to return a boolean");
        }

        self.block(&mut while_loop.body, true)
    }

    fn conditional(&mut self, conditional: &mut parser::Conditional) -> TypeResult<'a> {
        let mut return_type = None;

        for branch in &mut conditional.branches {
            if self.expression(&mut branch.condition)?.0 != lexer::Token::Id(String::from("bool")) {
                panic!("Conditional branch condition needs to return a boolean");
            }

            let tail = self.block(&mut branch.body, true)?;

            // The usual trick. You allow replacing the return_type, but only once
            if &tail != return_type.get_or_insert(tail.clone()) {
                panic!("Not all conditional branches return the same type");
            }
        }

        if let Some(tail) = &mut conditional.tail {
            let tail = self.block(tail, true)?;

            if &tail != return_type.get_or_insert(tail.clone()) {
                panic!("Not all conditional branches return the same type");
            }
        }

        Ok(return_type.unwrap())
    }

    fn call(&mut self, call: &mut parser::Call) -> TypeResult<'a> {
        let mut call_parameters = Vec::new();

        for parameter in call.parameters.iter_mut() {
            call_parameters.push(self.expression(parameter)?);
        }

        if let Some(Symbol::Function {parameters, return_type}) = self.table.get_symbol(&call.id, |_| true) {
            if parameters != &call_parameters {
                panic!("This function doesn't take parameters of these type")
            }

            Ok(return_type.clone())
        } else {
            panic!("No function exists with this name")
        }
    }
}

pub fn check<'a>(filename: &'a str, text: &'a str, program: parser::Program) -> Result<parser::Program, error::Error<'a>>  {
    TypeChecker {
        table: Table(Vec::new()),
        filename,
        text
    }.check(program)
}