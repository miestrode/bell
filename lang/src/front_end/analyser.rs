use super::parser;

pub struct Analyser<'a> {
    pub tree: parser::Program,
    pub filename: &'a str,
    pub text: &'a str,
}