use std::collections::HashSet;

use crate::core::span::Span;
use crate::core::{
    ast::Id,
    error::{Element, Error, Errors, Pattern, Reason},
};
use crate::{
    ast::{self, TypeHint},
    core::Name,
};

#[derive(Debug, Clone)]
pub struct Function {
    pub name: TypeHint<(Name, Span)>,
    pub parameters: Vec<TypeHint<(Name, Span)>>,
    pub body: Box<(Expression, Span)>,
}

#[derive(Debug, Clone)]
pub struct Structure {
    pub name: (Name, Span),
    pub fields: Vec<TypeHint<(Name, Span)>>,
}

type Field = ((Name, Span), (Expression, Span));

#[derive(Clone, Debug)]
pub enum AssignLocation {
    Variable(Name),
    Field {
        instance: Box<(Expression, Span)>,
        field: (Name, Span),
    },
}

#[derive(Debug, Clone)]
pub enum Expression {
    Unit,
    Int(i32),
    Boolean(bool),
    String(Name),
    Id(Name),
    Function(Function),
    Instance {
        object: (Name, Span),
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
        to: (AssignLocation, Span),
        from: Box<(Expression, Span)>,
    },
    Access {
        from: Box<(Expression, Span)>,
        id: (Name, Span),
    },
    Block {
        expressions: Vec<(Expression, Span)>,
        tail: Box<(Expression, Span)>,
    },
    Structure(Structure),
    Conditional {
        condition: Box<(Expression, Span)>,
        success: Box<(Expression, Span)>,
        failure: Box<(Expression, Span)>,
    },
    Break(Box<(Expression, Span)>),
    Return(Box<(Expression, Span)>),
    Continue,
    Loop(Box<(Expression, Span)>),
    Use((Name, Span)),
    Error,
}

impl From<Expression> for &'static str {
    fn from(expression: Expression) -> Self {
        match expression {
            Expression::Unit => "unit",
            Expression::Int(_) => "integer",
            Expression::Boolean(_) => "boolean",
            Expression::String(_) => "string",
            Expression::Id(_) => "identifier",
            Expression::Function(_) => "function",
            Expression::Instance { .. } => "instance",
            Expression::Call { .. } => "call",
            Expression::Declaration { .. } => "declaration",
            Expression::Assignment { .. } => "assignment",
            Expression::Access { .. } => "field",
            Expression::Block { .. } => "block",
            Expression::Structure(_) => "structure",
            Expression::Conditional { .. } => "conditional",
            Expression::Break(_) => "break",
            Expression::Return(_) => "return",
            Expression::Continue => "continue",
            Expression::Loop(_) => "loop",
            Expression::Use(_) => "import",
            Expression::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
pub enum TopLevel {
    Function(Function),
    Structure(Structure),
    Import((Id, Span)),
}

pub type Program = Vec<(TopLevel, Span)>;

pub enum Module {
    Program { name: Name, program: Program },
    Submodule { name: Name, modules: Vec<Module> },
}

pub trait ToHir<T> {
    fn to_hir(self, errors: &mut Errors) -> T;
}

impl ToHir<(Expression, Span)> for (ast::Expression, Span) {
    fn to_hir(self, errors: &mut Errors) -> (Expression, Span) {
        (
            match self.0 {
                ast::Expression::Int(value) => Expression::Int(value),
                ast::Expression::Boolean(value) => Expression::Boolean(value),
                ast::Expression::String(value) => Expression::String(value),
                ast::Expression::Identifier(id) => Expression::Id(id),
                ast::Expression::Function {
                    name,
                    parameters,
                    body,
                } => Expression::Function(Function {
                    name: name.map(|(name, span)| (Name::new_single(name), span), |ty| ty),
                    parameters: parameters
                        .into_iter()
                        .map(|parameter| TypeHint {
                            value: (Name::new_single(parameter.value.0), parameter.value.1),
                            type_hint: parameter.type_hint,
                        })
                        .collect(),
                    body: Box::new(body.to_hir(errors)),
                }),
                ast::Expression::Instance { object, fields } => Expression::Instance {
                    object,
                    fields: fields
                        .into_iter()
                        .map(|(name, expression)| (name, expression.to_hir(errors)))
                        .collect(),
                },
                ast::Expression::Call {
                    function,
                    parameters,
                } => Expression::Call {
                    function: Box::new(function.to_hir(errors)),
                    parameters: (
                        parameters
                            .0
                            .into_iter()
                            .map(|expression| expression.to_hir(errors))
                            .collect(),
                        parameters.1,
                    ),
                },
                ast::Expression::Declaration { name, value } => Expression::Declaration {
                    name: name.map(|(name, span)| (Name::new_single(name), span), |ty| ty),
                    value: Box::new(value.to_hir(errors)),
                },
                ast::Expression::Assignment { to, from } => {
                    let to = to.to_hir(errors);

                    (|| Expression::Assignment {
                        to: (
                            match to.0 {
                                Expression::Id(id) => AssignLocation::Variable(id),
                                Expression::Access { from, id } => AssignLocation::Field {
                                    instance: from,
                                    field: id,
                                },
                                _ => {
                                    errors.insert_error(Error::InvalidAssign(Element {
                                        value: to.0.into(),
                                        span: to.1.clone(),
                                    }));

                                    return Expression::Error; // A kind of early return.
                                }
                            },
                            to.1,
                        ),
                        from: Box::new(from.to_hir(errors)),
                    })()
                }
                ast::Expression::Access { from, field: id } => Expression::Access {
                    from: Box::new(from.to_hir(errors)),
                    id,
                },
                ast::Expression::Block { expressions, tail } => Expression::Block {
                    expressions: expressions
                        .into_iter()
                        .map(|expression| expression.to_hir(errors))
                        .collect(),
                    tail: Box::new(tail.map_or_else(
                        || (Expression::Unit, self.1.clone()),
                        |expression| expression.to_hir(errors),
                    )),
                },
                ast::Expression::Structure { name, fields } => Expression::Structure(Structure {
                    name: (Name::new_single(name.0), name.1),
                    fields,
                }),
                ast::Expression::Conditional { mut branches, tail } => {
                    let (condition, branch) = branches.remove(0);

                    // Generate the total span of the rest of the conditional, if it exists.
                    let rest_span = match (branches.first(), branches.last()) {
                        (Some(((_, span_a), _)), Some((_, (_, span_b)))) => Some(Span {
                            path: span_a.path,
                            range: span_a.range.start..span_b.range.end,
                        }),
                        _ => None,
                    };

                    Expression::Conditional {
                        condition: Box::new(condition.to_hir(errors)),
                        failure: Box::new(
                            (if let Some(span) = rest_span {
                                (ast::Expression::Conditional { branches, tail }, span)
                            } else if let Some(tail) = tail {
                                *tail
                            } else {
                                // The span of this unit expression can be thought of being the span of the else-if,
                                // since it's the reason this expression exists. (Kind of-ish).
                                (
                                    ast::Expression::Block {
                                        expressions: Vec::new(),
                                        tail: None,
                                    },
                                    branch.1.clone(),
                                )
                            })
                            .to_hir(errors),
                        ),
                        success: Box::new(branch.to_hir(errors)),
                    }
                }
                ast::Expression::Break(value) => Expression::Break(Box::new(value.to_hir(errors))),
                ast::Expression::Return(value) => {
                    Expression::Return(Box::new(value.to_hir(errors)))
                }
                ast::Expression::Continue => Expression::Continue,
                ast::Expression::Loop(body) => Expression::Loop(Box::new(body.to_hir(errors))),
                ast::Expression::Error => Expression::Error,
                // Use expressions are removed in the HIR, so this is gathered for origin information and then replaced with a "pass".
                ast::Expression::Import(id) => Expression::Use(id),
            },
            self.1,
        )
    }
}

impl ToHir<Program> for Vec<(ast::Expression, Span)> {
    fn to_hir(self, errors: &mut Errors) -> Program {
        self.into_iter()
            .filter_map(|expression| {
                let expression = expression.to_hir(errors);

                match expression.0 {
                    Expression::Function(function) => {
                        Some((TopLevel::Function(function), expression.1))
                    }
                    Expression::Structure(structure) => {
                        Some((TopLevel::Structure(structure), expression.1))
                    }
                    Expression::Use(id) => Some((TopLevel::Import(id), expression.1)),
                    _ => {
                        errors.insert_error(Error::Unexpected {
                            expected: HashSet::from_iter([Pattern::Construct(
                                "top-level declaration",
                            )]),
                            found: Element {
                                value: Pattern::Construct(expression.0.into()),
                                span: expression.1,
                            },
                            reason: Reason::Unexpected,
                            while_parsing: None,
                        });
                        None
                    }
                }
            })
            .collect()
    }
}

impl ToHir<Module> for ast::Module {
    fn to_hir(self, errors: &mut Errors) -> Module {
        match self {
            ast::Module::Submodule { name, modules } => Module::Submodule {
                name,
                modules: modules
                    .into_iter()
                    .map(|module| module.to_hir(errors))
                    .collect(),
            },
            ast::Module::Program { name, ast } => Module::Program {
                name,
                program: ast.to_hir(errors),
            },
        }
    }
}
