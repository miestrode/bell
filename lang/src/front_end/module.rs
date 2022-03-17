use std::collections::HashSet;

use crate::core::{
    ast::{Id, Module},
    error::Errors,
    Name,
};
use crate::core::{error::Error, file::Entry};
use crate::front_end;

use internment::Intern;

struct EntryTransformer<'a> {
    adjacent_names: HashSet<Name>,
    current_parent_id: Id,
    errors: &'a mut Errors,
}

// The entry transformers checks naming based on the rule that every module must have a unique name relative to it's adjacent modules
impl<'a> EntryTransformer<'a> {
    fn transform(&mut self, entry: Entry) -> Module {
        match entry {
            Entry::File { path, contents } => {
                let ast = front_end::generate_ast(path, &contents, self.errors);
                let name = Intern::new(path.file_stem().unwrap().to_string());

                if self.adjacent_names.get(&name).is_some() {
                    self.errors.insert_error(Error::ConflictingModuleNames {
                        parent: self.current_parent_id.clone(),
                        name,
                    })
                }

                self.errors.insert_source(path, contents);
                self.adjacent_names.insert(name);

                Module::Program { name, ast }
            }
            Entry::Directory { path, entries } => {
                let name = Intern::new(path.file_stem().unwrap().to_string());

                if self.adjacent_names.get(&name).is_some() {
                    self.errors.insert_error(Error::ConflictingModuleNames {
                        parent: self.current_parent_id.clone(),
                        name,
                    })
                }

                self.current_parent_id.0.push(name);

                let modules = entries
                    .into_iter()
                    .map(|entry| self.transform(entry))
                    .collect();

                self.current_parent_id.0.pop();

                // Once we finish using this hash set to make sure every adjacent name is unique, we need throw them out to avoid erroneous errors.
                self.adjacent_names.clear();

                Module::Submodule { name, modules }
            }
        }
    }
}

pub fn from(entry: Entry, errors: &mut Errors) -> Module {
    let mut transformer = EntryTransformer {
        adjacent_names: HashSet::new(),
        current_parent_id: Id::new(Vec::new()),
        errors,
    };

    transformer.transform(entry)
}
