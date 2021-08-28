use std::{fs, io, ops, process, time};

use lang::core::error;

const VERSION: &str = "0.3.1";
const LEVELS: ops::RangeInclusive<i32> = 1..=3;

// A simple trait, used only for displaying errors
trait Report {
    fn display(self) -> io::Result<()>;
}

// Display a list of strings in the form "x, y, or z". Assumes the list has at least 1 element
fn display_as_choice(list: &[&str]) -> String {
    format!(
        "{}{}",
        list[..(list.len() - 1)].join(", "),
        if list.len() > 1 {
            format!(", or {}", list.last().unwrap())
        } else {
            String::from(list[0])
        }
    )
}

// Displaying errors was moved to the CLI to reduce bloat for the core language crate
impl Report for error::Error<'_> {
    fn display(self) -> io::Result<()> {
        match self.error {
            error::ErrorKind::InvalidCharacter { range, character } => {
                ariadne::Report::build(ariadne::ReportKind::Error, self.filename, range.start)
                    .with_message(format!(
                        "Character {} is invalid.",
                        format!("`{}`", character)
                    ))
                    .with_code(1)
                    .with_label(
                        ariadne::Label::new((self.filename, range))
                            .with_message("Here.")
                            .with_color(ariadne::Color::Red),
                    )
                    .finish()
                    .eprint((self.filename, ariadne::Source::from(self.text)))
            }
            error::ErrorKind::UnterminatedBlockComment { range } => {
                ariadne::Report::build(ariadne::ReportKind::Error, self.filename, range.start)
                    .with_message("unterminated block comment.")
                    .with_code(2)
                    .with_label(
                        ariadne::Label::new((self.filename, range))
                            .with_message(format!(
                                "Expected a {}, somewhere in this range.",
                                "`*/`"
                            ))
                            .with_color(ariadne::Color::Red),
                    )
                    .finish()
                    .eprint((self.filename, ariadne::Source::from(self.text)))
            }
            error::ErrorKind::Expected {
                range,
                expected,
                found,
            } => ariadne::Report::build(ariadne::ReportKind::Error, self.filename, range.start)
                .with_message(format!(
                    "expected {}, found {}.",
                    display_as_choice(expected),
                    found
                ))
                .with_code(3)
                .with_label(
                    ariadne::Label::new((self.filename, range))
                        .with_message("Here.")
                        .with_color(ariadne::Color::Red),
                )
                .finish()
                .eprint((self.filename, ariadne::Source::from(self.text))),
            error::ErrorKind::DuplicateParameter {
                existing,
                found,
                name,
            } => ariadne::Report::build(ariadne::ReportKind::Error, self.filename, found.start)
                .with_message(format!("Duplicate parameter {}.", format!("`{}`", name)))
                .with_code(3)
                .with_label(
                    ariadne::Label::new((self.filename, existing))
                        .with_message(format!(
                            "You first declared {} here.",
                            format!("`{}`", name)
                        ))
                        .with_color(ariadne::Color::Blue),
                )
                .with_label(
                    ariadne::Label::new((self.filename, found))
                        .with_message("But you declared it again here.")
                        .with_color(ariadne::Color::Red),
                )
                .finish()
                .eprint((self.filename, ariadne::Source::from(self.text))),
            error::ErrorKind::DataTypeMismatch {
                expected,
                found,
                because,
                location,
            } => {
                let report = ariadne::Report::build(
                    ariadne::ReportKind::Error,
                    self.filename,
                    location.start,
                )
                .with_message(format!(
                    "Expected data of type {}, found {}.",
                    expected, found
                ))
                .with_code(4);
                if let Some(because) = because {
                    report.with_label(
                        ariadne::Label::new((self.filename, because))
                            .with_message(format!(
                                "Expected data of type {} because of this.",
                                expected
                            ))
                            .with_color(ariadne::Color::Blue),
                    )
                } else {
                    report
                }
                .with_label(
                    ariadne::Label::new((self.filename, location))
                        .with_message(format!("Got data of type {} because of this.", found))
                        .with_color(ariadne::Color::Red),
                )
                .finish()
                .eprint((self.filename, ariadne::Source::from(self.text)))
            }
            error::ErrorKind::UndeclaredSymbol { name, usage } => {
                ariadne::Report::build(ariadne::ReportKind::Error, self.filename, usage.start)
                    .with_message(format!("{} is used before it's declared.", name))
                    .with_code(5)
                    .with_label(
                        ariadne::Label::new((self.filename, usage))
                            .with_message(format!(
                                "{} is used here, but it isn't defined in any scope!",
                                name
                            ))
                            .with_color(ariadne::Color::Red),
                    )
                    .finish()
                    .eprint((self.filename, ariadne::Source::from(self.text)))
            }
        }
    }
}

fn main() {
    let matches = clap::App::new("The Flagship Bell Compiler CLI")
        .version(VERSION)
        .author("Yoav G. <miestrode@gmail.com>")
        .about("Compiles Bell files into data packs (written in MCfunction).")
        .arg(clap::Arg::with_name("file")
            .visible_aliases(&["source", "filename", "compile", "build"])
            .short("f")
            .long("file")
            .value_name("FILE")
            .takes_value(true)
            .required(true)
            .help("The file to compile to create a data pack from."))
        .arg(clap::Arg::with_name("level")
            .visible_aliases(&["stage", "up_to"])
            .short("l")
            .long("level")
            .value_name("INT")
            .takes_value(true)
            .required(false)
            .default_value("3")
            .help("The level of compilation. Compilation levels in ascending order are: lexing, parsing and type checking."))
        .get_matches();

    // Unwrap can be used, since Clap makes sure all the values are specified
    let file = matches.value_of("file").unwrap();

    // Get the compilation level. Mainly used for debugging, since sometimes you want to test out a particular part of the compiler.
    // Previous compilation steps however are mandatory. You cannot parse without tokenizing first
    let level = matches
        .value_of("level")
        .unwrap()
        .parse::<i32>()
        .unwrap_or_else(|_| {
            eprintln!(
                "{} Malformed integer for compilation level.",
                ariadne::Color::Red.paint("Error:")
            );

            process::exit(exitcode::DATAERR);
        });

    if !(LEVELS).contains(&level) {
        eprintln!(
            "{} Compilation level needs to be between {} to {}.",
            ariadne::Color::Red.paint("Error:"),
            ariadne::Color::Blue.paint(LEVELS.start()),
            ariadne::Color::Blue.paint(LEVELS.end())
        );

        process::exit(exitcode::DATAERR);
    }

    // Fetch the file the user wants to compile
    let text = fs::read_to_string(&file).unwrap_or_else(|_| {
        eprintln!(
            "{} Could not locate file: `{}`.",
            ariadne::Color::Red.paint("Error:"),
            ariadne::Color::Blue.paint(format!("`{}`", file))
        );

        process::exit(exitcode::DATAERR);
    });

    let now = time::Instant::now();
    let result = lang::compile(file, &text, level);

    println!(
        "{} {} {} in {:.2}s.\n",
        ariadne::Color::Green.paint("Finished"),
        match level {
            1 => "lexing",
            2 => "parsing",
            3 => "analysing",
            _ => panic!("Level is invalid, but should have already been checked"), // Internal error
        },
        ariadne::Color::Blue.paint(&file),
        now.elapsed().as_secs_f32()
    );

    process::exit(match result {
        Ok(value @ lang::CompileResult::LexResult(_)) => {
            println!("{:#?}", value);

            exitcode::OK
        }
        Ok(value @ lang::CompileResult::ParseResult(_)) => {
            println!("{:#?}", value);

            exitcode::OK
        }
        Ok(value @ lang::CompileResult::CheckResult(_)) => {
            println!("{:#?}", value);

            exitcode::OK
        }
        Err(error) => {
            error.display().unwrap();

            exitcode::OSFILE
        }
    })
}
