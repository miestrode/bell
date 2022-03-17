use std::{collections::HashMap, fmt::Display};

use super::{
    ast::{self, Id},
    span::Span,
    Name,
};

pub type TypeId = usize;

#[derive(Clone, Debug)]
pub enum TypeInfo {
    Unknown(bool),
    Reference(TypeId),
    // A link physically links two types together, all information discovered on one is discovered on another.
    // It's different than references, since those are talking about actual object references.
    Link {
        linked_to: TypeId,
        reason: LinkReason,
    },
    Unit,
    Integer,
    Boolean,
    String,
    Structure(HashMap<Name, TypeId>),
    Instance(TypeId),
    Function {
        parameters: Vec<TypeId>,
        return_type: TypeId,
    },
}

pub trait IntoTyInfo {
    fn into_ty(self, scopes: &Scopes) -> TypeInfo;
}

impl IntoTyInfo for ast::Type {
    fn into_ty(self, scopes: &Scopes) -> TypeInfo {
        match self {
            ast::Type::Integer => TypeInfo::Integer,
            ast::Type::Boolean => TypeInfo::Boolean,
            ast::Type::String => TypeInfo::String,
            ast::Type::Structure(id) => {
                // TODO: Refactor this.
                if let Some(&Variable { type_id, .. }) =
                    scopes.raw_scopes.search_id(&id, scopes.current)
                {
                    TypeInfo::Instance(type_id)
                } else {
                    // No need to report this as an error, this should ALWAYS be reported.
                    // TODO: Could be wrong! Please verify this!
                    TypeInfo::Unknown(true)
                }
            }
            ast::Type::Reference(data_type) => data_type.into_ty(scopes),
        }
    }
}

impl IntoTyInfo for Option<(ast::Type, Span)> {
    fn into_ty(self, scopes: &Scopes) -> TypeInfo {
        self.map(|(type_hint, _)| type_hint.into_ty(scopes))
            .unwrap_or(TypeInfo::Unknown(false))
    }
}

#[derive(Clone, Copy)]
pub struct Mismatch {
    pub a: TypeId,
    pub b: TypeId,
    pub reason: LinkReason,
}

pub type Types = Vec<(TypeInfo, Option<Span>)>;

pub struct Engine {
    pub types: Types,
    pub mismatches: Vec<Mismatch>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            mismatches: Vec::new(),
        }
    }

    pub fn new_with_types(types: Types) -> Self {
        Self {
            types,
            mismatches: Vec::new(),
        }
    }

    pub fn get(&self, type_id: TypeId) -> &(TypeInfo, Option<Span>) {
        &self.types[type_id]
    }

    pub fn insert_type(&mut self, data_type: TypeInfo, span: Option<Span>) -> TypeId {
        let id = self.types.len();
        self.types.push((data_type, span));

        id
    }

    pub fn remove_ref(&self, type_info: TypeInfo) -> TypeInfo {
        match type_info {
            TypeInfo::Link { linked_to, .. } => self.remove_ref(self.types[linked_to].0.clone()),
            _ => type_info,
        }
    }

    pub fn unify(&mut self, a: TypeId, b: TypeId, reason: LinkReason) {
        self.unify_with_context(a, b, UnifyCtx { reason, a, b })
    }

    fn unify_with_context(&mut self, a: TypeId, b: TypeId, context: UnifyCtx) {
        match (self.types[a].0.clone(), self.types[b].0.clone()) {
            // Overwrite unknowns.
            (TypeInfo::Unknown { .. }, _) => {
                self.types[a].0 = TypeInfo::Link {
                    linked_to: b,
                    reason: context.reason,
                }
            }
            (_, TypeInfo::Unknown { .. }) => {
                self.types[b].0 = TypeInfo::Link {
                    linked_to: a,
                    reason: context.reason,
                }
            }

            // Follow any links.
            (TypeInfo::Link { linked_to, .. }, _) => self.unify_with_context(linked_to, b, context),
            (_, TypeInfo::Link { linked_to, .. }) => self.unify_with_context(a, linked_to, context),
            (TypeInfo::Integer, TypeInfo::Integer) => (),
            (TypeInfo::Boolean, TypeInfo::Boolean) => (),
            (TypeInfo::String, TypeInfo::String) => (),
            (TypeInfo::Reference(a), TypeInfo::Reference(b)) => {
                self.unify_with_context(a, b, context)
            }

            // All identifiers at this point are absolute, so this code holds.
            (TypeInfo::Instance(id), TypeInfo::Instance(other)) if id == other => (),
            (TypeInfo::Structure(fields_a), TypeInfo::Structure(fields_b)) => {
                for (field, data_type) in fields_a {
                    if let Some(&other) = fields_b.get(&field) {
                        self.unify(data_type, other, LinkReason::Field);
                    } else {
                        self.mismatches.push(Mismatch {
                            a: context.a,
                            b: context.b,
                            reason: context.reason,
                        });

                        return;
                    }
                }
            }
            (
                TypeInfo::Function {
                    parameters: a_parameters,
                    return_type: a_return_type,
                },
                TypeInfo::Function {
                    parameters: b_parameters,
                    return_type: b_return_type,
                },
            ) if a_parameters.len() == b_parameters.len() => {
                for (a, b) in a_parameters.into_iter().zip(b_parameters) {
                    self.unify(a, b, LinkReason::Parameter);
                }

                self.unify(a_return_type, b_return_type, LinkReason::Return);
            }
            _ => self.mismatches.push(Mismatch {
                a: context.a,
                b: context.b,
                reason: context.reason,
            }),
        }
    }
}

#[derive(Debug)]
pub struct Fields(HashMap<Id, Type>);

impl Fields {
    pub fn new(fields: HashMap<Id, Type>) -> Self {
        Self(fields)
    }
}

impl Display for Fields {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{{{}}}",
            self.0
                .iter()
                .map(|(field, data_type)| format!("{}: {}", field, data_type))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

#[derive(Debug)]
pub enum Type {
    Unknown,
    Reference(Box<Type>),
    Unit,
    Integer,
    Boolean,
    String,
    Structure(Fields),
    Instance(Id),
    Function {
        parameters: Vec<Type>,
        return_type: Box<Type>,
    },
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => write!(f, "?"),
            Self::Reference(data_type) => write!(f, "&{}", data_type),
            Self::Unit => write!(f, "Unit"),
            Self::Integer => write!(f, "Int"),
            Self::Boolean => write!(f, "Bool"),
            Self::String => write!(f, "Str"),
            Self::Structure(fields) => write!(f, "{}", fields),
            Self::Instance(id) => write!(f, "{}", id),
            Self::Function {
                parameters,
                return_type,
            } => write!(
                f,
                "func({}) -> {}",
                parameters
                    .iter()
                    .map(|parameter| parameter.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                return_type
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LinkReason {
    Declaration,
    Assign,
    Structure,
    Call,
    Condition,
    Field,
    Conditional,
    Loop,
    Other,
    Parameter,
    Return,
}

impl Into<&'static str> for LinkReason {
    fn into(self) -> &'static str {
        // TODO: Confirm these sentences accurately describe the reason in an error message.
        match self {
            LinkReason::Assign => "it's being assigned to this identifier",
            LinkReason::Conditional => "all branches of a conditional must return the same type",
            LinkReason::Field => "it's being assigned to the field before it",
            LinkReason::Loop => "it's broken out of the loop",
            LinkReason::Return => "it's being returned from this function",
            LinkReason::Parameter => "it's passed as an argument in this call",
            LinkReason::Declaration => "it's declared as the previous variable",
            _ => unreachable!(),
        }
    }
}

#[derive(Clone)]
// This stands for unification context.
pub struct UnifyCtx {
    pub reason: LinkReason,
    pub a: TypeId,
    pub b: TypeId,
}

// For now all constraints are field constraints, I don't really know if I'll need more
// constraints for anything else, but for now this is fine.
#[derive(Clone, Copy, Debug)]
pub struct Constraint {
    pub object_id: TypeId,
    pub field_id: TypeId,
    pub field: Name,
}

impl Constraint {
    pub fn new(object_id: TypeId, field_id: TypeId, field: Name) -> Self {
        Self {
            object_id,
            field_id,
            field,
        }
    }
}

pub type ScopeId = usize;

#[derive(Debug, Clone)]
pub struct Variable {
    pub type_id: TypeId,
    pub shadowable: bool,
}

type Variables = Vec<(Name, Variable)>;
type Modules = Vec<(Name, ScopeId)>;

#[derive(Debug, Clone, Copy)]
pub enum ScopeConnection {
    Inclusive(ScopeId),
    Exclusive(ScopeId),
    None,
}

impl ScopeConnection {
    fn as_option(&self) -> Option<ScopeId> {
        match self {
            &ScopeConnection::Inclusive(scope_id) => Some(scope_id),
            &ScopeConnection::Exclusive(scope_id) => Some(scope_id),
            ScopeConnection::None => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub modules: Modules,
    pub variables: Variables,
    pub connection: ScopeConnection,
}

impl Scope {
    pub fn new(connection: ScopeConnection) -> Self {
        Self {
            modules: Modules::new(),
            variables: Variables::new(),
            connection,
        }
    }

    pub fn does_already_exist(&self, name: Name) -> bool {
        self.search_variable(name).is_some() || self.search_module(name).is_some()
    }

    pub fn search_variable(&self, name: Name) -> Option<&Variable> {
        self.variables.iter().rev().find_map(
            |(other, variable)| {
                if name == *other {
                    Some(variable)
                } else {
                    None
                }
            },
        )
    }

    pub fn search_module(&self, name: Name) -> Option<ScopeId> {
        self.modules.iter().rev().find_map(
            |&(other, scope_id)| {
                if name == other {
                    Some(scope_id)
                } else {
                    None
                }
            },
        )
    }

    fn search_name_by_type(&self, type_id: TypeId) -> Option<Name> {
        self.variables.iter().rev().find_map(|(name, data)| {
            if type_id == data.type_id {
                Some(*name)
            } else {
                None
            }
        })
    }

    pub fn insert_module(&mut self, name: Name, scope: ScopeId) {
        self.modules.push((name, scope));
    }

    pub fn insert_variable(&mut self, name: Name, variable: Variable) {
        self.variables.push((name, variable));
    }
}

#[derive(Debug, Clone)]
struct RawScopes(Vec<Scope>);

impl RawScopes {
    // This will search for a module within the current inclusive scopes (In practice, the current file).
    // This is needed in order to prevent naming collisions with top-level expressions in the module.
    fn search_local_module(&self, name: Name, starting_scope: ScopeId) -> Option<ScopeId> {
        let scope = &self.0[starting_scope];

        scope.search_module(name).or_else(|| {
            if let ScopeConnection::Inclusive(scope_id) = scope.connection {
                self.search_module(name, scope_id)
            } else {
                None
            }
        })
    }

    pub fn does_already_exist(&self, name: Name, starting_scope: ScopeId) -> bool {
        self.search_variable(name, starting_scope).is_some()
            || self.search_local_module(name, starting_scope).is_some()
    }

    pub fn get_id_origin_module(&self, id: &Id, mut current_module: ScopeId) -> Option<ScopeId> {
        // This code will explore the non-tail parts of the ID and eventually locate the origin module of that ID.
        for id_part in id.0[..id.0.len() - 1].iter().copied() {
            if let Some(scope_id) = self.search_module(id_part, current_module) {
                current_module = scope_id;
            } else {
                // We are looking at the non-tail parts, so the "ID part" won't be a reference to an ID.
                // This means that if the search_module call fails, the ID we are searching for doesn't exist.
                return None;
            }
        }

        Some(current_module)
    }

    // This code searchs for an ID that ultimately refers to a variable, not another module.
    pub fn search_id(&self, id: &Id, current_module: ScopeId) -> Option<&Variable> {
        let id_tail = *id.0.last().unwrap(); // We assume the ID refers to a variable, so the last part of the ID naturally is that variable name.

        self.get_id_origin_module(id, current_module)
            .and_then(|origin_module| {
                if let Some(variable) = self.search_variable(id_tail, origin_module) {
                    Some(variable)
                } else {
                    None
                }
            })
    }

    pub fn search_variable(&self, name: Name, starting_scope: ScopeId) -> Option<&Variable> {
        let scope = &self.0[starting_scope];

        scope.search_variable(name).or_else(|| {
            if let ScopeConnection::Inclusive(scope_id) = scope.connection {
                self.search_variable(name, scope_id)
            } else {
                None
            }
        })
    }

    pub fn search_module(&self, name: Name, starting_scope: ScopeId) -> Option<ScopeId> {
        let scope = &self.0[starting_scope];

        scope.search_module(name).or_else(|| {
            scope
                .connection
                .as_option()
                .and_then(|scope_id| self.search_module(name, scope_id))
        })
    }

    // Type ids are unique so we search all scopes.
    pub fn search_name_by_type(&self, type_id: TypeId) -> Option<Name> {
        self.0
            .iter()
            .find_map(|scope| scope.search_name_by_type(type_id))
    }
}

#[derive(Debug, Clone)]
pub struct Scopes {
    pub raw_scopes: RawScopes,
    pub current: ScopeId,
}

impl Scopes {
    pub fn new() -> Self {
        Scopes {
            raw_scopes: RawScopes(vec![Scope::new(ScopeConnection::None)]),
            current: 0,
        }
    }

    pub fn does_already_exist(&self, name: Name) -> bool {
        self.raw_scopes.does_already_exist(name, self.current)
    }

    pub fn new_with_scopes(raw_scopes: RawScopes) -> Self {
        Self {
            raw_scopes,
            current: 0,
        }
    }

    pub fn get_id_origin_module(&self, id: &Id) -> Option<ScopeId> {
        self.raw_scopes.get_id_origin_module(id, self.current)
    }

    pub fn create_scope(&mut self, connection: ScopeConnection) -> ScopeId {
        let assigned_scope_id = self.raw_scopes.0.len();

        self.raw_scopes.0.push(Scope::new(connection));

        self.current = assigned_scope_id;
        self.current
    }

    pub fn exit_current_scope(&mut self) {
        self.current = self.raw_scopes.0[self.current]
            .connection
            .as_option()
            .unwrap();
    }

    pub fn insert_module(&mut self, name: Name, scope_id: ScopeId) {
        self.raw_scopes.0[self.current].insert_module(name, scope_id)
    }

    pub fn insert_variable(&mut self, name: Name, variable: Variable) {
        self.raw_scopes.0[self.current].insert_variable(name, variable);
    }

    pub fn search_id(&self, id: &Id) -> Option<&Variable> {
        self.raw_scopes.search_id(id, self.current)
    }

    // This isn't implemented in the same manner as searching for types since it's more useful to search using the scope trees instead of the scope list.
    pub fn search_variable(&self, name: Name) -> Option<&Variable> {
        self.raw_scopes.search_variable(name, self.current)
    }

    pub fn search_module(&self, name: Name) -> Option<ScopeId> {
        self.raw_scopes.search_module(name, self.current)
    }

    // Type ids are unique so we search all scopes.
    fn search_name_by_type(&self, type_id: TypeId) -> Option<Name> {
        self.raw_scopes.search_name_by_type(type_id)
    }
}

#[derive(Clone)]
pub struct StaticScopes {
    raw_scopes: RawScopes,
    current: ScopeId,
    scope_inc: ScopeId,
}

impl StaticScopes {
    pub fn new_empty() -> Self {
        Self {
            raw_scopes: RawScopes(Vec::new()),
            current: 0,
            scope_inc: 0,
        }
    }

    pub fn new(raw_scopes: RawScopes) -> Self {
        Self {
            raw_scopes,
            current: 0,
            scope_inc: 0,
        }
    }

    pub fn from(mut other: Self) -> Self {
        other.current = 0;
        other.scope_inc = 0;

        other
    }

    pub fn enter_scope(&mut self) {
        self.scope_inc += 1;
        self.current = self.scope_inc;
    }

    pub fn exit_scope(&mut self) {
        self.current = self.raw_scopes.0[self.current]
            .connection
            .as_option()
            .unwrap();
    }

    pub fn does_already_exist(&self, name: Name) -> bool {
        self.raw_scopes.does_already_exist(name, self.current)
    }

    pub fn get_id_origin_module(&self, id: &Id) -> Option<ScopeId> {
        self.raw_scopes.get_id_origin_module(id, self.current)
    }

    pub fn insert_import(&mut self, name: Name, scope_id: ScopeId) {
        self.raw_scopes.0[self.current].insert_module(name, scope_id);
    }

    pub fn insert_variable(&mut self, name: Name, variable: Variable) {
        self.raw_scopes.0[self.current].insert_variable(name, variable);
    }

    pub fn search_id(&self, id: &Id) -> Option<&Variable> {
        self.raw_scopes.search_id(id, self.current)
    }

    pub fn search_variable(&self, name: Name) -> Option<&Variable> {
        self.raw_scopes.search_variable(name, self.current)
    }

    pub fn search_module(&self, name: Name) -> Option<ScopeId> {
        self.raw_scopes.search_module(name, self.current)
    }

    // Type ids are unique so we search all scopes.
    fn search_type_name_by_id(&self, type_id: TypeId) -> Option<Name> {
        self.raw_scopes.search_name_by_type(type_id)
    }
}

#[derive(Clone, Copy)]
pub enum ScopeContext {
    Function {
        return_type: TypeId,
    },
    FunctionLoop {
        function_return: TypeId,
        loop_return: TypeId,
    },
}

impl ScopeContext {
    pub fn get_function_ret_ty(self) -> TypeId {
        match self {
            ScopeContext::Function { return_type } => return_type,
            ScopeContext::FunctionLoop {
                function_return, ..
            } => function_return,
        }
    }
}
