use frontend::{lexer, mir, parser, type_checker};

use crate::core::error;

pub mod core;
mod frontend;

// The result of each compilation level
#[derive(Debug)]
pub enum CompileResult {
    LexResult(Vec<lexer::SpanToken>),
    ParseResult(parser::Program),
    CheckResult(parser::Program),
    MIRResult(mir::Program),
}

// Compile a file up to some step
pub fn compile<'a>(
    filename: &'a str,
    text: &'a str,
    level: i32,
) -> Result<CompileResult, error::Error<'a>> {
    let tokens = lexer::tokenize(filename, text)?;

    // Fall-through sadly isn't possible without some extra work
    if level > 1 {
        let program = parser::parse(filename, text, tokens)?;

        if level > 2 {
            let typed_program = type_checker::check(filename, text, program)?;

            if level > 3 {
                let mir_program = mir::lower(typed_program);

                Ok(CompileResult::MIRResult(mir_program))
            } else {
                Ok(CompileResult::CheckResult(typed_program))
            }
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
        crate::compile_text(
            r"
            fn factorial(number: int) -> int {
                let product = number;

                while number > 1 {
                    product = product * number;

                    number = number - 1;
                }

                product
            }
            ",
            1,
        )
        .unwrap();
    }

    #[test]
    fn comments_no_panic() {
        crate::compile_text(
            r"
            // Single line comment
            
            /* Block comment */
            
            /*
            Multiple line
            block comment
            */
            
            /*
            Nested comments!
            
            /*
            I am nested
            */
            
            more text
            */
            ",
            1,
        )
        .unwrap();
    }

    #[test]
    fn invalid_character() {
        assert!(matches!(
            crate::compile_text("@miestrode", 1).unwrap_err().error,
            error::Data::InvalidCharacter { .. }
        ));
    }

    #[test]
    fn unterminated_comment() {
        assert!(matches!(
            crate::compile_text("/* I am unterminated!", 1)
                .unwrap_err()
                .error,
            error::Data::UnterminatedBlockComment { .. }
        ));
    }

    #[test]
    fn parse_no_panic() {
        crate::compile_text(
            r"
        fn is_prime(num: int) -> bool {
            let check = 2;
            let is_prime = false;

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
        ",
            2,
        );
    }

    #[test]
    fn check_no_panic() {
        crate::compile_text(
            r"
        fn main() {
            let x = 2;
        }
        ",
            3,
        );
    }

    #[test]
    fn undeclared_variable() {
        assert!(matches!(
            crate::compile_text(
                r"
        fn main() {
            x = 0;
        }
        "
            ),
            error::Error {
                error: error::ErrorKind::UndeclaredSymbol { .. },
                ..
            }
        ))
    }

    #[test]
    fn type_mismatch() {
        assert!(matches!(
            crate::compile_text(
                r"
        fn main() {
            if true {
                1
            } else {
                true
            }
        }
        ",
                3
            ),
            error::Error {
                error: error::ErrorKind::UndeclaredSymbol { .. },
                ..
            }
        ));
    }

    #[test]
    fn lower_no_panic() {
        crate::compile_text(
            r"
            fn factorial(number: int) -> int {
                let product = number;

                while number > 1 {
                    product = product * number;

                    number = number - 1;
                }

                product
            }
            ",
            4,
        )
        .unwrap();
    }
}
