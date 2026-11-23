use crate::{boundary::*, *};
use smsl_core::graph::EdgeKind;
api!(smsl_graph_data(value: *const SmslGraph) -> *mut SmslData { data(&get(value)?.0) });
api!(smsl_graph_node(value: *const SmslGraph, branch: SmslString, state: SmslString) -> usize { Ok(get(value)?.0.node(text(branch)?, text(state)?)?.id) });
api!(smsl_graph_traverse(value: *const SmslGraph, operation: u32, source: usize, target: usize, kind: u32) -> *mut SmslData {
    let graph = &get(value)?.0;
    let kind = match kind { 0 => None, 1 => Some(EdgeKind::Operation), 2 => Some(EdgeKind::Reaction), _ => return Err(invalid("Unknown edge kind")) };
    if operation == 5 { return data(&graph.shortest_path(source, target, kind)?); }
    data(&match operation {
        0 => graph.outgoing(source, kind)?, 1 => graph.incoming(source, kind)?,
        2 => graph.successors(source, kind)?, 3 => graph.predecessors(source, kind)?,
        4 => graph.reachable(source, kind)?, _ => return Err(invalid("Unknown traversal operation")),
    })
});
api!(smsl_graph_dump(value: *const SmslGraph, format: u32) -> *mut SmslBuffer {
    let graph = &get(value)?.0;
    Ok(own(SmslBuffer(match format { 0 => graph.to_json(), 1 => graph.to_dot(), _ => return Err(invalid("Unknown graph format")) })))
});
free!(smsl_graph_free, SmslGraph);
