use std::collections::HashMap;

use crate::core::{
    error::Errors,
    types::{
        Constraint, Engine, Fields, IntoTyInfo, LinkReason, Mismatch, ScopeContext, StaticScopes,
        Type, TypeId, Types, Variable,
    },
};
use crate::core::{
    error::{Backtrace, Element, Error, OptElement, TraceElement},
    types::Scopes,
};
use crate::core::{span::Span, types::TypeInfo};
use crate::{
    core::ast::{Id, TypeHint},
    middle_end::hir::AssignLocation,
};

use crate::middle_end::hir::{Expression, Function, Structure, TopLevel};

use super::{
    gather::GatherOut,
    hir::{Module, Program},
};

pub struct Checker<'a> {
    constraints: Vec<Constraint>,
    scopes: StaticScopes,
    engine: Engine,
    errors: &'a mut Errors,
}

/*
You will often encounter unwraps of spans in here. They are here
because all type ids that are being used should be guaranteed to have a span,
as all type ids without a span are generally only used for a single unification.
*/
impl<'a> Checker<'a> {
    fn check(mut self, root_module: Module) -> Types {
        self.check_module(root_module);
        self.solve_constraints();

        for Mismatch { a, b, reason } in self.engine.mismatches.iter().copied() {
            self.errors.insert_error(Error::TypeMismatch {
                a: self.collect_trace(a),
                b: self.collect_trace(b),
                reason,
            })
        }

        self.engine.types
    }

    fn check_module(&mut self, module: Module) {
        self.enter_scope();

        match module {
            Module::Program { program, .. } => {
                self.check_program(program);
            }
            Module::Submodule { modules, .. } => {
                for module in modules {
                    self.check_module(module);
                }
            }
        }
    }

    fn check_expression(
        &mut self,
        expression: (Expression, Span),
        context: ScopeContext,
    ) -> TypeId {
        match expression.0 {
            Expression::Unit => self.engine.insert_type(TypeInfo::Unit, Some(expression.1)),
            Expression::Int(_) => self
                .engine
                .insert_type(TypeInfo::Integer, Some(expression.1)),
            Expression::Boolean(_) => self
                .engine
                .insert_type(TypeInfo::Boolean, Some(expression.1)),
            Expression::String(_) => self
                .engine
                .insert_type(TypeInfo::String, Some(expression.1)),
            Expression::Id(id) => {
                if let Some(symbol) = self.search_id(&id) {
                    self.engine.insert_type(
                        TypeInfo::Link {
                            linked_to: symbol.type_id,
                            reason: LinkReason::Other,
                        },
                        Some(expression.1),
                    )
                } else {
                    self.errors.insert_error(Error::MissingId {
                        id: Element {
                            value: id,
                            span: expression.1.clone(),
                        },
                    });
                    self.engine
                        .insert_type(TypeInfo::Unknown(true), Some(expression.1))
                }
            }
            Expression::Function(function) => self.check_function(function, expression.1),
            Expression::Instance { object, fields } => {
                if let Some(symbol) = self.search_id(&object.0) {
                    let mut field_types: HashMap<Id, (TypeId, Span)> =
                        HashMap::with_capacity(fields.len());

                    for (id, value) in fields {
                        if let Some((_, other_span, ..)) = field_types.get(&id.0) {
                            self.errors.insert_error(Error::ConflictingIds {
                                first: other_span.clone(),
                                second: id.1,
                                id: Id::new_single(id.0),
                            })
                        } else {
                            field_types.insert(id.0, (self.check_expression(value, context), id.1));
                        }
                    }

                    let given_type = self.engine.insert_type(
                        TypeInfo::Structure(
                            field_types
                                .into_iter()
                                .map(|(field, (type_id, _))| (field, type_id))
                                .collect(),
                        ),
                        Some(expression.1.clone()),
                    );

                    self.engine
                        .unify(symbol.type_id, given_type, LinkReason::Structure);

                    self.engine
                        .insert_type(TypeInfo::Instance(symbol.type_id), Some(expression.1))
                } else {
                    self.errors.insert_error(Error::MissingId {
                        id: Element {
                            value: object.0,
                            span: object.1,
                        },
                    });

                    self.engine
                        .insert_type(TypeInfo::Unknown(true), Some(expression.1))
                }
            }
            Expression::Call {
                function,
                parameters,
            } => {
                let parameter_types = parameters
                    .0
                    .into_iter()
                    .map(|parameter| self.check_expression(parameter, context))
                    .collect();

                let return_type = self
                    .engine
                    .insert_type(TypeInfo::Unknown(false), Some(expression.1.clone()));

                let expected_type = self.engine.insert_type(
                    TypeInfo::Function {
                        parameters: parameter_types,
                        return_type,
                    },
                    Some(parameters.1),
                );

                let found_type = self.check_expression(*function, context);
                self.engine
                    .unify(found_type, expected_type, LinkReason::Call);

                return_type
            }
            Expression::Declaration { name, value } => {
                let expected_type = self.check_expression(*value, context);
                let found_type = self
                    .engine
                    .insert_type(name.type_hint.into_ty(&self.scopes), Some(name.value.1));

                self.engine
                    .unify(expected_type, found_type, LinkReason::Other);

                self.scopes.insert_variable(
                    name.value,
                    Variable {
                        type_id: found_type,
                        shadowable: true,
                    },
                );

                self.engine.insert_type(TypeInfo::Unit, Some(expression.1))
            }
            Expression::Assignment { to, from } => {
                let expected_type = self.check_expression(*from, context);
                let found_type = match to.0 {
                    AssignLocation::Variable(id) => {
                        if let Some(symbol) = self.search_id(&id) {
                            self.engine
                                .unify(symbol.type_id, expected_type, LinkReason::Assign);

                            symbol.type_id
                        } else {
                            self.errors.insert_error(Error::MissingId {
                                id: Element {
                                    value: id,
                                    span: to.1.clone(),
                                },
                            });

                            self.engine.insert_type(TypeInfo::Unknown(true), Some(to.1))
                        }
                    }
                    AssignLocation::Field { instance, field } => {
                        let object_id = self.check_expression(*instance, context);
                        let field_id = self
                            .engine
                            .insert_type(TypeInfo::Unknown(false), Some(field.1));

                        self.insert_constraint(Constraint::new(object_id, field_id, field.0));

                        field_id
                    }
                };

                self.engine
                    .unify(found_type, expected_type, LinkReason::Assign);
                self.engine.insert_type(TypeInfo::Unit, Some(expression.1))
            }
            Expression::Access { from, id } => {
                let expression = self.check_expression(*from, context);
                let field_id = self
                    .engine
                    .insert_type(TypeInfo::Unknown(false), Some(id.1));

                self.insert_constraint(Constraint::new(expression, field_id, id.0));

                field_id
            }
            Expression::Block { expressions, tail } => {
                self.enter_scope();

                for expression in expressions {
                    self.check_expression(expression, context);
                }

                let result = self.check_expression(*tail, context);
                self.exit_scope();

                result
            }
            Expression::Structure(structure) => self.check_structure(structure, expression.1),
            Expression::Conditional {
                condition,
                success,
                failure,
            } => {
                let boolean = self.engine.insert_type(TypeInfo::Boolean, None);
                let condition = self.check_expression(*condition, context);

                self.engine.unify(condition, boolean, LinkReason::Condition);

                let success = self.check_expression(*success, context);
                let failure = self.check_expression(*failure, context);

                self.engine.unify(success, failure, LinkReason::Conditional);

                success
            }
            Expression::Break(expression) => {
                let unit = self
                    .engine
                    .insert_type(TypeInfo::Unit, Some(expression.1.clone()));

                if let ScopeContext::FunctionLoop { loop_return, .. } = context {
                    let found_ret_ty = self.check_expression(*expression, context);

                    self.engine
                        .unify(loop_return, found_ret_ty, LinkReason::Loop)
                } else {
                    self.errors.insert_error(Error::InvalidFlow {
                        span: expression.1.clone(),
                        construct: "break",
                    });
                }

                unit
            }
            Expression::Return(expression) => {
                let unit = self
                    .engine
                    .insert_type(TypeInfo::Unit, Some(expression.1.clone()));

                let function_ret_ty = context.get_function_ret_ty();
                let found_return_ty = self.check_expression(*expression, context);

                self.engine
                    .unify(function_ret_ty, found_return_ty, LinkReason::Return);

                unit
            }
            Expression::Continue => {
                if let ScopeContext::Function { .. } = context {
                    self.errors.insert_error(Error::InvalidFlow {
                        span: expression.1.clone(),
                        construct: "continue",
                    });
                }

                self.engine.insert_type(TypeInfo::Unit, Some(expression.1))
            }
            Expression::Loop(body) => {
                let loop_return = self
                    .engine
                    .insert_type(TypeInfo::Unknown(false), Some(expression.1));

                self.check_expression(
                    *body,
                    ScopeContext::FunctionLoop {
                        function_return: context.get_function_ret_ty(),
                        loop_return,
                    },
                )
            }
            Expression::Error => self
                .engine
                .insert_type(TypeInfo::Unknown(true), Some(expression.1)),
            Expression::Use(_) => self.engine.insert_type(TypeInfo::Unit, Some(expression.1)),
        }
    }

    fn check_function(&mut self, function: Function, span: Span) -> TypeId {
        // Parameter will be stored in a new scope, different than the function body scope so I can use `exit_scope` later to remove the parameters.
        self.enter_scope();
        let mut parameters = Vec::with_capacity(function.parameters.len());

        for TypeHint {
            value: (name, span),
            type_hint,
        } in function.parameters
        {
            let type_id = self.engine.insert_type(
                type_hint.into_ty(&self.modules.get(self.current_mod)),
                Some(span),
            );

            parameters.push(type_id);
            self.scopes.insert_variable(
                name,
                Variable {
                    type_id,
                    shadowable: true,
                },
            );
        }

        let given_return_type = self.engine.insert_type(
            function
                .name
                .type_hint
                .into_ty(&self.modules.get(self.current_mod)),
            Some(function.name.value.1.clone()),
        );
        let context = ScopeContext::Function {
            return_type: given_return_type,
        };

        let actual_return_type = self.check_expression(*function.body, context);
        self.engine
            .unify(actual_return_type, given_return_type, LinkReason::Return);

        let function_type = self.engine.insert_type(
            TypeInfo::Function {
                parameters,
                return_type: given_return_type,
            },
            Some(function.name.value.1),
        );

        self.exit_scope();

        self.unify_in_place(&function.name.value.0, function_type);
        self.engine.insert_type(TypeInfo::Unit, Some(span))
    }

    fn check_program(&mut self, program: Program) {
        for top_level in program {
            self.check_top_level(top_level);
        }
    }

    fn check_structure(&mut self, structure: Structure, span: Span) -> TypeId {
        let fields = structure
            .fields
            .into_iter()
            .map(
                |TypeHint {
                     value: data,
                     type_hint,
                 }| {
                    (
                        data.0,
                        self.engine.insert_type(
                            type_hint.into_ty(&self.modules.get(self.current_mod)),
                            Some(data.1),
                        ),
                    )
                },
            )
            .collect();

        let data_type = self
            .engine
            .insert_type(TypeInfo::Structure(fields), Some(span.clone()));
        self.unify_in_place(&structure.name.0, data_type);

        self.engine.insert_type(TypeInfo::Unit, Some(span))
    }

    fn check_top_level(&mut self, top_level: (TopLevel, Span)) -> TypeId {
        match top_level.0 {
            TopLevel::Function(function) => self.check_function(function, top_level.1),
            TopLevel::Structure(structure) => self.check_structure(structure, top_level.1),
            _ => self.engine.insert_type(TypeInfo::Unit, Some(top_level.1)),
        }
    }

    fn collect_trace(&self, mut type_id: TypeId) -> Backtrace {
        let mut backtrace = Backtrace::new();
        let mut reason = LinkReason::Other;

        loop {
            let data_type = self.engine.get(type_id).clone();

            backtrace.0.push(TraceElement {
                reason,
                data_type: OptElement {
                    value: self.into_concrete_ty(type_id),
                    span: data_type.1,
                },
            });

            match data_type.0 {
                TypeInfo::Link {
                    linked_to,
                    reason: next_reason,
                } => {
                    reason = next_reason;
                    type_id = linked_to;
                }
                _ => break,
            }
        }

        backtrace
    }

    fn insert_constraint(&mut self, constraint: Constraint) {
        self.constraints.push(constraint)
    }

    fn into_concrete_ty(&self, type_id: TypeId) -> Type {
        match self.engine.remove_ref(self.engine.get(type_id).0.clone()) {
            TypeInfo::Unknown(_) => Type::Unknown,
            TypeInfo::Reference(type_id) => {
                Type::Reference(Box::new(self.into_concrete_ty(type_id)))
            }
            TypeInfo::Link { .. } => unreachable!(),
            TypeInfo::Unit => Type::Unit,
            TypeInfo::Integer => Type::Integer,
            TypeInfo::Boolean => Type::Boolean,
            TypeInfo::String => Type::String,
            TypeInfo::Structure(fields) => Type::Structure(Fields::new(
                fields
                    .into_iter()
                    .map(|(name, type_id)| (name, self.into_concrete_ty(type_id)))
                    .collect(),
            )),
            TypeInfo::Instance(type_id) => Type::Instance(self.modules.search_type(type_id)),
            TypeInfo::Function {
                parameters,
                return_type,
            } => Type::Function {
                parameters: parameters
                    .into_iter()
                    .map(|type_id| self.into_concrete_ty(type_id))
                    .collect(),
                return_type: Box::new(self.into_concrete_ty(return_type)),
            },
        }
    }

    fn new(scopes: StaticScopes, engine: Engine, errors: &'a mut Errors) -> Self {
        Self {
            scopes,
            engine,
            constraints: Vec::new(),
            errors,
        }
    }

    // The result of the function represents if the constraint was solved or not.
    // Being "solved" means we either found the type of the field or found an error.
    fn solve_constraint(&mut self, constraint: Constraint) -> bool {
        let info = self.engine.get(constraint.object_id).0.clone();
        let access_span = self.engine.get(constraint.field_id).1.clone();

        match self.engine.remove_ref(info) {
            TypeInfo::Unknown(_) => return false,
            TypeInfo::Instance(type_id) => {
                match self.engine.remove_ref(self.engine.get(type_id).0.clone()) {
                    TypeInfo::Structure(fields) => {
                        if let Some(&type_id) = fields.get(&constraint.field) {
                            self.engine
                                .unify(constraint.field_id, type_id, LinkReason::Field);
                            true
                        } else {
                            self.errors.insert_error(Error::MissingField {
                                structure: Element {
                                    value: self.into_concrete_ty(constraint.object_id),
                                    span: access_span.unwrap(),
                                },
                                field_name: constraint.field,
                            });
                            true
                        }
                    }
                    _ => {
                        self.errors.insert_error(Error::MissingField {
                            structure: Element {
                                value: self.into_concrete_ty(constraint.object_id),
                                span: access_span.unwrap(),
                            },
                            field_name: constraint.field,
                        });
                        true
                    }
                }
            }
            _ => {
                self.errors.insert_error(Error::MissingField {
                    structure: Element {
                        value: self.into_concrete_ty(constraint.object_id),
                        span: access_span.unwrap(),
                    },
                    field_name: constraint.field,
                });
                true
            }
        }
    }

    fn solve_constraints(&mut self) {
        // Cursed do-while loop.
        while {
            let mut progressed = false;

            self.constraints = self
                .constraints
                .clone()
                .into_iter()
                .filter(|&constraint| {
                    let result = self.solve_constraint(constraint);
                    progressed = progressed || result;

                    !result
                })
                .collect();

            progressed
        } {}
    }

    fn unify_in_place(&mut self, id: &Id, type_id: TypeId) {
        self.engine.unify(
            self.search_id(id).unwrap().type_id,
            type_id,
            LinkReason::Other,
        );
    }
}

pub fn check(
    root_module: Module,
    GatherOut { engine, scopes }: GatherOut,
    errors: &mut Errors,
) -> Types {
    Checker::new(scopes, engine, errors).check(root_module)
}
