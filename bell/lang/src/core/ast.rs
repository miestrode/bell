use internment::Intern;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::ops::Deref;

use crate::core::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Identifier(pub Vec<Intern<String>>);

impl Identifier {
    pub fn new(id: Vec<Intern<String>>) -> Self {
        Identifier(id)
    }

    pub fn new_single(id: Intern<String>) -> Self {
        Identifier(vec![id])
    }
}

impl Display for Identifier {
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

impl From<Intern<String>> for Identifier {
    fn from(local: Intern<String>) -> Identifier {
        Identifier(vec![local])
    }
}

#[derive(Debug, Clone)]
pub enum PathEnd {
    Path(Path),
    This,
    End(Intern<String>),
}

#[derive(Debug, Clone)]
pub struct Path {
    pub body: (Identifier, Span),
    pub tail: Vec<(PathEnd, Span)>,
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
    Structure(Identifier),
    Reference(Box<Type>),
}

#[derive(Debug, Clone)]
pub struct WithHint<T> {
    pub data: T,
    pub type_hint: Option<(Type, Span)>,
}

impl<T> WithHint<T> {
    pub fn map<U>(
        self,
        mut data_map: impl FnMut(T) -> U,
        mut type_map: impl FnMut(Type) -> Type,
    ) -> WithHint<U> {
        WithHint {
            data: data_map(self.data),
            type_hint: self
                .type_hint
                .map(|(type_hint, span)| (type_map(type_hint), span)),
        }
    }
}

type Field = ((Intern<String>, Span), (Expression, Span));

#[derive(Debug, Clone)]
pub enum Expression {
    Int(i32),
    Boolean(bool),
    String(Intern<String>),
    Identifier(Identifier),
    Function {
        name: WithHint<(Intern<String>, Span)>,
        parameters: Vec<WithHint<(Intern<String>, Span)>>,
        body: Box<(Expression, Span)>,
    },
    Instance {
        object: (Identifier, Span),
        fields: Vec<Field>,
    },
    Call {
        function: Box<(Expression, Span)>,
        parameters: Vec<(Expression, Span)>,
    },
    Declaration {
        name: WithHint<(Intern<String>, Span)>,
        value: Box<(Expression, Span)>,
    },
    Assignment {
        to: Box<(Expression, Span)>,
        from: Box<(Expression, Span)>,
    },
    Access {
        from: Box<(Expression, Span)>,
        id: (Intern<String>, Span),
    },
    Block {
        expressions: Vec<(Expression, Span)>,
        tail: Option<Box<(Expression, Span)>>,
    },
    Structure {
        name: (Intern<String>, Span),
        fields: Vec<WithHint<(Intern<String>, Span)>>,
    },
    Conditional {
        branches: Vec<((Expression, Span), (Expression, Span))>,
        tail: Option<Box<(Expression, Span)>>,
    },
    Break(Box<(Expression, Span)>),
    Return(Box<(Expression, Span)>),
    Use(Path),
    Loop(Box<(Expression, Span)>),
    Continue,
    Error,
}

impl Default for Expression {
    fn default() -> Self {
        Self::Error
    }
}

type Program = Vec<(Expression, Span)>;

#[derive(Debug, Clone)]
pub enum Module {
    Module {
        name: Intern<String>,
        entries: Vec<Module>,
    },
    Program {
        name: Intern<String>,
        ast: Program,
    },
}
