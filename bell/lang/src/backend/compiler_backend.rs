use crate::backend::mir;

// Define what it means to be a Bell backend. For now only compilation to 1.17 MCfunction is available.
// But this defines a framework for more interesting things like "Only one command" creations or maybe even redstone
pub trait Backend {
    fn generate_code(program: mir::Program) -> (Self, Option<mir::Id>)
    where
        Self: Sized;
}
