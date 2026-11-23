#![deny(unsafe_op_in_unsafe_fn)]

macro_rules! api {
    ([$status:ident] $name:ident($($arg:ident: $ty:ty),* $(,)?) -> $out:ty $body:block) => {
        /// # Safety
        /// Pointers must satisfy the ownership, lifetime and storage contracts in smsl.h.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name($($arg: $ty,)* out: *mut $out, error: *mut *mut crate::SmslError) -> crate::SmslStatus {
            unsafe { crate::boundary::run_as(out, error, Some(crate::SmslStatus::$status), || $body) }
        }
    };
    ($name:ident($($arg:ident: $ty:ty),* $(,)?) -> $out:ty $body:block) => {
        /// # Safety
        /// Pointers must satisfy the ownership, lifetime and storage contracts in smsl.h.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name($($arg: $ty,)* out: *mut $out, error: *mut *mut crate::SmslError) -> crate::SmslStatus {
            unsafe { crate::boundary::run(out, error, || $body) }
        }
    };
}
macro_rules! free {
    ($name:ident, $ty:ty) => {
        /// # Safety
        /// The pointer must be null or a live, uniquely owned handle of this type.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(value: *mut $ty) {
            if !value.is_null() {
                drop(unsafe { Box::from_raw(value) });
            }
        }
    };
}
mod abstraction;
mod application;
mod boundary;
mod data;
mod definition;
mod event;
mod graph;
mod machine;
mod mechanisms;
mod memory;
mod modules;
mod providers;
mod records;
mod requests;
mod snapshot;
mod system;
mod types;
mod world;
pub use abstraction::*;
pub use application::*;
pub use data::*;
pub use definition::*;
pub use event::*;
pub use graph::*;
pub use machine::*;
pub use mechanisms::*;
pub use memory::*;
pub use modules::*;
pub use providers::*;
pub use records::*;
pub use requests::*;
pub use snapshot::*;
pub use system::*;
pub use types::*;
pub use world::*;
