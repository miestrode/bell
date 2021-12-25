use internment::Intern;

use std::collections::HashSet;

use crate::ast::{self, Identifier, Module, PathEnd, Type, WithHint};
use crate::core::error::{Element, Error, Pattern, Reason};
use crate::core::span::Span;

#[derive(Debug, Clone)]
pub struct Function {
    pub name: WithHint<(Identifier, Span)>,
    pub parameters: Vec<WithHint<(Identifier, Span)>>,
    pub body: Box<(Expression, Span)>,
}

#[derive(Debug, Clone)]
pub struct Structure {
    pub name: (Identifier, Span),
    pub fields: Vec<WithHint<(Intern<String>, Span)>>,
}

type Field = ((Intern<String>, Span), (Expression, Span));

#[derive(Clone, Debug)]
pub enum AssignLocation {
    Variable(Identifier),
    Field {
        instance: Box<(Expression, Span)>,
        field: (Intern<String>, Span),
    },
}

#[derive(Debug, Clone)]
pub enum Expression {
    Unit,
    Int(i32),
    Boolean(bool),
    String(Intern<String>),
    Identifier(Identifier),
    Function(Function),
    Instance {
        object: (Identifier, Span),
        fields: Vec<Field>,
    },
    Call {
        function: Box<(Expression, Span)>,
        parameters: Vec<(Expression, Span)>,
    },
    Declaration {
        name: WithHint<(Identifier, Span)>,
        value: Box<(Expression, Span)>,
    },
    Assignment {
        to: (AssignLocation, Span),
        from: Box<(Expression, Span)>,
    },
    Access {
        from: Box<(Expression, Span)>,
        id: (Intern<String>, Span),
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
    Error,
}

impl From<Expression> for &'static str {
    fn from(expression: Expression) -> Self {
        match expression {
            Expression::Unit => "unit",
            Expression::Int(_) => "integer",
            Expression::Boolean(_) => "boolean",
            Expression::String(_) => "string",
            Expression::Identifier(_) => "identifier",
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
            Expression::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
pub enum TopLevel {
    Function(Function),
    Structure(Structure),
}

#[derive(Debug)]
struct Imports(Vec<Vec<(Intern<String>, Identifier)>>);

impl Imports {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn enter_module(&mut self) {
        self.0.push(Vec::new());
    }

    fn exit_module(&mut self) {
        self.0.pop();
    }

    fn insert(&mut self, name: Intern<String>, origin: Identifier) {
        self.0.last_mut().unwrap().push((name, origin));
    }

    fn search(&self, name: Intern<String>) -> Option<Identifier> {
        self.0
            .iter()
            .cloned()
            .flatten()
            .rev()
            .find_map(|(other, origin)| if other == name { Some(origin) } else { None })
    }
}

struct Transformer {
    items: Imports,
    current: Identifier,
    errors: Vec<Error>,
}

impl Transformer {
    fn new() -> Self {
        Self {
            items: Imports::new(),
            current: Identifier(Vec::new()),
            errors: Vec::new(),
        }
    }

    fn resolve_local(&mut self, local: (Intern<String>, Span)) -> (Identifier, Span) {
        self.items.insert(local.0, self.current.clone());
        self.resolve_id(local.0.into(), local.1)
    }

    // Note that this will not work with locals (It will work with an imported path of length 1 though).
    // When using locals the `local_to_absolute` function should be used.
    fn resolve_id(&mut self, id: Identifier, span: Span) -> (Identifier, Span) {
        if let Some(mut origin) = self.items.search(id.0[0]) {
            origin.0.extend(id.0);

            // The origin must be fully qualified.
            (origin, span)
        } else {
            self.errors.push(Error::MissingId {
                id: Element {
                    data: id.clone(),
                    span: span.clone(),
                },
            });

            (id, span)
        }
    }

    fn resolve_type(&mut self, data_type: Type) -> Type {
        match data_type {
            Type::Structure(id) => Type::Structure(self.resolve_id(id, Default::default()).0),
            Type::Reference(data_type) => self.resolve_type(*data_type),
            _ => data_type,
        }
    }

    fn resolve_type_hint(&mut self, type_hint: Option<(Type, Span)>) -> Option<(Type, Span)> {
        type_hint.map(|(data_type, span)| (self.resolve_type(data_type), span))
    }

    fn transform_expression(&mut self, expression: (ast::Expression, Span)) -> (Expression, Span) {
        (
            match expression.0 {
                ast::Expression::Int(value) => Expression::Int(value),
                ast::Expression::Boolean(value) => Expression::Boolean(value),
                ast::Expression::String(value) => Expression::String(value),
                ast::Expression::Identifier(id) => {
                    return {
                        let (id, span) = self.resolve_id(id, expression.1.clone());
                        (Expression::Identifier(id), span)
                    }
                }
                ast::Expression::Function {
                    name,
                    parameters,
                    body,
                } => Expression::Function(Function {
                    name: WithHint {
                        data: self.resolve_local(name.data),
                        type_hint: self.resolve_type_hint(name.type_hint),
                    },
                    parameters: parameters
                        .into_iter()
                        .map(|parameter| WithHint {
                            data: self.resolve_local(parameter.data),
                            type_hint: self.resolve_type_hint(parameter.type_hint),
                        })
                        .collect(),
                    body: Box::new(self.transform_expression(*body)),
                }),
                ast::Expression::Instance { object, fields } => Expression::Instance {
                    object: self.resolve_id(object.0, object.1),
                    fields: fields
                        .into_iter()
                        .map(|(name, expression)| (name, self.transform_expression(expression)))
                        .collect(),
                },
                ast::Expression::Call {
                    function,
                    parameters,
                } => Expression::Call {
                    function: Box::new(self.transform_expression(*function)),
                    parameters: parameters
                        .into_iter()
                        .map(|expression| self.transform_expression(expression))
                        .collect(),
                },
                ast::Expression::Declaration { name, value } => Expression::Declaration {
                    name: WithHint {
                        data: self.resolve_local(name.data),
                        type_hint: self.resolve_type_hint(name.type_hint),
                    },
                    value: Box::new(self.transform_expression(*value)),
                },
                ast::Expression::Assignment { to, from } => {
                    let to = self.transform_expression(*to);

                    (|| Expression::Assignment {
                        to: (
                            match to.0 {
                                Expression::Identifier(id) => AssignLocation::Variable(id),
                                Expression::Access { from, id } => AssignLocation::Field {
                                    instance: from,
                                    field: id,
                                },
                                _ => {
                                    self.errors.push(Error::InvalidAssign(Element {
                                        data: to.0.into(),
                                        span: to.1.clone(),
                                    }));

                                    return Expression::Error; // A kind of early return.
                                }
                            },
                            to.1,
                        ),
                        from: Box::new(self.transform_expression(*from)),
                    })()
                }
                ast::Expression::Access { from, id } => Expression::Access {
                    from: Box::new(self.transform_expression(*from)),
                    id,
                },
                ast::Expression::Block { expressions, tail } => Expression::Block {
                    expressions: expressions
                        .into_iter()
                        .map(|expression| self.transform_expression(expression))
                        .collect(),
                    tail: Box::new(
                        tail.map(|expression| self.transform_expression(*expression))
                            .unwrap_or_else(|| (Expression::Unit, Default::default())),
                    ),
                },
                ast::Expression::Structure { name, fields } => Expression::Structure(Structure {
                    name: self.resolve_local(name),
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
                        condition: Box::new(self.transform_expression(condition)),
                        failure: Box::new(self.transform_expression(
                            if let Some(span) = rest_span {
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
                            },
                        )),
                        success: Box::new(self.transform_expression(branch)),
                    }
                }
                ast::Expression::Break(value) => {
                    Expression::Break(Box::new(self.transform_expression(*value)))
                }
                ast::Expression::Return(value) => {
                    Expression::Return(Box::new(self.transform_expression(*value)))
                }
                ast::Expression::Continue => Expression::Continue,
                ast::Expression::Loop(body) => {
                    Expression::Loop(Box::new(self.transform_expression(*body)))
                }
                ast::Expression::Error => Expression::Error,
                // Use expressions are removed in the HIR, so this is gathered for origin information and then replaced with a "pass".
                ast::Expression::Use(path) => {
                    for (tail, span) in path.tail {
                        match tail {
                            PathEnd::Path(path) => {
                                self.transform_expression((ast::Expression::Use(path), span));
                            }
                            end => {
                                let (id, span) = path.body.clone();

                                // In either case these path ends are all imported from some kind of module and modules always have absolute origin paths.
                                let (body, _) = self.resolve_id(id, span);

                                match end {
                                    PathEnd::This => {
                                        self.items.insert(*path.body.0 .0.last().unwrap(), body)
                                    }
                                    PathEnd::End(item) => self.items.insert(item, body),
                                    _ => unreachable!(),
                                }
                            }
                        }
                    }

                    Expression::Unit
                }
            },
            expression.1,
        )
    }

    fn transform_module(&mut self, module: Module) -> Vec<(TopLevel, Span)> {
        self.items.enter_module();

        let module = match module {
            Module::Module { name, entries } => {
                self.current.0.push(name);

                // These should be accessible throughout all the sub-modules.
                for entry in entries.iter() {
                    let name = match *entry {
                        Module::Module { name, .. } => name,
                        Module::Program { name, .. } => name,
                    };

                    let mut location = self.current.clone();

                    if self.items.search(name).is_some() {
                        // If we are here, a module with this same name has been defined before.
                        self.errors.push(Error::ConflictingModules {
                            first: self.items.search(name).unwrap(),
                            second: {
                                location.0.push(name);
                                location
                            },
                        })
                    } else {
                        self.items.insert(name, location);
                    }
                }

                entries
                    .into_iter()
                    .map(|entry| self.transform_module(entry))
                    .flatten()
                    .collect()
            }
            Module::Program { ast, name } => {
                self.current.0.push(name);

                ast.into_iter()
                    .filter_map(|expression| {
                        let (expression, span) = self.transform_expression(expression);

                        Some((
                            match expression {
                                Expression::Function(function) => TopLevel::Function(function),
                                Expression::Structure(structure) => TopLevel::Structure(structure),
                                // Pass expressions only come from what was previously a use expression, so these are allowed.
                                Expression::Unit => return None,
                                _ => {
                                    self.errors.push(Error::Unexpected {
                                        expected: HashSet::from([
                                            Pattern::Construct("function"),
                                            Pattern::Construct("structure"),
                                            Pattern::Construct("import"),
                                        ]),
                                        found: Element {
                                            data: Pattern::Construct("expression"),
                                            span,
                                        },
                                        reason: Reason::Unexpected,
                                        while_parsing: None,
                                    });

                                    return None;
                                }
                            },
                            span,
                        ))
                    })
                    .collect()
            }
        };

        self.current.0.pop();
        self.items.exit_module();

        module
    }
}

pub fn to_hir(module: Module) -> (Vec<(TopLevel, Span)>, Vec<Error>) {
    let mut transformer = Transformer::new();
    let module = transformer.transform_module(module);

    (module, transformer.errors)
}
