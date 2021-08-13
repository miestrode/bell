use std::{fs, io, process, time};

use lang::core::error;

static VERSION: &str = "0.2.0";

// A simple trait, used only for displaying errors
trait Report {
    fn display(self) -> io::Result<()>;
}

// Display a list of strings in the form "x, y, or z". Assumes the list has at least 1 element
fn display_as_choice(list: &[&str]) -> String {
    format!("{}{}", list[..(list.len() - 1)].join(", "), if list.len() > 1 {
        format!(", or {}", list.last().unwrap())
    } else {
        String::from(list[0])
    })
}

// Displaying errors was moved to the CLI to reduce bloat for the core language crate
impl Report for error::Error<'_> {
    fn display(self) -> io::Result<()> {
        match self {
            error::Error::InvalidCharacter { filename, text, range, character } =>
                ariadne::Report::build(ariadne::ReportKind::Error, filename, range.start)
                    .with_message(format!("Character {} is invalid.", ariadne::Color::Blue.paint(format!("`{}`", character))))
                    .with_code(1)
                    .with_label(ariadne::Label::new((filename, range))
                        .with_message("Here.")
                        .with_color(ariadne::Color::Red))
                    .finish()
                    .eprint((filename, ariadne::Source::from(text))),
            error::Error::UnterminatedBlockComment { filename, text, range } =>
                ariadne::Report::build(ariadne::ReportKind::Error, filename, range.start)
                    .with_message("unterminated block comment.")
                    .with_code(2)
                    .with_label(ariadne::Label::new((filename, range))
                        .with_message(format!("Expected a {}, somewhere in this range.", ariadne::Color::Blue.paint("`*/`")))
                        .with_color(ariadne::Color::Red))
                    .finish()
                    .eprint((filename, ariadne::Source::from(text))),
            error::Error::Expected { filename, text, range, expected, found } =>
                ariadne::Report::build(ariadne::ReportKind::Error, filename, range.start)
                    .with_message(format!("expected {}, found {}.", ariadne::Color::Blue.paint(display_as_choice(expected)), ariadne::Color::Blue.paint(found)))
                    .with_code(3)
                    .with_label(ariadne::Label::new((filename, range))
                        .with_message("Here.")
                        .with_color(ariadne::Color::Red))
                    .finish()
                    .eprint((filename, ariadne::Source::from(text)))
        }
    }
}

fn main() {
    let matches = clap::App::new("The Bell compiler CLI")
        .version(VERSION)
        .author("Yoav G. <miestrode@gmail.com>")
        .about("Compile Bell files into MCfunction.")
        .arg(clap::Arg::with_name("file")
            .visible_aliases(&["source", "filename", "compile", "build"])
            .short("f")
            .long("file")
            .value_name("FILE")
            .takes_value(true)
            .required(true)
            .help("The file to compile to MCfunction."))
        .arg(clap::Arg::with_name("level")
            .visible_aliases(&["stage", "up_to"])
            .short("l")
            .long("level")
            .value_name("INT")
            .takes_value(true)
            .required(false)
            .default_value("3")
            .help("The file to compile to MCfunction."))
        .get_matches();

    // Unwrap can be used, since Clap makes sure all the values are specified
    let file = matches
        .value_of("file")
        .unwrap();
    let level = matches
        .value_of("level")
        .unwrap()
        .parse::<i32>()
        .unwrap_or_else(|_| {
            eprintln!("{} Malformed integer for compilation level.", ariadne::Color::Red.paint("Error:"));

            process::exit(exitcode::DATAERR);
        });

    if !(1..=3).contains(&level) {
        eprintln!("{} Compilation level needs to be between 1 to 3.", ariadne::Color::Red.paint("Error:"));

        process::exit(exitcode::DATAERR);
    }

    // Fetch the file the user wants to compile
    let text = fs::read_to_string(&file).unwrap_or_else(|_| {
        eprintln!("{} Could not locate file: `{}`.", ariadne::Color::Red.paint("Error:"), ariadne::Color::Blue.paint(file));

        process::exit(exitcode::DATAERR);
    });

    let now = time::Instant::now();
    let result = lang::compile(file, &text, level);

    println!("{} {} {} in {:.2}s.", ariadne::Color::Green.paint("Finished"), match level {
        1 => "lexing",
        2 => "parsing",
        3 => "analysing",
        _ => panic!("Level is invalid, but should have already been checked")
    }, ariadne::Color::Blue.paint(&file), now.elapsed().as_secs_f32());

    process::exit(match result {
        Ok(value @ lang::CompileResult::LexResult(_)) => {
            println!("{:#?}", value);

            exitcode::OK
        }
        Ok(value @ lang::CompileResult::ParseResult(_)) => {
            println!("{:#?}", value);

            exitcode::OK
        }
        Ok(value @ lang::CompileResult::AnalysisResult(_)) => {
            println!("{:#?}", value);

            exitcode::OK
        }
        Err(error) => {
            error
                .display()
                .unwrap();

            exitcode::OSFILE
        }
    })
}