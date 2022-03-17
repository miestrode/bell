use ariadne::{Cache, Color, Config as ErrorConfig, Label, Report, ReportKind, Source};

use clap::{App, Arg};

use lang::core::{error::Errors, span::SourceMap};
use lang::OptLevel;
use lang::{
    core::error::{Error, Pattern, Reason},
    core::types,
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
    fn display(self, cache: &mut ErrorSources, compact: bool);
}

impl Display for Error {
    fn display(self, cache: &mut ErrorSources, compact: bool) {
        match self {
            Error::Basic(context) => display_basic_error(format!("{}.", context)),
            Error::IO { error, action } => display_basic_error(format!(
                "failed to {} because {}.",
                action,
                generate_cause(error)
            )),
            Error::ConflictingModuleNames { parent, name } => display_basic_error(format!(
                "The child module {} exists more than once in the parent module {}.",
                Color::Green.paint(parent),
                Color::Green.paint(name)
            )),
            _ => match self {
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
                        Color::Green.paint(found.value)
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

                    initial
                }
                Error::MissingId { id } => {
                    Report::build(ReportKind::Error, id.span.path, id.span.range.start)
                        .with_message(format!(
                            "Cannot find {} in scope.",
                            Color::Green.paint(id.value)
                        ))
                        .with_label(
                            Label::new((id.span.path, id.span.range))
                                .with_message("Here.")
                                .with_color(Color::Red),
                        )
                }
                Error::ConflictingIds { first, second, id } => {
                    Report::build(ReportKind::Error, first.path, second.range.start)
                        .with_message(format!(
                            "the name {} is conflicted between two identifiers.",
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
                                    "The second identifier is declared here, in the same scope.",
                                )
                                .with_color(Color::Red),
                        )
                        .with_note(
                            "To avoid ambiguities, non-variable identifiers must be unique in \
                             their scope.",
                        )
                }
                Error::TypeMismatch {
                    a: mut a_trace,
                    b: mut b_trace,
                    reason,
                } => {
                    let a = a_trace.0.remove(0).data_type;
                    let b = b_trace.0.remove(0).data_type;

                    assert!(!matches!((&a.span, &b.span), (None, None)));

                    let start = b.span.as_ref().map_or_else(
                        || a.span.as_ref().unwrap().range.start,
                        |span| span.range.start,
                    );

                    let path = b
                        .span
                        .as_ref()
                        .map_or_else(|| a.span.as_ref().unwrap().path, |span| span.path);

                    let mut report =
                        Report::build(ReportKind::Error, path, start).with_message(format!(
                            "Type mismatch between {} and {}.",
                            Color::Cyan.paint(&a.value),
                            Color::Magenta.paint(&b.value)
                        ));

                    match reason {
                        types::LinkReason::Other => (),
                        _ => {
                            report = report.with_note(match reason {
                                types::LinkReason::Assign => {
                                    "Assignments cannot change the type of a memory location. You \
                                     must shadow it in order to do that."
                                }
                                types::LinkReason::Condition => {
                                    "This condition needs to return a boolean."
                                }
                                types::LinkReason::Conditional => {
                                    "All branches in a conditional expression must return the same \
                                     type."
                                }
                                types::LinkReason::Field => {
                                    "A field must be assigned a value of it's compatible type."
                                }
                                types::LinkReason::Loop => {
                                    "Every break from a loop must be of the same type."
                                }
                                types::LinkReason::Parameter => {
                                    "A function must be called with arguments of matching types."
                                }
                                types::LinkReason::Return => {
                                    "All returns from a function must be of the same type."
                                }
                                types::LinkReason::Structure => {
                                    "You must fill out all fields of a structure when constructing \
                                     it."
                                }
                                types::LinkReason::Call => {
                                    "When calling a function, you must specify the values for it's \
                                     exact number of arguments."
                                }
                                types::LinkReason::Declaration => {
                                    "The type hint of this declaration doesn't match the value of \
                                     it."
                                }
                                _ => unreachable!(),
                            })
                        }
                    }

                    if let Some(span) = a.span {
                        report = report.with_label(
                            Label::new((span.path, span.range.clone()))
                                .with_color(Color::Cyan)
                                .with_message(format!(
                                    "This is of type {}.",
                                    Color::Cyan.paint(&a.value)
                                )),
                        )
                    }

                    for element in a_trace.0.iter().filter(|element| {
                        element.data_type.span.is_some()
                            && element.reason != types::LinkReason::Other
                    }) {
                        let span = element.data_type.span.as_ref().unwrap();
                        report = report.with_label(
                            Label::new((span.path, span.range.clone()))
                                .with_color(Color::Cyan)
                                .with_message(format!(
                                    "Because this is of type {} and {}.",
                                    Color::Cyan.paint(&element.data_type.value),
                                    <types::LinkReason as Into<&'static str>>::into(element.reason)
                                )),
                        )
                    }

                    if let Some(span) = b.span {
                        report = report.with_label(
                            Label::new((span.path, span.range.clone()))
                                .with_color(Color::Magenta)
                                .with_message(format!(
                                    "This is of type {}.",
                                    Color::Magenta.paint(&b.value)
                                )),
                        )
                    }

                    for element in b_trace.0.iter().filter(|element| {
                        element.data_type.span.is_some()
                            && element.reason != types::LinkReason::Other
                    }) {
                        let span = element.data_type.span.as_ref().unwrap();
                        report = report.with_label(
                            Label::new((span.path, span.range.clone()))
                                .with_color(Color::Magenta)
                                .with_message(format!(
                                    "Because this is of type {} and {}.",
                                    Color::Magenta.paint(&element.data_type.value),
                                    <types::LinkReason as Into<&'static str>>::into(element.reason)
                                )),
                        )
                    }

                    report
                }
                Error::InvalidAssign(location) => Report::build(
                    ReportKind::Error,
                    location.span.path,
                    location.span.range.start,
                )
                .with_message(format!("Cannot assign to a {}.", location.value))
                .with_label(
                    Label::new((location.span.path, location.span.range))
                        .with_message("Here.")
                        .with_color(Color::Red),
                )
                .with_note("Only identifiers and fields can be assigned to."),
                Error::MissingField {
                    structure,
                    field_name,
                } => Report::build(
                    ReportKind::Error,
                    structure.span.path,
                    structure.span.range.start,
                )
                .with_message(format!(
                    "Field {} doesn't exist for {}",
                    Color::Green.paint(field_name),
                    Color::Green.paint(&structure.value)
                ))
                .with_label(
                    Label::new((structure.span.path, structure.span.range))
                        .with_message(format!(
                            "You attempt to access {} here.",
                            Color::Green.paint(field_name)
                        ))
                        .with_color(Color::Red),
                ),
                Error::InvalidFlow {
                    span,
                    construct: loop_flow,
                } => Report::build(ReportKind::Error, span.path, span.range.start)
                    .with_message(format!(
                        "{} expression isn't inside a loop.",
                        Color::Green.paint(loop_flow)
                    ))
                    .with_label(
                        Label::new((span.path, span.range))
                            .with_message("Here.")
                            .with_color(Color::Red),
                    )
                    .with_note(format!(
                        "You cannot use a {} in the case of a nested function in a loop. ",
                        Color::Green.paint(loop_flow)
                    )),
                _ => unreachable!(),
            }
            .with_config(ErrorConfig::default().with_compact(compact))
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
    optimizations: OptLevel,
    path: String,
    compact_errors: bool,
}

fn get_config() -> Config {
    let matches = App::new("The Bell CLI")
        .author("Yoav Grimland, miestrode@gmail.com")
        .version("0.5.0")
        .about("Compile a Bell project/file")
        .arg(
            Arg::new("path")
                .short('p')
                .long("path")
                .value_name("FILE/FOLDER")
                .about("Is used to specify the path of the project/file to compile")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("optimizations")
                .short('o')
                .long("opt")
                .about("Enables more complicated code optimizations")
                .takes_value(false),
        )
        .arg(
            Arg::new("export")
                .short('e')
                .long("export")
                .value_name("FOLDER")
                .about(
                    "Specifies what folder to put your compiled data pack in.\n When unused, it \
                     will print the data pack out",
                )
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::new("compact")
                .short('c')
                .long("compact")
                .takes_value(false)
                .about("Makes error messages more compact."),
        )
        .get_matches();

    Config {
        optimizations: if matches.is_present("release") {
            OptLevel::Release
        } else {
            OptLevel::Debug
        },
        export_to: matches.value_of("export").map(|path| path.to_owned()),
        path: matches.value_of("path").unwrap().to_owned(),
        compact_errors: matches.is_present("compact"),
    }
}

pub fn run() -> RunResult {
    let config = get_config();
    println!(
        "{} {}\n",
        Color::Green.paint("Compiling").bold(),
        Color::Blue.paint(&config.path)
    );

    let path = PathBuf::from(&config.path);
    let time = Instant::now();

    match lang::compile(path, config.optimizations) {
        Ok(_) => {
            println!("{}", Color::RGB(128, 128, 128).paint("No output :)"));

            let elapsed = time.elapsed().as_secs_f32();

            println!(
                "\n{} ({} build) in {:.4}s",
                Color::Green.paint("Finished").bold(),
                Color::Blue.paint(match config.optimizations {
                    OptLevel::Debug => "debug",
                    OptLevel::Release => "release",
                }),
                elapsed
            );

            RunResult::Success
        }
        Err(Errors { errors, sources }) => {
            let mut cache = ErrorSources::from(sources);

            for error in errors {
                error.display(&mut cache, config.compact_errors);
                println!();
            }

            println!(
                "{} compilation due to the errors above.",
                Color::Red.paint("Failed").bold()
            );

            RunResult::Failure
        }
    }
}
