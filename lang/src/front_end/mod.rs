use crate::core::ast::Expression;
use crate::core::{error::Errors, span::Span};
use camino::Utf8PathBuf;
use internment::Intern;

// The default front-end of Bell. One could use structures defined in `ast` to make another front-end.
pub mod lex;
pub mod module;
pub mod parse;

// Perform the whole frontend on the source.
pub fn generate_ast(
    path: Intern<Utf8PathBuf>,
    text: &str,
    errors: &mut Errors,
) -> Vec<(Expression, Span)> {
    parse::parse(lex::lex(path, text, errors), errors)
}
