use crate::{SmslData, SmslString};

#[repr(C)]
pub struct SmslExprRecord {
    pub kind: u32,
    pub literal: *const SmslData,
    pub source: SmslString,
    pub field: SmslString,
    pub left: *const SmslExprRecord,
    pub right: *const SmslExprRecord,
}
#[repr(C)]
pub struct SmslFactRecord {
    pub name: SmslString,
    pub values: *const *const SmslData,
    pub value_count: usize,
    pub initial: *const SmslData,
    pub derived: *const SmslExprRecord,
}
#[repr(C)]
pub struct SmslAssignmentRecord {
    pub fact: SmslString,
    pub value: *const SmslData,
}
#[repr(C)]
pub struct SmslTransitionRecord {
    pub operation: SmslString,
    pub target: SmslString,
    pub updates: *const SmslUpdateRecord,
    pub update_count: usize,
}
#[repr(C)]
pub struct SmslBindingRecord {
    pub branch: SmslString,
    pub fact: SmslString,
}
#[repr(C)]
pub struct SmslReadRecord {
    pub alias: SmslString,
    pub branch: SmslString,
}
#[repr(C)]
pub struct SmslFieldRecord {
    pub name: SmslString,
    pub data_type: u32,
    pub initial: *const SmslData,
}
#[repr(C)]
pub struct SmslUpdateRecord {
    pub field: SmslString,
    pub value: *const SmslExprRecord,
}
#[repr(C)]
pub struct SmslReactionRecord {
    pub event: SmslString,
    pub target: *const SmslString,
    pub guard: *const SmslExprRecord,
    pub updates: *const SmslUpdateRecord,
    pub update_count: usize,
    pub emit: *const SmslString,
    pub emit_count: usize,
}
#[repr(C)]
pub struct SmslStateRecord {
    pub name: SmslString,
    pub values: *const SmslAssignmentRecord,
    pub value_count: usize,
    pub transitions: *const SmslTransitionRecord,
    pub transition_count: usize,
    pub reactions: *const SmslReactionRecord,
    pub reaction_count: usize,
}
#[repr(C)]
pub struct SmslBranchRecord {
    pub name: SmslString,
    pub initial: *const SmslString,
    pub activating: *const SmslExprRecord,
    pub facts: *const SmslFactRecord,
    pub fact_count: usize,
    pub states: *const SmslStateRecord,
    pub state_count: usize,
    pub bindings: *const SmslBindingRecord,
    pub binding_count: usize,
    pub data: *const SmslFieldRecord,
    pub data_count: usize,
    pub reads: *const SmslReadRecord,
    pub read_count: usize,
    pub mutable: u8,
    pub reactions: *const SmslReactionRecord,
    pub reaction_count: usize,
    pub labels: *const SmslPredicateRecord,
    pub label_count: usize,
    pub actions: *const SmslActionRecord,
    pub action_count: usize,
    pub abstractions: *const SmslAbstractionRecord,
    pub abstraction_count: usize,
}
#[repr(C)]
pub struct SmslAbstractionRecord {
    pub name: SmslString,
    pub facts: *const SmslString,
    pub fact_count: usize,
}
#[repr(C)]
pub struct SmslPredicateRecord {
    pub name: SmslString,
    pub expression: *const SmslExprRecord,
}
#[repr(C)]
pub struct SmslActionRecord {
    pub name: SmslString,
    pub parameters: *const SmslEventFieldRecord,
    pub parameter_count: usize,
    pub precondition: *const SmslExprRecord,
    pub outcomes: *const SmslString,
    pub outcome_count: usize,
}
#[repr(C)]
pub struct SmslEventFieldRecord {
    pub name: SmslString,
    pub data_type: u32,
    pub nullable: u8,
}
#[repr(C)]
pub struct SmslEventRecord {
    pub name: SmslString,
    pub fields: *const SmslEventFieldRecord,
    pub field_count: usize,
}
#[repr(C)]
pub struct SmslDefinitionRecord {
    pub version: SmslString,
    pub events: *const SmslEventRecord,
    pub event_count: usize,
    pub branches: *const SmslBranchRecord,
    pub branch_count: usize,
}
