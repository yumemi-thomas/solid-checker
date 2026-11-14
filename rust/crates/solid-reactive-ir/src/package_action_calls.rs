//! ADR 0972: a call into code of a package the build does not read, written in an owned scope, may throw `ACTION_CALLED_IN_OWNED_SCOPE`.
//!
//! SC2002 placed only the actions the program itself creates. An installed package whose source the build does not read can call Solid's
//! `action` itself (`export const runStep = (cb) => action(function* () { cb(); })()`), and the program shows no action at the project's
//! call: Chrome throws at `runStep(() => {})` in a component body, a memo's compute or a `createRoot` body while the checker was silent.
//!
//! Owner ruling (option B): each such call is a site of SC2002 ([`sites`]). Its callee comes from the compiler's resolution (a call's
//! resolved declaration, a tag's symbol through its alias): a declaration in an installed package or in a declaration file, outside the
//! standard library, outside the packages the build reads as source (their code is analyzed, ADR 0949 reports it at the project's site),
//! and outside the dialect's own runtime (its catalog answers for its exports: Solid's runtime calls no `action` of its own, checked on the
//! pinned release's source). The site ends silent only when
//!
//! - the backend decided from the package's source that its closure cannot reach a Solid package
//!   ([`solid_facts::package_reach::PackageReach::Clear`]: no Solid load, no computed load, no code generation, no unknown global); or
//! - the call's role allows an action (an event handler, a timer, an effect's apply, `onSettled`): the classifier SC2002's own rows use; or
//! - the call cannot run (module initialization, a function nothing reaches).
//!
//! Otherwise it gets an uncertifiable SC2002 row, blocker `package-may-call-action:<why>`, never a violation: nothing proves the package
//! does call an action. A component the program renders from such a package (`<Pkg />`) runs its body under an owner on every render, so
//! its tag is a site with no legal role. A function the program hands the package is the hand-off's concern (ADRs 0949, 0967), not this.
//!
//! A project binding that only names a package's function (`const run = pkg.runStep`, `const C = Pkg.Comp`) is followed through its single
//! initializer ([`ALIAS_DEPTH`] deep) to the declaration the compiler resolves it to, so an alias is the site its target is.
//!
//! The sites are found once per build ([`sites`]): the pipeline's row pass and the proof universe both read them.

use crate::{HashMap, HashSet};
use solid_facts::FileFacts;
use solid_facts::core::Span;
use solid_facts::module_reach::is_global_object_name;
use solid_facts::package_reach::{
    AMBIENT_PREFIX, AccessorFacts, COMPUTED_LOAD_PREFIX, LIMIT_PREFIX, PackageReach,
    READ_ANY_PREFIX, ReachBlocker, WRITE_ANY_PREFIX, WRITE_PREFIX, is_declaration_file,
    package_label, package_root_of,
};

use crate::execution_role::RootBodyGuard;
use crate::pipeline::AnalysisContext;
use crate::proof_log::{Premise, PremiseKind};
use crate::site_accounting::{SiteEnds, SiteSilence};

/// How far a chain of aliases (`const a = pkg.fn; const b = a`) is followed to the package declaration.
const ALIAS_DEPTH: usize = 4;

/// How the project enters the package's code at a site.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Entry {
    /// A call or `new`.
    Call,
    /// A tagged template's tag.
    Tag,
    /// A JSX element whose tag is the package's component.
    Render,
    /// A reference to the package's function handed on as a value (`createMemo(pkgFn)`, `[1].forEach(pkgFn)`): where it runs is not
    /// followed, so it has no legal role.
    Value,
    /// A member of an object the package made, evaluated for its value (`void clock.now`) where the package's closure may define an accessor under that name
    /// (ADR 0974): the getter runs where the read stands, with that role.
    Read,
}

impl Entry {
    pub(crate) const fn via(self) -> &'static str {
        match self {
            Self::Call => "call",
            Self::Tag => "tag",
            Self::Render => "render",
            Self::Value => "value",
            Self::Read => "read",
        }
    }
}

/// One project site that enters code of a package the build does not read, whose closure is not clear.
pub(crate) struct PackageSite {
    /// The call, tagged template or opening tag.
    pub(crate) at: Span,
    /// The callee, tag or element name.
    pub(crate) callee: Span,
    pub(crate) entry: Entry,
    pub(crate) reach: PackageReach,
    /// The package (its directory name), or the declaration file.
    pub(crate) package: String,
    /// Whose code the site enters.
    pub(crate) provider: Provider,
}

/// Whose code a site enters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Provider {
    /// An installed package the build does not read.
    Package,
    /// A global a project declaration file declares ([`AMBIENT_PREFIX`]), not a package's export: its provider is not said.
    Ambient,
    /// A value whose origin the walk could not follow to its end ([`LIMIT_PREFIX`]).
    Limit,
    /// A global the standard library declares that an installed package writes at run time ([`WRITE_PREFIX`]): the call runs the writer's code.
    Written,
    /// A global the standard library declares that an installed package may write at run time, under a key the scan does not decide ([`WRITE_ANY_PREFIX`]).
    WrittenAny,
    /// A member of the global object read by a key the program does not spell ([`READ_ANY_PREFIX`]): any global may be the one, so the code is that of a package that
    /// writes one.
    Unspelled,
}

/// How a site's callee is resolved to the code it enters.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Resolve {
    /// A call, `new` or tagged template: the compiler's resolved declaration of the call, else the callee's symbol.
    Call,
    /// A JSX tag: the tag's symbol, else the import it names, through the project bindings that only alias it.
    Tag,
}

/// What an import binding of a file names.
struct ImportBinding<'f> {
    module: &'f str,
    /// The span of the import declaration (the compiler's attestation of the import is keyed by it).
    declaration: Span,
    namespace: bool,
    /// The module is the project's (a relative path) or the dialect's runtime: the project's analysis and the dialect's catalog decide it.
    decided_elsewhere: bool,
    /// The export the binding names: `Some("a")` for `import { a as b }`, `Some("default")` for a default import, `None` for a namespace.
    imported: Option<&'f str>,
}

/// The value import bindings of `file` by the span of the local name, for an O(1) answer to "does this reference name an import".
fn import_bindings<'f>(
    ctx: &AnalysisContext<'_>,
    file: &'f FileFacts,
) -> HashMap<Span, ImportBinding<'f>> {
    let mut bindings = HashMap::default();
    for import in &file.ast.imports {
        if import.type_only {
            continue;
        }
        let module = import.module.as_str();
        let decided_elsewhere = module.starts_with('.')
            || module.starts_with('/')
            || is_runtime_module(ctx, module)
            || module.starts_with("node:");
        for binding in &import.bindings {
            if binding.type_only {
                continue;
            }
            bindings.insert(
                binding.local.span,
                ImportBinding {
                    module,
                    declaration: import.span,
                    namespace: binding.kind == solid_facts::ast::ImportKind::Namespace,
                    decided_elsewhere,
                    imported: match binding.kind {
                        solid_facts::ast::ImportKind::Namespace => None,
                        solid_facts::ast::ImportKind::Default => Some("default"),
                        _ => binding.imported.as_deref(),
                    },
                },
            );
        }
    }
    bindings
}

/// Whether `module` is one of the dialect's runtime modules: its catalog answers for what the module exports.
fn is_runtime_module(ctx: &AnalysisContext<'_>, module: &str) -> bool {
    ctx.dialect
        .primitive_defining_packages()
        .contains(&package_name(module))
        || ctx.dialect.modules().contains(&module)
}

/// The sites of every analyzed file, by position in [`crate::indexes::SemanticLookup::files`], found once per build.
pub(crate) fn sites<'c>(ctx: &'c AnalysisContext<'_>) -> &'c [Vec<PackageSite>] {
    ctx.package_sites.get_or_init(|| {
        EXPORTS.with(|known| known.borrow_mut().clear());
        let files = ctx.semantic_lookup.files();
        // The files the program analyzes itself (a package read as source, a JavaScript file `allowJs` brings in): their code is read, not assumed.
        let analyzed = files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<HashSet<_>>();
        files
            .iter()
            .map(|file| file_sites(ctx, &analyzed, file))
            .collect()
    })
}

/// The sites of one file, in source order.
fn file_sites(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
) -> Vec<PackageSite> {
    let lookup = ctx.semantic_lookup;
    let bound = import_bindings(ctx, file);
    let mut sites = Vec::new();
    let mut seen = HashSet::<(Span, Entry)>::default();
    // A read of a global runs a package's code only through an accessor a package defines on the global object: where none does, every question about one is no.
    let accessors_possible = ctx
        .package_reach
        .is_some_and(solid_facts::package_reach::PackageReachIndex::defines_global_accessors);
    let mut push = |at: Span, callee: Span, entry: Entry, declaration: Option<DeclarationAt>| {
        let Some(declaration) = declaration else {
            return;
        };
        let reach = match ctx.package_reach {
            Some(index) => index.for_declaration(&declaration.path),
            None => PackageReach::Undecided {
                blocker: ReachBlocker::NoFilesystem,
                detail: "this adapter reads no installed package".into(),
            },
        };
        if reach.is_clear() {
            return;
        }
        // A site two routes find (an identifier and the member it is the object of) stands once; a clear package's value handed to the same call never hides
        // another's. A read where the value is already handed on as one (`const { now } = clock`, an import that may be a function) is that site's row: the value
        // read is the callee's span, which is the site's own for a member.
        if entry == Entry::Read && seen.contains(&(callee, Entry::Value)) {
            return;
        }
        if !seen.insert((at, entry)) {
            return;
        }
        let package = match declaration
            .path
            .strip_prefix(WRITE_PREFIX)
            .or_else(|| declaration.path.strip_prefix(WRITE_ANY_PREFIX))
        {
            // `global-read-any:<writer root>`: a member of the global object under a key the program does not spell.
            _ if declaration.path.starts_with(READ_ANY_PREFIX) => {
                "a global read by a computed key".to_owned()
            }
            // `global-write:<name>:<writer root>`: the global the call is of; the closure the row names is the writer's.
            Some(written) => written
                .split_once(':')
                .map_or(written, |(name, _)| name)
                .to_owned(),
            None => package_root_of(&declaration.path).map_or_else(
                || {
                    declaration
                        .path
                        .strip_prefix(AMBIENT_PREFIX)
                        .or_else(|| declaration.path.strip_prefix(LIMIT_PREFIX))
                        .unwrap_or(&declaration.path)
                        .to_owned()
                },
                |root| installed_name(ctx, root),
            ),
        };
        sites.push(PackageSite {
            at,
            callee,
            entry,
            reach,
            package,
            provider: if declaration.path.starts_with(AMBIENT_PREFIX) {
                Provider::Ambient
            } else if declaration.path.starts_with(LIMIT_PREFIX) {
                Provider::Limit
            } else if declaration.path.starts_with(WRITE_PREFIX) {
                Provider::Written
            } else if declaration.path.starts_with(WRITE_ANY_PREFIX) {
                Provider::WrittenAny
            } else if declaration.path.starts_with(READ_ANY_PREFIX) {
                Provider::Unspelled
            } else {
                Provider::Package
            },
        });
    };
    for call in &file.ast.calls {
        // A dialect primitive is the catalog's; a reviewed test runner's registration (`describe`, `it`) runs the callback on its own stack and calls no action;
        // a reviewed reading of a package's API (ADR 0975) shows what it calls for the value it is handed.
        if lookup.primitive_at_call(file, call.span).is_some() {
            continue;
        }
        // What can be a site is asked first (a call no package's code runs ends here, as a site needs a declaration): the questions that clear a call are the dearer ones.
        let Some(declaration) =
            package_declaration(ctx, analyzed, file, &bound, call.callee, Resolve::Call)
        else {
            continue;
        };
        if lookup.reviewed_test_runner_registration(file, call)
            || crate::reviewed_api::clears_call(ctx, file, call)
        {
            continue;
        }
        push(call.span, call.callee, Entry::Call, Some(declaration));
    }
    for template in &file.ast.tagged_templates {
        push(
            template.span,
            template.tag,
            Entry::Tag,
            package_declaration(ctx, analyzed, file, &bound, template.tag, Resolve::Call),
        );
    }
    for element in &file.ast.jsx_elements {
        // An intrinsic element (`<button>`) is the DOM's; its tag names no code.
        if crate::object_members::intrinsic_element(file, element) {
            continue;
        }
        let opening = if element.opening.end > element.opening.start {
            element.opening
        } else {
            element.name.span
        };
        // `<Ns.Comp>`: a member of an import takes the verdict of the import's package; any other member tag (`<props.Comp>`) names no declaration this
        // pass reads. `<Comp>`: the symbol, else the import, through the bindings that only alias it.
        let declaration = match element.member_object {
            Some(object) => file
                .ast
                .reference_declaration(file.ast.peel_ts_sugar_span(object))
                .and_then(|declaration| bound.get(&declaration))
                .and_then(|import| import_declaration(ctx, analyzed, file, import)),
            None => {
                package_declaration(ctx, analyzed, file, &bound, element.name.span, Resolve::Tag)
            }
        };
        push(opening, element.name.span, Entry::Render, declaration);
    }
    // A reference to a package's function handed on as a value: `createMemo(pkgFn)`, `const run = pkgFn`, `[1].map(Pkg.fn)`, `{ pkgFn }`, `take(Pkg)`,
    // `[cb].forEach(kernel.run)`. The reference names an import (a package's, or the project's own module, whose re-export of a package function the compiler's
    // resolution follows) or a project binding that holds what an import makes ([`origin_of`]). A binding that only names it (`const run = pkgFn`) has its
    // initializer as a site too, which is not where `run` is handed on. The row stands at the call that receives the value, where a throw has its project frame.
    for identifier in &file.ast.identifiers {
        if identifier.role != solid_facts::ast::IdentifierRole::Reference {
            continue;
        }
        if !followable_root(ctx, file, &bound, identifier.span) {
            continue;
        }
        let namespace = file
            .ast
            .reference_declaration(identifier.span)
            .and_then(|declaration| bound.get(&declaration))
            .is_some_and(|import| import.namespace);
        // An import may be a function the facts say nothing of; a binding that only holds what an import makes is one only where the compiler says it is callable.
        let import = file
            .ast
            .reference_declaration(identifier.span)
            .is_some_and(|declaration| bound.contains_key(&declaration));
        let function = if import {
            namespace || may_be_function(ctx, file, identifier.span, identifier.span)
        } else {
            callable_fact(ctx, file, identifier.span) == Some(true)
        };
        // What can be a site is asked first: the walk of where the value came from is the dearer question.
        if !(function && handed_as_value(ctx, file, identifier.span)) {
            continue;
        }
        let (Origin::Package(origin) | Origin::Lazy(origin)) =
            origin_of(ctx, analyzed, file, &bound, identifier.span, ORIGIN_DEPTH)
        else {
            continue;
        };
        let at = handed_at(file, identifier.span);
        // A reviewed reading shows the assertion the value is handed to reads it as a primitive (ADR 0975).
        if crate::reviewed_api::clears_value_at(ctx, file, at) {
            continue;
        }
        push(at, identifier.span, Entry::Value, Some(origin));
    }
    for member in &file.ast.members {
        // `[1].forEach(kernel.run)`: the object is an import of any kind, a binding that holds what an import makes, or a longer chain (`pkg.helpers.run`).
        if !followable_root(ctx, file, &bound, member.span) {
            continue;
        }
        // A member of a namespace import is a function the facts may say nothing of; a member of any other object is one where the compiler says it is callable
        // (a call's argument is typed: `[..].forEach(kernel.run)`), never by the kind of a property signature, which a function and a number share.
        let function =
            if root_import(file, &bound, member.span).is_some_and(|import| import.namespace) {
                may_be_function(ctx, file, member.span, member.span)
            } else {
                callable_fact(ctx, file, member.span) == Some(true)
            };
        let value = function && handed_as_value(ctx, file, member.span);
        // What can be a site is asked first: the walk of where the value came from is the dearer question.
        if !value && !evaluated_member(file, member.span) {
            continue;
        }
        // A member of the global object read for its value (`void window.devicePixelRatio`, `typeof self.x`, `globalThis["x"]`) runs the accessor a package defined for
        // that global; a key the program does not spell may be any of them.
        if accessors_possible
            && evaluated_member(file, member.span)
            && matches!(
                origin_of(ctx, analyzed, file, &bound, member.object, ORIGIN_DEPTH),
                Origin::GlobalObject
            )
        {
            let accessor = match member_key(file, member) {
                Key::Name(name) => global_accessor(ctx, &name),
                Key::Unspelled => unspelled_accessor(ctx),
            };
            push(member.span, member.span, Entry::Read, accessor);
        }
        let (Origin::Package(origin) | Origin::Lazy(origin)) =
            origin_of(ctx, analyzed, file, &bound, member.span, ORIGIN_DEPTH)
        else {
            continue;
        };
        if value {
            let at = handed_at(file, member.span);
            if !crate::reviewed_api::clears_value_at(ctx, file, at) {
                push(at, member.span, Entry::Value, Some(origin));
            }
            continue;
        }
        // The member is evaluated for its value where it stands (`void clock.now`): a getter or a setter the package's closure defines under a name of the chain
        // (or under a name nothing fixes, or in code nobody read) runs there. A global the project declares is not followed: its provider is not a package's closure.
        if !is_pseudo(&origin.path)
            && accessor_facts(ctx, &origin.path).may_run(&chain_names(file, member.span))
        {
            push(member.span, member.span, Entry::Read, Some(origin));
        }
    }
    // A global a package defines an accessor for runs the accessor where the name is read: `void devicePixelRatio`, `typeof devicePixelRatio`, `[devicePixelRatio]`. A call of
    // it is the call's site, a member read of it the member's.
    for identifier in &file.ast.identifiers {
        if !accessors_possible
            || identifier.role != solid_facts::ast::IdentifierRole::Reference
            || file.ast.reference_declaration(identifier.span).is_some()
            || !evaluated_reference(file, identifier.span)
        {
            continue;
        }
        let Some(name) = file.source_text(identifier.span) else {
            continue;
        };
        push(
            identifier.span,
            identifier.span,
            Entry::Read,
            global_accessor(ctx, name),
        );
    }
    // A getter a package declares runs where an object pattern reads it too: `const { now } = clock` and `({ now } = clock)` read each key the pattern names off the
    // value, a computed key or a rest element reads any (ADR 0974). An array pattern reads the iterator, which is not followed.
    let slots = {
        let mut slots = HashMap::<Span, Vec<&solid_facts::ast::PatternElementFact>>::default();
        for element in &file.ast.pattern_elements {
            slots.entry(element.pattern).or_default().push(element);
        }
        slots
    };
    // A pattern written as a parameter reads each key it names off the argument the call passes (`(({ now = 0 }) => now)(clock)`, `show(clock)` with `function show({ now })`):
    // the same read, at the parameter's pattern. The function and the call are in this file; a function of another file reads its parameters where that file's own sites are found.
    let mut parameter_values = Vec::<(Span, Span)>::new();
    // Only a function with a parameter written as a pattern reads anything this way.
    let pattern_parameters = file.ast.functions.iter().any(|function| {
        function
            .parameters
            .iter()
            .any(|parameter| slots.contains_key(&parameter.pattern))
    });
    for call in file.ast.calls.iter().filter(|_| pattern_parameters) {
        let Some((function_file, function)) = lookup
            .function_called_at(file.path.as_str(), call.callee)
            .or_else(|| lookup.object_member_function(file, call.callee))
            // A function written where it is called: `(({ now }) => now)(clock)`.
            .or_else(|| {
                file.ast
                    .function_at(file.ast.peel_ts_sugar_span(call.callee))
                    .map(|function| (file, function))
            })
        else {
            continue;
        };
        if !std::ptr::eq(function_file, file) {
            continue;
        }
        for (index, argument) in call.arguments.iter().enumerate() {
            let Some(parameter) = function.parameters.get(index) else {
                break;
            };
            if argument.spread {
                break;
            }
            if slots.contains_key(&parameter.pattern) {
                parameter_values.push((parameter.pattern, argument.span));
            }
        }
    }
    for &(pattern, value) in file.ast.pattern_values.iter().chain(&parameter_values) {
        let at = file.ast.peel_ts_sugar_span(value);
        if !followable_root(ctx, file, &bound, at) {
            continue;
        }
        let origin = origin_of(ctx, analyzed, file, &bound, at, ORIGIN_DEPTH);
        // The pattern reads each key it names off the global object (`const { devicePixelRatio: ratio } = window`): the keys the pattern spells at its own level are
        // globals, the keys below are members of what those hold; a computed key or a rest element reads any global.
        if matches!(origin, Origin::GlobalObject) {
            if !accessors_possible {
                continue;
            }
            let mut accessor = None;
            let elements = slots.get(&pattern).map_or(&[][..], Vec::as_slice);
            for element in elements {
                accessor = accessor.or_else(|| match &element.slot {
                    solid_facts::ast::PatternSlot::Key(name) => global_accessor(ctx, name),
                    solid_facts::ast::PatternSlot::ComputedKey
                    | solid_facts::ast::PatternSlot::Rest => unspelled_accessor(ctx),
                    solid_facts::ast::PatternSlot::Index(_) => None,
                });
            }
            push(pattern, pattern, Entry::Read, accessor);
            continue;
        }
        let Origin::Package(origin) = origin else {
            continue;
        };
        if is_pseudo(&origin.path) {
            continue;
        }
        let mut names = Vec::<Option<&str>>::new();
        // The getter runs where the pattern reads the key (`{ now }`: the runtime reports the pattern's start), not where the value was named; the pattern is the
        // site's own place, so a hand-off of the value it destructures (an import that may be a function) is another row, not this one.
        if pattern_keys(&slots, pattern, &mut names, 0)
            && !names.is_empty()
            && accessor_facts(ctx, &origin.path).may_run(&names)
        {
            push(pattern, pattern, Entry::Read, Some(origin));
        }
    }
    for property in &file.ast.object_properties {
        // The facts answer the property's symbol at a shorthand's span, never the binding's: the binding is read through its import.
        let Some(binding) = property.shorthand_binding else {
            continue;
        };
        let Some(import) = bound.get(&binding) else {
            continue;
        };
        let (Origin::Package(origin) | Origin::Lazy(origin)) =
            import_origin(ctx, analyzed, file, import, binding, ORIGIN_DEPTH)
        else {
            continue;
        };
        if import.namespace || may_be_function(ctx, file, property.value, binding) {
            push(property.value, property.value, Entry::Value, Some(origin));
        }
    }
    // The global object handed to a call (`read(window)`, `Object.entries(globalThis)`, `new Reader(self)`): what the callee reads off it is not followed (the package side's
    // `GlobalObjectEscape`), so it may read any global, or call one: a route that is not decided is a read of any name. The row stands at the call that receives it, where a throw
    // inside the callee has its project frame.
    // It is a site only where a package that may reach a Solid action writes a global under a key nothing decides: where none does, no argument is asked.
    if let Origin::Package(origin) = unspelled_origin(ctx) {
        for call in &file.ast.calls {
            for argument in &call.arguments {
                let at = file.ast.peel_ts_sugar_span(argument.span);
                if !followable_root(ctx, file, &bound, at) {
                    continue;
                }
                if let Origin::GlobalObject =
                    origin_of(ctx, analyzed, file, &bound, at, ORIGIN_DEPTH)
                {
                    push(call.span, at, Entry::Value, Some(origin.clone()));
                }
            }
        }
    }
    // A call's result handed on as an argument (`[…].forEach(pick())`): the call that receives it runs it, and the value is what the inner call made.
    for call in &file.ast.calls {
        for argument in &call.arguments {
            let at = file.ast.peel_ts_sugar_span(argument.span);
            if file.ast.call_at(at).is_none() {
                continue;
            }
            let Origin::Package(origin) = origin_of(ctx, analyzed, file, &bound, at, ORIGIN_DEPTH)
            else {
                continue;
            };
            if callable_fact(ctx, file, at) == Some(true) && handed_as_value(ctx, file, at) {
                push(call.span, at, Entry::Value, Some(origin));
            }
        }
    }
    sites.sort_by_key(|site| (site.at.start, site.at.end));
    sites
}

/// The keys an object pattern reads off the value it destructures, a nested pattern's too (`{ a: { b } }` reads `a` off the value and `b` off that): `None` in `names` for
/// a computed key or a rest element. `false` for an array pattern, which reads the iterator.
fn pattern_keys<'f>(
    slots: &HashMap<Span, Vec<&'f solid_facts::ast::PatternElementFact>>,
    pattern: Span,
    names: &mut Vec<Option<&'f str>>,
    depth: usize,
) -> bool {
    use solid_facts::ast::PatternSlot;
    let Some(elements) = slots.get(&pattern) else {
        return true;
    };
    if depth > 8 {
        names.push(None);
        return true;
    }
    for element in elements {
        match &element.slot {
            PatternSlot::Index(_) => return false,
            PatternSlot::Key(name) => names.push(Some(name.as_str())),
            PatternSlot::ComputedKey | PatternSlot::Rest => names.push(None),
        }
        // A slot that binds a pattern reads its own keys off the member's value.
        if slots.contains_key(&element.target) {
            pattern_keys(slots, element.target, names, depth + 1);
        }
    }
    true
}

/// Where the closure of the package a declaration at `path` describes may define an accessor: open (any member may be one) where the adapter reads no filesystem.
fn accessor_facts(ctx: &AnalysisContext<'_>, path: &str) -> AccessorFacts {
    ctx.package_reach.map_or_else(AccessorFacts::open, |index| {
        index.accessors_for_declaration(path)
    })
}

/// Whether the member expression at `span` is evaluated for its value: not a callee, a tag, the object of a longer chain (the chain's last member is the read), an
/// import, export or type position, or an element's name. A getter runs where the member is evaluated, so a value handed to a scheduler, or an intrinsic element's event
/// attribute, is still a read: [`handed_as_value`]'s exemptions are for where a handed *function* runs.
fn evaluated_member(file: &FileFacts, span: Span) -> bool {
    file.ast.members_with_peeled_object(span).next().is_none() && evaluated_reference(file, span)
}

/// Whether the expression at `span` is evaluated for its value where it stands: not a callee, a tag, an import, export or type position, or an element's name. The object of
/// a longer chain is evaluated (`window` of `window.innerWidth` is read), though the chain's last member is the read [`evaluated_member`] asks about.
fn evaluated_reference(file: &FileFacts, span: Span) -> bool {
    let ast = &file.ast;
    if ast.calls_with_peeled_callee(span).next().is_some()
        || ast
            .tagged_templates
            .iter()
            .any(|template| ast.peel_ts_sugar_span(template.tag) == span)
        || ast.imports.iter().any(|import| import.span.contains(span))
        || ast.exports.iter().any(|export| {
            (export.module.is_some() && export.span.contains(span))
                || export
                    .specifiers
                    .iter()
                    .any(|specifier| specifier.local.span.contains(span))
        })
        || ast.type_names.iter().any(|name| name.contains(span))
        || ast.type_queries.iter().any(|query| query.contains(span))
        || ast
            .type_imports
            .iter()
            .any(|import| import.span.contains(span))
    {
        return false;
    }
    !ast.jsx_elements.iter().any(|element| {
        element.name.span.contains(span)
            || element
                .closing_name
                .is_some_and(|closing| closing.contains(span))
    })
}

/// The names read off a package's object by the member chain ending at `span` (`pkg.a.b[k]` is `a`, `b` and a computed key), from the member nearest the root.
fn chain_names(file: &FileFacts, span: Span) -> Vec<Option<&str>> {
    let ast = &file.ast;
    let mut names = Vec::new();
    let mut at = ast.peel_ts_sugar_span(span);
    while let Some(member) = ast.member_at(at) {
        if names.len() >= 32 {
            names.push(None);
            break;
        }
        names.push(if ast.computed_members.binary_search(&at).is_ok() {
            None
        } else {
            file.source_text(member.property)
        });
        at = ast.peel_ts_sugar_span(member.object);
    }
    names
}

/// What the compiler says of the value at `span`: `Some(true)` when it is callable or constructable, `Some(false)` when it is neither, `None` when the facts say
/// nothing (the demand plan asks for callability at a few spans only: a call's argument is one).
fn callable_fact(ctx: &AnalysisContext<'_>, file: &FileFacts, span: Span) -> Option<bool> {
    use typefacts::{Callability, Constructability};
    let entity = ctx.semantic_lookup.entity_at(file.path.as_str(), span)?;
    if matches!(
        entity.constructability,
        Some(Constructability::Constructable)
    ) {
        return Some(true);
    }
    entity
        .callability
        .map(|callability| callability != Callability::NonCallable)
}

/// Whether the value of the reference at `span` may be a function: the compiler types it callable or constructable, or says nothing and the declaration it names
/// is no type-level one (an interface, a type alias, an enum or a namespace holds no function value of its own). Known to be neither callable nor
/// constructable, it is not.
fn may_be_function(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    span: Span,
    symbol_at: Span,
) -> bool {
    if let Some(known) = callable_fact(ctx, file, span) {
        return known;
    }
    // No callability fact: the kind of the declaration the symbol names, through its import alias. The producer's kinds are `variable`, `function`,
    // `class` and `declaration` (everything else): only the last is no function value. `symbol_at` is where the import's own symbol is asked (a
    // shorthand property's span holds the property's).
    ctx.semantic_lookup
        .alias_root_declarations(file, symbol_at)
        .is_none_or(|declarations| {
            declarations.is_empty()
                || declarations
                    .iter()
                    .any(|declaration| declaration.kind.as_ref() != "declaration")
        })
}

/// Whether the reference at `span` hands its value on: not a callee, a tag, the object of a member (`pkg.run`, decided as that member's
/// call; `.call`, `.apply` and `.bind` hand it), an import, export or type position, an element's tag, the value of an intrinsic element's
/// event attribute (the browser's listener) or a direct argument of a standard-library scheduler that runs it on a fresh stack.
fn handed_as_value(ctx: &AnalysisContext<'_>, file: &FileFacts, span: Span) -> bool {
    let ast = &file.ast;
    let lookup = ctx.semantic_lookup;
    if ast.calls_with_peeled_callee(span).next().is_some()
        || ast
            .tagged_templates
            .iter()
            .any(|template| ast.peel_ts_sugar_span(template.tag) == span)
    {
        return false;
    }
    if let Some(member) = ast.members_with_peeled_object(span).next() {
        let property = file.source_text(member.property).unwrap_or_default();
        return matches!(property, "call" | "apply" | "bind");
    }
    if ast.imports.iter().any(|import| import.span.contains(span))
        // An export specifier (`export { runStep }`) or a re-export names the binding; the span of `export function f() { … }` holds the whole body.
        || ast.exports.iter().any(|export| {
            (export.module.is_some() && export.span.contains(span))
                || export
                    .specifiers
                    .iter()
                    .any(|specifier| specifier.local.span.contains(span))
        })
        || ast.type_names.iter().any(|name| name.contains(span))
        || ast.type_queries.iter().any(|query| query.contains(span))
        || ast
            .type_imports
            .iter()
            .any(|import| import.span.contains(span))
    {
        return false;
    }
    for element in &ast.jsx_elements {
        if element.name.span.contains(span)
            || element
                .closing_name
                .is_some_and(|closing| closing.contains(span))
        {
            return false;
        }
        if crate::object_members::intrinsic_element(file, element)
            && element.attributes.iter().any(|attribute| {
                attribute
                    .expression
                    .is_some_and(|value| ast.peel_ts_sugar_span(value) == span)
                    && file
                        .source_text(attribute.name)
                        .is_some_and(|name| name.starts_with("on"))
            })
        {
            return false;
        }
    }
    for (call, index) in ast.arguments_containing(span) {
        if ast.peel_ts_sugar_span(call.arguments[index].span) != span {
            continue;
        }
        let Some(resolved) = lookup.resolved_callee_call(file, call.callee) else {
            continue;
        };
        let callability = lookup
            .entity_at(file.path.as_str(), span)
            .and_then(|entity| entity.callability);
        if matches!(
            crate::runtime_semantics::argument_behavior(resolved, callability, index),
            Some(crate::runtime_semantics::RuntimeArgumentBehavior::FreshStackCallback)
        ) {
            return false;
        }
    }
    true
}

#[derive(Clone)]
struct DeclarationAt {
    path: String,
}

/// How many hops the origin of a value is followed (a call's callee, a member's object, a binding's initializer).
const ORIGIN_DEPTH: usize = 48;

/// How many hops one origin question takes in all (calls, members, bindings, array elements, returns, exports) before it stops and says so ([`limit`]): a limit is a
/// site, never a silence.
const ORIGIN_STEPS: usize = 4096;

/// How many array elements, returns and arguments one hop reads before it stops and says so.
const ORIGIN_WIDTH: usize = 256;

thread_local! {
    /// The steps the current origin question may still take: a question starts at [`ORIGIN_DEPTH`] and every walk it makes spends from this.
    static STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(ORIGIN_STEPS) };
}

/// Spends one step of the current origin question; `false` when none is left.
fn spend() -> bool {
    STEPS.with(|steps| match steps.get() {
        0 => false,
        left => {
            steps.set(left - 1);
            true
        }
    })
}

thread_local! {
    /// The functions and exports the current walk is inside of: a walk that comes back to one adds nothing the first visit does not (`return search(node.left)`,
    /// `export * from "./b"` and `export * from "./a"`).
    static ACTIVE: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    /// What the exports of the project's modules were found to be, by module and name: a barrel is asked about once per name.
    static EXPORTS: std::cell::RefCell<HashMap<(String, String), Origin>> = std::cell::RefCell::new(HashMap::default());
}

/// Runs `body` as the walk of `key` (`fn:<path>:<start>`, `export:<path>:<name>`), or answers `Origin::Unknown` when the walk is already inside it.
fn walking(key: String, body: impl FnOnce() -> Origin) -> Origin {
    let entered = ACTIVE.with(|active| {
        let mut active = active.borrow_mut();
        if active.contains(&key) {
            false
        } else {
            active.push(key);
            true
        }
    });
    if !entered {
        return Origin::Unknown;
    }
    let origin = body();
    ACTIVE.with(|active| {
        active.borrow_mut().pop();
    });
    origin
}

/// A value whose origin the walk stopped following at a limit: what runs for it is not decided, so a call of it is a site (`package-may-call-action:origin-limit`).
fn limit(what: &str) -> Origin {
    Origin::Package(DeclarationAt {
        path: format!("{LIMIT_PREFIX}{what}"),
    })
}

/// Whether `path` is a pseudo-path of a site that names no package ([`AMBIENT_PREFIX`], [`LIMIT_PREFIX`], [`COMPUTED_LOAD_PREFIX`]): no closure defines an accessor for it.
fn is_pseudo(path: &str) -> bool {
    path.starts_with(AMBIENT_PREFIX)
        || path.starts_with(LIMIT_PREFIX)
        || path.starts_with(COMPUTED_LOAD_PREFIX)
        || path.starts_with(WRITE_PREFIX)
        || path.starts_with(WRITE_ANY_PREFIX)
        || path.starts_with(READ_ANY_PREFIX)
}

/// Where the value an expression names came from.
#[derive(Clone)]
enum Origin {
    /// Code of a package the build does not read: the declaration path that names its package.
    Package(DeclarationAt),
    /// The component a dialect's `lazy` made of a module a package holds ([`lazy_origin`]): the value runs the package's code where it is rendered or called, and its
    /// members (`preload`) are the dialect's.
    Lazy(DeclarationAt),
    /// The host's own function or object, proven: a global the standard library declares, and a member of one that the standard library declares too (a member a
    /// package adds to the global's type is not this: `window.fzRun`, `Uint8Array.fzFrom`, see [`host_member_origin`]). No package's code runs for it.
    Host,
    /// The dialect's runtime or a `node:` builtin: its catalog answers for it and for its members.
    Runtime,
    /// The global object itself (`window`, `self`, `globalThis`, `top`, `parent`, `frames`, `global`, `window.self`, `document.defaultView`) or a value that may be it (a name
    /// it is bound to, an operand of a conditional, an element of an array): what a member of it, a pattern key of it or a call of that member is, is a global of that name
    /// ([`global_member_origin`]), or of any name where the key is not spelled.
    GlobalObject,
    /// The namespace of a module of the project (`import * as re from "./re"`, `export * as ns from "./re"`, `const r = re`): the path of its file. What a member of it is
    /// comes from what the module exports under that name ([`export_origin`]).
    Namespace(String),
    /// The chain ends where this pass does not follow it (a parameter, a property, the result of a host or runtime function that may hand back what it was given:
    /// `untrack`, `useContext`, `Object.freeze`): the callee's declaration decides.
    Unknown,
}

/// The installed path that names the package whose code a declaration at `path` describes: a file under `node_modules`, the link's `node_modules` spelling of a
/// file in a linked workspace package, or a project declaration file with runtime code beside it that the program does not read. `None` for the standard
/// library, a file the program analyzes itself (a package read as source, a JavaScript file `allowJs` brings in) and a declaration no installed package holds (a
/// global a script provides, a `declare module` shim: the import's file stands for the shim's package).
fn installed_path(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    path: &str,
) -> Option<String> {
    if path.starts_with("bundled:")
        || ctx.semantic_lookup.in_dependency_source(path)
        || !(is_declaration_file(path) || !analyzed.contains(path))
    {
        return None;
    }
    if path.contains("/node_modules/") {
        return Some(path.to_owned());
    }
    let index = ctx.package_reach?;
    index
        .linked_spelling(path)
        .or_else(|| index.is_project_js(path).then(|| path.to_owned()))
}

/// Where the value `span` names came from: what a call or `new` makes comes from its callee's code (a package function's result is the package's; a project
/// function's result is what it returns; a host or runtime function's result is not followed, except what a standard-library function that hands back, wraps or merges
/// its arguments carries and a host constructor's fresh object), a member is read off its object, an import names its package or its project module's export, an array
/// literal holds its elements, a conditional, logical or comma expression is one of its operands, and a project binding of one plain name to one initializer holds what
/// that makes. A global the standard library declares is the host's, whatever a package adds to its type. A walk that reaches a limit says so ([`limit`]).
fn origin_of(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    bound: &HashMap<Span, ImportBinding<'_>>,
    span: Span,
    depth: usize,
) -> Origin {
    if depth >= ORIGIN_DEPTH {
        STEPS.with(|steps| steps.set(ORIGIN_STEPS));
    }
    let lookup = ctx.semantic_lookup;
    let ast = &file.ast;
    let span = ast.peel_ts_sugar_span(span);
    if depth == 0 {
        // The chain is longer than the walk follows: a value that still has somewhere to come from is not decided.
        return if is_leaf(ctx, file, bound, span) {
            Origin::Unknown
        } else {
            limit(&format!("a chain of more than {ORIGIN_DEPTH} hops"))
        };
    }
    if !spend() {
        return limit(&format!("a question of more than {ORIGIN_STEPS} steps"));
    }
    if let Some(call) = ast.call_at(span) {
        // `lazy(() => import("pkg"))` makes a component of the module its loader imports, not of the dialect's `lazy`.
        if let Some(index) = lookup
            .primitive_at_call(file, call.span)
            .and_then(|primitive| ctx.dialect.component_loader_argument(primitive))
        {
            return lazy_origin(ctx, analyzed, file, call, index);
        }
        return match origin_of(ctx, analyzed, file, bound, call.callee, depth - 1) {
            Origin::Package(declaration) => Origin::Package(declaration),
            // What a component the dialect's `lazy` made returns is the package's.
            Origin::Lazy(declaration) => Origin::Package(declaration),
            Origin::Host => host_call_origin(ctx, analyzed, file, bound, call, true, depth - 1),
            // The dialect's runtime hands back no package's value (its catalog answers for what it returns): there is no argument of the standard library to carry.
            Origin::Runtime => Origin::Unknown,
            Origin::Namespace(_) | Origin::GlobalObject => Origin::Unknown,
            Origin::Unknown => function_return_origin(ctx, analyzed, file, bound, call, depth - 1),
        };
    }
    if let Some(member) = ast.member_at(span) {
        return match origin_of(ctx, analyzed, file, bound, member.object, depth - 1) {
            Origin::Package(declaration) => Origin::Package(declaration),
            // `Lazy.preload` is the dialect's wrapper, not the package's component.
            Origin::Lazy(_) | Origin::Runtime => Origin::Runtime,
            // A member of the global object is the global of that name, whatever route the object came by and however the key is spelled.
            Origin::GlobalObject => global_member_origin(ctx, analyzed, file, member),
            // A member of a global the standard library declares is the host's where the library alone declares the member, and code something else provides
            // where a package or a declaration file adds it (`window.__act`).
            Origin::Host if is_document_default_view(file, member) => Origin::GlobalObject,
            Origin::Host => host_member_origin(
                ctx,
                analyzed,
                file,
                member.span,
                member.property,
                None,
                false,
            ),
            // A member of a project module's namespace is what the module exports under that name.
            Origin::Namespace(path) => {
                namespace_export_origin(ctx, analyzed, file, member, &path, depth - 1)
            }
            // A member of a project object literal or class instance holds what it was given there; a project module's namespace member is the module's export.
            Origin::Unknown => match holder_member_origin(ctx, analyzed, file, member, depth - 1) {
                Origin::Unknown => namespace_member_origin(ctx, analyzed, file, bound, member),
                found => found,
            },
        };
    }
    if let Some(array) = ast.array_literal_at(span) {
        let mut held = Origin::Unknown;
        for element in array.elements.iter().take(ORIGIN_WIDTH) {
            if element.kind == solid_facts::ast::ArrayElementKind::Hole {
                continue;
            }
            match origin_of(ctx, analyzed, file, bound, element.span, depth - 1) {
                Origin::Package(declaration) => return Origin::Package(declaration),
                // An element may be the global object (`[window][0].btoa(x)`), read by a member of the array's own element.
                Origin::GlobalObject => held = Origin::GlobalObject,
                // Any other element is no package's code and not the global object: it adds nothing here (the whole array is then followed no further, below).
                Origin::Host
                | Origin::Runtime
                | Origin::Lazy(_)
                | Origin::Namespace(_)
                | Origin::Unknown => {}
            }
        }
        return if array.elements.len() > ORIGIN_WIDTH {
            limit(&format!("an array of more than {ORIGIN_WIDTH} elements"))
        } else {
            held
        };
    }
    // One of the operands: `flag ? () => {} : makeVoid()`, `cb ?? makeVoid()`, `(0, makeVoid())`.
    if let Some(conditional) = ast.conditional_at(span) {
        let consequent = origin_of(
            ctx,
            analyzed,
            file,
            bound,
            conditional.consequent,
            depth - 1,
        );
        let alternate = origin_of(ctx, analyzed, file, bound, conditional.alternate, depth - 1);
        return combine(consequent, alternate);
    }
    if let Some(logical) = ast.logical_at(span) {
        let left = origin_of(ctx, analyzed, file, bound, logical.left, depth - 1);
        let right = origin_of(ctx, analyzed, file, bound, logical.right, depth - 1);
        return combine(left, right);
    }
    if let Some(sequence) = ast.sequences.iter().find(|sequence| sequence.span == span) {
        return origin_of(ctx, analyzed, file, bound, sequence.tail, depth - 1);
    }
    let Some(declaration) = ast.reference_declaration(span) else {
        return global_origin(ctx, analyzed, file, span);
    };
    if let Some(import) = bound.get(&declaration) {
        return import_origin(ctx, analyzed, file, import, span, depth);
    }
    let Some((next, initializer)) = value_initializer(lookup, file, span) else {
        return Origin::Unknown;
    };
    let origin = if std::ptr::eq(next, file) {
        origin_of(ctx, analyzed, file, bound, initializer, depth - 1)
    } else {
        let next_bound = import_bindings(ctx, next);
        origin_of(ctx, analyzed, next, &next_bound, initializer, depth - 1)
    };
    // `const { reRun } = re`: the slot reads one member of what the initializer is, which is a project module's namespace here.
    match origin {
        Origin::Namespace(path) if std::ptr::eq(next, file) => match slot_key(file, declaration) {
            Some(key) => project_file(ctx, &path).map_or(Origin::Unknown, |target| {
                export_origin(ctx, analyzed, target, key, depth - 1)
            }),
            None => Origin::Namespace(path),
        },
        Origin::Namespace(_) => Origin::Unknown,
        // `const { btoa: enc } = window`: each slot reads one member of the global object, `{ [key]: enc }` and a rest read any.
        Origin::GlobalObject if std::ptr::eq(next, file) => {
            global_slot_origin(ctx, file, declaration)
        }
        Origin::GlobalObject => Origin::Unknown,
        other => other,
    }
}

/// Where the component a `lazy(loader)` call makes comes from (ADR 0974): the modules its loader function literal imports dynamically (`() => import("pkg")`,
/// `() => import("pkg").then((m) => ({ default: m.View }))`), the loader's own code being the project's. A relative or runtime module is the project's or the
/// dialect's; a package's is code of that package, which the first of them whose closure is not clear stands for; a module named by anything but a literal
/// may be any package's. A loader that is no function literal, or imports nothing, ends where this pass does not follow (the dialect's `lazy`).
fn lazy_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    loader: usize,
) -> Origin {
    let Some(loader) = call.arguments.get(loader) else {
        return Origin::Runtime;
    };
    let loader = file.ast.peel_ts_sugar_span(loader.span);
    let mut chosen = None::<DeclarationAt>;
    for load in &file.ast.module_loads {
        if load.kind != solid_facts::ast::ModuleLoadKind::DynamicImport
            || !loader.contains(load.span)
        {
            continue;
        }
        let declaration = match load.specifier.as_deref() {
            None => Some(DeclarationAt {
                path: format!("{COMPUTED_LOAD_PREFIX}import()"),
            }),
            // The project's own module, unless JavaScript the program does not analyze stands behind it (a shim) or the module hands on a package's export (a re-export of
            // any form); a rooted specifier names a path only the bundler resolves.
            Some(module) if module.starts_with('.') => unanalyzed_project_js(ctx, file, module)
                .or_else(|| relative_module_package(ctx, analyzed, file, load, module)),
            Some(module) if module.starts_with('/') => Some(DeclarationAt {
                path: format!("{COMPUTED_LOAD_PREFIX}{module}"),
            }),
            Some(module) => {
                let binding = ImportBinding {
                    module,
                    declaration: load.span,
                    namespace: false,
                    decided_elsewhere: is_runtime_module(ctx, module)
                        || module.starts_with("node:"),
                    imported: None,
                };
                import_declaration(ctx, analyzed, file, &binding)
            }
        };
        let Some(declaration) = declaration else {
            continue;
        };
        let clear = ctx
            .package_reach
            .is_some_and(|index| index.for_declaration(&declaration.path).is_clear());
        match &chosen {
            None => chosen = Some(declaration),
            Some(held)
                if !clear
                    && ctx
                        .package_reach
                        .is_some_and(|index| index.for_declaration(&held.path).is_clear()) =>
            {
                chosen = Some(declaration);
            }
            Some(_) => {}
        }
    }
    chosen.map_or(Origin::Runtime, Origin::Lazy)
}

/// The package whose component a project module hands `lazy` when the loader imports the module (`lazy(() => import("./view"))`): the module's `default` export where `lazy`
/// reads only that (the facts name the one export), any export of it where the loader reads the result another way (`.then((m) => ({ default: m.View }))`), each followed through
/// the module's own declarations and every re-export: `export { default } from "pkg"`, `export { View as default } from "pkg"`, `export * from "pkg"`,
/// `import View from "pkg"; export default View`. `None` where no export of it is a package's.
fn relative_module_package(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    load: &solid_facts::ast::ModuleLoadFact,
    module: &str,
) -> Option<DeclarationAt> {
    // A module the compiler did not attest, or the program does not analyze, exports what nobody read: the loader may hand `lazy` a package's component, so the row
    // stands (an unresolved hop is an uncertifiable row, never silence).
    let Some(target) = landing_file(ctx, file, load.specifier_span.unwrap_or(load.span), module)
    else {
        return Some(unresolved_hop(module));
    };
    let origin = match &load.consumer {
        Some(solid_facts::ast::ModuleLoadConsumer::Lazy { export, .. }) => {
            export_origin(ctx, analyzed, target, export, ORIGIN_DEPTH)
        }
        _ => any_export_origin(ctx, analyzed, target, ORIGIN_DEPTH),
    };
    match origin {
        Origin::Package(declaration) | Origin::Lazy(declaration) => Some(declaration),
        _ => None,
    }
}

/// The first package origin among the exports of the project module `module`, `default` included: the names it exports, and what each `export * from` brings (a package's
/// namespace is any of its exports, a project module's are its own).
fn any_export_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    module: &FileFacts,
    depth: usize,
) -> Origin {
    if depth == 0 {
        return limit(&format!("a chain of more than {ORIGIN_DEPTH} re-exports"));
    }
    if !spend() {
        return limit(&format!("a question of more than {ORIGIN_STEPS} steps"));
    }
    let mut names = vec!["default".to_owned()];
    let mut stars = Vec::new();
    for export in module
        .ast
        .module_level_exports()
        .filter(|export| !export.type_only)
    {
        names.extend(
            export
                .declarations
                .iter()
                .filter(|declaration| !declaration.type_only)
                .map(|declaration| declaration.exported.to_string()),
        );
        names.extend(
            export
                .specifiers
                .iter()
                .filter(|specifier| !specifier.type_only)
                .map(|specifier| specifier.exported.to_string()),
        );
        names.extend(export.namespace.iter().map(ToString::to_string));
        if export.kind == solid_facts::ast::ExportKind::All
            && export.namespace.is_none()
            && export.module.is_some()
        {
            stars.push(export);
        }
    }
    names.sort();
    names.dedup();
    for name in names.iter().take(ORIGIN_WIDTH) {
        if let Origin::Package(declaration) | Origin::Lazy(declaration) =
            export_origin(ctx, analyzed, module, name, depth - 1)
        {
            return Origin::Package(declaration);
        }
    }
    for export in stars.iter().take(ORIGIN_WIDTH) {
        let Some(from) = export.module.as_deref() else {
            continue;
        };
        let found = if from.starts_with('.') || from.starts_with('/') {
            match landing_file(ctx, module, export.span, from) {
                Some(target) => {
                    let key = format!("any-export:{}", target.path);
                    walking(key, || any_export_origin(ctx, analyzed, target, depth - 1))
                }
                None => unanalyzed_project_js(ctx, module, from)
                    .map_or_else(|| Origin::Package(unresolved_hop(from)), Origin::Package),
            }
        } else {
            reexport_origin(ctx, analyzed, module, export.span, from, None, depth - 1)
        };
        if let Origin::Package(declaration) = found {
            return Origin::Package(declaration);
        }
    }
    if names.len() > ORIGIN_WIDTH || stars.len() > ORIGIN_WIDTH {
        return limit(&format!(
            "a module with more than {ORIGIN_WIDTH} exports or `export *`"
        ));
    }
    Origin::Unknown
}

/// Where a global (a name no binding of the file declares) came from, by where the compiler says it is declared: the standard library's is the host's, and a package that
/// declares an overload of it (`declare global { function setTimeout(cb: () => void, fz: "fz"): number }`) answers for the calls that resolve to its overload, which
/// [`package_declaration`] asks of the signature the compiler chose (every global `@types/node` declares again is a declaration of it too, and its calls resolve to the
/// library's). An installed package's declaration is decided by that package; a project declaration file that no module of the program implements (`declare function
/// gtag(…)`) declares a global the host, a script or an installed package may provide (ADR 0974: a package that writes the global is code the program loads, and
/// Chrome ran it).
fn global_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    span: Span,
) -> Origin {
    // The global object goes by several names, whatever the library declares them as.
    if file.source_text(span).is_some_and(is_global_object_name) {
        return Origin::GlobalObject;
    }
    // A name the compiler resolves to nothing at all is unknown; one it resolves to a symbol no file declares is the language's own (`globalThis`, `undefined`):
    // the host's.
    let Some(declarations) = ctx.semantic_lookup.alias_root_declarations(file, span) else {
        // No fact says where the name is declared (the demand plan asks for the symbols of callees and arguments): a package that writes a global of this name is the code
        // an unbound reference runs, whether or not the compiler was asked.
        return file
            .source_text(span)
            .and_then(|name| written_global(ctx, name))
            .unwrap_or(Origin::Unknown);
    };
    let paths = declaration_paths_of(declarations);
    if paths.is_empty() || paths.iter().any(|path| path.starts_with("bundled:")) {
        // The library's declaration types the global; whether an installed package replaces it at run time is a fact of the packages' code, not of the type.
        return file
            .source_text(span)
            .and_then(|name| written_global(ctx, name))
            .unwrap_or(Origin::Host);
    }
    declarations_origin(ctx, analyzed, &paths).unwrap_or(Origin::Unknown)
}

/// The origin of a read of the global `name` (a member of the global object, a bare name, a pattern key, an operand of `typeof`) when some installed package the program
/// loads may define an accessor for it on the global object and may reach a Solid action: the accessor runs where the global is read. A data write is not one (a read of
/// it runs no code); a package that defines an accessor under a key nothing decides answers for a name the others do not define ([`WRITE_ANY_PREFIX`]).
fn global_accessor(ctx: &AnalysisContext<'_>, name: &str) -> Option<DeclarationAt> {
    // `undefined`, `NaN` and `Infinity` are properties of the global object that cannot be redefined.
    if matches!(name, "undefined" | "NaN" | "Infinity") {
        return None;
    }
    if !is_identifier_name(name) {
        return None;
    }
    let (root, named) = ctx.package_reach?.global_accessor_writers_of(name).next()?;
    let prefix = if named {
        WRITE_PREFIX
    } else {
        WRITE_ANY_PREFIX
    };
    Some(DeclarationAt {
        path: format!("{prefix}{name}:{root}"),
    })
}

/// [`global_accessor`] for a global the program reads by a key it does not spell: the accessor of any package that may define one.
fn unspelled_accessor(ctx: &AnalysisContext<'_>) -> Option<DeclarationAt> {
    ctx.package_reach?
        .global_accessor_writers_of_unspelled()
        .next()
        .map(|root| DeclarationAt {
            path: format!("{READ_ANY_PREFIX}{root}"),
        })
}

/// Whether `name` can be the name of a global: an identifier. The origin walk can end on the text of any expression (a project function written as an arrow function ends on
/// `() => {}`), and no package writes a global by that name, whatever any-name writer the program loads.
fn is_identifier_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|next| next.is_ascii_alphanumeric() || next == '_' || next == '$')
}

/// The origin of a global the standard library declares that some installed package the program loads writes at run time and may reach a Solid action there
/// (`globalThis.setTimeout = …`, `Object.defineProperty(window, "fetch", …)`, found by the scan of each package's closure): the writer's code is what a call of it runs,
/// whatever type the library declares the global with. The premise is that types are sound for declarations and runtime writes come from the scan. A writer under a key
/// nothing in its file decides may have written any global, so it answers for a name the others do not write ([`WRITE_ANY_PREFIX`]); one that writes the name answers first.
fn written_global(ctx: &AnalysisContext<'_>, name: &str) -> Option<Origin> {
    if !is_identifier_name(name) {
        return None;
    }
    let (root, named) = ctx.package_reach?.global_writers_of(name).next()?;
    let prefix = if named {
        WRITE_PREFIX
    } else {
        WRITE_ANY_PREFIX
    };
    Some(Origin::Package(DeclarationAt {
        path: format!("{prefix}{name}:{root}"),
    }))
}

/// Whether the expression at `span` is a reference to the global `name`: a reference no declaration of the file binds (so a local `document` is not it), spelled `name`.
fn is_unbound_global(file: &FileFacts, span: Span, name: &str) -> bool {
    file.ast.reference_declaration(span).is_none()
        && file.source_text(span).is_some_and(|text| text == name)
}

/// Whether the member is `document.defaultView`, the global object, of the unbound global `document`.
fn is_document_default_view(file: &FileFacts, member: &solid_facts::ast::MemberFact) -> bool {
    let ast = &file.ast;
    let object = ast.peel_ts_sugar_span(member.object);
    is_unbound_global(file, object, "document")
        && match member_key_text(file, member) {
            Some(name) => name == "defaultView",
            None => false,
        }
}

/// How a member of the global object is keyed: a name the program spells, or a key it does not.
enum Key {
    Name(String),
    Unspelled,
}

/// The name a member spells: a static property name, or a string-literal key (`window["btoa"]`).
fn member_key_text(file: &FileFacts, member: &solid_facts::ast::MemberFact) -> Option<String> {
    let ast = &file.ast;
    if ast.computed_members.binary_search(&member.span).is_err() {
        return file.source_text(member.property).map(str::to_owned);
    }
    ast.literal_computed_members
        .binary_search_by_key(&member.span, |known| known.span)
        .ok()
        .map(|at| ast.literal_computed_members[at].key.to_string())
}

/// The key a member reads: its spelled name, or the one string the file's own facts prove a computed key is (`const k = "btoa"; window[k]`: the `computed_keys` fact of facts
/// schema 95 holds what a name, a conditional, a concatenation or a template can be, proven from the expression alone); a key that may be several strings, or any, is not spelled.
fn member_key(file: &FileFacts, member: &solid_facts::ast::MemberFact) -> Key {
    if let Some(name) = member_key_text(file, member) {
        return Key::Name(name);
    }
    file.ast
        .computed_keys
        .iter()
        .find(|fact| {
            fact.site == solid_facts::ast::ComputedKeySite::Member && fact.read == member.span
        })
        .filter(|fact| fact.proven && fact.texts.len() == 1)
        .map_or(Key::Unspelled, |fact| Key::Name(fact.texts[0].to_string()))
}

/// What a member of the global object is, by the key it is read with: the global of that name (the global object again for a name it goes by), or, where the key is not
/// spelled, any global ([`unspelled_origin`]).
fn global_member_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    member: &solid_facts::ast::MemberFact,
) -> Origin {
    match member_key(file, member) {
        Key::Name(name) if is_global_object_name(&name) => Origin::GlobalObject,
        Key::Name(name) => host_member_origin(
            ctx,
            analyzed,
            file,
            member.span,
            member.property,
            Some(&name),
            file.ast
                .computed_members
                .binary_search(&member.span)
                .is_ok(),
        ),
        Key::Unspelled => unspelled_origin(ctx),
    }
}

/// What a value is that is read off the global object by a key the program does not spell: the code of a package that writes any global, which a call of it runs
/// ([`READ_ANY_PREFIX`]); not followed where no package that may reach a Solid action writes one.
fn unspelled_origin(ctx: &AnalysisContext<'_>) -> Origin {
    ctx.package_reach
        .and_then(|index| index.global_writers_of_unspelled().next())
        .map_or(Origin::Unknown, |root| {
            Origin::Package(DeclarationAt {
                path: format!("{READ_ANY_PREFIX}{root}"),
            })
        })
}

/// The global object's slot a destructured binding reads: the key of each pattern level from the outermost, so `const { document: { defaultView: { btoa: enc } } } = window`
/// reads `document`, then `defaultView`, then `btoa`. The origin the walk ends on; a key that is computed, a rest element and a binding no pattern declares read any global.
fn global_slot_origin(ctx: &AnalysisContext<'_>, file: &FileFacts, declaration: Span) -> Origin {
    use solid_facts::ast::PatternSlot;
    let elements = &file.ast.pattern_elements;
    // The slots from the binding out to the pattern the initializer destructures.
    let mut chain = Vec::new();
    let mut target = declaration;
    for _ in 0..ORIGIN_DEPTH {
        let Some(element) = elements.iter().find(|element| element.target == target) else {
            break;
        };
        chain.push(&element.slot);
        target = element.pattern;
    }
    if chain.is_empty() {
        // A name the initializer is bound to whole: the global object itself.
        return Origin::GlobalObject;
    }
    let mut origin = Origin::GlobalObject;
    for slot in chain.into_iter().rev() {
        origin = match (origin, slot) {
            (Origin::GlobalObject, PatternSlot::Key(name)) => {
                if is_global_object_name(name) {
                    Origin::GlobalObject
                } else {
                    written_global(ctx, name).unwrap_or(Origin::Host)
                }
            }
            (Origin::GlobalObject, _) => unspelled_origin(ctx),
            // A member of a package's value, the host's or anything else stays what it was.
            (other, _) => other,
        };
    }
    origin
}

/// Where a member read off a global the standard library declares came from, by where the compiler says the member itself is declared (the symbol of its property
/// name: the member expression starts where its object does, and names the object's symbol; a member that is the callee of a call has the call's resolved declaration
/// when no symbol is recorded). The host's own only where the standard library alone declares the member: a member a package adds to the global's type, or an
/// overload it adds to one the library declares (`Uint8Array.from`), is declared elsewhere too, and what it runs is that package's; one only a project declaration
/// file adds is ambient (`window.__act` under `interface Window { __act(cb: () => void): void }`, `globalThis.__act` under `declare function __act`). A member the facts
/// say nothing of (an `any`, an index signature) is not proven the host's: it ends where this pass does not follow it, and the callee's declaration decides.
fn host_member_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    member: Span,
    property: Span,
    global: Option<&str>,
    computed: bool,
) -> Origin {
    let lookup = ctx.semantic_lookup;
    // A member of the global object (`global` is the name it is read by) is a global: one an installed package writes at run time is that package's code, whatever the
    // library's declaration says.
    let written = || global.and_then(|name| written_global(ctx, name));
    // The span of a computed key is the key's expression, whose symbol (a `const k`) is not the member's.
    let paths = match lookup
        .alias_root_declarations(file, property)
        .filter(|_| !computed)
    {
        Some(declarations) => declaration_paths_of(declarations),
        None => match lookup
            .resolved_callee_call(file, member)
            .and_then(|resolved| resolved.declaration.as_ref())
        {
            Some(declaration) if declaration.standard_library => {
                return written().unwrap_or(Origin::Host);
            }
            Some(declaration) => vec![declaration.location.path.to_string()],
            None => Vec::new(),
        },
    };
    if paths.iter().any(|path| path.starts_with("bundled:"))
        && let Some(origin) = written()
    {
        return origin;
    }
    if !paths.is_empty() && paths.iter().all(|path| path.starts_with("bundled:")) {
        return Origin::Host;
    }
    // The standard library declares the member too, with a package's or a project's declaration beside it (an overload, a global `@types/node` declares again): the
    // signature the call resolves to says whose it is ([`package_declaration`]).
    if paths.iter().any(|path| path.starts_with("bundled:")) {
        return Origin::Unknown;
    }
    // No fact says where the member is declared (a member read that is no call's callee): the global of that name is what a package that writes it replaced.
    if paths.is_empty()
        && let Some(origin) = written()
    {
        return origin;
    }
    declarations_origin(ctx, analyzed, &paths).unwrap_or(Origin::Unknown)
}

fn declaration_paths_of(declarations: &[typefacts::Declaration]) -> Vec<String> {
    declarations
        .iter()
        .map(|declaration| declaration.location.path.to_string())
        .collect()
}

/// The origin the declarations of a member (or of a global the standard library does not declare) give it: an installed package's declaration names that package
/// (also where the standard library declares the member too: a package's overload is the package's), a project declaration file alone makes it ambient. `None` when
/// they name neither (no declaration at all, project source, or the standard library with a project declaration file beside it: the host implements the member).
fn declarations_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    paths: &[String],
) -> Option<Origin> {
    if let Some(path) = paths
        .iter()
        .find_map(|path| installed_path(ctx, analyzed, path))
    {
        return Some(Origin::Package(DeclarationAt { path }));
    }
    if paths.iter().any(|path| path.starts_with("bundled:")) {
        return None;
    }
    paths
        .iter()
        .find(|path| is_declaration_file(path) && !ctx.semantic_lookup.in_dependency_source(path))
        .map(|path| {
            Origin::Package(DeclarationAt {
                path: format!("{AMBIENT_PREFIX}{path}"),
            })
        })
}

/// Whether the expression at `span` has nowhere further to come from: no call, member, array, operand expression, import or project binding to follow.
fn is_leaf(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    bound: &HashMap<Span, ImportBinding<'_>>,
    span: Span,
) -> bool {
    let ast = &file.ast;
    if ast.call_at(span).is_some()
        || ast.member_at(span).is_some()
        || ast.array_literal_at(span).is_some()
        || ast.conditional_at(span).is_some()
        || ast.logical_at(span).is_some()
        || ast.sequences.iter().any(|sequence| sequence.span == span)
    {
        return false;
    }
    let Some(declaration) = ast.reference_declaration(span) else {
        // A global is followed ([`global_origin`]); anything that is no name at all (a literal) has nowhere to come from.
        return ast
            .identifiers
            .binary_search_by_key(&span, |identifier| identifier.span)
            .is_err();
    };
    !bound.contains_key(&declaration)
        && value_initializer(ctx.semantic_lookup, file, span).is_none()
}

/// What a value is that is one of two (a conditional's arms, a logical expression's operands): a package's where either may be, else the host's or the runtime's only
/// where both are, else not followed.
fn combine(left: Origin, right: Origin) -> Origin {
    match (left, right) {
        (Origin::Package(a), Origin::Package(b)) => {
            // A package that is named beats a limit that is not.
            if a.path.starts_with(LIMIT_PREFIX) && !b.path.starts_with(LIMIT_PREFIX) {
                Origin::Package(b)
            } else {
                Origin::Package(a)
            }
        }
        (Origin::Package(a) | Origin::Lazy(a), _) | (_, Origin::Package(a) | Origin::Lazy(a)) => {
            Origin::Package(a)
        }
        // Either operand may be the global object, and a member read off the whole is a member read off it.
        (Origin::GlobalObject, _) | (_, Origin::GlobalObject) => Origin::GlobalObject,
        (Origin::Host, Origin::Host) => Origin::Host,
        (Origin::Runtime, Origin::Runtime) => Origin::Runtime,
        _ => Origin::Unknown,
    }
}

/// The standard-library functions whose result is, holds or wraps one of the values they are handed, by the compiler's qualified name of the declaration the call
/// resolves to: the result of `Object.assign({}, pkg)` has `pkg`'s members, `Array.from([run])[0]` is `run`, `new Proxy(pkg, {})` answers for `pkg`.
const CARRIES_ARGUMENT: &[&str] = &[
    "ObjectConstructor.assign",
    "ObjectConstructor.freeze",
    "ObjectConstructor.seal",
    "ObjectConstructor.preventExtensions",
    "ObjectConstructor.setPrototypeOf",
    "ArrayConstructor.from",
    "ArrayConstructor.of",
    "ProxyConstructor.construct",
];

/// What the call of a host or runtime function makes. A standard-library function that carries one of its arguments has that argument's package ([`CARRIES_ARGUMENT`]); a
/// host constructor makes a fresh host object; any other host or runtime function may hand back what it was given or a value a package made, which is not followed.
fn host_call_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    bound: &HashMap<Span, ImportBinding<'_>>,
    call: &solid_facts::ast::CallFact,
    host: bool,
    depth: usize,
) -> Origin {
    if let Some(declaration) = ctx
        .semantic_lookup
        .resolved_callee_call(file, call.callee)
        .and_then(|resolved| resolved.declaration.as_ref())
        && declaration.standard_library
        && CARRIES_ARGUMENT.contains(&declaration.qualified_name.as_ref())
    {
        for argument in call.arguments.iter().take(ORIGIN_WIDTH) {
            if let Origin::Package(declaration) =
                origin_of(ctx, analyzed, file, bound, argument.span, depth)
            {
                return Origin::Package(declaration);
            }
        }
        if call.arguments.len() > ORIGIN_WIDTH {
            return limit(&format!("a call with more than {ORIGIN_WIDTH} arguments"));
        }
    }
    if host
        && call.construct
        && file
            .ast
            .reference_declaration(file.ast.peel_ts_sugar_span(call.callee))
            .is_none()
    {
        Origin::Host
    } else {
        Origin::Unknown
    }
}

/// The key a `const` object-destructuring slot reads: the name of the pattern property that binds the identifier declared at `declaration` (`reRun` of
/// `const { reRun } = re` and of `const { reRun: go } = re`), `None` for a binding no pattern slot declares.
fn slot_key(file: &FileFacts, declaration: Span) -> Option<&str> {
    file.ast.pattern_elements.iter().find_map(|element| {
        if element.target != declaration {
            return None;
        }
        match &element.slot {
            solid_facts::ast::PatternSlot::Key(name) => Some(name.as_str()),
            _ => None,
        }
    })
}

/// The origin of a member read off a project module's namespace: what the module exports under the member's name (a computed key reads any export, which is not followed).
fn namespace_export_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    member: &solid_facts::ast::MemberFact,
    path: &str,
    depth: usize,
) -> Origin {
    let Some(target) = project_file(ctx, path) else {
        return Origin::Unknown;
    };
    if file
        .ast
        .computed_members
        .binary_search(&member.span)
        .is_ok()
    {
        return Origin::Unknown;
    }
    let Some(name) = file.source_text(member.property) else {
        return Origin::Unknown;
    };
    export_origin(ctx, analyzed, target, name, depth)
}

/// The origin of what a project function returns when it is called by `call`: the first package origin among its return values (a `return` argument, an arrow's
/// expression body), where a returned parameter is the argument the call passed. A function with more returns than the walk reads is a limit. Anything else is not
/// followed.
fn function_return_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    bound: &HashMap<Span, ImportBinding<'_>>,
    call: &solid_facts::ast::CallFact,
    depth: usize,
) -> Origin {
    // A function called by name, or a method of a literal or class the program resolves a member call to (`box.mk()`).
    let Some((function_file, function)) = ctx
        .semantic_lookup
        .function_called_at(file.path.as_str(), call.callee)
        .or_else(|| {
            ctx.semantic_lookup
                .object_member_function(file, call.callee)
        })
        .or_else(|| holder_member_function(ctx, file, call.callee))
    else {
        return Origin::Unknown;
    };
    walking(
        format!("fn:{}:{}", function_file.path, function.span.start),
        || {
            function_returns(
                ctx,
                analyzed,
                file,
                bound,
                call,
                function_file,
                function,
                depth,
            )
        },
    )
}

/// [`function_return_origin`] of the function a call resolved to.
#[allow(clippy::too_many_arguments)]
fn function_returns(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    bound: &HashMap<Span, ImportBinding<'_>>,
    call: &solid_facts::ast::CallFact,
    function_file: &FileFacts,
    function: &solid_facts::ast::FunctionFact,
    depth: usize,
) -> Origin {
    let table = import_bindings(ctx, function_file);
    let ast = &function_file.ast;
    // The `return` statements of this function's own body (not of a function written inside it), and an arrow's expression body.
    let nested = ast
        .functions
        .iter()
        .filter(|inner| inner.span != function.span && function.body.contains(inner.span))
        .map(|inner| inner.span)
        .collect::<Vec<_>>();
    let returned = ast
        .returns
        .iter()
        .filter(|returned| {
            function.body.contains(returned.span)
                && !nested.iter().any(|inner| inner.contains(returned.span))
        })
        .chain(function.expression_return.iter())
        .filter_map(|returned| returned.argument)
        .collect::<Vec<_>>();
    // A function that returns the global object, or one of its names, makes a call of it the global object.
    let mut held = Origin::Unknown;
    for value in returned.iter().copied().take(ORIGIN_WIDTH) {
        match origin_of(ctx, analyzed, function_file, &table, value, depth) {
            Origin::Package(declaration) => return Origin::Package(declaration),
            Origin::GlobalObject => held = Origin::GlobalObject,
            // Any other returned value is no package's code and not the global object: it adds nothing to this answer (a returned parameter is read next).
            Origin::Host
            | Origin::Runtime
            | Origin::Lazy(_)
            | Origin::Namespace(_)
            | Origin::Unknown => {}
        }
        // A returned parameter is what the call passed (`function id(f) { return f; }`, `id(makeVoid())`).
        let value = ast.peel_ts_sugar_span(value);
        let parameter = ast.reference_declaration(value).and_then(|declaration| {
            function
                .parameters
                .iter()
                .position(|parameter| parameter.names.iter().any(|name| name.span == declaration))
        });
        if let Some(index) = parameter
            && call
                .arguments
                .iter()
                .take(index + 1)
                .all(|argument| !argument.spread)
            && let Some(argument) = call.arguments.get(index)
            && let Origin::Package(declaration) =
                origin_of(ctx, analyzed, file, bound, argument.span, depth)
        {
            return Origin::Package(declaration);
        }
    }
    if returned.len() > ORIGIN_WIDTH {
        return limit(&format!("a function with more than {ORIGIN_WIDTH} returns"));
    }
    held
}

/// The project file a relative import or re-export written in `file` at `span` lands on, when the program analyzes it: the compiler's attested resolution, else the path
/// the relative specifier names.
fn landing_file<'a>(
    ctx: &AnalysisContext<'a>,
    file: &FileFacts,
    span: Span,
    module: &str,
) -> Option<&'a FileFacts> {
    let lookup = ctx.semantic_lookup;
    let path = match lookup.resolved_imports() {
        Some(imports) => {
            let solid_facts::SpecifierAttestation::Attested(row) =
                imports.specifier(file.path.as_str(), span, module)
            else {
                return None;
            };
            if row.resolved_path.is_empty() {
                return None;
            }
            if row.included_path.is_empty() {
                row.resolved_path.to_string()
            } else {
                row.included_path.to_string()
            }
        }
        None => solid_facts::resolve_relative_module_path(
            file.path.as_str(),
            module,
            lookup.files().iter().map(|file| file.path.as_str()),
        )?
        .to_owned(),
    };
    project_file(ctx, &path)
}

/// The facts of the file the program analyzes at `path`: the one place this pass reads another file's facts by path (the lookup records the read of that file, so an edit to it
/// invalidates what was followed through it; the site is the lint's one allowed read, `scripts/ir-read-lint.allowlist.json`).
fn project_file<'a>(ctx: &AnalysisContext<'a>, path: &str) -> Option<&'a FileFacts> {
    ctx.semantic_lookup.file_by_path(path)
}

/// Where the export `name` of the project module `module` came from, followed through the module's own declarations and every barrel it re-exports through
/// (`export { a as name } from "m"`, `import { a } from "m"; export { a as name }`, `export * as name from "m"`, `export * from "m"`): the initializer of a `const` it
/// exports, a package's import, JavaScript nobody reads, another module's namespace. A name no export says is not followed.
fn export_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    module: &FileFacts,
    name: &str,
    depth: usize,
) -> Origin {
    let key = (module.path.to_string(), name.to_owned());
    if let Some(known) = EXPORTS.with(|known| known.borrow().get(&key).cloned()) {
        return known;
    }
    let origin = walking(format!("export:{}:{name}", module.path), || {
        export_origin_walk(ctx, analyzed, module, name, depth)
    });
    // A limit says how far this question could go, not what the export is: it is not kept.
    if !matches!(&origin, Origin::Package(declaration) if declaration.path.starts_with(LIMIT_PREFIX))
    {
        EXPORTS.with(|known| known.borrow_mut().insert(key, origin.clone()));
    }
    origin
}

/// [`export_origin`] when the walk is not already inside this export.
fn export_origin_walk(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    module: &FileFacts,
    name: &str,
    depth: usize,
) -> Origin {
    if depth == 0 {
        return limit(&format!("a chain of more than {ORIGIN_DEPTH} re-exports"));
    }
    if !spend() {
        return limit(&format!("a question of more than {ORIGIN_STEPS} steps"));
    }
    let table = import_bindings(ctx, module);
    let mut stars = Vec::new();
    for export in module
        .ast
        .module_level_exports()
        .filter(|export| !export.type_only)
    {
        // `export const stored = makeVoid()`: declared here.
        if let Some(declaration) = export
            .declarations
            .iter()
            .find(|declaration| declaration.exported == name && !declaration.type_only)
        {
            return module_binding_origin(
                ctx,
                analyzed,
                module,
                &table,
                declaration.local.span,
                depth - 1,
            );
        }
        if let Some(specifier) = export
            .specifiers
            .iter()
            .find(|specifier| specifier.exported == name && !specifier.type_only)
        {
            let Some(local) = module.source_text(specifier.local.span) else {
                return Origin::Unknown;
            };
            return match export.module.as_deref() {
                // `export { a as name } from "m"`
                Some(from) => reexport_origin(
                    ctx,
                    analyzed,
                    module,
                    export.span,
                    from,
                    Some(local),
                    depth - 1,
                ),
                // `import { a } from "m"; export { a as name }`, `const a = …; export { a as name }`
                None => module_name_origin(ctx, analyzed, module, &table, local, depth - 1),
            };
        }
        if export.namespace.as_deref() == Some(name)
            && let Some(from) = export.module.as_deref()
        {
            // `export * as name from "m"`
            return reexport_origin(ctx, analyzed, module, export.span, from, None, depth - 1);
        }
        if export.kind == solid_facts::ast::ExportKind::All
            && export.namespace.is_none()
            && export.module.is_some()
        {
            stars.push(export);
        }
    }
    // `export * from "m"`: a name no export of the module says is one its star modules export.
    let mut found = Origin::Unknown;
    for export in stars.iter().take(ORIGIN_WIDTH) {
        let Some(from) = export.module.as_deref() else {
            continue;
        };
        match reexport_origin(
            ctx,
            analyzed,
            module,
            export.span,
            from,
            Some(name),
            depth - 1,
        ) {
            Origin::Package(declaration) => return Origin::Package(declaration),
            Origin::Unknown => {}
            other => found = other,
        }
    }
    if stars.len() > ORIGIN_WIDTH {
        return limit(&format!(
            "a module with more than {ORIGIN_WIDTH} `export *`"
        ));
    }
    found
}

/// The origin of what a `const` of the module `module` declared at `local` holds: its initializer's. A function or a class is the project's own code.
fn module_binding_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    module: &FileFacts,
    table: &HashMap<Span, ImportBinding<'_>>,
    local: Span,
    depth: usize,
) -> Origin {
    let binding = module
        .ast
        .bindings
        .iter()
        .find(|binding| binding.names.iter().any(|name| name.span == local));
    match binding {
        Some(binding) => binding
            .initializer
            .filter(|_| binding.immutable)
            .map_or(Origin::Unknown, |initializer| {
                origin_of(ctx, analyzed, module, table, initializer, depth)
            }),
        // `export default View`, `export default makeView()`: no name is declared, the exported expression is what is followed.
        None => origin_of(ctx, analyzed, module, table, local, depth),
    }
}

/// The origin of the module-level binding named `name` in `module`: an import's, or what a `const`'s initializer is (`import { a } from "m"; export { a as b }`).
fn module_name_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    module: &FileFacts,
    table: &HashMap<Span, ImportBinding<'_>>,
    name: &str,
    depth: usize,
) -> Origin {
    for import in module.ast.imports.iter().filter(|import| !import.type_only) {
        for binding in &import.bindings {
            if !binding.type_only && module.source_text(binding.local.span) == Some(name) {
                return table
                    .get(&binding.local.span)
                    .map_or(Origin::Unknown, |bound| {
                        import_origin(ctx, analyzed, module, bound, binding.local.span, depth)
                    });
            }
        }
    }
    module
        .ast
        .bindings
        .iter()
        .find(|binding| {
            binding.immutable
                && !module
                    .ast
                    .functions
                    .iter()
                    .any(|function| function.span.contains(binding.declaration))
                && binding
                    .names
                    .iter()
                    .any(|binding_name| module.source_text(binding_name.span) == Some(name))
        })
        .and_then(|binding| binding.initializer)
        .map_or(Origin::Unknown, |initializer| {
            origin_of(ctx, analyzed, module, table, initializer, depth)
        })
}

/// The origin of what a module re-exports from `from` (written in `module` at `span`): a project module's export, or its namespace when `name` is `None`; JavaScript the
/// program does not analyze behind a shim; the dialect's runtime; a package's import.
fn reexport_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    module: &FileFacts,
    span: Span,
    from: &str,
    name: Option<&str>,
    depth: usize,
) -> Origin {
    if from.starts_with('.') || from.starts_with('/') {
        if let Some(target) = landing_file(ctx, module, span, from) {
            return match name {
                Some(name) => export_origin(ctx, analyzed, target, name, depth),
                None => Origin::Namespace(target.path.to_string()),
            };
        }
        return unanalyzed_project_js(ctx, module, from)
            .map_or_else(|| Origin::Package(unresolved_hop(from)), Origin::Package);
    }
    if from.starts_with("node:") || is_runtime_module(ctx, from) {
        return Origin::Runtime;
    }
    let binding = ImportBinding {
        module: from,
        declaration: span,
        namespace: name.is_none(),
        decided_elsewhere: false,
        imported: name,
    };
    import_declaration(ctx, analyzed, module, &binding).map_or(Origin::Unknown, Origin::Package)
}

/// What the object of a member is (`box` of `box.f`) when it is a project `const` of one initializer: an object literal, or a `new` of a class the same file declares.
enum Holder<'a> {
    /// The properties written directly in the literal.
    Literal {
        file: &'a FileFacts,
        properties: Vec<&'a solid_facts::ast::ObjectPropertyFact>,
    },
    /// The elements of the class body (a field's initializer, a method's function).
    Class {
        file: &'a FileFacts,
        class: &'a solid_facts::ast::ClassFact,
    },
}

/// The holder of the members of the value `object` names, when it is a project `const` bound to an object literal or to `new Class()`.
fn holder_of<'a>(ctx: &AnalysisContext<'a>, file: &FileFacts, object: Span) -> Option<Holder<'a>> {
    let object = file.ast.peel_ts_sugar_span(object);
    file.ast.reference_declaration(object)?;
    let (binding_file, binding, _) = ctx
        .semantic_lookup
        .binding_at_reference(file.path.as_str(), object)?;
    if !binding.immutable {
        return None;
    }
    let ast = &binding_file.ast;
    let initializer = ast.peel_ts_sugar_span(binding.initializer?);
    if let Some(construction) = ast.call_at(initializer) {
        if !construction.construct {
            return None;
        }
        let callee = ast.peel_ts_sugar_span(construction.callee);
        let declaration = ast.reference_declaration(callee)?;
        let class = ast.classes.iter().find(|class| {
            class
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
        })?;
        return Some(Holder::Class {
            file: binding_file,
            class,
        });
    }
    if binding.initializer_value_kind != solid_facts::ast::RuntimeValueKind::Object {
        return None;
    }
    let written = ast
        .object_properties
        .iter()
        .filter(|property| initializer.contains(property.span))
        .collect::<Vec<_>>();
    let properties = written
        .iter()
        .copied()
        .filter(|property| {
            !written
                .iter()
                .any(|outer| outer.span != property.span && outer.value.contains(property.span))
        })
        .collect::<Vec<_>>();
    Some(Holder::Literal {
        file: binding_file,
        properties,
    })
}

/// What a holder keeps under `name`: the value a data property or a field is initialized with, or the function of a method.
enum Kept {
    Value(Span),
    Function(Span),
}

impl<'a> Holder<'a> {
    fn file(&self) -> &'a FileFacts {
        match self {
            Holder::Literal { file, .. } | Holder::Class { file, .. } => file,
        }
    }

    /// What the holder keeps under the static name `name`. `None` where the name is written by no property or element, or twice (an accessor, a spread: not followed).
    fn keeps(&self, name: &str) -> Option<Kept> {
        let file = self.file();
        match self {
            Holder::Literal { properties, .. } => {
                let mut named = properties.iter().filter(|property| {
                    file.source_text(property.key) == Some(name) || property.computed
                });
                let property = named.next()?;
                if named.next().is_some() || property.computed {
                    return None;
                }
                if property.method {
                    Some(Kept::Function(property.value))
                } else if property.data {
                    Some(Kept::Value(property.value))
                } else {
                    None
                }
            }
            Holder::Class { class, .. } => {
                let mut named = class.elements.iter().filter(|element| {
                    !element.r#static
                        && (element.computed
                            || element.key.and_then(|key| file.source_text(key)) == Some(name))
                });
                let element = named.next()?;
                if named.next().is_some() || element.computed {
                    return None;
                }
                match element.kind {
                    solid_facts::ast::ClassElementKind::Field => element.value.map(Kept::Value),
                    solid_facts::ast::ClassElementKind::Method => element.value.map(Kept::Function),
                    _ => None,
                }
            }
        }
    }
}

/// The origin of a member of a project object literal or class instance: what a data property or a field holds. A method is the project's own function, whose result
/// ([`function_return_origin`]) is what is followed.
fn holder_member_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    member: &solid_facts::ast::MemberFact,
    depth: usize,
) -> Origin {
    if file
        .ast
        .computed_members
        .binary_search(&member.span)
        .is_ok()
    {
        return Origin::Unknown;
    }
    let Some(name) = file.source_text(member.property) else {
        return Origin::Unknown;
    };
    let Some(holder) = holder_of(ctx, file, member.object) else {
        return Origin::Unknown;
    };
    match holder.keeps(name) {
        Some(Kept::Value(value)) => {
            let holder_file = holder.file();
            let table = import_bindings(ctx, holder_file);
            origin_of(ctx, analyzed, holder_file, &table, value, depth)
        }
        Some(Kept::Function(_)) | None => Origin::Unknown,
    }
}

/// The function a member callee names when its object is a project object literal or class instance ([`holder_of`]): the method written there.
fn holder_member_function<'a>(
    ctx: &AnalysisContext<'a>,
    file: &FileFacts,
    callee: Span,
) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)> {
    let callee = file.ast.peel_ts_sugar_span(callee);
    let member = file.ast.member_at(callee)?;
    if file
        .ast
        .computed_members
        .binary_search(&member.span)
        .is_ok()
    {
        return None;
    }
    let name = file.source_text(member.property)?;
    let holder = holder_of(ctx, file, member.object)?;
    match holder.keeps(name)? {
        Kept::Function(function) => {
            let holder_file = holder.file();
            Some((holder_file, holder_file.ast.function_at(function)?))
        }
        Kept::Value(_) => None,
    }
}

/// The origin of a member read off a namespace import of the project's own module (`import * as re from "./re"; re.reRun`): the member's own symbol is the
/// re-export, which the compiler's resolution ends on the package's declaration.
fn namespace_member_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    bound: &HashMap<Span, ImportBinding<'_>>,
    member: &solid_facts::ast::MemberFact,
) -> Origin {
    let ast = &file.ast;
    let object = ast.peel_ts_sugar_span(member.object);
    let namespace_of_project_module = ast
        .reference_declaration(object)
        .and_then(|declaration| bound.get(&declaration))
        .is_some_and(|import| import.namespace && import.decided_elsewhere);
    if !namespace_of_project_module {
        return Origin::Unknown;
    }
    ctx.semantic_lookup
        .alias_root_declarations(file, member.property)
        .and_then(|declarations| {
            declarations
                .iter()
                .find_map(|declaration| installed_path(ctx, analyzed, &declaration.location.path))
        })
        .map_or(Origin::Unknown, |path| {
            Origin::Package(DeclarationAt { path })
        })
}

/// A relative module a hop of the origin walk could not land on (the compiler did not attest the specifier, or the program does not analyze the file): what it exports is not
/// decided, so a value that comes through it is a site (`computed-load:`, undecided `dynamic-load`), never silence.
fn unresolved_hop(module: &str) -> DeclarationAt {
    DeclarationAt {
        path: format!("{COMPUTED_LOAD_PREFIX}{module}"),
    }
}

/// The JavaScript a relative specifier names that the program does not analyze (a declaration file or an ambient wildcard module stands for it): the bundler loads it, so
/// its code runs where the importing module's value does.
fn unanalyzed_project_js(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    module: &str,
) -> Option<DeclarationAt> {
    let index = ctx.package_reach?;
    let base = solid_facts::package_reach::join_relative(file.path.as_str(), module);
    solid_facts::package_reach::script_candidates(&base)
        .into_iter()
        .find(|candidate| index.is_project_js(candidate))
        .map(|path| DeclarationAt { path })
}

/// Where the value an import binding names came from. `symbol_at` is where the binding's own symbol is asked (a shorthand property's span holds the property's).
fn import_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    import: &ImportBinding<'_>,
    symbol_at: Span,
    depth: usize,
) -> Origin {
    let lookup = ctx.semantic_lookup;
    if import.module.starts_with("node:") || is_runtime_module(ctx, import.module) {
        return Origin::Runtime;
    }
    if !import.decided_elsewhere {
        return import_declaration(ctx, analyzed, file, import)
            .map_or(Origin::Unknown, Origin::Package);
    }
    // JavaScript the import names that the program does not analyze (a declaration file or an ambient `declare module` stands for it).
    if let Some(declaration) = unanalyzed_project_js(ctx, file, import.module) {
        return Origin::Package(declaration);
    }
    // The project's own module: what it exports of one plain name bound to one initializer is what that makes; a re-export of a package's function is the
    // declaration the compiler's resolution ends on.
    if depth > 1
        && let Some((next, initializer)) = value_initializer(lookup, file, symbol_at)
    {
        let next_bound = import_bindings(ctx, next);
        let origin = origin_of(ctx, analyzed, next, &next_bound, initializer, depth - 1);
        if !matches!(origin, Origin::Unknown) {
            return origin;
        }
    }
    if let Some(path) = lookup
        .alias_root_declarations(file, symbol_at)
        .and_then(|declarations| {
            declarations
                .iter()
                .find_map(|declaration| installed_path(ctx, analyzed, &declaration.location.path))
        })
    {
        return Origin::Package(DeclarationAt { path });
    }
    // The module graph of the project: the export the import names, through every barrel (`export { a } from "m"`, `export * as ns from "m"`), or the module's namespace.
    let Some(target) = landing_file(ctx, file, import.declaration, import.module) else {
        return Origin::Unknown;
    };
    match import.imported {
        None => Origin::Namespace(target.path.to_string()),
        Some(name) => export_origin(ctx, analyzed, target, name, depth.saturating_sub(1)),
    }
}

/// Whether the expression at `span` roots in something [`origin_of`] can follow (an import, a project binding of one initializer, a call, a global), so that the
/// value loops ask it only there: a member chain is walked to its root.
fn followable_root(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    bound: &HashMap<Span, ImportBinding<'_>>,
    span: Span,
) -> bool {
    let ast = &file.ast;
    let mut at = ast.peel_ts_sugar_span(span);
    for _ in 0..ORIGIN_DEPTH {
        if let Some(member) = ast.member_at(at) {
            at = ast.peel_ts_sugar_span(member.object);
            continue;
        }
        if ast.call_at(at).is_some() {
            return true;
        }
        return match ast.reference_declaration(at) {
            Some(declaration) => {
                bound.contains_key(&declaration)
                    || value_initializer(ctx.semantic_lookup, file, at).is_some()
            }
            // A global (a name no binding of the file declares): [`global_origin`] says whose it is.
            None => ast
                .identifiers
                .binary_search_by_key(&at, |identifier| identifier.span)
                .is_ok(),
        };
    }
    false
}

/// Where a value handed on is reported: the call it is a direct argument of (a throw while that call runs has its project frame there), else the reference.
fn handed_at(file: &FileFacts, span: Span) -> Span {
    let ast = &file.ast;
    ast.arguments_containing(span)
        .find(|(call, index)| ast.peel_ts_sugar_span(call.arguments[*index].span) == span)
        .map_or(span, |(call, _)| call.span)
}

/// The initializer the value at `span` comes from: the single initializer of a project binding of one plain name ([`SemanticLookup::exact_initializer_at_reference`]),
/// or the initializer of a destructured `const` (`const { run } = kernel`): each name of the pattern is read off what the initializer makes.
fn value_initializer<'a>(
    lookup: &crate::indexes::SemanticLookup<'a>,
    file: &FileFacts,
    span: Span,
) -> Option<(&'a FileFacts, Span)> {
    if let Some(found) = lookup.exact_initializer_at_reference(file, span) {
        return Some(found);
    }
    let (binding_file, binding, _) = lookup.binding_at_reference(file.path.as_str(), span)?;
    binding
        .initializer
        .filter(|_| binding.immutable)
        .map(|initializer| (binding_file, initializer))
}

/// The prefix of the path of an import the compiler's attestation does not cover: no file, so the reach index has no verdict for it.
const UNATTESTED: &str = "unattested:";

/// The declaration of the code `callee` enters, when it is a package's the build does not read: an installed file or a declaration file,
/// outside the standard library, the analyzed packages and the dialect's runtime. `None` for project code, the standard library, a callee
/// nothing resolves (the handed-callee obligations' concern) and Solid's own runtime.
fn package_declaration(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    bound: &HashMap<Span, ImportBinding<'_>>,
    callee: Span,
    resolve: Resolve,
) -> Option<DeclarationAt> {
    let lookup = ctx.semantic_lookup;
    let runtime = ctx.dialect.primitive_defining_packages();
    // `action(step)()`, `createMemo(f)()`: the inner call is the dialect's, whose catalog answers for what it returns.
    if let Some(inner) = file.ast.call_at(file.ast.peel_ts_sugar_span(callee))
        && lookup.primitive_at_call(file, inner.span).is_some()
    {
        return None;
    }
    // The code a callee runs comes from where its value came from, not from where its signature type is declared: the import its root names, through the
    // bindings that only hold a value a call made (`const handle = makeHandle()`). A global of the standard library is the host's, whatever a package augments.
    let resolved_declaration = || {
        lookup
            .resolved_callee_call(file, callee)
            .and_then(|resolved| resolved.declaration.as_ref())
    };
    match origin_of(ctx, analyzed, file, bound, callee, ORIGIN_DEPTH) {
        Origin::Package(declaration) | Origin::Lazy(declaration) => return Some(declaration),
        // The host's own function, unless the signature the compiler chose for this call is a package's: a package overloads a global (`setTimeout(cb, "fz")`,
        // `new Float32Array("fz", cb)`) and replaces it where it loads.
        Origin::Host => {
            if !resolved_declaration().is_some_and(|declaration| !declaration.standard_library) {
                return None;
            }
        }
        Origin::Runtime => return None,
        Origin::Namespace(_) | Origin::GlobalObject | Origin::Unknown => {}
    }
    let mut paths = Vec::<String>::new();
    if resolve == Resolve::Call
        && let Some(declaration) = resolved_declaration()
    {
        if !declaration.origin_module.is_empty()
            && is_runtime_module(ctx, &declaration.origin_module)
        {
            return None;
        }
        // The standard library's signature types a value anything made (`VoidFunction`): where the code comes from is where the member is declared, below.
        if !declaration.standard_library {
            paths.push(declaration.location.path.to_string());
        }
    }
    // The value's own declaration: the symbol the compiler gives it, and the file of the import its root names. A tag follows the project bindings
    // that only name another value (`const View = Pkg.Comp`) to that value's.
    let mut unattested = None;
    let mut hop = (file, file.ast.peel_ts_sugar_span(callee));
    for _ in 0..=if resolve == Resolve::Tag {
        ALIAS_DEPTH
    } else {
        0
    } {
        let (hop_file, span) = hop;
        let hop_imports;
        let hop_table = if std::ptr::eq(hop_file, file) {
            bound
        } else {
            hop_imports = import_bindings(ctx, hop_file);
            &hop_imports
        };
        let import = root_import(hop_file, hop_table, span);
        if import.is_some_and(|import| import.module.starts_with("node:")) {
            return None;
        }
        if import.is_some_and(|import| is_runtime_module(ctx, import.module)) {
            return None;
        }
        if let Some(declarations) = lookup.alias_root_declarations(hop_file, span) {
            paths.extend(
                declarations
                    .iter()
                    .map(|declaration| declaration.location.path.to_string()),
            );
        }
        // A member callee's own declaration: the property a package declares with a standard-library function type (`interface Window { fzHook: VoidFunction }`).
        if let Some(member) = hop_file.ast.member_at(span)
            && let Some(declarations) = lookup.alias_root_declarations(hop_file, member.property)
        {
            paths.extend(
                declarations
                    .iter()
                    .map(|declaration| declaration.location.path.to_string()),
            );
        }
        if let Some(import) = import.filter(|import| !import.decided_elsewhere)
            && let Some(declaration) = import_declaration(ctx, analyzed, hop_file, import)
        {
            if declaration.path.starts_with(UNATTESTED) {
                unattested.get_or_insert(declaration.path);
            } else {
                paths.push(declaration.path);
            }
        }
        if resolve != Resolve::Tag {
            break;
        }
        let Some((next_file, initializer)) = lookup.exact_initializer_at_reference(hop_file, span)
        else {
            break;
        };
        hop = (next_file, next_file.ast.peel_ts_sugar_span(initializer));
    }
    // The call resolves to the standard library's signature and the standard library declares the callee too: the host's, whatever a package adds beside it.
    if resolved_declaration().is_some_and(|declaration| declaration.standard_library)
        && paths.iter().any(|path| path.starts_with("bundled:"))
    {
        return None;
    }
    // Installed code only (`installed_path`): not a file the program analyzes itself, and no declaration a script provides (a global, a `declare module`
    // shim: the import's file stands for the shim's package).
    let installed = paths
        .into_iter()
        .find_map(|path| installed_path(ctx, analyzed, &path));
    let path = match installed {
        Some(path) => path,
        // An import the compiler's attestation does not cover (its answer is unknown, not "unresolved"): undecided, so a site.
        None => unattested?,
    };
    if let Some(root) = package_root_of(&path)
        && runtime.contains(&installed_name(ctx, root).as_str())
    {
        return None;
    }
    Some(DeclarationAt { path })
}

/// The import binding a value's root names: the identifier `span` is, or the object of the member chain it is (`Pkg` of `Pkg.a.b`).
fn root_import<'t, 'f>(
    file: &FileFacts,
    bound: &'t HashMap<Span, ImportBinding<'f>>,
    span: Span,
) -> Option<&'t ImportBinding<'f>> {
    let ast = &file.ast;
    let mut at = ast.peel_ts_sugar_span(span);
    let mut hops = 0;
    while let Some(member) = ast.member_at(at) {
        hops += 1;
        if hops > 32 {
            return None;
        }
        at = ast.peel_ts_sugar_span(member.object);
    }
    bound.get(&ast.reference_declaration(at)?)
}

/// The package declaration an import loads, as the compiler attested it: `None` for the project's own modules, the dialect's runtime and an import
/// the compiler could not resolve (TypeScript reports it, and the boundary's SC9005 stands for the missing source). An import the attestation does
/// not cover is `unattested:<module>`, for which the reach index has no verdict, so it is undecided. A linked workspace package is named by the
/// `node_modules` spelling of its path, which the backend walks through the link.
fn import_declaration(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    import: &ImportBinding<'_>,
) -> Option<DeclarationAt> {
    if import.decided_elsewhere {
        return None;
    }
    let lookup = ctx.semantic_lookup;
    let path = match lookup
        .resolved_imports()
        .map(|attested| attested.specifier(file.path.as_str(), import.declaration, import.module))
    {
        Some(solid_facts::SpecifierAttestation::Attested(row)) if row.resolved_path.is_empty() => {
            return None;
        }
        Some(solid_facts::SpecifierAttestation::Attested(row)) => {
            // A linked workspace package resolves to its real path, which the program analyzes when it is source: that code is read, not assumed.
            if !is_declaration_file(&row.resolved_path) && analyzed.contains(&*row.resolved_path) {
                return None;
            }
            if row.symlink_path.is_empty() {
                row.resolved_path.to_string()
            } else {
                row.symlink_path.to_string()
            }
        }
        _ => format!("{UNATTESTED}{}", import.module),
    };
    if path.starts_with(UNATTESTED) {
        return Some(DeclarationAt { path });
    }
    installed_path(ctx, analyzed, &path).map(|path| DeclarationAt { path })
}

/// What the package at `root` is called: the `name` its manifest gives (the backend read it), else the directory's own name.
fn installed_name(ctx: &AnalysisContext<'_>, root: &str) -> String {
    ctx.package_reach
        .and_then(|index| index.name_of(root))
        .map_or_else(|| package_label(root), str::to_owned)
}

/// The package name of a module specifier (`@solidjs/web/storage` is `@solidjs/web`).
fn package_name(module: &str) -> &str {
    let mut parts = module.splitn(3, '/');
    let first = parts.next().unwrap_or_default();
    if first.starts_with('@') {
        let second = parts.next().unwrap_or_default();
        &module[..(first.len() + 1 + second.len()).min(module.len())]
    } else {
        first
    }
}

/// The rows and silences of the sites ([`file_sites`]) of every analyzed file: an uncertifiable SC2002 row where the call may run under an
/// owner or nothing places it, a typed silence where its role allows an action or it cannot run.
pub(crate) fn package_action_calls(
    ctx: &AnalysisContext<'_>,
    rows: &mut Vec<crate::StaticViolation>,
    ends: &mut SiteEnds,
) {
    let lookup = ctx.semantic_lookup;
    for (file, file_sites) in lookup.files().iter().zip(sites(ctx)) {
        if file_sites.is_empty() {
            continue;
        }
        let allowed = crate::execution_role::allowed_callback_spans(file, lookup);
        for site in file_sites {
            if matches!(site.entry, Entry::Call | Entry::Tag | Entry::Read) {
                // A call edge reaches a call; a member read is no call, so it is placed wherever the function that holds it may run.
                let reached = match site.entry {
                    Entry::Read => {
                        crate::local_access::unreached_write_may_run(file, site.at, lookup)
                    }
                    _ => ctx
                        .reachable_calls
                        .contains_key(&crate::location(file.path.shared(), site.callee)),
                };
                if !reached && !crate::local_access::unreached_write_may_run(file, site.at, lookup)
                {
                    ends.end_with(file, site.at, SiteSilence::PackageCallCannotRun, || {
                        vec![Premise::at(PremiseKind::SiteReach, Some(site.at))]
                    });
                    continue;
                }
                let role = if reached {
                    let role = crate::execution_role::semantic_write_execution_role(
                        file,
                        site.callee,
                        &allowed,
                        ctx.entities,
                        ctx.symbol_names,
                        lookup,
                        RootBodyGuard::Rejects,
                    );
                    crate::local_access::handed_write_execution(
                        file,
                        site.callee,
                        &allowed,
                        ctx.entities,
                        ctx.symbol_names,
                        lookup,
                        RootBodyGuard::Rejects,
                        role,
                    )
                } else {
                    crate::ExecutionRole::Unknown
                };
                if !(role.reports_disallowed_write() || role.reports_unplaced()) {
                    ends.end_at(
                        file,
                        site.at,
                        SiteSilence::PackageCallRoleAllows,
                        PremiseKind::RoleDoesNotReport,
                        || Some(site.callee),
                    );
                    continue;
                }
            }
            rows.push(row(file, site));
        }
    }
}

fn row(file: &FileFacts, site: &PackageSite) -> crate::StaticViolation {
    let entered = match site.entry {
        Entry::Call => "this call runs code of",
        Entry::Tag => "this tagged template runs code of",
        Entry::Render => "this element renders a component of",
        // The global object handed on: the callee may read any global off it.
        Entry::Value if site.provider == Provider::Unspelled => {
            "this hands the global object on, where any global may be read, to code of"
        }
        Entry::Value => "this hands on a function of",
        // A global read runs the accessor a package defined for it.
        Entry::Read
            if matches!(
                site.provider,
                Provider::Written | Provider::WrittenAny | Provider::Unspelled
            ) =>
        {
            "this read runs an accessor of"
        }
        Entry::Read => "this reads a member of an object of",
    };
    crate::StaticViolation {
        id: "SC2002".into(),
        rule: "action-called-in-owned-scope".into(),
        message: format!(
            "{entered} `{}`, {}, where an owner may be active; {}, so whether it calls a Solid \
             action (which throws `ACTION_CALLED_IN_OWNED_SCOPE` in the dev build) is not proven",
            site.package,
            match site.provider {
                Provider::Ambient => "a declaration file whose global the host, a script or a package this build does not read as source may provide",
                Provider::Limit => "a value whose origin this build's walk could not follow to its end",
                Provider::Written => "a global the standard library declares, which an installed package this build does not read as source writes at run time",
                Provider::WrittenAny => "a global the standard library declares, which an installed package this build does not read as source may write at run time (it writes globals under a key the scan does not decide)",
                Provider::Unspelled => "a member of the global object that this build cannot name (the key is computed), which an installed package this build does not read as source may have written at run time",
                Provider::Package => "a package this build does not read as source",
            },
            site.reach.describe()
        ),
        hint: "Call the package from an event handler, an effect's apply function or onSettled; or let the build read the package as \
               source (a declared host with the Solid Vite plugin reads the packages that name Solid)."
            .into(),
        location: crate::location(file.path.shared(), site.at),
        analysis_context: format!("{}:{}", crate::blockers::PACKAGE_MAY_CALL_ACTION, site.reach.word()),
        fixes: Vec::new(),
        uncertain: true,
    }
}

/// The sites of the proof universe: every site outside the analyzed packages, as an action site of [`crate::proof_universe`].
pub(crate) fn universe_sites<'a>(
    ctx: &AnalysisContext<'a>,
) -> Vec<(&'a FileFacts, Span, Span, &'static str)> {
    let lookup = ctx.semantic_lookup;
    let mut found = Vec::new();
    for (file, file_sites) in lookup.files().iter().zip(sites(ctx)) {
        if lookup.in_dependency_source(file.path.as_str()) {
            continue;
        }
        for site in file_sites {
            found.push((file, site.at, site.callee, site.entry.via()));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::{is_identifier_name, package_name};

    #[test]
    fn a_module_names_its_package() {
        assert_eq!(package_name("solid-js"), "solid-js");
        assert_eq!(package_name("solid-js/store"), "solid-js");
        assert_eq!(package_name("@solidjs/web/storage"), "@solidjs/web");
        assert_eq!(package_name("@solidjs/web"), "@solidjs/web");
        assert_eq!(package_name("fz-step"), "fz-step");
    }

    /// ADR 0974: only an identifier can name a global; the text of any other expression the origin walk ends on is none (an any-name writer matches no function literal).
    #[test]
    fn only_an_identifier_names_a_global() {
        for name in ["btoa", "JSON", "_x", "$", "XRWebGLBinding", "a1"] {
            assert!(is_identifier_name(name), "{name}");
        }
        for name in [
            "",
            "() => {}",
            "1a",
            "a.b",
            "a b",
            "async () => 1",
            "{}",
            "x[0]",
        ] {
            assert!(!is_identifier_name(name), "{name}");
        }
    }
}
