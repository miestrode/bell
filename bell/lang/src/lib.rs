use front_end::{lexer, parser, type_checker};

use crate::core::error;

pub mod core;
mod front_end;

// The result of each compilation level
#[derive(Debug)]
pub enum CompileResult {
    LexResult(Vec<lexer::SpanToken>),
    ParseResult(parser::Program),
    CheckResult(parser::Program),
}

// Compile a file up to some step
pub fn compile<'a>(filename: &'a str, text: &'a str, level: i32) -> Result<CompileResult, error::Error<'a>> {
    let tokens = lexer::tokenize(filename, text)?;

    // Fall-through sadly isn't possible without some extra work
    if level > 1 {
        let program = parser::parse(filename, text, tokens)?;

        if level > 2 {
            Ok(CompileResult::CheckResult(type_checker::check(filename, text, program)?))
        } else {
            Ok(CompileResult::ParseResult(program))
        }
    } else {
        Ok(CompileResult::LexResult(tokens))
    }
}

pub fn compile_text(text: &str, level: i32) -> Result<CompileResult, error::Error> {
    compile("<unknown>", text, level)
}

#[cfg(test)]
mod tests {
    use crate::core::error;

    #[test]
    fn tokenize_no_panic() {
        crate::compile_text(&String::from(r"
            fn factorial(number: int) -> int {
                var product = number;

                while number > 1 {
                    product = product * number;

                    number = number - 1;
                }

                product
            }
            "), 1).unwrap();
    }

    #[test]
    fn comments_no_panic() {
        crate::compile_text(&String::from(r"
            // Single line comment
            /* block comment */
            /*
            multiple line
            block comment
            */
            "), 1).unwrap();
    }

    #[test]
    fn invalid_character() {
        assert!(matches!(crate::compile_text(&String::from("@miestrode"), 1).unwrap_err().error, error::Data::InvalidCharacter {..}));
    }

    #[test]
    fn unterminated_comment() {
        assert!(matches!(crate::compile_text(&String::from("/* I am unterminated!"), 1).unwrap_err().error, error::Data::UnterminatedBlockComment {..}));
    }

    #[test]
    fn parse_no_panic() {
        crate::compile_text(r"
        fn is_prime(num: int) -> bool {
            var check = 2;
            var is_prime = false;

            // Cannot do early return yet so this is less efficient
            while check < num {
                if num % check == 0 {
                    is_prime = true;
                }
            }

        is_prime
        }

        fn main() {
            println(is_prime(100));
        }
        ", 2);
    }

    #[test]
    fn check_no_panic() {
        crate::compile_text(r"
        fn main() {
            var x = 2;
        }
        ", 2);
    }
}