use internment::Intern;

use std::{collections::HashMap, fmt::Debug};

use crate::core::error::{Element, Error};
use crate::core::span::Span;
use crate::{
    core::ast::{self, Identifier, WithHint},
    middle_end::hir::AssignLocation,
};

use crate::middle_end::hir::{Expression, Function, Structure, TopLevel};

pub type TypeId = usize;

#[derive(Clone, Debug)]
pub enum TypeInfo {
    Unknown(bool),
    Reference(TypeId),
    // A link physically links two types together, all information discovered on one is discovered on another.
    // It's different than references, since those are talking about actual object references.
    Link(TypeId),
    Unit,
    Integer,
    Boolean,
    String,
    Structure(HashMap<Intern<String>, TypeId>),
    Instance(TypeId),
    Function {
        parameters: Vec<TypeId>,
        return_type: TypeId,
    },
}

trait IntoTyInfo {
    fn into_ty(self, scopes: &Scopes, current: ScopeId) -> TypeInfo;
}

impl IntoTyInfo for ast::Type {
    fn into_ty(self, scopes: &Scopes, current: ScopeId) -> TypeInfo {
        match self {
            ast::Type::Integer => TypeInfo::Integer,
            ast::Type::Boolean => TypeInfo::Boolean,
            ast::Type::String => TypeInfo::String,
            ast::Type::Structure(id) => {
                if let Some(Symbol { type_id, .. }) = scopes.search(&id, current) {
                    TypeInfo::Instance(type_id)
                } else {
                    // No need to report this as an error, this should ALWAYS be reported.
                    // TODO: verfiy this.
                    TypeInfo::Unknown(true)
                }
            }
            ast::Type::Reference(data_type) => (*data_type).into_ty(scopes, current),
        }
    }
}

impl IntoTyInfo for Option<(ast::Type, Span)> {
    fn into_ty(self, scopes: &Scopes, current: ScopeId) -> TypeInfo {
        self.map(|(data_type, _)| match data_type {
            ast::Type::Integer => TypeInfo::Integer,
            ast::Type::Boolean => TypeInfo::Boolean,
            ast::Type::String => TypeInfo::String,
            ast::Type::Structure(id) => {
                if let Some(Symbol { type_id, .. }) = scopes.search(&id, current) {
                    TypeInfo::Instance(type_id)
                } else {
                    // No need to report this as an error, this should ALWAYS be reported.
                    // TODO: verfiy this.
                    TypeInfo::Unknown(true)
                }
            }
            ast::Type::Reference(data_type) => (*data_type).into_ty(scopes, current),
        })
        .unwrap_or_else(|| TypeInfo::Unknown(false))
    }
}

pub enum Type {
    Unknown,
    Reference(Box<Type>),
    Unit,
    Integer,
    Boolean,
    String,
    Structure(HashMap<Intern<String>, Type>),
    Instance(Identifier),
    Function {
        parameters: Vec<Type>,
        return_type: Box<Type>,
    },
}

impl Debug for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => write!(f, "?"),
            Self::Reference(data_type) => write!(f, "&{:?}", data_type),
            Self::Unit => write!(f, "Unit"),
            Self::Integer => write!(f, "Int"),
            Self::Boolean => write!(f, "Bool"),
            Self::String => write!(f, "Str"),
            Self::Structure(fields) => write!(f, "{:?}", fields),
            Self::Instance(id) => write!(f, "{}", id),
            Self::Function {
                parameters,
                return_type,
            } => write!(
                f,
                "({}) |-> {:?}",
                parameters
                    .iter()
                    .map(|parameter| format!("{:?}", parameter))
                    .collect::<Vec<_>>()
                    .join(", "),
                return_type
            ),
        }
    }
}

#[derive(Clone)]
struct Mismatch(TypeId, TypeId);

// For now all constraints are field constraints, I don't really know if I'll need more
// constraints for anything else, but for now this is fine.
// TODO: Consider adding support for constraint trees, that would store which constraints effect others!
#[derive(Clone, Copy)]
struct Constraint {
    object_id: TypeId,
    field_id: TypeId,
    field: Intern<String>,
}

impl Constraint {
    fn new(object_id: TypeId, field_id: TypeId, field: Intern<String>) -> Self {
        Self {
            object_id,
            field_id,
            field,
        }
    }
}

type ScopeId = usize;

#[derive(Debug)]
pub struct Scope {
    items: Vec<(Identifier, Symbol)>,
    previous: Option<ScopeId>,
}

#[derive(Debug, Clone)]
struct Symbol {
    type_id: TypeId,
    shadowable: bool,
}

impl Scope {
    fn new(previous: Option<ScopeId>) -> Self {
        Self {
            items: Vec::new(),
            previous,
        }
    }

    fn search(&self, id: &Identifier) -> Option<Symbol> {
        self.items.iter().rev().find_map(|(other, symbol)| {
            if id == other {
                Some(symbol.clone())
            } else {
                None
            }
        })
    }

    fn inv_search(&self, type_id: TypeId) -> Option<Identifier> {
        self.items.iter().rev().find_map(|(id, symbol)| {
            if type_id == symbol.type_id {
                Some(id.clone())
            } else {
                None
            }
        })
    }

    fn insert(&mut self, id: Identifier, type_id: TypeId, shadowable: bool) {
        self.items.push((
            id,
            Symbol {
                type_id,
                shadowable,
            },
        ));
    }
}

struct Scopes(Vec<Scope>);

impl Scopes {
    fn new() -> Self {
        Scopes(vec![Scope::new(None)])
    }

    // This isn't implemented in the same manner as `inv_search` since it's more useful to search using the scope trees instead of the scope list.
    // Type identifiers however are unique, so we can iterate over the scope list.
    fn search(&self, id: &Identifier, starting: ScopeId) -> Option<Symbol> {
        let scope = &self.0[starting];

        scope.search(id).or_else(|| {
            scope
                .previous
                .and_then(|scope_id| self.search(id, scope_id))
        })
    }

    fn inv_search(&self, type_id: TypeId) -> Option<Identifier> {
        self.0.iter().find_map(|scope| scope.inv_search(type_id))
    }
}

struct Checker {
    scopes: Scopes,
    constraints: Vec<Constraint>,
    types: Vec<(TypeInfo, Span)>,
    mismatches: Vec<Mismatch>,
    current: ScopeId,
    errors: Vec<Error>,
}

impl Checker {
    fn new() -> Self {
        Self {
            scopes: Scopes::new(),
            types: Vec::new(),
            mismatches: Vec::new(),
            constraints: Vec::new(),
            current: 0,
            errors: Vec::new(),
        }
    }

    fn generate_concrete(&self, type_id: TypeId, scopes: &Scopes) -> Type {
        match self.types[type_id].0.clone() {
            TypeInfo::Unknown(_) => Type::Unknown,
            TypeInfo::Reference(type_id) => {
                Type::Reference(Box::new(self.generate_concrete(type_id, scopes)))
            }
            TypeInfo::Link(type_id) => self.generate_concrete(type_id, scopes),
            TypeInfo::Unit => Type::Unit,
            TypeInfo::Integer => Type::Integer,
            TypeInfo::Boolean => Type::Boolean,
            TypeInfo::String => Type::String,
            TypeInfo::Structure(fields) => Type::Structure(
                fields
                    .into_iter()
                    .map(|(name, type_id)| (name, self.generate_concrete(type_id, scopes)))
                    .collect(),
            ),
            TypeInfo::Instance(id) => Type::Instance(scopes.inv_search(id).unwrap()),
            TypeInfo::Function {
                parameters,
                return_type,
            } => Type::Function {
                parameters: parameters
                    .into_iter()
                    .map(|type_id| self.generate_concrete(type_id, scopes))
                    .collect(),
                return_type: Box::new(self.generate_concrete(return_type, scopes)),
            },
        }
    }

    fn insert_type(&mut self, data_type: TypeInfo, span: Span) -> TypeId {
        let id = self.types.len();
        self.types.push((data_type, span));

        id
    }

    fn unify(&mut self, a: TypeId, b: TypeId) {
        match (self.types[a].0.clone(), self.types[b].0.clone()) {
            // Follow any links.
            (TypeInfo::Link(a), _) => self.unify(a, b),
            (_, TypeInfo::Link(b)) => self.unify(a, b),
            // Overwrite unknowns.
            (TypeInfo::Unknown { .. }, _) => self.types[a].0 = TypeInfo::Link(b),
            (_, TypeInfo::Unknown { .. }) => self.types[b].0 = TypeInfo::Link(a),
            (TypeInfo::Integer, TypeInfo::Integer) => (),
            (TypeInfo::Boolean, TypeInfo::Boolean) => (),
            (TypeInfo::String, TypeInfo::String) => (),
            (TypeInfo::Reference(a), TypeInfo::Reference(b)) => self.unify(a, b),
            // All identifiers at this point are absolute, so this code holds.
            (TypeInfo::Instance(id), TypeInfo::Instance(other)) if id == other => (),
            (TypeInfo::Structure(fields_a), TypeInfo::Structure(fields_b)) => {
                for (field, data_type) in fields_a {
                    if let Some(&other) = fields_b.get(&field) {
                        self.unify(data_type, other);
                    } else {
                        self.mismatches.push(Mismatch(a, b));

                        return;
                    }
                }
            }
            (
                TypeInfo::Function {
                    parameters: parameters_a,
                    return_type: return_type_a,
                },
                TypeInfo::Function {
                    parameters: parameters_b,
                    return_type: return_type_b,
                },
            ) if parameters_a.len() == parameters_b.len() => {
                for (a, b) in parameters_a.into_iter().zip(parameters_b) {
                    self.unify(a, b);
                }

                self.unify(return_type_a, return_type_b);
            }
            _ => self.mismatches.push(Mismatch(a, b)),
        }
    }

    fn remove_ref(&mut self, type_info: TypeInfo) -> TypeInfo {
        match type_info {
            TypeInfo::Reference(type_id) => self.remove_ref(self.types[type_id].0.clone()),
            _ => type_info,
        }
    }

    fn search(&self, id: &Identifier, scope_id: ScopeId) -> Option<Symbol> {
        let scope = &self.scopes.0[scope_id];

        scope.search(id).or_else(|| {
            self.search(
                id,
                match scope.previous {
                    Some(scope_id) => scope_id,
                    None => return None,
                },
            )
        })
    }

    fn insert_variable(&mut self, id: Identifier, type_id: TypeId, shadowable: bool) {
        let current = &mut self.scopes.0[self.current];

        match current.search(&id) {
            Some(symbol) if !symbol.shadowable => self.errors.push(Error::Conflicting {
                first: self.types[type_id].1.clone(),
                second: self.types[symbol.type_id].1.clone(),
                id,
            }),
            _ => current.insert(id, type_id, shadowable),
        }
    }

    fn unify_in_place(&mut self, id: &Identifier, type_id: TypeId) {
        self.unify(self.search(id, self.current).unwrap().type_id, type_id);
    }

    fn create_scope(&mut self, connected: bool) {
        let scope_id = self.scopes.0.len();

        self.scopes.0.push(Scope::new(if connected {
            Some(self.current)
        } else {
            None
        }));

        self.current = scope_id;
    }

    // Will attempt to get to the previous scope unless one doesn't exist.
    fn exit_scope(&mut self) {
        self.current = self.scopes.0[self.current].previous.unwrap();
    }

    fn insert_constraint(&mut self, constraint: Constraint) {
        self.constraints.push(constraint)
    }

    fn solve_constraint(&mut self, constraint: Constraint) -> bool {
        let (info, span) = self.types[constraint.object_id].clone();

        match self.remove_ref(info) {
            TypeInfo::Unknown(_) => return false,
            TypeInfo::Instance(type_id) => match &self.types[type_id].0 {
                TypeInfo::Structure(fields) => {
                    if let Some(&type_id) = fields.get(&constraint.field) {
                        self.unify(constraint.field_id, type_id);

                        true
                    } else {
                        self.errors.push(Error::MissingField {
                            data_type: Element {
                                data: self.generate_concrete(type_id, &self.scopes),
                                span,
                            },
                            field_name: constraint.field,
                        });

                        false
                    }
                }
                _ => {
                    self.errors.push(Error::MissingField {
                        data_type: Element {
                            data: self.generate_concrete(constraint.object_id, &self.scopes),
                            span,
                        },
                        field_name: constraint.field,
                    });

                    false
                }
            },
            _ => {
                self.errors.push(Error::MissingField {
                    data_type: Element {
                        data: self.generate_concrete(constraint.object_id, &self.scopes),
                        span,
                    },
                    field_name: constraint.field,
                });

                false
            }
        }
    }

    fn gather_expression(&mut self, expression: &(Expression, Span)) {
        match &expression.0 {
            Expression::Function(function) => self.gather_function(function),
            Expression::Instance { fields, .. } => {
                for field in fields {
                    self.gather_expression(&field.1);
                }
            }
            Expression::Call {
                function,
                parameters,
            } => {
                self.gather_expression(function);

                for parameter in parameters {
                    self.gather_expression(parameter);
                }
            }
            // Variables are not added in at this stage.
            Expression::Declaration { value, .. } => self.gather_expression(value),
            Expression::Assignment { to, from } => {
                if let AssignLocation::Field { instance, .. } = &to.0 {
                    self.gather_expression(instance);
                }

                self.gather_expression(from);
            }
            Expression::Access { from, .. } => self.gather_expression(from),
            Expression::Block { expressions, tail } => {
                self.create_scope(true);

                for expression in expressions {
                    self.gather_expression(expression);
                }

                self.gather_expression(tail);
                self.exit_scope();
            }
            Expression::Structure(structure) => self.gather_structure(structure),
            Expression::Conditional {
                condition,
                success,
                failure,
            } => {
                self.gather_expression(condition);
                self.gather_expression(success);
                self.gather_expression(failure);
            }
            Expression::Break(value) => self.gather_expression(value),
            Expression::Return(value) => self.gather_expression(value),
            Expression::Loop(body) => self.gather_expression(body),
            _ => (), // Some expression variants don't contain expressions or produce items.
        }
    }

    fn gather_function(&mut self, function: &Function) {
        // This is always a block expression.
        self.gather_expression(&function.body);

        let unknown = self.insert_type(TypeInfo::Unknown(false), function.name.data.1.clone());

        self.insert_variable(function.name.data.0.clone(), unknown, true);
    }

    fn gather_structure(&mut self, structure: &Structure) {
        let unknown = self.insert_type(TypeInfo::Unknown(false), structure.name.1.clone());
        self.insert_variable(structure.name.0.clone(), unknown, true);
    }

    fn gather_top_level(&mut self, top_level: &(TopLevel, Span)) {
        match &top_level.0 {
            TopLevel::Function(function) => self.gather_function(function),
            TopLevel::Structure(structure) => self.gather_structure(structure),
        }
    }

    fn check_function(&mut self, function: Function, span: Span) -> TypeId {
        // Parameter will be stored in a new scope, different than the function body scope so I can use `exit_scope` later to remove the parameters.
        self.create_scope(true);
        let mut parameters = Vec::with_capacity(function.parameters.len());

        for WithHint {
            data: (id, span),
            type_hint,
        } in function.parameters
        {
            let type_id = self.insert_type(type_hint.into_ty(&self.scopes, self.current), span);

            parameters.push(type_id);
            self.insert_variable(id, type_id, false);
        }

        let actual_return_type = self.check_expression(*function.body);
        let given_return_type = self.insert_type(
            function.name.type_hint.into_ty(&self.scopes, self.current),
            function.name.data.1.clone(),
        );

        self.unify(actual_return_type, given_return_type);

        let function_type = self.insert_type(
            TypeInfo::Function {
                parameters,
                return_type: actual_return_type,
            },
            function.name.data.1,
        );

        self.exit_scope();
        self.unify_in_place(&function.name.data.0, function_type);

        self.insert_type(TypeInfo::Unit, span)
    }

    fn check_structure(&mut self, structure: Structure, span: Span) -> TypeId {
        let fields = structure
            .fields
            .into_iter()
            .map(|WithHint { data, type_hint }| {
                (
                    data.0,
                    self.insert_type(type_hint.into_ty(&self.scopes, self.current), data.1),
                )
            })
            .collect();

        self.insert_type(TypeInfo::Structure(fields), span)
    }

    fn check_expression(&mut self, expression: (Expression, Span)) -> TypeId {
        match expression.0 {
            Expression::Unit => self.insert_type(TypeInfo::Unit, expression.1),
            Expression::Int(_) => self.insert_type(TypeInfo::Integer, expression.1),
            Expression::Boolean(_) => self.insert_type(TypeInfo::Boolean, expression.1),
            Expression::String(_) => self.insert_type(TypeInfo::String, expression.1),
            Expression::Identifier(id) => {
                if let Some(symbol) = self.search(&id, self.current) {
                    symbol.type_id
                } else {
                    self.errors.push(Error::MissingId {
                        id: Element {
                            data: id,
                            span: expression.1.clone(),
                        },
                    });

                    self.insert_type(TypeInfo::Unknown(true), expression.1)
                }
            }
            Expression::Function(function) => self.check_function(function, expression.1),
            Expression::Instance { object, fields } => {
                if let Some(symbol) = self.search(&object.0, self.current) {
                    let mut field_types: HashMap<Intern<String>, (TypeId, Span)> =
                        HashMap::with_capacity(fields.len());

                    for (id, value) in fields {
                        if let Some((_, other_span, ..)) = field_types.get(&id.0) {
                            self.errors.push(Error::Conflicting {
                                first: other_span.clone(),
                                second: id.1,
                                id: Identifier::new_single(id.0),
                            })
                        } else {
                            field_types.insert(id.0, (self.check_expression(value), id.1));
                        }
                    }

                    let given_type = self.insert_type(
                        TypeInfo::Structure(
                            field_types
                                .into_iter()
                                .map(|(field, (type_id, _))| (field, type_id))
                                .collect(),
                        ),
                        expression.1,
                    );

                    self.unify(symbol.type_id, given_type);

                    symbol.type_id
                } else {
                    self.errors.push(Error::MissingId {
                        id: Element {
                            data: object.0,
                            span: object.1,
                        },
                    });

                    self.insert_type(TypeInfo::Unknown(true), expression.1)
                }
            }
            Expression::Call {
                function,
                parameters,
            } => {
                let parameter_types = parameters
                    .into_iter()
                    .map(|parameter| self.check_expression(parameter))
                    .collect();

                let return_type = self.insert_type(TypeInfo::Unknown(false), expression.1.clone());

                let expected_type = self.insert_type(
                    TypeInfo::Function {
                        parameters: parameter_types,
                        return_type,
                    },
                    expression.1,
                );
                let found_type = self.check_expression(*function);

                self.unify(found_type, expected_type);
                return_type
            }
            Expression::Declaration { name, value } => {
                let expected_type = self.check_expression(*value);
                let found_type = self.insert_type(
                    name.type_hint.into_ty(&self.scopes, self.current),
                    name.data.1,
                );
                self.unify(expected_type, found_type);

                self.insert_variable(name.data.0, found_type, true);
                self.insert_type(TypeInfo::Unit, expression.1)
            }
            Expression::Assignment { to, from } => {
                let expected_type = self.check_expression(*from);
                let found_type = match to.0 {
                    AssignLocation::Variable(id) => {
                        if let Some(symbol) = self.search(&id, self.current) {
                            self.unify(symbol.type_id, expected_type);

                            symbol.type_id
                        } else {
                            self.errors.push(Error::MissingId {
                                id: Element {
                                    data: id,
                                    span: to.1.clone(),
                                },
                            });

                            self.insert_type(TypeInfo::Unknown(true), to.1)
                        }
                    }
                    AssignLocation::Field { instance, field } => {
                        let object_id = self.check_expression(*instance);
                        let field_id = self.insert_type(TypeInfo::Unknown(false), field.1);

                        self.insert_constraint(Constraint::new(object_id, field_id, field.0));

                        field_id
                    }
                };

                self.unify(found_type, expected_type);
                self.insert_type(TypeInfo::Unit, expression.1)
            }
            Expression::Access { from, id } => {
                let expression = self.check_expression(*from);
                let field_id = self.insert_type(TypeInfo::Unknown(false), id.1);

                self.insert_constraint(Constraint::new(expression, field_id, id.0));

                field_id
            }
            Expression::Block { expressions, tail } => {
                for expression in expressions {
                    self.check_expression(expression);
                }

                self.check_expression(*tail)
            }
            Expression::Structure(structure) => self.check_structure(structure, expression.1),
            Expression::Conditional { condition, success, failure } => {
                // TODO: Figure out what span to use in this case. Maybe even rework the whole span system for types and errors alike.
                let boolean = self.insert_type(TypeInfo::Boolean, Default::default());
                let condition = self.check_expression(*condition);

                self.unify(condition, boolean);

                let success = self.check_expression(*success);
                let failure = self.check_expression(*failure);

                self.unify(success, failure);

                success
            },
            // When checking breaks and returns explicitly from this function, we report an error,
            // since we don't have the context to find the enclosing scope.
            // Instead, when you need to check breaks and returns, extract the expression and manually place it in here. 
            Expression::Break(_) => todo!(),
            Expression::Return(_) => todo!(),
            Expression::Continue => todo!(),
            Expression::Loop(_) => todo!(),
            Expression::Error => todo!(),
        }
    }

    fn check_top_level(&mut self, top_level: (TopLevel, Span)) -> TypeId {
        match top_level.0 {
            TopLevel::Function(function) => self.check_function(function, top_level.1),
            TopLevel::Structure(_) => todo!(),
        }
    }

    fn solve_constraints(&mut self) {
        while self
            .constraints
            .clone()
            .into_iter()
            .map(|constraint| self.solve_constraint(constraint))
            .reduce(|previous, current| previous || current)
            .unwrap()
        {}
    }
}

pub fn check(program: Vec<(TopLevel, Span)>) -> Result<Vec<(TypeInfo, Span)>, Vec<Error>> {
    let mut gatherer = Checker::new();

    for declaration in &program {
        gatherer.gather_top_level(declaration);
    }

    for declaration in program {
        gatherer.check_top_level(declaration);
    }

    // TODO: Remove this `.clone()` by moving the `generate_concrete` method into another type when this becomes too slow.
    for mismatch in gatherer.mismatches.clone().into_iter() {
        gatherer.errors.push(Error::TypeMismatch {
            a: Element {
                data: gatherer.generate_concrete(mismatch.0, &gatherer.scopes),
                span: gatherer.types[mismatch.0].1.clone(),
            },
            b: Element {
                data: gatherer.generate_concrete(mismatch.1, &gatherer.scopes),
                span: gatherer.types[mismatch.1].1.clone(),
            },
        })
    }

    gatherer.solve_constraints();

    if gatherer.errors.is_empty() {
        Ok(gatherer.types)
    } else {
        Err(gatherer.errors)
    }
}
