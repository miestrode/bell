use camino::Utf8PathBuf;
use std::borrow::Cow;
use std::fs;

use internment::Intern;

use crate::core::error::Error;

pub(crate) enum Entry {
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
    pub(crate) fn new(path: Utf8PathBuf) -> Result<Entry, Vec<Error>> {
        let mut errors = Vec::new();

        let root_entry = Entry::Directory {
            entries: std::fs::read_dir(&path)
                .map_err(|io_error| {
                    vec![Error::IO {
                        error: io_error,
                        action: Cow::from("get the root module directory"),
                    }]
                })?
                .filter_map(|entry| {
                    let entry = match entry {
                        Ok(entry) => match Utf8PathBuf::from_path_buf(entry.path()) {
                            Ok(path) => Some(path),
                            Err(path) => {
                                errors.push(Error::Basic(format!(
                                    "the path {} is not encoded in UTF-8",
                                    path.to_string_lossy()
                                )));

                                return None;
                            }
                        },
                        Err(io_error) => {
                            errors.push(Error::IO {
                                error: io_error,
                                action: Cow::from("get an entries path in a directory"),
                            });

                            return None;
                        }
                    };

                    entry
                        .map(|path| {
                            Some(
                                // Available via short-circuiting.
                                if path.is_file() {
                                    // The compiler will ignore files that don't have the `bell` extension.
                                    // This is done so you can store utility files inside modules.
                                    if path.extension().unwrap() == "bell" {
                                        let path = Intern::new(path);

                                        Entry::File {
                                            path,
                                            contents: match fs::read_to_string(&*path) {
                                                Ok(text) => text,
                                                Err(io_error) => {
                                                    errors.push(Error::IO {
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
                                    match Self::new(path) {
                                        Ok(directory) => directory,
                                        Err(directory_errors) => {
                                            errors.extend(directory_errors);

                                            return None;
                                        }
                                    }
                                },
                            )
                        })
                        .flatten()
                })
                .collect(),
            path: Intern::new(path),
        };

        if errors.is_empty() {
            Ok(root_entry)
        } else {
            Err(errors)
        }
    }
}
