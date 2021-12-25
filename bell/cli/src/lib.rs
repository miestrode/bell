use ariadne::{Cache, Color, Label, Report, ReportKind, Source};

use clap::{App, Arg};

use lang::core::span::SourceMap;
use lang::OptimizationLevel;
use lang::{
    core::error::{Error, Pattern, Reason},
    CompileErrors,
};

use internment::Intern;

use camino::Utf8PathBuf;

use std::collections::HashMap;
use std::fmt::Debug;
use std::fmt::Display as FmtDisplay;
use std::io::Error as IOError;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::Instant;

fn display_as_choice(list: &[Pattern]) -> String {
    format!(
        "{}{}",
        list[..list.len() - 1]
            .iter()
            .map(|pattern| pattern.to_string())
            .collect::<Vec<String>>()
            .join(", "),
        if list.len() > 1 {
            format!(", or {}", list.last().unwrap())
        } else {
            list.first().unwrap().to_string()
        }
    )
}

fn display_basic_error(message: String) {
    eprintln!("{} {}", Color::Red.paint("Error:"), message);
}

// This exists as a bypass to the orphan rule.
struct ErrorSources(HashMap<Intern<Utf8PathBuf>, Source>);

impl ErrorSources {
    fn from(sources: SourceMap) -> Self {
        Self(
            sources
                .0
                .into_iter()
                // A space is currently added to each source since ariadne doesn't handle empty files.
                .map(|(path, contents)| {
                    (
                        path,
                        Source::from(if contents.is_empty() { " " } else { &contents }),
                    )
                })
                .collect(),
        )
    }
}

impl Cache<Intern<Utf8PathBuf>> for ErrorSources {
    fn fetch(&mut self, id: &Intern<Utf8PathBuf>) -> Result<&Source, Box<dyn Debug + '_>> {
        Ok(self.0.get(id).unwrap())
    }

    fn display<'a>(&self, id: &'a Intern<Utf8PathBuf>) -> Option<Box<dyn FmtDisplay + 'a>> {
        Some(Box::new(id))
    }
}

fn generate_cause(error: IOError) -> String {
    // I might need to add more variants in the future, we will see.
    format!(
        "{} (error no. {})",
        match error.kind() {
            ErrorKind::NotFound => "it was not found",
            ErrorKind::PermissionDenied => "permission was denied",
            ErrorKind::Interrupted => "the operation was interrupted",
            _ => unreachable!(),
        },
        error
            .raw_os_error()
            .map_or(String::from("?"), |os_error| os_error.to_string())
    )
}

trait Display {
    fn display(self, cache: &mut ErrorSources);
}

impl Display for Error {
    fn display(self, cache: &mut ErrorSources) {
        match self {
            Error::Basic(context) => display_basic_error(format!("{}.", context)),
            Error::IO {
                error,
                action: reason,
            } => display_basic_error(format!(
                "failed to {} because {}.",
                Color::Green.paint(reason),
                Color::Green.paint(generate_cause(error))
            )),
            Error::ConflictingModules { first, second } => display_basic_error(format!(
                "The modules {} and {} have the same leaf name.",
                Color::Green.paint(first),
                Color::Green.paint(second)
            )),
            Error::UnterminatedBlockComment { span } => {
                Report::build(ReportKind::Error, span.path, span.range.start)
                    .with_message("unterminated block comment.")
                    .with_label(
                        Label::new((span.path, span.range))
                            .with_message("A termination is needed somewhere in this range.")
                            .with_color(Color::Red),
                    )
                    .with_note(format!(
                        "A block comment termination looks like {}.",
                        Color::Green.paint("*/")
                    ))
                    .finish()
                    .eprint(cache)
                    .unwrap()
            }
            Error::UnterminatedString { span } => {
                Report::build(ReportKind::Error, span.path, span.range.start)
                    .with_message("unterminated string.")
                    .with_label(
                        Label::new((span.path, span.range))
                            .with_message("A termination is needed somewhere in this range.")
                            .with_color(Color::Red),
                    )
                    .with_note(format!(
                        "A string termination looks like {}.",
                        Color::Green.paint('"')
                    ))
                    .finish()
                    .eprint(cache)
                    .unwrap()
            }
            Error::Unexpected {
                expected,
                found,
                reason,
                while_parsing,
            } => {
                let message = format!(
                    "expected {} but found {}",
                    Color::Green.paint(display_as_choice(
                        &(expected.into_iter().collect::<Vec<_>>())
                    )),
                    Color::Green.paint(found.data)
                );

                let mut initial =
                    Report::build(ReportKind::Error, found.span.path, found.span.range.start)
                        .with_message(if let Some(while_parsing) = while_parsing {
                            format!(
                                "{} while parsing {}.",
                                message,
                                Color::Green.paint(while_parsing)
                            )
                        } else {
                            format!("{}.", message)
                        })
                        .with_label(
                            Label::new((found.span.path, found.span.range))
                                .with_message("Here.")
                                .with_color(Color::Red),
                        );

                if let Reason::UnclosedDelimiter(delimiter) = reason {
                    initial = initial.with_label(
                        Label::new((delimiter.span.path, delimiter.span.range))
                            .with_message("Because of this delimiter.")
                            .with_color(Color::Blue),
                    )
                }

                initial.finish().eprint(cache).unwrap()
            }
            Error::MissingId { id } => {
                Report::build(ReportKind::Error, id.span.path, id.span.range.start)
                    .with_message(format!(
                        "Cannot find the identifier {} in scope.",
                        Color::Green.paint(id.data)
                    ))
                    .with_label(
                        Label::new((id.span.path, id.span.range))
                            .with_message("Here.")
                            .with_color(Color::Red),
                    )
                    .finish()
                    .eprint(cache)
                    .unwrap()
            }
            Error::Conflicting { first, second, id } => {
                Report::build(ReportKind::Error, first.path, second.range.start)
                    .with_message(format!(
                        "Identifiers with the name {} are conflicting.",
                        Color::Green.paint(id)
                    ))
                    .with_label(
                        Label::new((first.path, first.range))
                            .with_message("The first identifier is declared here.")
                            .with_color(Color::Blue),
                    )
                    .with_label(
                        Label::new((second.path, second.range))
                            .with_message(
                                "The second identifier is declared here, in the same file scope.",
                            )
                            .with_color(Color::Red),
                    )
                    .with_note(
                        "To avoid ambiguities, Identifiers must have a unique name while in the \
                         same file scope.",
                    )
                    .finish()
                    .eprint(cache)
                    .unwrap()
            }
            Error::TypeMismatch { a, b } => {
                Report::build(ReportKind::Error, b.span.path, b.span.range.start)
                    .with_message(format!(
                        "Type mismatch between {:?} and {:?}.",
                        Color::Green.paint(&a.data),
                        Color::Green.paint(&b.data)
                    ))
                    .with_label(
                        Label::new((a.span.path, a.span.range))
                            .with_message(format!(
                                "This has type {:?}.",
                                Color::Green.paint(&a.data)
                            ))
                            .with_color(Color::Blue),
                    )
                    .with_label(
                        Label::new((b.span.path, b.span.range))
                            .with_message(format!(
                                "But this has type {:?}.",
                                Color::Green.paint(&b.data)
                            ))
                            .with_color(Color::Red),
                    )
                    .with_note("These types should be equal.")
                    .finish()
                    .eprint(cache)
                    .unwrap()
            }
            Error::InvalidAssign(location) => Report::build(
                ReportKind::Error,
                location.span.path,
                location.span.range.start,
            )
            .with_message(format!("Cannot assign to a {}.", location.data))
            .with_label(
                Label::new((location.span.path, location.span.range))
                    .with_message("Here.")
                    .with_color(Color::Red),
            )
            .with_note("Only identifiers and fields can be assigned to.")
            .finish()
            .eprint(cache)
            .unwrap(),
        }
    }
}

pub enum RunResult {
    Success,
    Failure,
}

struct Config {
    #[allow(unused)]
    export_to: Option<String>,
    optimizations: OptimizationLevel,
    path: String,
}

fn get_config() -> Config {
    let matches = App::new("The Bell CLI")
        .author("Yoav Grimland, miestrode@gmail.com")
        .version("0.5.0")
        .about("Compile a Bell project/file.")
        .arg(
            Arg::new("path")
                .short('p')
                .long("path")
                .value_name("FILE/FOLDER")
                .about("Is used to specify the path of the project/file to compile.")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("release")
                .short('r')
                .long("release")
                .about("Enables more complicated code optimizations.")
                .takes_value(false),
        )
        .arg(
            Arg::new("export")
                .short('e')
                .long("export")
                .value_name("FOLDER")
                .about(
                    "Let's you specify where to put your compiled code in the specified folder.\n \
                     When unspecified, it will print the data pack out.",
                )
                .required(false)
                .takes_value(true),
        )
        .get_matches();

    Config {
        optimizations: if matches.is_present("release") {
            OptimizationLevel::Release
        } else {
            OptimizationLevel::Debug
        },
        export_to: matches.value_of("export").map(|path| path.to_owned()),
        path: matches.value_of("path").unwrap().to_owned(),
    }
}

pub fn run() -> RunResult {
    let config = get_config();
    println!(
        "{} `{}`\n",
        Color::Green.paint("Compiling").bold(),
        config.path
    );

    let path = PathBuf::from(&config.path);
    let time = Instant::now();

    match lang::compile(path, config.optimizations) {
        Ok(expression) => {
            let elapsed = time.elapsed().as_secs_f32();

            println!("{:#?}", expression);
            println!(
                "\n{} ({} build) in {:.5}s",
                Color::Green.paint("Finished").bold(),
                Color::Blue.paint(match config.optimizations {
                    OptimizationLevel::Debug => "debug",
                    OptimizationLevel::Release => "release",
                }),
                elapsed
            );

            RunResult::Success
        }
        Err(CompileErrors { errors, sources }) => {
            let mut cache = ErrorSources::from(sources);

            for error in errors {
                error.display(&mut cache);
                println!();
            }

            println!(
                "{} compilation due to the errors above.",
                Color::Red.paint("Halted").bold()
            );

            RunResult::Failure
        }
    }
}
