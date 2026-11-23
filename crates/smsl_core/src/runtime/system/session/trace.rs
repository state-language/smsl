use crate::Diagnostic;
use crate::runtime::system::{TraceEntry, TracePage, error};

#[derive(Default)]
pub(super) struct TraceLog {
    entries: Vec<TraceEntry>,
    start: usize,
    next: u64,
    retention: Option<u64>,
}

impl TraceLog {
    pub fn entries(&self) -> &[TraceEntry] {
        &self.entries[self.start..]
    }

    pub fn next(&self) -> u64 {
        self.next
    }

    pub fn push(&mut self, entry: TraceEntry) {
        self.next = self.next.checked_add(1).expect("trace index exhausted");
        self.entries.push(entry);
        self.prune();
    }

    pub fn retain(&mut self, limit: Option<u64>) -> Result<(), Diagnostic> {
        if limit == Some(0) {
            return Err(error(
                "invalid_trace_retention",
                "Trace retention must be positive or null",
            ));
        }
        self.retention = limit;
        self.prune();
        self.entries.shrink_to_fit();
        Ok(())
    }

    pub fn read(&self, cursor: u64, limit: u64) -> Result<TracePage, Diagnostic> {
        if limit == 0 {
            return Err(error(
                "invalid_trace_limit",
                "Trace read limit must be positive",
            ));
        }
        if cursor > self.next {
            return Err(error(
                "invalid_trace_cursor",
                "Trace cursor is beyond the session's next index",
            ));
        }
        let retained = self.entries();
        let first = retained.first().map_or(self.next, |entry| entry.index);
        let effective = cursor.max(first);
        let offset = usize::try_from(effective - first).expect("retained trace offset");
        let count = limit.min((retained.len() - offset) as u64) as usize;
        let entries = retained[offset..offset + count].to_vec();
        let next = effective + entries.len() as u64;
        Ok(TracePage {
            entries,
            next,
            first,
            truncated: cursor < first,
        })
    }

    fn prune(&mut self) {
        if let Some(limit) = self.retention {
            let retained = self.entries.len() - self.start;
            let excess = retained - limit.min(retained as u64) as usize;
            self.entries[self.start..self.start + excess]
                .iter_mut()
                .for_each(|entry| {
                    entry.detail = None;
                });
            self.start += excess;
            if self.start > self.entries.len() / 2 {
                self.entries.drain(..self.start);
                self.start = 0;
            }
        }
    }
}
