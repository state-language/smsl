use super::{Contract, Effect, Freshness, Port};

pub fn catalog() -> Vec<Contract> {
    use Effect::{Evolution, Execution, Pure};
    [
        (
            "model.advance_state",
            "state:state action:action",
            "state:state",
            Evolution,
        ),
        (
            "symbolic.abstract_state",
            "state:state",
            "knowledge:knowledge",
            Pure,
        ),
        (
            "symbolic.concretize",
            "knowledge:knowledge",
            "condition:condition",
            Pure,
        ),
        (
            "symbolic.plan",
            "knowledge:knowledge goal:goal",
            "plan:plan",
            Pure,
        ),
        (
            "symbolic.refine_plan",
            "plan:plan condition:condition",
            "plan:plan",
            Pure,
        ),
        (
            "symbolic.predict",
            "knowledge:knowledge action:action",
            "prediction:prediction",
            Pure,
        ),
        (
            "symbolic.rollout",
            "knowledge:knowledge plan:plan",
            "prediction:prediction",
            Pure,
        ),
        ("symbolic.inspect", "knowledge:knowledge", "view:view", Pure),
        (
            "symbolic.ground",
            "estimate:estimate",
            "updates:knowledge_updates",
            Pure,
        ),
        (
            "symbolic.abstract_knowledge",
            "knowledge:knowledge",
            "knowledge:knowledge",
            Pure,
        ),
        (
            "subsymbolic.predict",
            "estimate:estimate target:target",
            "trajectory:trajectory",
            Pure,
        ),
        (
            "subsymbolic.assess",
            "estimate:estimate trajectory:trajectory",
            "assessment:assessment",
            Pure,
        ),
        (
            "subsymbolic.refine_skill",
            "action:action condition:condition",
            "target:target",
            Pure,
        ),
        (
            "subsymbolic.execute_skill",
            "invocation:invocation feedback?:feedback",
            "outcome:outcome",
            Execution,
        ),
        (
            "subsymbolic.inspect",
            "estimate:estimate",
            "view:view",
            Pure,
        ),
        (
            "subsymbolic.estimate",
            "observation:observation",
            "estimate:estimate",
            Pure,
        ),
        (
            "subsymbolic.control_step",
            "target:target estimate:estimate",
            "command:command",
            Execution,
        ),
    ]
    .into_iter()
    .map(|(id, inputs, outputs, effect)| Contract {
        id: id.into(),
        inputs: ports(inputs),
        outputs: ports(outputs),
        effect,
    })
    .collect()
}

fn ports(source: &str) -> std::collections::BTreeMap<String, Port> {
    source
        .split_whitespace()
        .map(|entry| {
            let (name, schema) = entry.split_once(':').expect("static catalog port");
            (
                name.trim_end_matches('?').into(),
                Port {
                    schema: schema.into(),
                    required: !name.ends_with('?'),
                    unit: None,
                    frame: None,
                    freshness: Freshness::Current,
                },
            )
        })
        .collect()
}
