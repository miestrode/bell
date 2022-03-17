use internment::Intern;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::ops::Deref;

use crate::core::span::Span;

use super::Name;

#[derive(Debug, Clone, PartialEq)]
pub struct Id(pub Vec<Intern<String>>);

impl Id {
    pub fn new(id: Vec<Intern<String>>) -> Self {
        Id(id)
    }

    pub fn new_single(id: Intern<String>) -> Self {
        Id(vec![id])
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(
            self.0
                .iter()
                .copied()
                .map(|item| item.deref().clone())
                .collect::<Vec<_>>()
                .join("::")
                .as_str(),
        )
    }
}

impl From<Intern<String>> for Id {
    fn from(local: Intern<String>) -> Id {
        Id(vec![local])
    }
}

#[derive(Debug, Copy, Clone)]
pub enum FlowKind {
    Return,
    Continue,
    Break,
}

#[derive(Debug, Clone)]
pub enum Type {
    Integer,
    Boolean,
    String,
    Structure(Id),
    Reference(Box<Type>),
}

#[derive(Debug, Clone)]
pub struct TypeHint<T> {
    pub value: T,
    pub type_hint: Option<(Type, Span)>,
}

impl<T> TypeHint<T> {
    pub fn map<U>(
        self,
        mut data_map: impl FnMut(T) -> U,
        mut type_map: impl FnMut(Type) -> Type,
    ) -> TypeHint<U> {
        TypeHint {
            value: data_map(self.value),
            type_hint: self
                .type_hint
                .map(|(type_hint, span)| (type_map(type_hint), span)),
        }
    }
}

type Field = ((Name, Span), (Expression, Span));

struct Path(Vec<Id>);

#[derive(Debug, Clone)]
pub enum Expression {
    Int(i32),
    Boolean(bool),
    String(Id),
    Identifier(Id),
    Function {
        name: TypeHint<(Name, Span)>,
        parameters: Vec<TypeHint<(Name, Span)>>,
        body: Box<(Expression, Span)>,
    },
    Instance {
        object: (Id, Span),
        fields: Vec<Field>,
    },
    Call {
        function: Box<(Expression, Span)>,
        parameters: (Vec<(Expression, Span)>, Span),
    },
    Declaration {
        name: TypeHint<(Name, Span)>,
        value: Box<(Expression, Span)>,
    },
    Assignment {
        to: Box<(Expression, Span)>,
        from: Box<(Expression, Span)>,
    },
    Access {
        from: Box<(Expression, Span)>,
        field: (Name, Span),
    },
    Block {
        expressions: Vec<(Expression, Span)>,
        tail: Option<Box<(Expression, Span)>>,
    },
    Structure {
        name: (Name, Span),
        fields: Vec<TypeHint<(Name, Span)>>,
    },
    Conditional {
        branches: Vec<((Expression, Span), (Expression, Span))>,
        tail: Option<Box<(Expression, Span)>>,
    },
    Break(Box<(Expression, Span)>),
    Return(Box<(Expression, Span)>),
    Import((Id, Span)),
    Loop(Box<(Expression, Span)>),
    Continue,
    Error,
}

type Program = Vec<(Expression, Span)>;

#[derive(Debug, Clone)]
pub enum Module {
    Submodule { name: Name, modules: Vec<Module> },
    Program { name: Name, ast: Program },
}
