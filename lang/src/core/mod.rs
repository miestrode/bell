use internment::Intern;

pub mod ast;
pub mod error;
pub mod file;
pub mod span;
pub mod token;
pub mod types;

pub type Name = Intern<String>;
