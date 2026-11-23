#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Role {
    pub id: &'static str,
    pub name: &'static str,
    pub layer: &'static str,
    pub responsibility: &'static str,
}

/// Standard role metadata; component role IDs remain open to application-defined values.
pub fn roles() -> &'static [Role] {
    &ROLES
}

const ROLES: [Role; 10] = [
    Role {
        id: "symbolic-planner",
        name: "Symbolic Planner",
        layer: "symbolic",
        responsibility: "Construct or revise task models and select actions or plans toward a goal.",
    },
    Role {
        id: "symbolic-predictor",
        name: "Symbolic Predictor",
        layer: "symbolic",
        responsibility: "Predict symbolic successors and task progress.",
    },
    Role {
        id: "symbolic-estimator",
        name: "Symbolic Estimator",
        layer: "symbolic",
        responsibility: "Ground evidence into facts, identify missing evidence; perform abstraction and concretization.",
    },
    Role {
        id: "symbolic-visualizer",
        name: "Symbolic Visualizer",
        layer: "symbolic",
        responsibility: "Present symbolic state, plans and evidence.",
    },
    Role {
        id: "symbolic-discoverer",
        name: "Symbolic Discoverer",
        layer: "symbolic",
        responsibility: "Propose new fact and action definitions with grounding or execution bindings and evidence.",
    },
    Role {
        id: "subsymbolic-estimator",
        name: "Subsymbolic Estimator",
        layer: "subsymbolic",
        responsibility: "Estimate detailed configurations from observations.",
    },
    Role {
        id: "subsymbolic-predictor",
        name: "Subsymbolic Predictor",
        layer: "subsymbolic",
        responsibility: "Predict dynamics, trajectories and feasibility.",
    },
    Role {
        id: "subsymbolic-planner-dispatcher",
        name: "Subsymbolic Planner & Dispatcher",
        layer: "subsymbolic",
        responsibility: "Refine requests into skills and coordinate execution.",
    },
    Role {
        id: "subsymbolic-actor",
        name: "Subsymbolic Actor",
        layer: "subsymbolic",
        responsibility: "Generate control or tool commands for selected behavior.",
    },
    Role {
        id: "subsymbolic-visualizer",
        name: "Subsymbolic Visualizer",
        layer: "subsymbolic",
        responsibility: "Present observations, configurations and trajectories.",
    },
];
