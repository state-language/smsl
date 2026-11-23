use serde::{Deserialize, Serialize};

use crate::diagnostic::Location;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    String(String),
    UInt(u64),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub version: String,
    pub events: Vec<EventDeclaration>,
    pub branches: Vec<Branch>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventDeclaration {
    pub name: String,
    pub fields: Vec<EventField>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventField {
    pub name: String,
    #[serde(rename = "type")]
    pub data_type: DataType,
    pub nullable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Branch {
    pub name: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub initial: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub activating: Option<Expr>,
    pub mutable: bool,
    pub facts: Vec<Fact>,
    pub sub_branches: Vec<Binding>,
    pub data: Vec<DataField>,
    pub reads: Vec<ReadBinding>,
    pub states: Vec<State>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub named_states: Vec<Predicate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invariants: Vec<Predicate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operations: Vec<Operation>,
    pub reactions: Vec<Reaction>,
    pub labels: Vec<Predicate>,
    pub abstractions: Vec<Abstraction>,
    pub actions: Vec<Action>,
    #[serde(skip)]
    pub location: Option<Location>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Fact {
    pub name: String,
    pub values: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<UIntRange>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub initial: Option<Datum>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub derived: Option<Expr>,
    #[serde(skip)]
    pub location: Option<Location>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UIntRange {
    pub min: u64,
    pub max: u64,
}

impl Fact {
    pub fn contains(&self, value: &Value) -> bool {
        match (self.range, value) {
            (Some(range), Value::UInt(value)) => (range.min..=range.max).contains(value),
            (None, value) => self.values.contains(value),
            _ => false,
        }
    }

    pub fn accepts(&self, value: &Datum) -> bool {
        match value {
            Datum::Unknown(_) => true,
            Datum::Bool(value) => self.contains(&Value::Bool(*value)),
            Datum::String(value) => self.contains(&Value::String(value.clone())),
            Datum::Time(value) => self.contains(&Value::UInt(*value)),
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub name: String,
    pub values: Vec<Assignment>,
    pub transitions: Vec<Transition>,
    pub reactions: Vec<Reaction>,
    #[serde(skip)]
    pub location: Option<Location>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub fact: String,
    pub value: Value,
    #[serde(skip)]
    pub location: Option<Location>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub branch: String,
    pub fact: String,
    #[serde(skip)]
    pub location: Option<Location>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub operation: String,
    pub target: String,
    pub updates: Vec<Update>,
    #[serde(skip)]
    pub location: Option<Location>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DataType {
    Bool,
    String,
    Time,
    Event,
    UInt,
}

impl DataType {
    pub fn accepts(self, value: &Datum) -> bool {
        value.data_type() == Some(self) || self == Self::UInt && matches!(value, Datum::Time(_))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Datum {
    Unknown(UnknownValue),
    Null(()),
    Bool(bool),
    String(String),
    /// Unsigned scalar carrier for both `uint` and `time` declarations.
    Time(u64),
    Event(EventRef),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UnknownValue {
    Unknown {},
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Predicate {
    pub name: String,
    pub expression: Expr,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Abstraction {
    pub name: String,
    pub facts: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub name: String,
    pub parameters: Vec<EventField>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub precondition: Option<Expr>,
    pub outcomes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub name: String,
    pub guard: Option<Expr>,
    pub updates: Vec<Update>,
    pub postcondition: Option<Expr>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventRef {
    pub scope: String,
    pub step: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DataField {
    pub name: String,
    #[serde(rename = "type")]
    pub data_type: DataType,
    pub initial: Datum,
    #[serde(skip)]
    pub location: Option<Location>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReadBinding {
    pub alias: String,
    pub branch: String,
    #[serde(skip)]
    pub location: Option<Location>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Expr {
    Literal { value: Datum },
    Read { source: String, field: String },
    Not { expr: Box<Expr> },
    IsUnknown { expr: Box<Expr> },
    And { left: Box<Expr>, right: Box<Expr> },
    Or { left: Box<Expr>, right: Box<Expr> },
    Eq { left: Box<Expr>, right: Box<Expr> },
    Ne { left: Box<Expr>, right: Box<Expr> },
    Lt { left: Box<Expr>, right: Box<Expr> },
    Le { left: Box<Expr>, right: Box<Expr> },
    Gt { left: Box<Expr>, right: Box<Expr> },
    Ge { left: Box<Expr>, right: Box<Expr> },
    Add { left: Box<Expr>, right: Box<Expr> },
    Sub { left: Box<Expr>, right: Box<Expr> },
    Mul { left: Box<Expr>, right: Box<Expr> },
    Div { left: Box<Expr>, right: Box<Expr> },
    Rem { left: Box<Expr>, right: Box<Expr> },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    pub field: String,
    pub value: Expr,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reaction {
    pub event: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub guard: Option<Expr>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub target: Option<String>,
    pub updates: Vec<Update>,
    pub emit: Vec<String>,
    #[serde(skip)]
    pub location: Option<Location>,
}

impl Datum {
    pub fn unknown() -> Self {
        Self::Unknown(UnknownValue::Unknown {})
    }

    /// Carrier type; numeric semantic types require their field declaration.
    pub fn data_type(&self) -> Option<DataType> {
        match self {
            Self::Null(()) | Self::Unknown(_) => None,
            Self::Bool(_) => Some(DataType::Bool),
            Self::String(_) => Some(DataType::String),
            Self::Time(_) => Some(DataType::Time),
            Self::Event(_) => Some(DataType::Event),
        }
    }
}
