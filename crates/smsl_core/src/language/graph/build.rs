use std::collections::HashMap;

use crate::language::model::Document;

use super::{Dependency, DependencyKind, Edge, EdgeKind, Graph, Node};

impl Graph {
    pub(super) fn from_document(document: &Document) -> Self {
        let mut graph = Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            dependencies: Vec::new(),
            names: HashMap::new(),
            outgoing: Vec::new(),
            incoming: Vec::new(),
        };
        for branch in &document.branches {
            let names = graph.names.entry(branch.name.clone()).or_default();
            for state in &branch.states {
                let id = graph.nodes.len();
                names.insert(state.name.clone(), id);
                graph.nodes.push(Node {
                    id,
                    branch: branch.name.clone(),
                    state: state.name.clone(),
                    values: state.values.clone(),
                    initial: branch.initial.as_ref() == Some(&state.name),
                    activating: branch
                        .activating
                        .as_ref()
                        .and_then(crate::language::machine::control_activation)
                        == Some(state.name.as_str()),
                });
            }
        }
        graph.outgoing = vec![Vec::new(); graph.nodes.len()];
        graph.incoming = vec![Vec::new(); graph.nodes.len()];
        for branch in &document.branches {
            for state in &branch.states {
                let source = graph.names[&branch.name][&state.name];
                for transition in &state.transitions {
                    graph.push_edge(Edge {
                        id: graph.edges.len(),
                        source,
                        target: graph.names[&branch.name][&transition.target],
                        kind: EdgeKind::Operation,
                        label: transition.operation.clone(),
                        guard: None,
                        updates: transition.updates.clone(),
                        emit: Vec::new(),
                    });
                }
                for reaction in state.reactions.iter().chain(&branch.reactions) {
                    graph.push_edge(Edge {
                        id: graph.edges.len(),
                        source,
                        target: reaction
                            .target
                            .as_ref()
                            .map(|target| graph.names[&branch.name][target])
                            .unwrap_or(source),
                        kind: EdgeKind::Reaction,
                        label: reaction.event.clone(),
                        guard: reaction.guard.clone(),
                        updates: reaction.updates.clone(),
                        emit: reaction.emit.clone(),
                    });
                }
            }
            graph
                .dependencies
                .extend(branch.sub_branches.iter().map(|binding| Dependency {
                    source: branch.name.clone(),
                    target: binding.branch.clone(),
                    kind: DependencyKind::Binding,
                    name: binding.fact.clone(),
                }));
            graph
                .dependencies
                .extend(branch.reads.iter().map(|read| Dependency {
                    source: branch.name.clone(),
                    target: read.branch.clone(),
                    kind: DependencyKind::Read,
                    name: read.alias.clone(),
                }));
        }
        graph
    }

    fn push_edge(&mut self, edge: Edge) {
        self.outgoing[edge.source].push(edge.id);
        self.incoming[edge.target].push(edge.id);
        self.edges.push(edge);
    }
}
