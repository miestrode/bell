use camino::Utf8PathBuf;
use std::borrow::Cow;
use std::fs;

use internment::Intern;

use crate::core::error::Error;

use super::error::Errors;

pub enum Entry {
    Directory {
        path: Intern<Utf8PathBuf>,
        entries: Vec<Entry>,
    },
    File {
        path: Intern<Utf8PathBuf>,
        contents: String,
    },
}

impl Entry {
    pub fn from(path: Utf8PathBuf, errors: &mut Errors) -> Option<Entry> {
        Some(Entry::Directory {
            entries: match std::fs::read_dir(&path) {
                Err(io_error) => {
                    errors.insert_error(Error::IO {
                        error: io_error,
                        action: Cow::from("get the root module directory"),
                    });

                    return None;
                }
                Ok(directory) => directory
                    .filter_map(|entry| {
                        let entry = match entry {
                            Ok(entry) => match Utf8PathBuf::from_path_buf(entry.path()) {
                                Ok(path) => Some(path),
                                Err(path) => {
                                    errors.insert_error(Error::Basic(format!(
                                        "the path {} is not encoded in UTF-8",
                                        path.to_string_lossy()
                                    )));

                                    return None;
                                }
                            },
                            Err(io_error) => {
                                errors.insert_error(Error::IO {
                                    error: io_error,
                                    action: Cow::from("get an entries path in a directory"),
                                });

                                return None;
                            }
                        };

                        entry
                            .map(|path| {
                                Some(if path.is_file() {
                                    // The compiler will ignore files that don't have the `bell` extension.
                                    // This is done so you can store utility files inside modules.
                                    if path.extension().unwrap() == "bell" {
                                        let path = Intern::new(path);

                                        Entry::File {
                                            path,
                                            contents: match fs::read_to_string(&*path) {
                                                Ok(text) => text,
                                                Err(io_error) => {
                                                    errors.insert_error(Error::IO {
                                                        error: io_error,
                                                        action: Cow::from(format!(
                                                            "read the contents of the path `{}`",
                                                            path
                                                        )),
                                                    });

                                                    return None;
                                                }
                                            },
                                        }
                                    } else {
                                        return None;
                                    }
                                } else {
                                    Self::from(path, errors)?
                                })
                            })
                            .flatten()
                    })
                    .collect(),
            },
            path: Intern::new(path),
        })
    }
}
