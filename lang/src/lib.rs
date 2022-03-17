use crate::core::error::Error;
use crate::core::error::Errors;
use crate::core::span::SourceMap;
use crate::core::types::Types;
use crate::middle_end::hir::ToHir;
use std::borrow::Cow;

use crate::core::ast;

use crate::core::file::Entry;
use camino::Utf8PathBuf;
use front_end::module;
use internment::Intern;
use middle_end::{check::check, gather};
use std::fs;
use std::path::PathBuf;

pub mod core;
pub mod front_end;
pub mod middle_end;

#[derive(Copy, Clone)]
pub enum OptLevel {
    Debug,
    Release,
}

#[allow(unused)]
pub fn compile(path: PathBuf, optimizations: OptLevel) -> Result<Types, Errors> {
    let mut errors = Errors {
        errors: Vec::new(),
        sources: SourceMap::new(),
    };

    let module = if path.is_file() {
        let path = match Utf8PathBuf::from_path_buf(path) {
            Ok(path) => path,
            Err(path) => {
                return {
                    errors.insert_error(Error::Basic(format!(
                        "the path {} is not encoded in UTF-8",
                        path.to_string_lossy()
                    )));

                    Err(errors)
                };
            }
        };

        module::from(
            Entry::File {
                contents: match fs::read_to_string(&path) {
                    Ok(contents) => contents,
                    Err(error) => {
                        return {
                            errors.insert_error(Error::IO {
                                error,
                                action: Cow::from(format!("read file {}", path)),
                            });

                            Err(errors)
                        };
                    }
                },
                path: Intern::new(path),
            },
            &mut errors,
        )
    } else if let Some(entry) = Entry::from(
        match Utf8PathBuf::from_path_buf(path) {
            Ok(path) => path,
            Err(path) => {
                return {
                    errors.insert_error(Error::Basic(format!(
                        "the path {} is not encoded in UTF-8",
                        path.to_string_lossy()
                    )));
                    Err(errors)
                };
            }
        },
        &mut errors,
    ) {
        module::from(entry, &mut errors)
    } else {
        return Err(errors);
    };

    let types = check(
        gather::gather(module.to_hir(&mut errors), &mut errors),
        &mut errors,
    );

    if errors.is_empty() {
        Ok(types)
    } else {
        Err(errors)
    }
}
