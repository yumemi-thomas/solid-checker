//! Fresh-versus-incremental oracles beyond the fixed scripts of
//! `symbol_integration`: a session driven through seeded random edit
//! sequences, every generation byte-compared with a session that replays the
//! same updates and materializes the whole batch from a program that never
//! saw the earlier generations. Updates retain no answer, so the two must be
//! equal for every family, including the cross-file ones (the reference space
//! and the runtime identity).

use super::project_support::Scratch;
use super::symbol_integration::{
    STRICT_CONFIG, assert_full_transitions_identical, groups, is_ident, open_closure, paths,
};
use crate::closure::DemandClosure;
use crate::context::Context;
use typefacts::Location;
use typefacts::v3::{EntityDemand, FileChange};

/// Demands from the source text alone, so that deriving them mints no identity:
/// every identifier that follows a declaration keyword or a dot, or precedes a
/// call's parenthesis, asks for the symbol, its references, its descriptor and
/// its resolved call. Producer source facts would mint ids while the demands
/// are built and so reorder the claims of the batch that follows.
fn text_demands(closure: &mut DemandClosure) -> Vec<EntityDemand> {
    text_demands_with(closure, true)
}

/// Only the symbol and its references: the smallest demand that still mints.
fn symbol_only_demands(closure: &mut DemandClosure) -> Vec<EntityDemand> {
    text_demands_with(closure, false)
}

fn text_demands_with(closure: &mut DemandClosure, families: bool) -> Vec<EntityDemand> {
    let ctx = Context::new();
    let mut demands = Vec::new();
    for source in closure.source_files(&ctx).unwrap() {
        if !source.path.ends_with(".ts") || source.path.ends_with(".d.ts") {
            continue;
        }
        let path = crate::gocompat::clean(&source.path);
        let bytes = &source.source;
        let mut seen = std::collections::BTreeSet::new();
        let mut index = 0;
        while index < bytes.len() {
            if !is_ident(bytes[index]) || (index > 0 && is_ident(bytes[index - 1])) {
                index += 1;
                continue;
            }
            let start = index;
            while index < bytes.len() && is_ident(bytes[index]) {
                index += 1;
            }
            let word = &bytes[start..index];
            let after = bytes.get(index).copied();
            let before = bytes[..start]
                .iter()
                .rposition(|byte| !byte.is_ascii_whitespace());
            let previous_word = before.map(|end| {
                let mut from = end + 1;
                while from > 0 && is_ident(bytes[from - 1]) {
                    from -= 1;
                }
                &bytes[from..=end]
            });
            let declares = previous_word.is_some_and(|word| {
                matches!(
                    word,
                    b"const" | b"let" | b"var" | b"function" | b"class" | b"interface" | b"type"
                )
            });
            let member = before.is_some_and(|at| bytes[at] == b'.');
            let calls = after == Some(b'(');
            let numeric = word[0].is_ascii_digit();
            if numeric || !(declares || member || calls) || !seen.insert((start, index)) {
                continue;
            }
            demands.push(EntityDemand {
                location: Location {
                    path: path.as_str().into(),
                    start_byte: start as u64,
                    end_byte: index as u64,
                },
                symbol: true,
                references: true,
                type_descriptor: families,
                resolved_call: families,
                callability: families,
                runtime_value_domain: families,
                reference_space: families,
                runtime_identity: families,
                ..EntityDemand::default()
            });
        }
    }
    demands
}

/// Drives `script` through a retained closure and compares each generation with
/// a replayed whole-batch one.
fn assert_script_matches_fresh(
    files: &[(&str, &str)],
    script: impl FnOnce(&Scratch) -> Vec<FileChange>,
    demands_of: fn(&mut DemandClosure) -> Vec<EntityDemand>,
) {
    let ctx = Context::new();
    let scratch = Scratch::new(files);
    let config = scratch.config();
    let changes = script(&scratch);
    let mut incremental = open_closure(&config);
    let initial = demands_of(&mut incremental);
    incremental
        .demand_table_for_groups(&ctx, 1, &groups(&initial), &paths(&initial))
        .unwrap();
    for (step, change) in changes.iter().enumerate() {
        incremental
            .update(&ctx, std::slice::from_ref(change))
            .unwrap_or_else(|error| panic!("incremental update {step}: {error}"));
        let generation = step as u64 + 2;
        let demands = demands_of(&mut incremental);
        let retained = incremental
            .demand_table_for_groups(&ctx, generation, &groups(&demands), &[])
            .unwrap_or_else(|error| panic!("incremental materialization {step}: {error}"));
        let mut fresh = open_closure(&config);
        for (replay, change) in changes.iter().enumerate().take(step + 1) {
            fresh
                .update(&ctx, std::slice::from_ref(change))
                .unwrap_or_else(|error| panic!("fresh replay {replay}/{step}: {error}"));
        }
        let fresh_demands = demands_of(&mut fresh);
        let whole = fresh
            .demand_table_for_groups(&ctx, generation, &groups(&fresh_demands), &[])
            .unwrap_or_else(|error| panic!("fresh materialization {step}: {error}"));
        let (left, right) = (describe(&retained), describe(&whole));
        if left != right {
            let mut report = String::new();
            for line in left.iter().filter(|line| !right.contains(line)).take(6) {
                report.push_str(&format!("  retained only: {line}\n"));
            }
            for line in right.iter().filter(|line| !left.contains(line)).take(6) {
                report.push_str(&format!("  fresh only: {line}\n"));
            }
            panic!("step {step}: retained table differs from fresh\n{report}");
        }
        assert_full_transitions_identical("retained table", step, &config, &retained, &whole);
    }
}

/// One line per entity row, in table order.
pub(super) fn describe(table: &typefacts::producer::ProducerTable) -> Vec<String> {
    let mut lines = Vec::new();
    table
        .entities
        .for_each(|entity| lines.push(format!("{entity:?}")));
    lines
}

fn change(scratch: &Scratch, name: &str, version: u64, source: &str) -> FileChange {
    FileChange {
        path: scratch.path(name),
        source: source.as_bytes().to_vec(),
        deleted: false,
        version,
    }
}

/// Two files use different instantiations of one generic member: the first
/// mint claims the declaration's durable id, the other gets a counter. An edit
/// to a third file must not let the second claim the id the first retained.
#[test]
fn instantiations_of_one_member_keep_distinct_identities_across_edits() {
    let files = [
        ("tsconfig.json", STRICT_CONFIG),
        (
            "a.ts",
            "declare const xs: string[];\nexport const ma = xs.map((x) => x);\n",
        ),
        (
            "b.ts",
            "declare const ys: number[];\nexport const mb = ys.map((y) => y);\n",
        ),
        ("c.ts", "export const c = 1;\n"),
    ];
    assert_script_matches_fresh(
        &files,
        |scratch| {
            (1..=3)
                .map(|version| {
                    change(
                        scratch,
                        "c.ts",
                        version,
                        &format!("export const c = {version};\n"),
                    )
                })
                .collect()
        },
        text_demands,
    );
}

#[test]
fn symbol_only_instantiations_keep_distinct_identities_across_edits() {
    let files = [
        ("tsconfig.json", STRICT_CONFIG),
        (
            "a.ts",
            "declare const xs: string[];\nexport const ma = xs.map((x) => x);\n",
        ),
        (
            "b.ts",
            "declare const ys: number[];\nexport const mb = ys.map((y) => y);\n",
        ),
        ("c.ts", "export const c = 1;\n"),
    ];
    assert_script_matches_fresh(
        &files,
        |scratch| {
            (1..=3)
                .map(|version| {
                    change(
                        scratch,
                        "c.ts",
                        version,
                        &format!("export const c = {version};\n"),
                    )
                })
                .collect()
        },
        symbol_only_demands,
    );
}

// ---- seeded random edit sequences ----------------------------------------

/// xorshift64*: deterministic, dependency-free.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// Every module is a list of independent statement lines, so an edit is a line
/// edit and the project is always a valid compilation unit (type errors are
/// fine; the producer must stay exact on broken code too).
#[derive(Clone, Default)]
struct World {
    /// File name to its statement lines. Script files (`g*.ts`) have no
    /// imports or exports and join the global scope.
    files: std::collections::BTreeMap<String, Vec<String>>,
    /// Files deleted earlier, restorable with their last content.
    graveyard: std::collections::BTreeMap<String, Vec<String>>,
    config: usize,
    counter: usize,
    /// Only constructs whose ids no other symbol can claim (consts, plain
    /// functions, their uses), so answers stay retainable and the retention
    /// logic is exercised instead of bypassed.
    plain: bool,
}

const CONFIGS: [&str; 4] = [
    STRICT_CONFIG,
    r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext","strict":false},"include":["*.ts"]}"#,
    r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"es2017","strict":true,"noUncheckedIndexedAccess":true},"include":["*.ts"]}"#,
    r#"{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext","strict":true},"include":["m*.ts","g*.ts"]}"#,
];

const TYPES: [&str; 5] = ["number", "string", "number[]", "string[]", "boolean"];

impl World {
    fn fresh_name(&mut self, prefix: &str) -> String {
        self.counter += 1;
        format!("{prefix}{}", self.counter)
    }

    fn render(lines: &[String]) -> String {
        let mut text = lines.join("\n");
        text.push('\n');
        text
    }

    fn module_names(&self) -> Vec<String> {
        self.files
            .keys()
            .filter(|name| name.starts_with('m'))
            .cloned()
            .collect()
    }

    /// `(module, exported name, kind)` for every export of every module.
    fn exports(&self) -> Vec<(String, String, &'static str)> {
        let mut found = Vec::new();
        for (file, lines) in &self.files {
            if !file.starts_with('m') {
                continue;
            }
            for line in lines {
                for (keyword, kind) in [
                    ("export const ", "const"),
                    ("export function ", "function"),
                    ("export async function ", "function"),
                    ("export class ", "class"),
                    ("export interface ", "interface"),
                ] {
                    if let Some(rest) = line.strip_prefix(keyword) {
                        let name: String =
                            rest.chars().take_while(|c| c.is_alphanumeric()).collect();
                        let generic = rest[name.len()..].starts_with('<');
                        let kind = match (kind, generic) {
                            ("function", true) => "gfunction",
                            ("class", true) => "gclass",
                            ("class", false) => "class",
                            ("interface", true) => "ginterface",
                            (other, _) => other,
                        };
                        found.push((file.trim_end_matches(".ts").to_owned(), name, kind));
                    }
                }
            }
        }
        found
    }

    fn random_statement(&mut self, rng: &mut Rng, owner: &str) -> String {
        let name = self.fresh_name("v");
        let exports: Vec<_> = self
            .exports()
            .into_iter()
            .filter(|(module, _, _)| format!("{module}.ts") != owner)
            .collect();
        let ty = *rng.pick(&TYPES);
        let selector = if self.plain {
            [0, 1, 2, 3, 4, 99, 99, 99][rng.below(8)]
        } else {
            rng.below(24)
        };
        match selector {
            0 => format!("export const {name} = {};", rng.below(1000)),
            1 => format!("export const {name} = \"s{}\";", rng.below(100)),
            2 => format!(
                "export const {name} = [{}, {}, {}];",
                rng.below(9),
                rng.below(9),
                rng.below(9)
            ),
            3 => format!("export const {name} = {{ a: {}, b: \"x\" }};", rng.below(9)),
            4 => format!("export function {name}(a: {ty}): {ty} {{ return a; }}"),
            5 => format!(
                "export async function {name}(): Promise<number> {{ await Promise.resolve({}); return {}; }}",
                rng.below(9),
                rng.below(9)
            ),
            6 => format!("export class {name}<T> {{ v!: T; get(): T {{ return this.v; }} }}"),
            7 => format!("export interface {name}<T> {{ get(): T }}"),
            8 => format!(
                "export const {name} = [1, 2, 3].map((x) => x + {});",
                rng.below(9)
            ),
            9 => format!(
                "export const {name} = [\"a\", \"b\"].map((s) => s.length + {});",
                rng.below(9)
            ),
            10 => format!(
                "export const {name} = new Map<string, {}>().get(\"k\");",
                rng.pick(&["number", "string"])
            ),
            11 => format!(
                "export const {name} = Promise.resolve({}).then((v) => v);",
                rng.below(9)
            ),
            12 => format!("export function {name}<T>(x: T): T {{ return x; }}"),
            13 => format!("export class {name} {{ method(p: number): number {{ return p; }} }}"),
            14 => format!(
                "export const {name} = {{ m(a: number) {{ return a; }} }}.m({});",
                rng.below(9)
            ),
            15 => format!(
                "export const {name} = (null as unknown as Readonly<{{ x: number; y: string }}>).{};",
                rng.pick(&["x", "y"])
            ),
            16 => format!("export const {name} = (null as unknown as Partial<{{ a: number }}>).a;"),
            17 => format!("export interface {name} {{ plain: number; call(a: string): void }}"),
            _ => {
                // A use of another module's export.
                let Some((module, target, kind)) =
                    (!exports.is_empty()).then(|| rng.pick(&exports).clone())
                else {
                    return format!("export const {name} = {};", rng.below(9));
                };
                let _ = module;
                use_statement(rng, &name, &target, kind)
            }
        }
    }

    /// Adds the import lines the statements of `file` need.
    fn imports_for(&self, file: &str) -> Vec<String> {
        let lines = &self.files[file];
        let mut wanted: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
            Default::default();
        for (module, name, _) in self.exports() {
            if format!("{module}.ts") == file {
                continue;
            }
            let used = lines.iter().any(|line| {
                !line.starts_with("import ")
                    && line
                        .split(|c: char| !c.is_alphanumeric())
                        .any(|word| word == name)
                    && !line.contains(&format!("export const {name} ="))
            });
            if used {
                wanted.entry(module).or_default().insert(name);
            }
        }
        wanted
            .into_iter()
            .map(|(module, names)| {
                format!(
                    "import {{ {} }} from \"./{module}\";",
                    names.into_iter().collect::<Vec<_>>().join(", ")
                )
            })
            .collect()
    }

    fn normalize_imports(&mut self, file: &str) {
        let Some(lines) = self.files.get(file) else {
            return;
        };
        if !file.starts_with('m') {
            return;
        }
        let body: Vec<String> = lines
            .iter()
            .filter(|line| !line.starts_with("import "))
            .cloned()
            .collect();
        self.files.insert(file.to_owned(), body);
        let imports = self.imports_for(file);
        let body = self.files.remove(file).unwrap();
        let mut all = imports;
        all.extend(body);
        self.files.insert(file.to_owned(), all);
    }

    fn initial(rng: &mut Rng, modules: usize, plain: bool) -> Self {
        let mut world = Self {
            plain,
            ..Self::default()
        };
        for index in 0..modules {
            let file = format!("m{index}.ts");
            world.files.insert(file.clone(), Vec::new());
            for _ in 0..(3 + rng.below(4)) {
                let statement = world.random_statement(rng, &file);
                world.files.get_mut(&file).unwrap().push(statement);
            }
            world.normalize_imports(&file);
        }
        // A global script that augments an instantiable library interface.
        let mut globals = vec!["declare var G0: number;".to_owned()];
        if !plain {
            globals.push("interface Array<T> { firstOr(d: T): T }".to_owned());
        }
        world.files.insert("g0.ts".to_owned(), globals);
        let file = "m0.ts".to_owned();
        let uses = if plain {
            "export const usesGlobal = 1 + G0;"
        } else {
            "export const usesGlobal = [1, 2].firstOr(0) + G0;"
        };
        world.files.get_mut(&file).unwrap().push(uses.to_owned());
        for name in world.module_names() {
            world.normalize_imports(&name);
        }
        world
    }
}

/// A statement that uses an exported name of the given kind.
fn use_statement(rng: &mut Rng, name: &str, target: &str, kind: &str) -> String {
    match kind {
        "function" => format!("export const {name} = {target}({});", rng.below(9)),
        "gfunction" => format!(
            "export const {name} = {target}({});",
            rng.pick(&["1", "\"s\"", "[1]"])
        ),
        "gclass" => format!(
            "export const {name} = new {target}<{}>().get();",
            rng.pick(&["number", "string"])
        ),
        "class" => format!(
            "export const {name} = new {target}().method({});",
            rng.below(9)
        ),
        "ginterface" => format!("export const {name}: {target}<string> | undefined = undefined;"),
        "interface" => format!("export const {name}: {target} | undefined = undefined;"),
        _ => format!("export const {name} = {target};"),
    }
}

/// One edit: the human description and the resulting overlay changes.
type Edit = (String, Vec<(String, Option<String>)>);

fn mutate_digits(line: &str, rng: &mut Rng) -> String {
    let digits: Vec<usize> = line
        .char_indices()
        .filter(|(_, c)| c.is_ascii_digit())
        .map(|(at, _)| at)
        .collect();
    if digits.is_empty() {
        return format!("{line} // {}", rng.below(100));
    }
    let at = *rng.pick(&digits);
    let old = line.as_bytes()[at] - b'0';
    let new = (old + 1 + rng.below(8) as u8) % 10;
    format!("{}{}{}", &line[..at], new, &line[at + 1..])
}

impl World {
    /// Applies one random edit; returns its description and the files whose
    /// text changed (`None` is a deletion).
    #[allow(clippy::too_many_lines)]
    fn random_edit(&mut self, rng: &mut Rng) -> Edit {
        let before = self.files.clone();
        let mut config_change = None;
        let description = loop {
            let kind = rng.below(14);
            let names: Vec<String> = self.files.keys().cloned().collect();
            let module_names = self.module_names();
            match kind {
                // Body edit, same span (digits keep their width) or shifting.
                0 | 1 => {
                    let file = rng.pick(&module_names).clone();
                    let lines = self.files.get_mut(&file).unwrap();
                    if lines.is_empty() {
                        continue;
                    }
                    let at = rng.below(lines.len());
                    if lines[at].starts_with("import ") {
                        continue;
                    }
                    let old = lines[at].clone();
                    lines[at] = mutate_digits(&old, rng);
                    break format!("body edit {file}: {old} -> {}", lines[at]);
                }
                // Comment-only edits: a prefix shifts every span, an append does not.
                2 => {
                    let file = rng.pick(&names).clone();
                    let lines = self.files.get_mut(&file).unwrap();
                    if rng.chance(50) {
                        lines.insert(0, format!("// edit {}", rng.below(1000)));
                        break format!("prefix comment {file}");
                    }
                    lines.push(format!("// edit {}", rng.below(1000)));
                    break format!("append comment {file}");
                }
                // Signature edit: swap a type keyword in a function or const.
                3 | 4 => {
                    let file = rng.pick(&module_names).clone();
                    let lines = self.files.get_mut(&file).unwrap();
                    let candidates: Vec<usize> = lines
                        .iter()
                        .enumerate()
                        .filter(|(_, line)| line.contains("number") || line.contains("string"))
                        .map(|(at, _)| at)
                        .collect();
                    if candidates.is_empty() {
                        continue;
                    }
                    let at = *rng.pick(&candidates);
                    let old = lines[at].clone();
                    lines[at] = if old.contains("number") {
                        old.replacen("number", "string", 1)
                    } else {
                        old.replacen("string", "number", 1)
                    };
                    break format!("signature edit {file}: {old} -> {}", lines[at]);
                }
                // Export add: a new exported statement, imported by another module.
                5 => {
                    let file = rng.pick(&module_names).clone();
                    let statement = self.random_statement(rng, &file);
                    self.files.get_mut(&file).unwrap().push(statement.clone());
                    for name in self.module_names() {
                        if rng.chance(40) || name == file {
                            self.normalize_imports(&name);
                        }
                    }
                    break format!("export add {file}: {statement}");
                }
                // Export remove: drop a statement.
                6 => {
                    let file = rng.pick(&module_names).clone();
                    let lines = self.files.get_mut(&file).unwrap();
                    let candidates: Vec<usize> = lines
                        .iter()
                        .enumerate()
                        .filter(|(_, line)| line.starts_with("export "))
                        .map(|(at, _)| at)
                        .collect();
                    if candidates.len() < 2 {
                        continue;
                    }
                    let at = *rng.pick(&candidates);
                    let removed = lines.remove(at);
                    break format!("export remove {file}: {removed}");
                }
                // Import add (a use of an export the file did not use) / remove.
                7 => {
                    let file = rng.pick(&module_names).clone();
                    let exports: Vec<_> = self
                        .exports()
                        .into_iter()
                        .filter(|(module, _, _)| format!("{module}.ts") != file)
                        .collect();
                    if exports.is_empty() {
                        continue;
                    }
                    let (module, name, kind) = rng.pick(&exports).clone();
                    let fresh = self.fresh_name("u");
                    let statement = use_statement(rng, &fresh, &name, kind);
                    self.files.get_mut(&file).unwrap().push(statement.clone());
                    self.normalize_imports(&file);
                    break format!("import add {file} <- {module}.{name}: {statement}");
                }
                8 => {
                    let file = rng.pick(&module_names).clone();
                    let lines = self.files.get_mut(&file).unwrap();
                    let Some(at) = lines.iter().position(|line| line.starts_with("import ")) else {
                        continue;
                    };
                    let removed = lines.remove(at);
                    break format!("import remove {file}: {removed}");
                }
                // File add.
                9 => {
                    let file = self.fresh_name("m") + ".ts";
                    self.files.insert(file.clone(), Vec::new());
                    for _ in 0..(2 + rng.below(3)) {
                        let statement = self.random_statement(rng, &file);
                        self.files.get_mut(&file).unwrap().push(statement);
                    }
                    self.normalize_imports(&file);
                    break format!("file add {file}");
                }
                // File delete, then (a later draw) restore.
                10 => {
                    if module_names.len() < 3 {
                        continue;
                    }
                    let file = rng.pick(&module_names).clone();
                    let lines = self.files.remove(&file).unwrap();
                    self.graveyard.insert(file.clone(), lines);
                    break format!("file delete {file}");
                }
                11 => {
                    let Some(file) = self.graveyard.keys().next().cloned() else {
                        continue;
                    };
                    let lines = self.graveyard.remove(&file).unwrap();
                    self.files.insert(file.clone(), lines);
                    break format!("file restore {file}");
                }
                // Rename an exported function or const, in some importers.
                12 => {
                    let exports = self.exports();
                    if exports.is_empty() {
                        continue;
                    }
                    let (module, old, _) = rng.pick(&exports).clone();
                    let new = self.fresh_name("r");
                    let everywhere = rng.chance(60);
                    for (file, lines) in &mut self.files {
                        let declaring = *file == format!("{module}.ts");
                        if !(declaring || everywhere) || file.starts_with('g') {
                            continue;
                        }
                        for line in lines.iter_mut() {
                            let renamed: String = line
                                .split_inclusive(|c: char| !c.is_alphanumeric())
                                .map(|part| {
                                    let word: String =
                                        part.chars().take_while(|c| c.is_alphanumeric()).collect();
                                    if word == old {
                                        format!("{new}{}", &part[word.len()..])
                                    } else {
                                        part.to_owned()
                                    }
                                })
                                .collect();
                            *line = renamed;
                        }
                    }
                    break format!("rename {module}.{old} -> {new} (importers {everywhere})");
                }
                // Global scope or tsconfig.
                _ => {
                    if rng.chance(50) {
                        let lines = self.files.get_mut("g0.ts").unwrap();
                        match rng.below(3) {
                            0 => {
                                let name = format!("G{}", rng.below(5));
                                lines.push(format!("declare var {name}: {};", rng.pick(&TYPES)));
                                break format!("global add {name}");
                            }
                            1 => {
                                lines.retain(|line| !line.starts_with("interface Array"));
                                lines.push(format!(
                                    "interface Array<T> {{ firstOr(d: T): {} }}",
                                    rng.pick(&["T", "number", "T | undefined"])
                                ));
                                break "global interface edit".to_owned();
                            }
                            _ => {
                                if lines.len() < 2 {
                                    continue;
                                }
                                let at = rng.below(lines.len());
                                let removed = lines.remove(at);
                                break format!("global remove {removed}");
                            }
                        }
                    }
                    let next = (self.config + 1 + rng.below(CONFIGS.len() - 1)) % CONFIGS.len();
                    self.config = next;
                    config_change = Some(CONFIGS[next].to_owned());
                    break format!("tsconfig -> variant {next}");
                }
            }
        };
        let mut changes: Vec<(String, Option<String>)> = Vec::new();
        for (file, lines) in &self.files {
            if before.get(file) != Some(lines) {
                changes.push((file.clone(), Some(Self::render(lines))));
            }
        }
        for file in before.keys() {
            if !self.files.contains_key(file) {
                changes.push((file.clone(), None));
            }
        }
        if let Some(config) = config_change {
            changes.push(("tsconfig.json".to_owned(), Some(config)));
        }
        (description, changes)
    }
}

/// Seeds to run: `TSRS_RANDOM_SEEDS=a..b` or a count; default a few.
fn seed_range() -> std::ops::Range<u64> {
    match std::env::var("TSRS_RANDOM_SEEDS") {
        Ok(value) => match value.split_once("..") {
            Some((from, to)) => from.parse().unwrap()..to.parse().unwrap(),
            None => 0..value.parse().unwrap(),
        },
        Err(_) => 0..3,
    }
}

fn step_count() -> usize {
    std::env::var("TSRS_RANDOM_STEPS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(12)
}

/// Every symbol id named in a row's debug text.
fn ids_in(lines: &[String]) -> Vec<std::sync::Arc<str>> {
    let mut seen = std::collections::HashSet::new();
    let mut ids = Vec::new();
    for line in lines {
        let mut rest = line.as_str();
        while let Some(at) = rest.find("\"symbol:") {
            let tail = &rest[at + 1..];
            let end = tail.find('"').unwrap_or(tail.len());
            let id = std::sync::Arc::<str>::from(&tail[..end]);
            if seen.insert(id.clone()) {
                ids.push(id);
            }
            rest = &tail[end..];
        }
    }
    ids
}

/// Renames every generation-scoped counter id (`symbol:<generation>:<n>`) by
/// first appearance, so two tables that name the same partition of symbols
/// with differently numbered counters compare equal. Declaration-hash ids are
/// left alone: those must match exactly.
pub(super) struct Renamer(pub(super) std::collections::HashMap<String, usize>);

impl Renamer {
    pub(super) fn apply(&mut self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(at) = rest.find("symbol:") {
            out.push_str(&rest[..at]);
            let tail = &rest[at..];
            let end = tail
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == ':'))
                .unwrap_or(tail.len());
            let id = &tail[..end];
            let counter = id.strip_prefix("symbol:").is_some_and(|rest| {
                rest.split(':')
                    .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
            });
            if counter {
                let next = self.0.len() + 1;
                let number = *self.0.entry(id.to_owned()).or_insert(next);
                out.push_str(&format!("counter#{number}"));
            } else {
                out.push_str(id);
            }
            rest = &tail[end..];
        }
        out.push_str(rest);
        out
    }
}

struct Probe {
    table: typefacts::producer::ProducerTable,
    lines: Vec<String>,
    symbols: Vec<String>,
    files: Vec<String>,
    modules: String,
    source_facts: Vec<String>,
}

/// Everything the producer answers for a generation, as comparable text:
/// the entity table, the symbol evidence for every id it names, the file rows,
/// the module graph and the per-file source facts.
fn probe(closure: &mut DemandClosure, generation: u64, retained: bool) -> Probe {
    let ctx = Context::new();
    let demands = text_demands(closure);
    let table = closure
        .demand_table_for_groups(
            &ctx,
            generation,
            &groups(&demands),
            &if retained {
                Vec::new()
            } else {
                paths(&demands)
            },
        )
        .unwrap_or_else(|error| panic!("materialization at {generation}: {error}"));
    let raw_lines = describe(&table);
    let lines = raw_lines;
    let queries: Vec<typefacts::v3::SymbolQuery> = ids_in(&lines)
        .into_iter()
        .map(|id| typefacts::v3::SymbolQuery {
            id,
            references: true,
            references_only: false,
        })
        .collect();
    let symbols: Vec<String> = closure
        .resolve_symbol_evidence(&ctx, &queries)
        .unwrap()
        .iter()
        .map(|fact| format!("{fact:?}"))
        .collect();
    let files: Vec<String> = table.files.iter().map(|file| format!("{file:?}")).collect();
    let modules = format!(
        "{:?}",
        closure
            .module_graph(
                &ctx,
                &typefacts::v3::ModuleGraphRequest {
                    imports: true,
                    packages: true,
                    ..Default::default()
                },
            )
            .unwrap()
    );
    let mut source_facts = Vec::new();
    for source in closure.source_files(&ctx).unwrap() {
        if !source.path.ends_with(".ts") || source.path.ends_with(".d.ts") {
            continue;
        }
        let path = crate::gocompat::clean(&source.path);
        let backend = &mut closure.backend;
        source_facts.push(format!(
            "{path}\n{:?}\n{:?}\n{:?}\n{:?}",
            backend.source_calls(&ctx, &path),
            backend.source_bindings(&ctx, &path),
            backend.source_functions(&ctx, &path),
            backend.source_async_functions(&ctx, &path),
        ));
    }
    let mut renamer = Renamer(Default::default());
    let lines = lines.iter().map(|line| renamer.apply(line)).collect();
    let symbols = symbols.iter().map(|line| renamer.apply(line)).collect();
    let files = files.iter().map(|line| renamer.apply(line)).collect();
    let source_facts = source_facts
        .iter()
        .map(|line| renamer.apply(line))
        .collect();
    Probe {
        table,
        lines,
        symbols,
        files,
        modules,
        source_facts,
    }
}

fn first_difference(what: &str, left: &[String], right: &[String]) -> Option<String> {
    if left == right {
        return None;
    }
    let mut report = format!(
        "{what} differs ({} retained rows, {} fresh rows)\n",
        left.len(),
        right.len()
    );
    // Rows are paired by position when the counts agree, which localizes the
    // first differing byte of multi-line rows such as per-file source facts.
    if left.len() == right.len() {
        for (retained, fresh) in left.iter().zip(right).filter(|(l, r)| l != r).take(3) {
            let at = retained
                .bytes()
                .zip(fresh.bytes())
                .position(|(l, r)| l != r)
                .unwrap_or(retained.len().min(fresh.len()));
            let from = at.saturating_sub(160);
            let clip = |text: &str| {
                text[text
                    .char_indices()
                    .map(|(i, _)| i)
                    .find(|i| *i >= from)
                    .unwrap_or(0)..]
                    .chars()
                    .take(420)
                    .collect::<String>()
            };
            report.push_str(&format!(
                "  first difference at byte {at}\n    retained: ...{}\n    fresh:    ...{}\n",
                clip(retained),
                clip(fresh)
            ));
        }
        return Some(report);
    }
    for line in left.iter().filter(|line| !right.contains(line)).take(4) {
        report.push_str(&format!(
            "  retained only: {}\n",
            &line[..line.len().min(700)]
        ));
    }
    for line in right.iter().filter(|line| !left.contains(line)).take(4) {
        report.push_str(&format!(
            "  fresh only:    {}\n",
            &line[..line.len().min(700)]
        ));
    }
    Some(report)
}

/// One seeded run: a retained closure follows `steps` random edits; after each,
/// every family is compared with a closure that replays the same updates and
/// materializes the whole batch.
fn run_seed(seed: u64, steps: usize, modules: usize, plain: bool) -> usize {
    let ctx = Context::new();
    let mut rng = Rng::new(seed);
    let mut world = World::initial(&mut rng, modules, plain);
    let mut files: Vec<(String, String)> =
        vec![("tsconfig.json".to_owned(), CONFIGS[0].to_owned())];
    for (name, lines) in &world.files {
        files.push((name.clone(), World::render(lines)));
    }
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let scratch = Scratch::new(&borrowed);
    let config = scratch.config();
    let mut incremental = open_closure(&config);
    let initial = probe(&mut incremental, 1, false);
    let mut log: Vec<String> = Vec::new();
    let mut history: Vec<Vec<FileChange>> = Vec::new();
    let mut versions: std::collections::HashMap<String, u64> = Default::default();
    let mut retained_total = 0usize;
    drop(initial);
    for step in 0..steps {
        // An update that changes no file is not sent by the client, and the
        // producer keeps its checker across it, so draw again.
        let (description, edits) = loop {
            let (description, edits) = world.random_edit(&mut rng);
            if !edits.is_empty() {
                break (description, edits);
            }
        };
        log.push(format!("step {step}: {description}"));
        let mut changes = Vec::new();
        for (name, content) in edits {
            let version = versions.entry(name.clone()).or_default();
            *version += 1;
            changes.push(FileChange {
                path: scratch.path(&name),
                deleted: content.is_none(),
                source: content.unwrap_or_default().into_bytes(),
                version: *version,
            });
        }
        // One update per edit, however many files it touches.
        incremental.update(&ctx, &changes).unwrap_or_else(|error| {
            panic!("incremental update {step}: {error}\n{}", log.join("\n"))
        });
        history.push(changes);
        let generation = step as u64 + 2;
        let retained = probe(&mut incremental, generation, true);
        retained_total += incremental.stats().retention.retained_files;

        // The oracle replays every edit so far as its own update, as the
        // retained side saw them, and then materializes the whole batch.
        let mut fresh = open_closure(&config);
        for (replay, changes) in history.iter().enumerate() {
            fresh
                .update(&ctx, changes)
                .unwrap_or_else(|error| panic!("fresh replay {replay}/{step}: {error}"));
        }
        let whole = probe(&mut fresh, generation, false);
        let mut failures = Vec::new();
        failures.extend(first_difference(
            "entity rows",
            &retained.lines,
            &whole.lines,
        ));
        failures.extend(first_difference(
            "symbol evidence",
            &retained.symbols,
            &whole.symbols,
        ));
        failures.extend(first_difference("file rows", &retained.files, &whole.files));
        failures.extend(first_difference(
            "source facts",
            &retained.source_facts,
            &whole.source_facts,
        ));
        if retained.modules != whole.modules {
            failures.push("module graph differs".to_owned());
        }
        if failures.is_empty() {
            assert_full_transitions_identical(
                "retained table",
                step,
                &config,
                &retained.table,
                &whole.table,
            );
        } else {
            panic!(
                "seed {seed} step {step}: retained generation differs from fresh\n{}\n{}",
                failures.join("\n"),
                log.join("\n")
            );
        }
    }
    eprintln!("seed {seed}: {steps} steps, {retained_total} retained file-generations");
    retained_total
}

#[test]
fn seeded_random_edit_sequences_match_fresh() {
    for seed in seed_range() {
        run_seed(seed, step_count(), 5, false);
    }
}

/// The same edit kinds over files whose ids are all declaration hashes (the
/// shape that used to be retained), with nothing reused across updates.
#[test]
fn seeded_random_edit_sequences_over_retainable_files_match_fresh() {
    let mut retained = 0;
    for seed in seed_range() {
        retained += run_seed(seed, step_count(), 8, true);
    }
    assert_eq!(retained, 0, "an update reused a contribution");
}

/// A mapped type over a namespace object copies its functions into transient
/// members that share the originals' declarations. Another file holds an original's id
/// from before the spread appeared; the retained answer must not be reused with
/// the copy claiming the same id.
#[test]
fn mapped_copies_of_namespace_functions_do_not_share_the_original_id() {
    let files = [
        ("tsconfig.json", STRICT_CONFIG),
        ("ns.ts", "export function f(): number { return 1; }\n"),
        (
            "a.ts",
            "import { f } from \"./ns\";\nexport const x = f();\n",
        ),
        (
            "b.ts",
            "import * as ns from \"./ns\";\nexport const b = ns;\n",
        ),
        ("z.ts", "export const z = 1;\n"),
    ];
    assert_script_matches_fresh(
        &files,
        |scratch| {
            vec![
                change(
                    scratch,
                    "b.ts",
                    1,
                    "import * as ns from \"./ns\";\nexport const b: Readonly<typeof ns> = ns;\nexport const c = b.f();\n",
                ),
                change(scratch, "z.ts", 2, "export const z = 2;\n"),
                change(scratch, "z.ts", 3, "export const z = 3;\n"),
            ]
        },
        symbol_only_demands,
    );
}

/// Within one generation the ids a table carries must not depend on the order
/// the demands arrived in: a demand set that grows (b.ts joins) is answered as
/// a fresh closure answers the whole set, and so is one demanded in a
/// different order.
#[test]
fn ids_within_a_generation_do_not_depend_on_demand_order() {
    let ctx = Context::new();
    let files = [
        ("tsconfig.json", STRICT_CONFIG),
        (
            "a.ts",
            "declare const xs: string[];\nexport const ma = xs.map((x) => x);\n",
        ),
        (
            "b.ts",
            "declare const ys: number[];\nexport const mb = ys.map((y) => y);\n",
        ),
        ("c.ts", "export const c = 1;\n"),
    ];
    let scratch = Scratch::new(&files);
    let config = scratch.config();
    for (demands_of, first) in [
        (
            text_demands as fn(&mut DemandClosure) -> Vec<EntityDemand>,
            "a.ts",
        ),
        (symbol_only_demands, "a.ts"),
        (text_demands, "b.ts"),
        (symbol_only_demands, "b.ts"),
    ] {
        let mut fresh = open_closure(&config);
        let all = demands_of(&mut fresh);
        let whole = describe(
            &fresh
                .demand_table_for_groups(&ctx, 1, &groups(&all), &[])
                .unwrap(),
        );
        let whole = Renamer(Default::default()).apply(&whole.join("\n"));

        // The demand set grows within the generation: one file alone (a.ts or b.ts, the later in path order claims second in a whole batch), then all.
        let mut growing = open_closure(&config);
        let all = demands_of(&mut growing);
        let first_path = crate::gocompat::clean(&scratch.path(first));
        let only_first: Vec<_> = all
            .iter()
            .filter(|d| crate::gocompat::clean(&d.location.path) == first_path)
            .cloned()
            .collect();
        growing
            .demand_table_for_groups(&ctx, 1, &groups(&only_first), &paths(&only_first))
            .unwrap();
        let rest: Vec<_> = all
            .iter()
            .filter(|d| crate::gocompat::clean(&d.location.path) != first_path)
            .cloned()
            .collect();
        let grown = describe(
            &growing
                .demand_table_for_groups(&ctx, 1, &groups(&all), &paths(&rest))
                .unwrap(),
        );
        assert_eq!(
            Renamer(Default::default()).apply(&grown.join("\n")),
            whole,
            "a demand set that grew within the generation differs from a fresh whole batch"
        );

        // The same demands, files in reverse order of first demand.
        let mut reversed = open_closure(&config);
        let mut all = demands_of(&mut reversed);
        all.reverse();
        let table = reversed
            .demand_table_for_groups(&ctx, 1, &groups(&all), &[])
            .unwrap();
        let mut rows = describe(&table);
        rows.sort();
        let mut expected = describe(
            &open_closure(&config)
                .demand_table_for_groups(
                    &ctx,
                    1,
                    &groups(&demands_of(&mut open_closure(&config))),
                    &[],
                )
                .unwrap(),
        );
        expected.sort();
        assert_eq!(
            Renamer(Default::default()).apply(&rows.join("\n")),
            Renamer(Default::default()).apply(&expected.join("\n")),
            "demand order changed the ids of a generation"
        );
    }
}
