use crate::core::ast::Expression;
use crate::core::{error::Error, span::Span};
use camino::Utf8PathBuf;
use internment::Intern;

// The default front-end of Bell. One could use structures defined in `ast` to make another front-end.
pub mod lexer;
pub mod module;
pub mod parser;

// Perform the whole frontend on the source.
pub fn generate_ast(
    path: Intern<Utf8PathBuf>,
    text: &str,
) -> (Option<Vec<(Expression, Span)>>, Vec<Error>) {
    let (tokens, mut lex_errors) = lexer::lex(path, text);

    if let Some(tokens) = tokens {
        let (ast, errors) = parser::parse(tokens);
        lex_errors.extend(errors);

        (Some(ast), lex_errors)
    } else {
        (None, lex_errors)
    }
}
