use super::{compiler_backend, mir};
use crate::backend::compiler_backend::Backend;

pub enum ScoreboardOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Set,
    SetIfLesser,
    SetIfGreater,
}

impl ToString for ScoreboardOperation {
    fn to_string(&self) -> String {
        String::from(match self {
            ScoreboardOperation::Add => "+=",
            ScoreboardOperation::Subtract => "-=",
            ScoreboardOperation::Multiply => "*=",
            ScoreboardOperation::Divide => "/=",
            ScoreboardOperation::Modulo => "%=",
            ScoreboardOperation::Set => "=",
            ScoreboardOperation::SetIfLesser => "<",
            ScoreboardOperation::SetIfGreater => ">",
        })
    }
}

// todo: Add support for "matches"
#[derive(PartialEq)]
pub enum ScoreboardComparison {
    Lesser,
    Greater,
    LesserEqual,
    GreaterEqual,
    Equal,
    NotEqual,
}

impl ToString for ScoreboardComparison {
    fn to_string(&self) -> String {
        String::from(match self {
            ScoreboardComparison::Lesser => "<",
            ScoreboardComparison::Greater => ">",
            ScoreboardComparison::LesserEqual => "<=",
            ScoreboardComparison::GreaterEqual => ">=",
            ScoreboardComparison::Equal => "=",
            ScoreboardComparison::NotEqual => "=", // The compiler adds an "unless" for the execute
        })
    }
}

// All commands currently conform to 1.17 MCfunction
// todo: Add support for "scoreboard players add/remove" when the time is right and formalize the command system to include all commands
pub enum Command {
    Initialize,
    ScoreboardSet(mir::Id, i32),
    ScoreboardOperation {
        target: mir::Id,
        left: mir::Id,
        right: mir::Id,
        operation: ScoreboardOperation,
    },
    ScoreboardComparison {
        target: mir::Id,
        left: mir::Id,
        right: mir::Id,
        operation: ScoreboardComparison,
    },
    Call(mir::BlockId),
    CallIfTrue {
        block: mir::BlockId,
        check: mir::Id,
    },
    CallIfFalse {
        block: mir::BlockId,
        check: mir::Id,
    },
    Not(mir::Id, mir::Id),
    Print(mir::Id),
}

impl ToString for Command {
    fn to_string(&self) -> String {
        match self {
            Command::ScoreboardSet(set, value) => {
                format!("scoreboard players set #{} bell {}", set, value)
            }
            Command::ScoreboardOperation {
                target,
                left,
                right,
                operation,
            } => match operation {
                ScoreboardOperation::Set
                | ScoreboardOperation::SetIfGreater
                | ScoreboardOperation::SetIfLesser => format!(
                    "scoreboard players operation #{} bell {} #{} bell",
                    left,
                    operation.to_string(),
                    right
                ),
                _ => format!(
                    "scoreboard players operation #t bell = #{} bell\n\
                scoreboard players operation #t bell {} #{} bell\n\
                scoreboard players operation #{} bell = #t bell",
                    left,
                    operation.to_string(),
                    right,
                    target
                ),
            },
            Command::ScoreboardComparison {
                target,
                left,
                right,
                operation,
            } => format!(
                "execute store result score #{} bell {} score #{} bell {} #{} bell",
                target,
                if operation == &ScoreboardComparison::NotEqual {
                    "unless"
                } else {
                    "if"
                },
                left,
                operation.to_string(),
                right
            ),
            Command::Call(block_id) => format!("function project:_{}", block_id),
            Command::CallIfTrue { block, check } => format!(
                "execute if score #{} bell matches 1 run function project:_{}",
                check, block,
            ),
            Command::CallIfFalse { block, check } => format!(
                "execute if score #{} bell matches 0 run function project:_{}",
                check, block
            ),
            Command::Not(target, id) => format!(
                "execute store result score #{} bell unless score #{} bell matches 1",
                target, id
            ),
            Command::Print(id) => format!(
                "tellraw @a {{\"score\": {{\"name\": \"#{}\", \"objective\": \"bell\"}}}}",
                id
            ),
            Command::Initialize => String::from(
                "scoreboard objectives remove bell\nscoreboard objectives add bell dummy",
            ),
        }
    }
}

pub struct Function {
    pub commands: Vec<Command>,
    pub name: String,
}

impl ToString for Function {
    fn to_string(&self) -> String {
        self.commands
            .iter()
            .map(|command| {
                let mut command = command.to_string();
                command.push('\n');

                command
            })
            .collect()
    }
}

pub struct Datapack(pub Vec<Function>);

impl ToString for Datapack {
    fn to_string(&self) -> String {
        self.0
            .iter()
            .map(|function| format!("@{}.mcfunction\n{}", function.name, function.to_string()))
            .collect::<Vec<String>>()
            .join("\n")
    }
}

impl compiler_backend::Backend for Datapack {
    fn generate_code(program: mir::Program) -> (Self, Option<mir::Id>) {
        (
            {
                let mut datapack: Self = Self(
                    program
                        .blocks
                        .into_iter()
                        .enumerate()
                        .map(|(id, block)| {
                            Function {
                                commands: block
                                    .instructions
                                    .into_iter()
                                    .flat_map(|instruction| match instruction {
                                        mir::Instruction::Bind(id, value) => vec![match value {
                                            mir::BindValue::Id(other) => {
                                                Command::ScoreboardOperation {
                                                    target: id,
                                                    left: id,
                                                    right: other,
                                                    operation: ScoreboardOperation::Set,
                                                }
                                            }
                                            mir::BindValue::Int(int) => {
                                                Command::ScoreboardSet(id, int)
                                            }
                                            mir::BindValue::Add(a, b) => {
                                                Command::ScoreboardOperation {
                                                    target: id,
                                                    left: a,
                                                    right: b,
                                                    operation: ScoreboardOperation::Add,
                                                }
                                            }
                                            mir::BindValue::Subtract(a, b) => {
                                                Command::ScoreboardOperation {
                                                    target: id,
                                                    left: a,
                                                    right: b,
                                                    operation: ScoreboardOperation::Subtract,
                                                }
                                            }
                                            mir::BindValue::Multiply(a, b) => {
                                                Command::ScoreboardOperation {
                                                    target: id,
                                                    left: a,
                                                    right: b,
                                                    operation: ScoreboardOperation::Multiply,
                                                }
                                            }
                                            mir::BindValue::Divide(a, b) => {
                                                Command::ScoreboardOperation {
                                                    target: id,
                                                    left: a,
                                                    right: b,
                                                    operation: ScoreboardOperation::Divide,
                                                }
                                            }
                                            mir::BindValue::Modulo(a, b) => {
                                                Command::ScoreboardOperation {
                                                    target: id,
                                                    left: a,
                                                    right: b,
                                                    operation: ScoreboardOperation::Modulo,
                                                }
                                            }
                                            mir::BindValue::Lesser(a, b) => {
                                                Command::ScoreboardComparison {
                                                    target: id,
                                                    left: a,
                                                    right: b,
                                                    operation: ScoreboardComparison::Lesser,
                                                }
                                            }
                                            mir::BindValue::LesserEqual(a, b) => {
                                                Command::ScoreboardComparison {
                                                    target: id,
                                                    left: a,
                                                    right: b,
                                                    operation: ScoreboardComparison::LesserEqual,
                                                }
                                            }
                                            mir::BindValue::Greater(a, b) => {
                                                Command::ScoreboardComparison {
                                                    target: id,
                                                    left: a,
                                                    right: b,
                                                    operation: ScoreboardComparison::Greater,
                                                }
                                            }
                                            mir::BindValue::GreaterEqual(a, b) => {
                                                Command::ScoreboardComparison {
                                                    target: id,
                                                    left: a,
                                                    right: b,
                                                    operation: ScoreboardComparison::GreaterEqual,
                                                }
                                            }
                                            mir::BindValue::Equal(a, b) => {
                                                Command::ScoreboardComparison {
                                                    target: id,
                                                    left: a,
                                                    right: b,
                                                    operation: ScoreboardComparison::Equal,
                                                }
                                            }
                                            mir::BindValue::NotEqual(a, b) => {
                                                Command::ScoreboardComparison {
                                                    target: id,
                                                    left: a,
                                                    right: b,
                                                    operation: ScoreboardComparison::NotEqual,
                                                }
                                            }
                                            mir::BindValue::Or(a, b) => {
                                                Command::ScoreboardOperation {
                                                    target: id,
                                                    left: a,
                                                    right: b,
                                                    operation: ScoreboardOperation::SetIfGreater,
                                                }
                                            }
                                            mir::BindValue::And(a, b) => {
                                                Command::ScoreboardOperation {
                                                    target: id,
                                                    left: a,
                                                    right: b,
                                                    operation: ScoreboardOperation::SetIfLesser,
                                                }
                                            }
                                            mir::BindValue::Not(other) => Command::Not(id, other),
                                        }],
                                        mir::Instruction::Run(block_id) => {
                                            vec![Command::Call(block_id)]
                                        }
                                        mir::Instruction::Conditional {
                                            check,
                                            blocks,
                                            tail,
                                        } => {
                                            // The number of commands is always going to be the number of branches this conditional has
                                            let mut commands = Vec::with_capacity(
                                                blocks.len() + (tail.is_some() as usize),
                                            );

                                            for (check, block_id) in
                                                check.iter().zip(blocks.into_iter())
                                            {
                                                commands.push(Command::CallIfTrue {
                                                    block: block_id,
                                                    check: *check,
                                                });
                                            }

                                            if let Some(tail) = tail {
                                                commands.push(Command::CallIfFalse {
                                                    block: tail,
                                                    check: *check.last().unwrap(),
                                                });
                                            }

                                            commands
                                        }
                                        mir::Instruction::Print(id) => vec![Command::Print(id)],
                                    })
                                    .collect(),
                                name: format!("_{}", id),
                            }
                        })
                        .collect(),
                );

                // A main function to be used by people running the data pack
                if let Some(entry) = program.entry {
                    datapack.0.push(Function {
                        commands: vec![Command::Initialize, Command::Call(entry)],
                        name: String::from("main"),
                    })
                }

                datapack
            },
            program.entry,
        )
    }
}

pub fn generate_code(program: mir::Program) -> (Datapack, Option<mir::Id>) {
    Datapack::generate_code(program)
}
