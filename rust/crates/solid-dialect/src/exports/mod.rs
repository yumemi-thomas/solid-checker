//! Which module of a Solid package exports which name.
//!
//! A separate question from the primitive vocabulary in [`crate::Dialect`], and
//! separated because conflating them under-reports. The vocabulary earns a name
//! a place only when the checker models a reactive obligation for it, so
//! `Portal` is not in it — but `import { Portal } from "solid-js"` is still an
//! import from the wrong module, and answering that out of the vocabulary means
//! ten of 1.x's `solid-js/web` names cannot be checked at all.
//!
//! The tables are generated from the installed packages by `solid-contract-gen`
//! and checked by `make contracts-check`. Nothing here is hand-written; the
//! defect that motivated the generator was seven exports that existed only in a
//! hand-maintained list.

pub mod solid_js_1x;
pub mod solid_js_2;
pub mod solidjs_web;

/// Which side of the erasure boundary a name is imported on.
///
/// `import type { X }` erases, so TypeScript lets it name either a type or a
/// value — which is why a type-position lookup consults both tables and a
/// value-position lookup consults only one. Getting that backwards reports
/// `import type { createSignal } from "solid-js"`, which is legal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Position {
    Value,
    Type,
}

/// The modules in `table` that export `name`.
///
/// Empty means this package does not export the name from anywhere, which is
/// deliberately not the same answer as "from the package root": a checker that
/// cannot see a name must say nothing about where it came from.
fn lookup(
    table: &'static [(&'static str, &'static [&'static str])],
    name: &str,
) -> &'static [&'static str] {
    table
        .binary_search_by(|(candidate, _)| (*candidate).cmp(name))
        .map_or(&[], |index| table[index].1)
}

/// The modules of one package that export `name` in `position`.
pub fn modules(
    values: &'static [(&'static str, &'static [&'static str])],
    types: &'static [(&'static str, &'static [&'static str])],
    name: &str,
    position: Position,
) -> Vec<&'static str> {
    let mut found = lookup(values, name).to_vec();
    if position == Position::Type {
        for module in lookup(types, name) {
            if !found.contains(module) {
                found.push(module);
            }
        }
    }
    found
}
