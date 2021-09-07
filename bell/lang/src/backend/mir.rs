use std::collections;

use crate::{core::util, lexer, parser};

pub type Id = usize;
pub type BlockId = usize;

fn extract_int(integer: lexer::SpanToken) -> i32 {
    if let lexer::SpanToken(lexer::Token::Int(integer), ..) = integer {
        integer
    } else {
        panic!("Expected token to be an integer")
    }
}

// The MIR has only a single type the signed 32 bit integer, so extracting booleans return 0 or 1
fn extract_bool(boolean: lexer::SpanToken) -> i32 {
    if let lexer::SpanToken(lexer::Token::Bool(boolean), ..) = boolean {
        boolean.into()
    } else {
        panic!("Expected token to be a boolean")
    }
}

#[derive(Debug)]
pub enum BindValue {
    Id(Id),
    Int(i32),
    Add(Id, Id),
    Subtract(Id, Id),
    Multiply(Id, Id),
    Divide(Id, Id),
    Modulo(Id, Id),
    Lesser(Id, Id),
    LesserEqual(Id, Id),
    Greater(Id, Id),
    GreaterEqual(Id, Id),
    Equal(Id, Id),
    NotEqual(Id, Id),
    Or(Id, Id),
    And(Id, Id),
    Not(Id),
}

#[derive(Debug)]
pub enum Instruction {
    Bind(Id, BindValue),
    Run(BlockId),
    Conditional {
        check: Vec<Id>,
        blocks: Vec<BlockId>,
        tail: Option<BlockId>,
    },
    Print(Id),
}

#[derive(Debug)]
pub struct BasicBlock {
    pub instructions: Vec<Instruction>,
}

// Stores a function in the hash map. Only relevant data is stored.
// The function can be later looked up by a callee so it knows how to call the function, and what to do with the result
#[derive(Debug)]
pub struct FunctionStorage {
    pub block: BlockId,
    pub parameters: Vec<Id>,
    pub tail: Id,
}

#[derive(Debug)]
pub struct Program {
    pub blocks: Vec<BasicBlock>,
    pub(crate) entry: Option<BlockId>,
}

impl Program {
    fn add_block(&mut self, instructions: Vec<Instruction>) -> Id {
        let id = self.blocks.len();

        self.blocks.push(BasicBlock { instructions });

        id
    }
}

// All the strings in the hashmaps do is give you a way to look up a variable. If you already have it's Id, theres no need for them.
// Therefore you will see throughout this code initializations of variables with an empty name, that's the reason why
#[derive(Debug)]
struct Table {
    functions: collections::HashMap<String, FunctionStorage>,
    variables: collections::HashMap<String, Id>,
    current_id: Id,
    scope_level: usize,
}

impl Table {
    /*
    Variables can be re-declared in the same scope, in that case the variable can just be changed.
    But if the variable is declared in a different scope level than the other, it should be added separately from it

    All values are passed by value, so no references exist. Therefore you will see this function invoked with an empty string for many times in this code
    */

    fn add_variable(&mut self, name: String) -> Id {
        let current_id = self.current_id;

        self.variables
            .insert(format!("{}{}", self.scope_level, name), current_id);

        self.current_id += 1;

        current_id
    }

    // No get_function method exists, since they are trivial to find. Since the variables name is mangled, a method is required
    fn get_variable(&self, name: String) -> Id {
        // Look in every scope for the variable
        for scope in (0..=self.scope_level).into_iter().rev() {
            if let Some(id) = self.variables.get(format!("{}{}", scope, name).as_str()) {
                return *id;
            }
        }

        panic!("Variable lookup could not find variable, but it should have in this context")
    }
}

struct Transformer {
    table: Table,
    program: Program,
}

impl Transformer {
    fn transform(mut self, program: parser::Program) -> Program {
        for function in program.0 {
            self.function(function);
        }

        self.program
    }

    fn function(&mut self, function: parser::Function) {
        let parameters = function
            .parameters
            .into_iter()
            .map(|(id, _)| self.table.add_variable(util::extract_id(id).0))
            .collect();

        let (instructions, result_id) = self.block(function.body);
        let block_id = self.program.add_block(instructions);

        let name = util::extract_id(function.id).0;

        // For now this works since functions cannot be re-declared
        if name.as_str() == "main" {
            self.program.entry = Some(block_id);
        }

        self.table.functions.insert(
            name,
            FunctionStorage {
                // Create a basic block for the function to use
                block: block_id,
                parameters,
                tail: result_id,
            },
        );
    }

    fn block(&mut self, block: parser::Block) -> (Vec<Instruction>, Id) {
        self.table.scope_level += 1;
        let mut instructions = Vec::new();

        for expression in block.expressions {
            instructions.append(&mut self.expression(expression).0);
        }

        let result_id = if let Some(tail) = block.tail {
            let mut tail = self.expression(tail);
            instructions.append(&mut tail.0);

            tail.1
        } else {
            self.table.add_variable(String::from(""))
        };

        self.table.scope_level -= 1;

        (instructions, result_id)
    }

    fn expression(&mut self, expression: parser::Expression) -> (Vec<Instruction>, Id) {
        match expression {
            parser::Expression::Call(call) => self.call(call),
            parser::Expression::Block(block) => self.block(*block),
            parser::Expression::Conditional(conditional) => self.conditional(*conditional),
            parser::Expression::While(while_loop) => self.while_loop(*while_loop),
            _ => {
                let temporary = self.table.add_variable(String::from(""));

                match expression {
                    parser::Expression::Function(function) => {
                        self.function(*function);

                        (Vec::new(), temporary)
                    }
                    parser::Expression::Declaration(declaration) => {
                        let (mut instructions, expression_id) = self.expression(declaration.value);
                        let variable_id =
                            self.table.add_variable(util::extract_id(declaration.id).0);

                        instructions
                            .push(Instruction::Bind(variable_id, BindValue::Id(expression_id)));

                        (instructions, temporary)
                    }
                    parser::Expression::Assign(assign) => {
                        let (mut instructions, expression_id) = self.expression(assign.value);
                        let variable_id = self.table.get_variable(util::extract_id(assign.id).0);

                        instructions
                            .push(Instruction::Bind(variable_id, BindValue::Id(expression_id)));

                        (instructions, temporary)
                    }
                    parser::Expression::Int(integer) => (
                        vec![Instruction::Bind(
                            temporary,
                            BindValue::Int(extract_int(integer)),
                        )],
                        temporary,
                    ),
                    parser::Expression::Bool(boolean) => (
                        vec![Instruction::Bind(
                            temporary,
                            BindValue::Int(extract_bool(boolean)),
                        )],
                        temporary,
                    ),
                    parser::Expression::Id(identifier) => (
                        vec![Instruction::Bind(
                            temporary,
                            BindValue::Id(self.table.get_variable(util::extract_id(identifier).0)),
                        )],
                        temporary,
                    ),
                    _ => unreachable!(),
                }
            }
        }
    }

    fn conditional(&mut self, conditional: parser::Conditional) -> (Vec<Instruction>, Id) {
        let mut instructions = Vec::new();

        let return_storage = self.table.add_variable(String::from(""));

        let mut ids = Vec::with_capacity(conditional.branches.len());
        let mut blocks = Vec::with_capacity(conditional.branches.len() + 1);

        for branch in conditional.branches {
            let (mut branch_instructions, condition_id, block_id) =
                self.branch(branch, return_storage);

            instructions.append(&mut branch_instructions);

            ids.push(condition_id);
            blocks.push(block_id);
        }

        instructions.push(Instruction::Conditional {
            check: ids,
            blocks,
            tail: if let Some(tail) = conditional.tail {
                let mut block = self.block(tail);

                block
                    .0
                    .push(Instruction::Bind(return_storage, BindValue::Id(block.1)));

                Some(self.program.add_block(block.0))
            } else {
                None
            },
        });

        (instructions, return_storage)
    }

    // The while loop is made of two blocks. The first just checks if the while condition is satisfied,
    // and calls the second block which is the body of the while loop. In the end of the body, a call to the first block is made
    fn while_loop(&mut self, while_loop: parser::While) -> (Vec<Instruction>, Id) {
        // While loops don't anything
        let (body, _) = self.block(while_loop.body);
        let body_id = self.program.add_block(body);

        let mut condition_id = self.expression(while_loop.condition);

        condition_id.0.push(Instruction::Conditional {
            check: vec![condition_id.1],
            blocks: vec![body_id],
            tail: None,
        });

        let condition_id = self.program.add_block(condition_id.0);
        self.program.blocks[body_id]
            .instructions
            .push(Instruction::Run(condition_id));

        (
            vec![Instruction::Run(condition_id)],
            self.table.add_variable(String::from("")),
        )
    }

    #[inline]
    fn branch(
        &mut self,
        branch: parser::Branch,
        return_into: Id,
    ) -> (Vec<Instruction>, Id, BlockId) {
        let mut instructions = Vec::new();

        let mut condition = self.expression(branch.condition);
        let mut block = self.block(branch.body);

        instructions.append(&mut condition.0);

        block
            .0
            .push(Instruction::Bind(return_into, BindValue::Id(block.1)));

        (instructions, condition.1, self.program.add_block(block.0))
    }

    fn call(&mut self, call: parser::Call) -> (Vec<Instruction>, Id) {
        let name = util::extract_id(call.id).0;

        let mut parameters = Vec::with_capacity(call.parameters.len());
        let mut instructions = call
            .parameters
            .into_iter()
            .flat_map(|parameter| {
                let parameter = self.expression(parameter);
                parameters.push(parameter.1); // Functions expect concrete values, not possible values, so let's do that

                parameter.0
            })
            .collect::<Vec<Instruction>>();

        // For now functions cannot be shadowed, so this implementation is fine
        let result = match name.as_str() {
            "add" => {
                let result = self.table.add_variable(String::from(""));
                instructions.push(Instruction::Bind(
                    result,
                    BindValue::Add(parameters[0], parameters[1]),
                ));

                result
            }
            "subtract" => {
                let result = self.table.add_variable(String::from(""));
                instructions.push(Instruction::Bind(
                    result,
                    BindValue::Subtract(parameters[0], parameters[1]),
                ));

                result
            }
            "multiply" => {
                let result = self.table.add_variable(String::from(""));
                instructions.push(Instruction::Bind(
                    result,
                    BindValue::Multiply(parameters[0], parameters[1]),
                ));

                result
            }
            "divide" => {
                let result = self.table.add_variable(String::from(""));
                instructions.push(Instruction::Bind(
                    result,
                    BindValue::Divide(parameters[0], parameters[1]),
                ));

                result
            }
            "modulo" => {
                let result = self.table.add_variable(String::from(""));
                instructions.push(Instruction::Bind(
                    result,
                    BindValue::Modulo(parameters[0], parameters[1]),
                ));

                result
            }
            "lesser" => {
                let result = self.table.add_variable(String::from(""));
                instructions.push(Instruction::Bind(
                    result,
                    BindValue::Lesser(parameters[0], parameters[1]),
                ));

                result
            }
            "greater" => {
                let result = self.table.add_variable(String::from(""));
                instructions.push(Instruction::Bind(
                    result,
                    BindValue::Greater(parameters[0], parameters[1]),
                ));

                result
            }
            "lesser_equal" => {
                let result = self.table.add_variable(String::from(""));
                instructions.push(Instruction::Bind(
                    result,
                    BindValue::LesserEqual(parameters[0], parameters[1]),
                ));

                result
            }
            "greater_equal" => {
                let result = self.table.add_variable(String::from(""));
                instructions.push(Instruction::Bind(
                    result,
                    BindValue::GreaterEqual(parameters[0], parameters[1]),
                ));

                result
            }
            "equal" => {
                let result = self.table.add_variable(String::from(""));
                instructions.push(Instruction::Bind(
                    result,
                    BindValue::Equal(parameters[0], parameters[1]),
                ));

                result
            }
            "not_equal" => {
                let result = self.table.add_variable(String::from(""));
                instructions.push(Instruction::Bind(
                    result,
                    BindValue::NotEqual(parameters[0], parameters[1]),
                ));

                result
            }
            "and" => {
                let result = self.table.add_variable(String::from(""));
                instructions.push(Instruction::Bind(
                    result,
                    BindValue::And(parameters[0], parameters[1]),
                ));

                result
            }
            "or" => {
                let result = self.table.add_variable(String::from(""));
                instructions.push(Instruction::Bind(
                    result,
                    BindValue::Or(parameters[0], parameters[1]),
                ));

                result
            }
            "not" => {
                let result = self.table.add_variable(String::from(""));
                instructions.push(Instruction::Bind(result, BindValue::Not(parameters[0])));

                result
            }
            "negate" => {
                let result = self.table.add_variable(String::from(""));

                instructions.push(Instruction::Bind(result, BindValue::Int(-1)));
                instructions.push(Instruction::Bind(
                    result,
                    BindValue::Multiply(parameters[0], result),
                ));

                result
            }
            "println" => {
                instructions.push(Instruction::Print(parameters[0]));

                self.table.add_variable(String::from(""))
            }
            name => {
                let function_data = self.table.functions.get(name).unwrap();

                for (call_parameter, parameter) in
                    parameters.into_iter().zip(function_data.parameters.iter())
                {
                    instructions.push(Instruction::Bind(*parameter, BindValue::Id(call_parameter)));
                }

                instructions.push(Instruction::Run(function_data.block));

                function_data.tail
            }
        };

        (instructions, result)
    }
}

pub fn lower(program: parser::Program) -> Program {
    Transformer {
        table: Table {
            functions: collections::HashMap::new(),
            variables: collections::HashMap::new(),
            current_id: 0,
            scope_level: 0,
        },
        program: Program {
            blocks: Vec::new(),
            entry: None,
        },
    }
    .transform(program)
}
