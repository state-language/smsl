pub mod abstraction;
pub mod actions;
pub mod application;
pub use persistence::codec;
mod derived;
pub mod evolution;
mod state;
mod step;
mod types;
mod validation;
mod world;

pub use state::RuntimeState;
pub use step::{RuntimeStep, advance, confirm, initial, reset};
pub(crate) use world::ExecutionCheckpoint;
pub use world::{ActionTicket, EventRecord, World};

pub const SNAPSHOT_VERSION: &str = "1";
pub const WORLD_VERSION: &str = "1";
pub use types::{BranchSnapshot, Context, Event, Snapshot, Step};
pub use validation::validate_snapshot;

pub mod persistence;
pub mod system;
pub(crate) use persistence::snapshot::check_json_profile;
pub use persistence::snapshot::{dump_snapshot, parse_event, restore};
