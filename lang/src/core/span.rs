use camino::Utf8PathBuf;
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::Range;

use internment::Intern;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub path: Intern<Utf8PathBuf>,
    pub range: Range<usize>,
}

pub struct SourceMap(pub HashMap<Intern<Utf8PathBuf>, String>);

impl Default for SourceMap {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceMap {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn insert(&mut self, id: Intern<Utf8PathBuf>, contents: String) {
        self.0.insert(id, contents);
    }

    pub fn merge(&mut self, other: Self) {
        self.0.extend(other.0.into_iter())
    }

    #[allow(clippy::ptr_arg)]
    pub fn get_contents(&self, id: &Utf8PathBuf) -> Option<String> {
        self.0.get(id).cloned()
    }
}
