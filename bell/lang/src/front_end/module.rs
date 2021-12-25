use crate::core::ast::Module;
use crate::core::error::Error;
use crate::core::file::Entry;
use crate::core::span::SourceMap;
use crate::front_end;

use internment::Intern;

// TODO: The module system, should complain when a module element has the same name as the previous one.
impl Module {
    pub(crate) fn from(entry: Entry, sources: &mut SourceMap) -> (Option<Module>, Vec<Error>) {
        match entry {
            Entry::File { path, contents } => {
                let (ast, errors) = front_end::generate_ast(path, &contents);
                sources.insert(path, contents);

                if errors.is_empty() {
                    (
                        Some(Module::Program {
                            name: Intern::new(path.file_stem().unwrap().to_string()),
                            ast: ast.unwrap(),
                        }),
                        errors, // The errors will be empty.
                    )
                } else {
                    (None, errors)
                }
            }
            Entry::Directory { path, entries } => {
                let (modules, errors) = entries
                    .into_iter()
                    .map(|entry| Module::from(entry, sources))
                    .fold(
                        (Vec::new(), Vec::new()),
                        |(mut modules, mut errors), (module, new_errors)| {
                            if let Some(module) = module {
                                modules.push(module);
                            }

                            errors.extend(new_errors);
                            (modules, errors)
                        },
                    );

                (
                    Some(Module::Module {
                        name: Intern::new(path.file_stem().unwrap().to_string()),
                        entries: modules,
                    }),
                    errors,
                )
            }
        }
    }
}
