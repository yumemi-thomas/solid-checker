//! ADR 1028: an unknown global decided by the global-write census.
//!
//! A package file that looks up a global no pinned library declares (`this.__createBinding`, `process`, `window.trustedTypes`, `XRWebGLBinding`) leaves its closure
//! undecided: the host, a script or a package may provide the global, and what it provides may run anything. Who may provide a global is a fact of the whole program:
//! the global-write census of every package closure walked, of the project's own files and of the scripts its HTML entries run. A lookup of the global `N` is decided
//! when nothing in the program writes `N` (a data write or an accessor, by name) and nothing writes globals under a name the syntax does not fix, except a package
//! whose own closure is clear (it writes only its own code, which names no Solid package). Then `N` is the host's or absent: a host object runs no project or package
//! code of its own, and an absent global is `undefined` or throws a `ReferenceError`. The lookup is then no problem of the closure, and the closure's verdict is its next
//! problem, or clear.
//!
//! The project's side fails closed: its own files are scanned as the packages' are; its HTML entries (every `.html` file below the package root that holds the project,
//! `node_modules` and hidden directories aside) are read as source, each script that runs (inline or external, classic or module) scanned, an inline event handler or a
//! `javascript:` URL scanned as a function body. A script the census cannot read (a remote `src`, a file that is missing or does not parse, a script whose own loads
//! the program does not hold), unanalyzed project JavaScript, or no HTML entry at all is a writer of any name, and nothing is decided for the project.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use solid_facts::module_reach::scan_module;
use solid_facts::package_reach::{PackageReach, ReachBlocker};

use super::Closure;

/// What the project's own side of the program writes on the global object.
#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct ProjectWrites {
    /// The names a project file or an HTML entry's script writes or defines an accessor under.
    pub(super) names: BTreeSet<String>,
    /// Some code of the project may write a global under any name, or the census could not read all of it: nothing is decided. `why` says what.
    pub(super) any: Option<String>,
}

impl ProjectWrites {
    fn writes(&mut self, scan: &solid_facts::module_reach::ModuleScan, what: &str) {
        let writes = &scan.global_writes;
        self.names.extend(writes.names.iter().cloned());
        self.names.extend(writes.accessor_names.iter().cloned());
        if (writes.any_name || writes.accessors_any_name) && self.any.is_none() {
            self.any = Some(format!("{what} writes a global under a name the syntax does not fix"));
        }
    }

    fn any(&mut self, why: String) {
        if self.any.is_none() {
            self.any = Some(why);
        }
    }
}

/// The project's global writes: its program files (outside every `node_modules`), its unanalyzed JavaScript, and the scripts of its HTML entries below `package_root`
/// (`None`: no root is known, so nothing is decided).
pub(super) fn project_writes(
    facts: &solid_facts::ProjectFacts,
    project_js: &BTreeSet<String>,
    package_root: Option<&Path>,
) -> ProjectWrites {
    let mut writes = ProjectWrites::default();
    let program = facts
        .files
        .iter()
        .filter(|file| {
            let path = file.path.as_str();
            !path.contains("/node_modules/")
                && facts.dependency_source_root(path).is_none()
                && !solid_facts::package_reach::is_declaration_file(path)
        })
        .map(|file| (PathBuf::from(file.path.as_str()), file))
        .collect::<HashMap<_, _>>();
    for (path, file) in &program {
        writes.writes(&scan_module(path, &file.source), file.path.as_str());
    }
    if let Some(js) = project_js.iter().next() {
        writes.any(format!(
            "{js}: the project's JavaScript beside it is not analyzed"
        ));
    }
    let Some(root) = package_root else {
        writes.any("the project's package root is not known".to_owned());
        return writes;
    };
    let pages = html_entries(root);
    if pages.is_empty() {
        writes.any(format!(
            "no HTML entry below {}: the page that loads the program is not read",
            root.display()
        ));
    }
    let program_paths = program
        .keys()
        .filter_map(|path| path.canonicalize().ok())
        .collect::<BTreeSet<_>>();
    for page in pages {
        page_writes(&page, root, &program_paths, &mut writes);
    }
    writes
}

/// Every `.html` file below `root`, `node_modules` and hidden directories aside, in path order.
fn html_entries(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if name != "node_modules" && !name.starts_with('.') {
                    stack.push(entry.path());
                }
            } else if kind.is_file() && name.ends_with(".html") {
                found.push(entry.path());
            }
        }
    }
    found.sort();
    found
}

/// The script types that run as JavaScript (HTML's JavaScript MIME type essence match, `module`, and none).
fn runs_as_script(kind: Option<&str>) -> Option<bool> {
    let Some(kind) = kind.map(|kind| kind.trim().to_ascii_lowercase()) else {
        return Some(false);
    };
    match kind.as_str() {
        "" => Some(false),
        "module" => Some(true),
        "application/ecmascript" | "application/javascript" | "application/x-ecmascript" | "application/x-javascript"
        | "text/ecmascript" | "text/javascript" | "text/javascript1.0" | "text/javascript1.1" | "text/javascript1.2"
        | "text/javascript1.3" | "text/javascript1.4" | "text/javascript1.5" | "text/jscript" | "text/livescript"
        | "text/x-ecmascript" | "text/x-javascript" => Some(false),
        // Any other type is a data block the browser does not run (`importmap`, `application/json`, `speculationrules`, ...).
        _ => None,
    }
}

/// One HTML entry: every script it runs, scanned.
fn page_writes(
    page: &Path,
    root: &Path,
    program: &BTreeSet<PathBuf>,
    writes: &mut ProjectWrites,
) {
    let Ok(text) = fs::read_to_string(page) else {
        writes.any(format!("{}: the HTML entry cannot be read", page.display()));
        return;
    };
    page_text_writes(&text, page, root, program, writes);
}

/// [`page_writes`] of the entry's text.
fn page_text_writes(
    text: &str,
    page: &Path,
    root: &Path,
    program: &BTreeSet<PathBuf>,
    writes: &mut ProjectWrites,
) {
    use html5ever::tendril::TendrilSink;
    use markup5ever_rcdom::{NodeData, RcDom};
    let shown = page.display().to_string();
    let text = text.to_owned();
    let dom = html5ever::parse_document(RcDom::default(), html5ever::ParseOpts::default()).one(text);
    // `dom` holds the tree for the whole walk: a dropped `rcdom` node empties the children of its descendants (its iterative `Drop`), so a walk that let the
    // document go read an empty tree.
    let mut stack = vec![dom.document.clone()];
    while let Some(node) = stack.pop() {
        if let NodeData::Element { name, attrs, .. } = &node.data {
            let attrs = attrs.borrow();
            let attr = |wanted: &str| {
                attrs
                    .iter()
                    .find(|attr| attr.name.ns.is_empty() && &*attr.name.local == wanted)
                    .map(|attr| attr.value.to_string())
            };
            for held in attrs.iter() {
                let local = held.name.local.to_ascii_lowercase();
                let value = held.value.trim_start().to_ascii_lowercase();
                // An inline event handler and a `javascript:` URL run as code.
                if local.starts_with("on") || value.starts_with("javascript:") {
                    let body = if local.starts_with("on") {
                        held.value.to_string()
                    } else {
                        held.value.trim_start()["javascript:".len()..].to_string()
                    };
                    let what = format!("{shown}: the `{local}` attribute");
                    snippet(&format!("(function (event) {{\n{body}\n}});\n"), false, &what, writes);
                }
            }
            if name.ns == html5ever::ns!(html) && &*name.local == "script" {
                let Some(module) = runs_as_script(attr("type").as_deref()) else {
                    continue;
                };
                match attr("src") {
                    Some(src) => external(page, root, &src, module, program, writes),
                    None => {
                        let mut body = String::new();
                        for child in node.children.borrow().iter() {
                            if let NodeData::Text { contents } = &child.data {
                                body.push_str(&contents.borrow());
                            }
                        }
                        snippet(&body, module, &format!("{shown}: an inline script"), writes);
                    }
                }
                continue;
            }
        }
        stack.extend(node.children.borrow().iter().cloned());
    }
}

/// Code an HTML entry runs, scanned as a file of its kind: a module (`.mjs`) or a classic script (`.cjs`, whose top-level `this` is the global object, as the scan reads
/// any script's). Its loads are code the census does not hold.
fn snippet(code: &str, module: bool, what: &str, writes: &mut ProjectWrites) {
    let path = Path::new(if module { "inline.mjs" } else { "inline.js" });
    let scan = scan_module(path, code);
    if scan
        .hazards
        .iter()
        .any(|hazard| hazard.kind == solid_facts::module_reach::ModuleHazardKind::Unparseable)
    {
        writes.any(format!("{what} does not parse"));
        return;
    }
    if !scan.loads.is_empty() {
        writes.any(format!("{what} loads code the census does not read"));
    }
    writes.writes(&scan, what);
}

/// A script an HTML entry loads by `src`: a file of the program is scanned with it; any other local file is scanned here, and it may load nothing; a remote or missing
/// one is a writer of any name.
fn external(
    page: &Path,
    root: &Path,
    src: &str,
    module: bool,
    program: &BTreeSet<PathBuf>,
    writes: &mut ProjectWrites,
) {
    let shown = page.display();
    let src = src.trim();
    let local = src.split(['?', '#']).next().unwrap_or(src);
    if local.contains("://") || local.starts_with("//") || local.starts_with("data:") || local.is_empty() {
        writes.any(format!("{shown}: the script `{src}` is not read"));
        return;
    }
    let base = if local.starts_with('/') { root } else { page.parent().unwrap_or(root) };
    let file = base.join(local.trim_start_matches('/'));
    let Ok(file) = file.canonicalize() else {
        writes.any(format!("{shown}: the script `{src}` is not found"));
        return;
    };
    if program.contains(&file) {
        return;
    }
    let Ok(code) = fs::read_to_string(&file) else {
        writes.any(format!("{shown}: the script `{src}` cannot be read"));
        return;
    };
    snippet(&code, module, &format!("{shown}: the script `{src}`"), writes);
}

/// Decides each closure's lookups of unknown globals by the census, to a fixed point: a lookup of `N` is no problem when no package closure and no project code writes
/// `N` and every package closure that writes globals under any name is clear. A closure whose problems stopped at their limit is not changed.
pub(super) fn decide(closures: &mut HashMap<PathBuf, Closure>, project: &ProjectWrites) {
    if project.any.is_some() {
        return;
    }
    let mut named = project.names.clone();
    let mut any_writers = Vec::new();
    for (root, closure) in closures.iter() {
        let writes = &closure.global_writes;
        named.extend(writes.names.iter().cloned());
        named.extend(writes.accessor_names.iter().cloned());
        if writes.any_name || writes.accessors_any_name {
            any_writers.push(root.clone());
        }
    }
    loop {
        if !any_writers
            .iter()
            .all(|root| closures.get(root).is_some_and(|closure| closure.reach.is_clear()))
        {
            return;
        }
        let mut changed = false;
        for closure in closures.values_mut() {
            if closure.truncated || closure.reach.is_clear() {
                continue;
            }
            let decided = |index: usize| {
                matches!(
                    closure.problems[index],
                    PackageReach::Undecided { blocker: ReachBlocker::UnknownGlobal, .. }
                ) && !closure.problem_globals[index].is_empty()
                    && closure.problem_globals[index].iter().all(|name| !named.contains(name))
            };
            let reach = (0..closure.problems.len())
                .find(|&index| !decided(index))
                .map_or(PackageReach::Clear, |index| closure.problems[index].clone());
            if reach != closure.reach {
                closure.reach = reach;
                changed = true;
            }
        }
        if !changed {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(html: &str) -> ProjectWrites {
        let mut writes = ProjectWrites::default();
        let root = Path::new("/nowhere");
        page_text_writes(html, &root.join("index.html"), root, &BTreeSet::new(), &mut writes);
        writes
    }

    #[test]
    fn an_inline_classic_script_writes_its_globals() {
        let writes = read("<!doctype html><html><body><script>window.fzPolicy = {};</script></body></html>");
        assert_eq!(writes.any, None);
        assert!(writes.names.contains("fzPolicy"), "{writes:?}");
    }

    #[test]
    fn an_event_handler_and_a_javascript_url_are_code() {
        let writes = read(r#"<body onload="window.fzEnv = 1"><a href="javascript:window.fzUrl = 1">x</a></body>"#);
        assert!(writes.names.contains("fzEnv") && writes.names.contains("fzUrl"), "{writes:?}");
    }

    #[test]
    fn a_remote_script_writes_any_global() {
        assert!(read(r#"<script src="https://cdn.example.com/tag.js"></script>"#).any.is_some());
        assert!(read(r#"<script type="module" src="./missing.js"></script>"#).any.is_some());
    }

    #[test]
    fn a_data_block_runs_nothing() {
        let writes = read(r#"<script type="importmap">{"imports":{}}</script><script type="application/json">{"a":1}</script><template><script>window.x = 1</script></template>"#);
        assert_eq!(writes, ProjectWrites::default());
    }
}
