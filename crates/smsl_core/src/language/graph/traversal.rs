use std::collections::{HashSet, VecDeque};

use crate::Diagnostic;

use super::{EdgeKind, Graph};

impl Graph {
    pub fn outgoing(&self, node: usize, kind: Option<EdgeKind>) -> Result<Vec<usize>, Diagnostic> {
        self.validate_node(node)?;
        Ok(self.filtered(&self.outgoing[node], kind).collect())
    }

    pub fn incoming(&self, node: usize, kind: Option<EdgeKind>) -> Result<Vec<usize>, Diagnostic> {
        self.validate_node(node)?;
        Ok(self.filtered(&self.incoming[node], kind).collect())
    }

    pub fn successors(
        &self,
        node: usize,
        kind: Option<EdgeKind>,
    ) -> Result<Vec<usize>, Diagnostic> {
        let mut seen = HashSet::new();
        Ok(self
            .outgoing(node, kind)?
            .into_iter()
            .map(|edge| self.edges[edge].target)
            .filter(|node| seen.insert(*node))
            .collect())
    }

    pub fn predecessors(
        &self,
        node: usize,
        kind: Option<EdgeKind>,
    ) -> Result<Vec<usize>, Diagnostic> {
        let mut seen = HashSet::new();
        Ok(self
            .incoming(node, kind)?
            .into_iter()
            .map(|edge| self.edges[edge].source)
            .filter(|node| seen.insert(*node))
            .collect())
    }

    pub fn reachable(
        &self,
        source: usize,
        kind: Option<EdgeKind>,
    ) -> Result<Vec<usize>, Diagnostic> {
        self.validate_node(source)?;
        let mut visited = vec![false; self.nodes.len()];
        visited[source] = true;
        let mut queue = VecDeque::from([source]);
        let mut reached = Vec::new();
        while let Some(node) = queue.pop_front() {
            reached.push(node);
            for edge in self.filtered(&self.outgoing[node], kind) {
                let target = self.edges[edge].target;
                if !visited[target] {
                    visited[target] = true;
                    queue.push_back(target);
                }
            }
        }
        Ok(reached)
    }

    pub fn shortest_path(
        &self,
        source: usize,
        target: usize,
        kind: Option<EdgeKind>,
    ) -> Result<Option<Vec<usize>>, Diagnostic> {
        self.validate_node(source)?;
        self.validate_node(target)?;
        if source == target {
            return Ok(Some(Vec::new()));
        }
        let mut visited = vec![false; self.nodes.len()];
        let mut previous = vec![None; self.nodes.len()];
        visited[source] = true;
        let mut queue = VecDeque::from([source]);
        while let Some(node) = queue.pop_front() {
            for edge in self.filtered(&self.outgoing[node], kind) {
                let next = self.edges[edge].target;
                if visited[next] {
                    continue;
                }
                visited[next] = true;
                previous[next] = Some(edge);
                if next == target {
                    let mut path = Vec::new();
                    let mut current = target;
                    while current != source {
                        let edge = previous[current].expect("BFS visited every preceding node");
                        path.push(edge);
                        current = self.edges[edge].source;
                    }
                    path.reverse();
                    return Ok(Some(path));
                }
                queue.push_back(next);
            }
        }
        Ok(None)
    }

    fn filtered<'a>(
        &'a self,
        edges: &'a [usize],
        kind: Option<EdgeKind>,
    ) -> impl Iterator<Item = usize> + 'a {
        edges
            .iter()
            .copied()
            .filter(move |edge| kind.is_none_or(|kind| self.edges[*edge].kind == kind))
    }
}
