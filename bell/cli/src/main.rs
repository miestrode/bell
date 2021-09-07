use std::{fs, io, ops, process, time};

use lang::core::error;

const VERSION: &str = "1.0.0";
const LEVELS: ops::RangeInclusive<i32> = 1..=5;

struct CompileContext<'a> {
    level: i32,
    file: &'a str, // Might switch to multiple files to allow modules in the future
    export_path: Option<&'a str>,
    run_on_reload: bool,
    description: &'a str,
}

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
                    .with_message(format!("Character `{}` is invalid.", character))
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
                            .with_message("Expected a `*/`, somewhere in this range.")
                            .with_color(ariadne::Color::Red),
                    )
                    .finish()
                    .eprint((self.filename, ariadne::Source::from(self.text)))
            }
            error::ErrorKind::ExpectedDifferentToken {
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
            error::ErrorKind::DataTypeMismatch {
                expected,
                got: found,
                because,
                got_location: location,
            } => {
                let mut report = ariadne::Report::build(
                    ariadne::ReportKind::Error,
                    self.filename,
                    location.start,
                )
                    .with_message(format!(
                        "Expected data of type `{}`, got `{}`.",
                        expected, found
                    ))
                    .with_code(4);

                report = if let Some(because) = because {
                    report
                        .with_label(
                            ariadne::Label::new((self.filename, because))
                                .with_message(format!(
                                    "Expected data of type `{}` because of this.",
                                    expected
                                ))
                                .with_color(ariadne::Color::Blue),
                        )
                        .with_label(
                            ariadne::Label::new((self.filename, location))
                                .with_message(format!(
                                    "But got data of type `{}` because of this.",
                                    found
                                ))
                                .with_color(ariadne::Color::Red),
                        )
                } else {
                    report.with_label(
                        ariadne::Label::new((self.filename, location))
                            .with_message("Here.")
                            .with_color(ariadne::Color::Red),
                    )
                };

                report
                    .finish()
                    .eprint((self.filename, ariadne::Source::from(self.text)))
            }
            error::ErrorKind::ShadowedSymbol {
                name,
                old,
                new,
                symbol,
            } => ariadne::Report::build(ariadne::ReportKind::Error, self.filename, old.start)
                .with_message(format!("A {} named `{}` already exists.", symbol, name))
                .with_code(5)
                .with_label(
                    ariadne::Label::new((self.filename, old))
                        .with_message(format!("`{}` is first declared here.", name))
                        .with_color(ariadne::Color::Blue),
                )
                .with_label(
                    ariadne::Label::new((self.filename, new))
                        .with_message(format!("But then `{}` is declared again here!", name))
                        .with_color(ariadne::Color::Red),
                )
                .with_note("Only variables can be re-declared; Functions and parameters cannot.")
                .finish()
                .eprint((self.filename, ariadne::Source::from(self.text))),
            error::ErrorKind::UndeclaredSymbol { name, usage } => {
                ariadne::Report::build(ariadne::ReportKind::Error, self.filename, usage.start)
                    .with_message(format!("`{}` is used before it's declared.", name))
                    .with_code(6)
                    .with_label(
                        ariadne::Label::new((self.filename, usage))
                            .with_message(format!(
                                "`{}` is used here, but it isn't declared in any scope!",
                                name
                            ))
                            .with_color(ariadne::Color::Red),
                    )
                    .finish()
                    .eprint((self.filename, ariadne::Source::from(self.text)))
            }
            error::ErrorKind::NoElseBranch { location } =>
                ariadne::Report::build(ariadne::ReportKind::Error, self.filename, location.start)
                    .with_message("Conditional expression doesn't contain an `else` branch.")
                    .with_code(7)
                    .with_label(ariadne::Label::new((self.filename, location))
                        .with_message("This conditional must always return a value. But it doesn't, since it has no else branch.")
                        .with_color(ariadne::Color::Red))
                    .with_note("When a conditionals other branches return the `unit` type, an else branch isn't needed.")
                    .finish()
                    .eprint((self.filename, ariadne::Source::from(self.text))),
            error::ErrorKind::ParameterCountMismatch { expected_count, got_count, because, got_location, function } => {
                let report = ariadne::Report::build(ariadne::ReportKind::Error, self.filename, got_location.start)
                    .with_message(format!("Function `{}` expected {} parameter(s), but got {} parameter(s)", function, expected_count, got_count))
                    .with_code(8);

                if let Some(because) = because {
                    report
                        .with_label(ariadne::Label::new((self.filename, because))
                            .with_message(format!("Expected all calls to {} to take {} parameter(s) because of this.", function, expected_count))
                            .with_color(ariadne::Color::Blue))
                        .with_label(ariadne::Label::new((self.filename, got_location))
                            .with_message(format!("But this call to {} gave {} parameter(s).", function, got_count))
                            .with_color(ariadne::Color::Red))
                } else {
                    report
                        .with_label(ariadne::Label::new((self.filename, got_location))
                            .with_message("Here.")
                            .with_color(ariadne::Color::Red))
                }
                    .finish()
                    .eprint((self.filename, ariadne::Source::from(self.text)))
            }
        }
    }
}

fn main() {
    // todo: Allow for more complex configurations by sending data directly to the compiler in the form of a compilation context
    let matches = clap::App::new("The Bell Compiler CLI")
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
        .arg(clap::Arg::with_name("export")
            .visible_aliases(&["path", "store"])
            .short("e")
            .long("export")
            .value_name("PATH")
            .takes_value(true)
            .required(false)
            .help("Where to store the data pack created after compiling. Only used when compiling. The path should include the name of the datapack you want."))
        .arg(clap::Arg::with_name("level")
            .visible_aliases(&["stage", "up_to"])
            .short("l")
            .long("level")
            .value_name("INT")
            .takes_value(true)
            .required(false)
            .default_value("5")
            .help("The level of compilation. Compilation levels in ascending order are: lexing, parsing, type checking, lowering to MIR and code generation."))
        .arg(clap::Arg::with_name("description")
            .visible_aliases(&["description"])
            .short("d")
            .long("description")
            .value_name("TEXT")
            .takes_value(true)
            .required(false)
            .default_value("Generated by Bell.")
            .help("The description of the data pack. Only used when compiling."))
        .arg(clap::Arg::with_name("reload")
            .short("r")
            .long("run on reload")
            .takes_value(false)
            .required(false)
            .help("When on the program will run after every time `/reload` is used."))
        .get_matches();

    let context = CompileContext {
        // Get the compilation level. Mainly used for debugging, since sometimes you want to test out a particular part of the compiler.
        // Previous compilation steps however are mandatory. You cannot parse without tokenizing first
        level: matches
            .value_of("level")
            .unwrap()
            .parse::<i32>()
            .unwrap_or_else(|_| {
                eprintln!(
                    "{} Malformed integer for compilation level.",
                    ariadne::Color::Red.paint("Error:")
                );

                process::exit(exitcode::DATAERR);
            }),
        // Unwrap can be used, since Clap makes sure the file is specified
        file: matches.value_of("file").unwrap(),
        export_path: matches.value_of("export"),
        run_on_reload: matches.is_present("reload"),
        // Unwrap can be used, since Clap makes sure the description is specified
        description: matches.value_of("description").unwrap(),
    };

    if !(LEVELS).contains(&context.level) {
        eprintln!(
            "{} Compilation level needs to be between {} to {}.",
            ariadne::Color::Red.paint("Error:"),
            LEVELS.start(),
            LEVELS.end()
        );

        process::exit(exitcode::DATAERR);
    }

    let file = fs::read_to_string(&context.file).unwrap_or_else(|_| {
        eprintln!(
            "{} Could not locate file: `{}`.",
            ariadne::Color::Red.paint("Error:"),
            context.file
        );

        process::exit(exitcode::DATAERR);
    });

    let now = time::Instant::now();
    let result = lang::compile(context.file, &file, context.level);

    println!(
        "{} {} {} in {:.2}s.\n",
        ariadne::Color::Green.paint("Finished"),
        match context.level {
            1 => "lexing",
            2 => "parsing",
            3 => "type checking",
            4 => "lowering",
            5 => "compiling",
            _ => unreachable!(),
        },
        ariadne::Color::Blue.paint(&context.file),
        now.elapsed().as_secs_f32()
    );

    process::exit(match result {
        Ok(lang::CompileResult::LexResult(tokens)) => {
            println!("{:#?}", tokens);

            exitcode::OK
        }
        Ok(lang::CompileResult::ParseResult(program)) => {
            println!("{:#?}", program);

            exitcode::OK
        }
        Ok(lang::CompileResult::CheckResult(typed_program)) => {
            println!("{:#?}", typed_program);

            exitcode::OK
        }
        Ok(lang::CompileResult::MIRResult(mir_program)) => {
            println!("{:#?}", mir_program);

            exitcode::OK
        }
        Ok(lang::CompileResult::Result(program)) => {
            // Exporting can only occur when the compilation level is 5, so here
            if let Some(export_path) = context.export_path {
                // Create the directory the datapack will be placed in
                match fs::create_dir(export_path) {
                    Ok(_) => (),
                    Err(error) => {
                        if let Some(17) = error.raw_os_error() {
                            // Try creating it again
                            fs::remove_dir_all(export_path).expect("Could not create datapack");

                            fs::create_dir(export_path).expect("Could not create datapack");
                        }
                    }
                }

                fs::write(
                    format!("{}/pack.mcmeta", export_path),
                    format!(
                        "{{\"pack\": {{\"pack_format\": 7, \"description\": \"{}\"}}}}",
                        context.description
                    ),
                )
                .expect("Could not create datapack");

                // Create the directories for code storage and configuration
                fs::create_dir_all(format!("{}/data/project/functions", export_path))
                    .expect("Could not create datapack");
                fs::create_dir_all(format!("{}/data/minecraft/tags/functions", export_path))
                    .expect("Could not create datapack");

                // Tell minecraft if our code should run when we do "/reload"
                fs::write(
                    format!("{}/data/minecraft/tags/functions/load.json", export_path),
                    if program.1.is_some() && context.run_on_reload {
                        "{\"values\": [\"project:main\"]}"
                    } else {
                        "{\"values\": []}"
                    },
                )
                .expect("Could not create datapack");

                // Export all the files
                for function in program.0 .0 {
                    fs::write(
                        format!(
                            "{}/data/project/functions/{}.mcfunction",
                            export_path, function.name
                        ),
                        function.to_string(),
                    )
                    .expect("Could not create datapack");
                }
            } else {
                println!("{}", program.0.to_string());
            }

            exitcode::OK
        }
        Err(error) => {
            error.display().unwrap();

            exitcode::OSFILE
        }
    })
}
