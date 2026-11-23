use std::collections::BTreeMap;
use std::fmt::Write;

use super::{DependencyKind, Edge, EdgeKind, Graph, Node};
use crate::language::syntax::serialize::{expression, identifier, value};

impl Graph {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("Graph is JSON serializable") + "\n"
    }

    pub fn to_dot(&self) -> String {
        let mut branches: BTreeMap<&str, Vec<&Node>> = self
            .names
            .keys()
            .map(|name| (name.as_str(), Vec::new()))
            .collect();
        for node in self.nodes() {
            branches.entry(&node.branch).or_default().push(node);
        }
        let branch_ids: BTreeMap<_, _> = branches
            .keys()
            .enumerate()
            .map(|(id, name)| (*name, id))
            .collect();
        let mut dot = String::from("digraph smsl {\n  rankdir=LR;\n");
        for (branch, nodes) in branches {
            let id = branch_ids[branch];
            writeln!(dot, "  subgraph cluster_{id} {{").unwrap();
            writeln!(dot, "    label={};", quoted(branch)).unwrap();
            writeln!(dot, "    b{id} [shape=box, label={}];", quoted(branch)).unwrap();
            for node in nodes {
                let shape = if node.activating {
                    "doublecircle"
                } else {
                    "ellipse"
                };
                let width = if node.initial { 2 } else { 1 };
                writeln!(
                    dot,
                    "    n{} [shape={shape}, penwidth={width}, label={}];",
                    node.id,
                    quoted(&node_label(node))
                )
                .unwrap();
            }
            dot.push_str("  }\n");
        }
        for edge in self.edges() {
            let style = match edge.kind {
                EdgeKind::Operation => "solid",
                EdgeKind::Reaction => "dashed",
            };
            writeln!(
                dot,
                "  n{} -> n{} [id=\"e{}\", style={style}, label={}];",
                edge.source,
                edge.target,
                edge.id,
                quoted(&edge_label(edge))
            )
            .unwrap();
        }
        for (id, dependency) in self.dependencies().iter().enumerate() {
            let kind = match dependency.kind {
                DependencyKind::Binding => "bind",
                DependencyKind::Read => "read",
            };
            writeln!(
                dot,
                "  b{} -> b{} [id=\"d{id}\", style=dotted, constraint=false, label={}];",
                branch_ids[dependency.source.as_str()],
                branch_ids[dependency.target.as_str()],
                quoted(&format!("{kind}: {}", dependency.name))
            )
            .unwrap();
        }
        dot.push_str("}\n");
        dot
    }
}

fn node_label(node: &Node) -> String {
    let mut label = node.state.clone();
    if node.initial {
        label.push_str("\n(initial)");
    }
    for assignment in &node.values {
        write!(
            label,
            "\n{} = {}",
            identifier(&assignment.fact),
            value(&assignment.value)
        )
        .unwrap();
    }
    label
}

fn edge_label(edge: &Edge) -> String {
    let mut label = match edge.kind {
        EdgeKind::Operation => identifier(&edge.label),
        EdgeKind::Reaction => format!("on {}", identifier(&edge.label)),
    };
    if let Some(guard) = &edge.guard {
        write!(label, "\nwhen {}", expression(guard)).unwrap();
    }
    if !edge.updates.is_empty() {
        let updates = edge
            .updates
            .iter()
            .map(|update| {
                format!(
                    "{} = {}",
                    identifier(&update.field),
                    expression(&update.value)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        write!(label, "\nset({updates})").unwrap();
    }
    if !edge.emit.is_empty() {
        let events = edge
            .emit
            .iter()
            .map(|event| identifier(event))
            .collect::<Vec<_>>()
            .join(", ");
        write!(label, "\nemit({events})").unwrap();
    }
    label
}

fn quoted(text: &str) -> String {
    let mut result = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            c if c.is_control() => write!(result, "\\\\u{:04x}", c as u32).unwrap(),
            c => result.push(c),
        }
    }
    result.push('"');
    result
}

#[cfg(test)]
mod tests {
    use super::quoted;

    #[test]
    fn quotes_dot_labels_without_substitution_or_control_bytes() {
        assert_eq!(
            quoted("a\"b\\N\n\0\t\r雪"),
            "\"a\\\"b\\\\N\\n\\\\u0000\\\\u0009\\\\u000d雪\""
        );
    }
}
