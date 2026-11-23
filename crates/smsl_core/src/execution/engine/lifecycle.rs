use super::super::{Block, Session, Status, session::Progress};
use crate::{Diagnostic, runtime::Step};
use std::collections::BTreeMap;

impl Session {
    pub(crate) fn subtree(&self, id: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut pending = vec![id.to_owned()];
        while let Some(id) = pending.pop() {
            pending.extend(self.definition.blocks[&id].members().into_iter().rev());
            result.push(id);
        }
        result
    }

    pub(super) fn exclusive(&self, left: &str, right: &str) -> bool {
        let mut parents = BTreeMap::new();
        for (id, block) in &self.definition.blocks {
            for child in block.members() {
                parents.insert(child, id.as_str());
            }
        }
        let path = |id: &str| {
            let mut result = vec![id.to_owned()];
            let mut current = id;
            while let Some(parent) = parents.get(current) {
                result.push((*parent).to_owned());
                current = parent;
            }
            result.reverse();
            result
        };
        let left = path(left);
        let right = path(right);
        let common = left
            .iter()
            .zip(&right)
            .take_while(|(left, right)| left == right)
            .count();
        common < left.len()
            && common < right.len()
            && !matches!(
                self.definition.blocks[&left[common - 1]],
                Block::Parallel { .. }
            )
    }

    pub(crate) fn finish_stops(&mut self, steps: &mut Vec<Step>) -> Result<(), Diagnostic> {
        let mut scopes: Vec<_> = self
            .stopping
            .iter()
            .map(|id| (self.subtree(id).len(), id.clone()))
            .collect();
        scopes.sort();
        for (_, id) in scopes {
            let subtree = self.subtree(&id);
            if subtree.iter().any(|id| self.pending.contains_key(id)) {
                continue;
            }
            let Block::Scope {
                member,
                exit,
                retain,
                ..
            } = self.definition.blocks[&id].clone()
            else {
                unreachable!("scope stop")
            };
            if let Some(exit) = exit {
                self.hook(&exit, steps)?;
            }
            let progress = self.progress.get_mut(&id).expect("scope block");
            progress.entered = false;
            progress.status = Status::Idle;
            if !retain {
                for child in self.subtree(&member) {
                    self.progress.insert(child, Progress::default());
                }
            }
            self.stopping.remove(&id);
        }
        Ok(())
    }
}
