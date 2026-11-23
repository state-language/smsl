mod capture;
mod lifecycle;
mod profile;
mod request;
mod world;

pub use capture::{
    Capture, CaptureOptions, CapturePlan, capture, capture_plan, create_world, validate_world,
};
pub use lifecycle::{LifecycleBinding, LifecycleProjection};
pub use profile::project_profile;
pub use request::{profile_request, request, world_request};
pub use world::{
    KnowledgeUpdate, begin_action, cancel_action, check_basis, project_knowledge, world_basis,
};

fn error(message: impl Into<String>) -> crate::Diagnostic {
    crate::Diagnostic::new("invalid_application", message, None)
}
