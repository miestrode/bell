use crate::core::{
    ast::Id,
    error::{Element, Error, Errors},
    span::Span,
    types::{Engine, LinkReason, ScopeConnection, Scopes, TypeInfo, Variable},
};

use super::hir::{
    self, AssignLocation, Expression, Function, Module, Program, Structure, TopLevel,
};

struct Gatherer<'a> {
    scopes: Scopes,
    engine: Engine,
    errors: &'a mut Errors,
}

impl<'a> Gatherer<'a> {
    fn new(errors: &'a mut Errors) -> Self {
        Gatherer {
            scopes: Scopes::new(),
            engine: Engine::new(),
            errors,
        }
    }

    // This code assumes the module has already been inserted into the scope tree, and is just populating it.
    fn gather_module(&mut self, module: &Module) {
        match module {
            Module::Program { name, program } => {
                // We store the previous scope since we need to modify the module scope which is disconnected.
                let previous_scope = self.scopes.current;

                /*
                This search will always work for two reasons:

                1. This code can't go back to an earlier scope since the current scope is ALWAYS one of a module,
                so it is always disconnected from the other scopes.
                2. The module is assumed to be located at the current scope,
                since a requirment of this function is that we already inserted it.
                */
                self.scopes.current = self.scopes.search_module(name).unwrap();

                self.gather_program(program);

                self.scopes.current = previous_scope;
            }
            Module::Submodule { name, modules } => {
                // We only gather imports in programs, so theres no need to keep track of the previous scope in here.
                for module in modules {
                    self.gather_module(module);
                }
            }
        }
    }

    // We need to first insert all modules, before populating them so that imports will work.
    // The problem is that imports can refer to other modules that haven't been populated yet, hence this code.
    fn insert_hir_module(&mut self, module: &Module) {
        // This is the scope of the module that will later be populated.
        let module_scope = self
            .scopes
            .create_scope(ScopeConnection::Exclusive(self.scopes.current));

        match module {
            Module::Program { name, program } => {
                self.gather_program(program);

                self.scopes.exit_current_scope();

                self.scopes.insert_module(name, module_scope);
            }
            Module::Submodule { name, modules } => {
                for module in modules {
                    self.insert_hir_module(module);
                }

                self.scopes.exit_current_scope();

                self.scopes.insert_module(name, module_scope);
            }
        }
    }

    fn gather_program(&mut self, program: &Program) {
        for top_level in program {
            self.gather_top_level(top_level);
        }
    }

    fn gather_top_level(&mut self, top_level: &(TopLevel, Span)) {
        match &top_level.0 {
            TopLevel::Function(function) => self.gather_function(function),
            TopLevel::Structure(structure) => self.gather_structure(structure),
            TopLevel::Import((id, span)) => self.gather_import(id, span),
        }
    }

    // This function actually gives priority to modules when importing IDs, however that shouldn't matter,
    // since the only situation when this could occur,
    // already disallows having modules have names already used by variables.
    fn gather_import(&mut self, id: &Id, span: &Span) {
        if self
            .scopes
            .get_id_origin_module(id)
            .and_then(|origin_module| {
                let id_tail = *id.0.last().unwrap();

                if let Some(scope_id) = self.scopes.raw_scopes.search_module(id_tail, origin_module)
                {
                    self.scopes.insert_module(id_tail, scope_id);
                    Some(())
                } else if let Some(variable) = self
                    .scopes
                    .raw_scopes
                    .search_variable(id_tail, origin_module)
                {
                    self.scopes.insert_variable(
                        id_tail,
                        Variable {
                            type_id: self.engine.insert_type(
                                TypeInfo::Link {
                                    linked_to: variable.type_id,
                                    reason: LinkReason::Other,
                                },
                                Some(span.clone()),
                            ),
                            shadowable: false,
                        },
                    );

                    Some(())
                } else {
                    None
                }
            })
            .is_none()
        {
            self.errors.insert_error(Error::MissingId {
                id: Element {
                    value: id,
                    span: span.clone(),
                },
            });
        }
    }

    fn gather_structure(&mut self, structure: &Structure) {
        let (name, span) = structure.name.clone();

        if self.scopes.does_already_exist(name) {
            self.errors.insert_error(Error::ConflictingIds {
                first: span,
                second: todo!(),
                id: todo!(),
            })
        }
        self.scopes.insert_variable(
            name,
            Variable {
                type_id: self.engine.insert_type(TypeInfo::Unknown(true), Some(span)),
                shadowable: false,
            },
        );
    }

    fn gather_function(&mut self, function: &Function) {
        let (id, span) = function.name.value;

        self.scopes.insert_variable(
            id,
            Variable {
                type_id: self.engine.insert_type(TypeInfo::Unknown(true), Some(span)),
                shadowable: false,
            },
        );

        self.scopes.enter_scope();
        self.gather_expression(&function.body);
        self.scopes.exit_current_scope();
    }

    fn gather_expression(&mut self, expression: &(Expression, Span)) {
        match &expression.0 {
            Expression::Function(function) => self.gather_function(function),
            Expression::Instance { fields, .. } => {
                for (_, expression) in fields {
                    self.gather_expression(expression);
                }
            }
            Expression::Call {
                function,
                parameters,
            } => {
                self.gather_expression(function.as_ref());

                for parameter in &parameters.0 {
                    self.gather_expression(parameter);
                }
            }
            Expression::Declaration { value, .. } => self.gather_expression(value),
            Expression::Assignment { to, from } => {
                if let AssignLocation::Field { instance, .. } = &to.0 {
                    self.gather_expression(instance);
                }

                self.gather_expression(from);
            }
            Expression::Access { from, .. } => self.gather_expression(from),
            Expression::Block { expressions, tail } => {
                self.scopes.enter_scope();

                for expression in expressions {
                    self.gather_expression(expression);
                }
                self.gather_expression(tail);

                self.scopes.exit_current_scope();
            }
            Expression::Conditional {
                condition,
                success,
                failure,
            } => {
                self.gather_expression(condition);
                self.gather_expression(success);
                self.gather_expression(failure);
            }
            Expression::Break(expression) => self.gather_expression(expression),
            Expression::Return(expression) => self.gather_expression(expression),
            Expression::Loop(expression) => self.gather_expression(expression),
            _ => (), // Some expression variants don't produce any items or scopes.
        }
    }
}

pub struct GatherOut {
    pub engine: Engine,
    pub scopes: Scopes,
}

pub fn gather(module: hir::Module, errors: &mut Errors) -> GatherOut {
    let mut gatherer = Gatherer::new(errors);

    gatherer.insert_hir_module(&module);
    gatherer.gather_module(&module);

    GatherOut {
        engine: gatherer.engine,
        scopes: gatherer.scopes,
    }
}
