pub mod cairo_compile;
pub mod error;
// Re-export cairo_vm structs returned by this crate for ease of use
pub use cairo_vm::{
    types::relocatable::{MaybeRelocatable, Relocatable},
    vm::{runners::cairo_runner::CairoRunner, vm_core::VirtualMachine},
    Felt252,
};
