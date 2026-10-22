//! The transport manifest (Go: `transport_manifest.go`): the exact rows that
//! may differ from the immediately preceding table. It is private to the
//! producer; the wire schema never carries it.

use std::collections::HashSet;
use std::sync::Arc;

use super::ProducerTable;

#[derive(Clone, Debug, Default)]
pub struct TransportChanges {
    pub base_generation: u64,
    pub base_state_id: u64,
    pub source_paths: HashSet<String>,
    pub entity_paths: HashSet<String>,
    pub file_paths: HashSet<String>,
    pub symbol_ids: HashSet<Arc<str>>,
    pub exact: bool,
}

/// What the demand closure learned about symbols while building a table
/// (Go: the fields of `closureBuilder` the manifest reads).
#[derive(Clone, Debug, Default)]
pub struct ChangedClosure {
    pub reference_changes_exact: bool,
    pub changed_symbols: Vec<Arc<str>>,
}

/// Go's `filepath.Clean` for slash-separated paths.
pub(super) fn clean_path(path: &str) -> String {
    if path.is_empty() {
        return ".".to_owned();
    }
    let rooted = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for element in path.split('/') {
        match element {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|last| *last != "..") {
                    parts.pop();
                } else if !rooted {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    let joined = parts.join("/");
    match (rooted, joined.is_empty()) {
        (true, _) => format!("/{joined}"),
        (false, true) => ".".to_owned(),
        (false, false) => joined,
    }
}

/// Symbols reachable from the changed paths through alias edges.
#[must_use]
pub fn retained_symbol_candidates(
    table: Option<&ProducerTable>,
    changed_paths: &HashSet<String>,
) -> HashSet<Arc<str>> {
    let mut candidates = HashSet::new();
    let Some(table) = table else {
        return candidates;
    };
    for path in changed_paths {
        collect_path_symbols(table, &clean_path(path), &mut candidates);
    }
    let mut queue: Vec<Arc<str>> = candidates.iter().cloned().collect();
    let mut index = 0;
    while index < queue.len() {
        let target = table
            .symbol(&queue[index])
            .map(|fact| fact.alias_target.clone())
            .filter(|target| !target.is_empty());
        index += 1;
        if let Some(target) = target
            && candidates.insert(target.clone())
        {
            queue.push(target);
        }
    }
    candidates
}

/// Names the rows that may differ from `previous`, or `None` when the closure
/// could not prove its reference changes exact.
#[must_use]
pub fn transport_manifest(
    previous: Option<&ProducerTable>,
    next: &ProducerTable,
    builder: Option<&ChangedClosure>,
    changed_paths: &HashSet<String>,
) -> Option<TransportChanges> {
    let previous = previous?;
    let builder = builder.filter(|builder| builder.reference_changes_exact)?;
    let mut manifest = TransportChanges {
        base_generation: previous.generation,
        base_state_id: previous.state_id,
        exact: true,
        ..TransportChanges::default()
    };
    for path in changed_paths {
        let path = clean_path(path);
        collect_path_symbols(previous, &path, &mut manifest.symbol_ids);
        collect_path_symbols(next, &path, &mut manifest.symbol_ids);
        manifest.source_paths.insert(path.clone());
        manifest.entity_paths.insert(path.clone());
        manifest.file_paths.insert(path);
    }
    manifest
        .symbol_ids
        .extend(builder.changed_symbols.iter().cloned());

    // Symbol closure follows only alias edges. Expanding old and new targets
    // from every changed seed therefore captures additions, removals, and
    // full-tier reference changes without walking unrelated symbol rows.
    let mut queue: Vec<Arc<str>> = manifest.symbol_ids.iter().cloned().collect();
    let mut index = 0;
    while index < queue.len() {
        let id = queue[index].clone();
        index += 1;
        for table in [previous, next] {
            if let Some(fact) = table.symbol(&id)
                && !fact.alias_target.is_empty()
                && manifest.symbol_ids.insert(fact.alias_target.clone())
            {
                queue.push(fact.alias_target.clone());
            }
        }
    }
    Some(manifest)
}

/// Names only path rows. The active protocol hands symbol closure ownership to
/// the Rust client, so the producer emits no second symbol delta.
#[must_use]
pub fn rust_owned_transport_manifest(
    previous: Option<&ProducerTable>,
    changed_paths: Option<&HashSet<String>>,
) -> Option<TransportChanges> {
    let previous = previous?;
    let changed_paths = changed_paths?;
    let mut manifest = TransportChanges {
        base_generation: previous.generation,
        base_state_id: previous.state_id,
        exact: true,
        ..TransportChanges::default()
    };
    for path in changed_paths {
        let path = clean_path(path);
        manifest.source_paths.insert(path.clone());
        manifest.entity_paths.insert(path.clone());
        manifest.file_paths.insert(path);
    }
    Some(manifest)
}

fn collect_path_symbols(table: &ProducerTable, path: &str, symbols: &mut HashSet<Arc<str>>) {
    if let Some(path_symbols) = &table.path_symbols {
        if let Some(ids) = path_symbols.get(path) {
            symbols.extend(ids.iter().cloned());
        }
        return;
    }
    // Go scans only the flat rows, because a borrowed-run table always carries
    // `path_symbols`. Reading either representation can only name more
    // candidates, never fewer.
    for entity in table.entities.path(path) {
        if !entity.symbol.is_empty() {
            symbols.insert(entity.symbol.clone());
        }
        if let Some(call) = &entity.resolved_call
            && !call.target.is_empty()
        {
            symbols.insert(call.target.clone());
        }
    }
    if let Some(file) = table.file(path) {
        for function in file.async_functions.iter() {
            if !function.symbol.is_empty() {
                symbols.insert(function.symbol.clone());
            }
            if !function.target.is_empty() {
                symbols.insert(function.target.clone());
            }
        }
    }
}
