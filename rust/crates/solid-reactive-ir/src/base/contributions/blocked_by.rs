//! ADR 0947: why a standard call is withheld, as typed facts.
//!
//! A standard call's row is held by a *formula* over whole-program hazards (`handed_root_patch`, `impostor_veto`), not by one
//! fact: lifting the veto takes every true disjunct out, so "the fact that would decide it" depends on evaluation order. Each
//! formula therefore lists its atoms, the hazards it can read, and a row names the atoms that were present. `fired` is the set
//! of present atoms, whatever order the predicate reads them in, so two runs and two machines agree.
//!
//! The atom lists restate what the predicates read; `every_hazard_a_formula_reads_is_one_of_its_atoms` fails when one stops
//! doing so.

use std::collections::{BTreeMap, BTreeSet};

use super::*;

/// A hazard a veto formula can read, by the id a row and the report name it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HazardAtom {
    pub(crate) id: String,
    pub(crate) hazard: Hazard,
}

fn atom(id: impl Into<String>, hazard: Hazard) -> HazardAtom {
    HazardAtom {
        id: id.into(),
        hazard,
    }
}

/// A census key as an id reads it: `*` for a key no fact names, `<symbol>` for a key a site took for a symbol.
fn key_name(key: &str) -> &str {
    match key {
        ANY_MEMBER => "*",
        SYMBOL_SITE => "<symbol>",
        SYMBOL_REPLACE => "<symbol-replace>",
        key => key,
    }
}

/// The census key an id's name stands for ([`key_name`] read back).
fn key_of(name: &str) -> &str {
    match name {
        "*" => ANY_MEMBER,
        "<symbol>" => SYMBOL_SITE,
        "<symbol-replace>" => SYMBOL_REPLACE,
        name => name,
    }
}

/// The hazards [`handed_root_patch`] can read for `qualified_name`. Its formula: `written && (StandardRootHanded ||
/// ((StandardRootReceived || StandardRootIterated) && RootPatch) || (StandardRootIterated && iterated_open))`, `written`
/// being an open write of the member (or of any key), a reflective definer spelled, or an open write under a symbol with
/// `Symbol` replaceable.
pub(crate) fn handed_root_atoms(qualified_name: &str) -> Vec<HazardAtom> {
    let member = qualified_name
        .rsplit_once('.')
        .map_or(qualified_name, |(_, member)| member);
    let mut atoms = vec![
        atom(
            format!("OpenMemberWrite({})", key_name(ANY_MEMBER)),
            open_member_write(ANY_MEMBER),
        ),
        atom(
            format!("OpenMemberWrite({})", key_name(member)),
            open_member_write(member),
        ),
        atom(
            "ReflectiveDefinitionSpelling",
            Hazard::ReflectiveDefinitionSpelling,
        ),
        atom(
            format!("OpenMemberWrite({})", key_name(SYMBOL_SITE)),
            open_member_write(SYMBOL_SITE),
        ),
        atom("StandardPatch(.Symbol)", standard_patch("", "Symbol")),
        atom("StandardPatch(.*)", standard_patch("", ANY_MEMBER)),
        atom(
            "StandardImpostorWrite(Symbol)",
            standard_impostor_write("Symbol"),
        ),
        atom("StandardRootHanded", Hazard::StandardRootHanded),
        atom("StandardRootReceived", Hazard::StandardRootReceived),
        atom("StandardRootIterated", Hazard::StandardRootIterated),
        atom("RootPatch", Hazard::RootPatch),
        atom(
            format!("StandardImpostor({})", key_name(ANY_MEMBER)),
            standard_impostor(ANY_MEMBER),
        ),
        atom(
            format!("StandardOverride(Array.{})", key_name(ANY_MEMBER)),
            standard_override("Array", ANY_MEMBER),
        ),
    ];
    for name in ROOT_ITERATORS {
        atoms.push(atom(
            format!("StandardImpostor({name})"),
            standard_impostor(name),
        ));
        atoms.push(atom(
            format!("StandardOverride(Array.{name})"),
            standard_override("Array", name),
        ));
    }
    atoms
}

/// The hazards [`impostor_veto`]'s named arms can read for `qualified_name`: an impostor of the member or of any key, and
/// the `Symbol.replace` site for `replace` and `replaceAll`. The arm that asks whether a definer defines the member
/// reflectively, and the receiver proof the formula ends in, are no hazards: `fired` does not list them.
pub(crate) fn impostor_atoms(qualified_name: &str) -> Vec<HazardAtom> {
    let member = qualified_name
        .rsplit_once('.')
        .map_or(qualified_name, |(_, member)| member);
    let mut atoms = vec![
        atom(
            format!("StandardImpostor({})", key_name(member)),
            standard_impostor(member),
        ),
        atom(
            format!("StandardImpostor({})", key_name(ANY_MEMBER)),
            standard_impostor(ANY_MEMBER),
        ),
    ];
    if matches!(member, "replace" | "replaceAll") {
        atoms.push(atom(
            format!("StandardImpostor({})", key_name(SYMBOL_REPLACE)),
            standard_impostor(SYMBOL_REPLACE),
        ));
    }
    atoms
}

/// ADR 0950 (`--assume`): the hazard an id names, the inverse of the ids above (`OpenMemberWrite(*)`, `RootPatch`, `StandardImpostor(forEach)`,
/// `StandardOverride(Array.map)`, `StandardPatch(.Symbol)`, `StandardPatch(*.then)`, ...), or none. Only the hazards a veto formula reads by
/// name or by a census key; the ordinal ones (`Eval`, `SynchronousDispatch`, ...) have no id and no seam.
#[cfg(feature = "what-if")]
pub(crate) fn hazard_of_id(id: &str) -> Option<Hazard> {
    let (head, argument) = match id.split_once('(') {
        Some((head, rest)) => (head, Some(rest.strip_suffix(')')?)),
        None => (id, None),
    };
    Some(match (head, argument) {
        ("ReflectiveDefinitionSpelling", None) => Hazard::ReflectiveDefinitionSpelling,
        ("StandardRootHanded", None) => Hazard::StandardRootHanded,
        ("StandardRootReceived", None) => Hazard::StandardRootReceived,
        ("StandardRootIterated", None) => Hazard::StandardRootIterated,
        ("RootPatch", None) => Hazard::RootPatch,
        ("AnyStandardPatch", None) => Hazard::AnyStandardPatch,
        ("OpenMemberWrite", Some(key)) => open_member_write(key_of(key)),
        ("StandardImpostor", Some(key)) => standard_impostor(key_of(key)),
        ("StandardImpostorWrite", Some(key)) => standard_impostor_write(key_of(key)),
        ("StandardOverride", Some(rest)) => {
            let (owner, member) = rest.split_once('.')?;
            standard_override(owner, key_of(member))
        }
        ("StandardPatch", Some(rest)) => {
            let (owner, member) = rest.split_once('.')?;
            standard_patch(owner, key_of(member))
        }
        _ => return None,
    })
}

/// The ids of the `atoms` the program has, sorted, so a row names the same ones in any build.
fn fired(atoms: Vec<HazardAtom>, hazard: &impl Fn(Hazard) -> bool) -> Vec<String> {
    let mut fired = atoms
        .into_iter()
        .filter(|atom| hazard(atom.hazard))
        .map(|atom| atom.id)
        .collect::<Vec<_>>();
    fired.sort_unstable();
    fired.dedup();
    fired
}

/// The atoms of [`handed_root_patch`] present, when its formula holds for `qualified_name`: the cause of the withholding
/// `impostor_withheld` asks first. A function of the name and the program's hazards, so a caller may keep it by name.
pub(crate) fn handed_root_cause(
    qualified_name: &str,
    hazard: &impl Fn(Hazard) -> bool,
) -> Option<Vec<String>> {
    handed_root_patch(qualified_name, hazard)
        .then(|| fired(handed_root_atoms(qualified_name), hazard))
}

/// The atoms of [`impostor_veto`] present for `qualified_name`, once the caller has found that it holds (it needs the call's
/// file and receiver, so the answer is the caller's).
pub(crate) fn impostor_cause(
    qualified_name: &str,
    hazard: &impl Fn(Hazard) -> bool,
) -> Vec<String> {
    fired(impostor_atoms(qualified_name), hazard)
}

/// A site that contributes a hazard: where the program holds it.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct HazardSiteAt {
    pub(crate) path: String,
    pub(crate) start: u32,
    pub(crate) end: u32,
}

/// The sites that contribute each hazard of `ids` (ids of [`handed_root_atoms`] or [`impostor_atoms`]), by path then span,
/// where the census records them: an open write of a key (`OpenMemberWrite(key)`) and a function put at a member
/// (`StandardImpostor(member)`). The others (`StandardRootHanded`, a patch, ...) keep no per-site record, so their sites
/// are absent, never guessed. Every id is a key of the answer. One pass over `files`: the census's per-file scan runs once
/// per file, not once per id.
pub(crate) fn hazard_sites(
    files: &[FileFacts],
    ids: &BTreeSet<String>,
) -> BTreeMap<String, Vec<HazardSiteAt>> {
    // The census site each id asks for: its variant and key.
    let wanted = ids
        .iter()
        .filter_map(|id| {
            let (variant, key) = id.strip_suffix(')')?.split_once('(')?;
            matches!(variant, "OpenMemberWrite" | "StandardImpostor")
                .then(|| (id.as_str(), variant, key_of(key)))
        })
        .collect::<Vec<_>>();
    let mut found = ids
        .iter()
        .map(|id| (id.clone(), Vec::new()))
        .collect::<BTreeMap<String, Vec<HazardSiteAt>>>();
    if wanted.is_empty() {
        return found;
    }
    let reads_writes = wanted
        .iter()
        .any(|(_, variant, _)| *variant == "OpenMemberWrite");
    for file in files {
        let (members, _, open) = impostor_sites(file);
        let writes = if reads_writes {
            open_writes(file, open)
        } else {
            Vec::new()
        };
        for (id, variant, key) in &wanted {
            let spans = match *variant {
                "OpenMemberWrite" => writes
                    .iter()
                    .filter(|(held, _)| held == key)
                    .map(|(_, span)| *span)
                    .collect::<Vec<_>>(),
                _ => members
                    .iter()
                    .filter(|(member, _)| member == key)
                    .map(|(_, span)| *span)
                    .collect::<Vec<_>>(),
            };
            if let Some(sites) = found.get_mut(*id) {
                sites.extend(spans.into_iter().map(|span| HazardSiteAt {
                    path: file.path.as_str().to_owned(),
                    start: span.start,
                    end: span.end,
                }));
            }
        }
    }
    for sites in found.values_mut() {
        sites.sort();
        sites.dedup();
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// ADR 0950: `--assume` takes the ids a row's `fired` names, so every atom's id reads back to its hazard.
    #[cfg(feature = "what-if")]
    #[test]
    fn every_atom_id_names_its_hazard() {
        for name in ["Number.isFinite", "Array.map", "ReadonlyArray.forEach", "Promise.then", "String.replace", "String.replaceAll"] {
            for atom in handed_root_atoms(name).into_iter().chain(impostor_atoms(name)) {
                assert_eq!(hazard_of_id(&atom.id), Some(atom.hazard), "{name}: {}", atom.id);
            }
        }
        // the patch census ids a veto formula names (`patch_veto`): an owner and a member, the reached owner `*`
        assert_eq!(hazard_of_id("StandardPatch(Promise.then)"), Some(standard_patch("Promise", "then")));
        assert_eq!(hazard_of_id("StandardPatch(*.then)"), Some(standard_patch("*", "then")));
        for bad in ["", "Eval", "OpenMemberWrite", "RootPatch(x)", "StandardPatch(then)", "StandardOverride(Array)", "StandardImpostor(a", "standardrootHanded"] {
            assert_eq!(hazard_of_id(bad), None, "{bad}");
        }
    }

    #[test]
    fn every_hazard_a_formula_reads_is_one_of_its_atoms() {
        for name in [
            "Number.isFinite",
            "Array.map",
            "ReadonlyArray.forEach",
            "Promise.then",
            "String.replace",
        ] {
            let atoms = handed_root_atoms(name);
            let hazards = atoms.iter().map(|atom| atom.hazard).collect::<Vec<_>>();
            // Every present-set of up to two atoms: the predicate stops reading when it decides, so the sets
            // reach every disjunct's reads.
            let mut sets = vec![Vec::new()];
            for (index, first) in hazards.iter().enumerate() {
                sets.push(vec![*first]);
                for second in &hazards[index + 1..] {
                    sets.push(vec![*first, *second]);
                }
            }
            for present in sets {
                // The hazards the predicate reads, with `present` standing for the program's.
                let read = RefCell::new(Vec::new());
                let hazard = |hazard: Hazard| {
                    read.borrow_mut().push(hazard);
                    present.contains(&hazard)
                };
                let _ = handed_root_patch(name, &hazard);
                for hazard in read.into_inner() {
                    assert!(
                        hazards.contains(&hazard),
                        "{name}: handed_root_patch read {hazard:?}, no atom of its list"
                    );
                }
            }
        }
    }

    #[test]
    fn a_row_names_the_present_atoms_of_the_formula_that_holds_it() {
        let present = [
            open_member_write("isFinite"),
            Hazard::StandardRootHanded,
            Hazard::StandardRootReceived,
        ];
        let hazard = |hazard: Hazard| present.contains(&hazard);
        assert_eq!(
            handed_root_cause("Number.isFinite", &hazard),
            Some(vec![
                "OpenMemberWrite(isFinite)".to_owned(),
                "StandardRootHanded".to_owned(),
                "StandardRootReceived".to_owned(),
            ])
        );
        // Nothing handed: the formula does not hold, and the impostor veto is the caller's answer.
        let none = |_: Hazard| false;
        assert_eq!(handed_root_cause("Number.isFinite", &none), None);
        let impostor = |hazard: Hazard| hazard == standard_impostor("map");
        assert_eq!(
            impostor_cause("Array.map", &impostor),
            ["StandardImpostor(map)"]
        );
    }

    #[test]
    fn an_id_reads_back_to_the_census_key_it_names() {
        for key in [ANY_MEMBER, SYMBOL_SITE, SYMBOL_REPLACE, "isFinite"] {
            assert_eq!(key_of(key_name(key)), key);
        }
    }
}
