use crate::core::error::Error;
use crate::core::span::SourceMap;
use crate::core::span::Span;
use std::borrow::Cow;

use crate::core::ast;
use crate::middle_end::{checker, hir};

use crate::core::file::Entry;
use camino::Utf8PathBuf;
use internment::Intern;
use middle_end::checker::TypeInfo;
use std::fs;
use std::path::PathBuf;

pub mod core;
pub mod front_end;
pub mod middle_end;

#[derive(Copy, Clone)]
pub enum OptimizationLevel {
    Debug,
    Release,
}

pub struct CompileErrors {
    pub errors: Vec<Error>,
    pub sources: SourceMap,
}

#[allow(unused)]
pub fn compile(
    path: PathBuf,
    optimizations: OptimizationLevel,
) -> Result<Vec<(TypeInfo, Span)>, CompileErrors> {
    let mut sources = SourceMap::new();

    let (module, errors) = if path.is_file() {
        let path = match Utf8PathBuf::from_path_buf(path) {
            Ok(path) => path,
            Err(path) => {
                return Err(CompileErrors {
                    errors: vec![Error::Basic(format!(
                        "the path {} is not encoded in UTF-8",
                        path.to_string_lossy()
                    ))],
                    sources,
                });
            }
        };

        ast::Module::from(
            Entry::File {
                contents: match fs::read_to_string(&path) {
                    Ok(contents) => contents,
                    Err(error) => {
                        return Err(CompileErrors {
                            errors: vec![Error::IO {
                                error,
                                action: Cow::from(format!("read file {}", path)),
                            }],
                            sources,
                        });
                    }
                },
                path: Intern::new(path),
            },
            &mut sources,
        )
    } else {
        let result = Entry::new(match Utf8PathBuf::from_path_buf(path) {
            Ok(path) => path,
            Err(path) => {
                return Err(CompileErrors {
                    errors: vec![Error::Basic(format!(
                        "the path {} is not encoded in UTF-8",
                        path.to_string_lossy()
                    ))],
                    sources,
                });
            }
        });

        ast::Module::from(
            match result {
                Ok(entry) => entry,
                Err(errors) => return Err(CompileErrors { errors, sources }),
            },
            &mut sources,
        )
    };

    // TODO: Error recovery.
    if errors.is_empty() {
        let (module, errors) = hir::to_hir(module.unwrap());

        if errors.is_empty() {
            checker::check(module).map_err(|errors| CompileErrors { errors, sources })
        } else {
            Err(CompileErrors { errors, sources })
        }
    } else {
        Err(CompileErrors { errors, sources })
    }
}
