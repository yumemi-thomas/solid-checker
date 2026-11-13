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
use solid_facts::package_reach::{
    AMBIENT_PREFIX, AccessorFacts, COMPUTED_LOAD_PREFIX, PackageReach, ReachBlocker,
    is_declaration_file, package_label, package_root_of,
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
    /// (ADR 0973): the getter runs where the read stands, with that role.
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
    /// The callee is a global a project declaration file declares ([`AMBIENT_PREFIX`]), not a package's export: its provider is not said.
    pub(crate) ambient: bool,
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
        // another's.
        if !seen.insert((at, entry)) {
            return;
        }
        let package = package_root_of(&declaration.path)
            .map(|root| installed_name(ctx, root))
            .unwrap_or_else(|| {
                declaration
                    .path
                    .strip_prefix(AMBIENT_PREFIX)
                    .unwrap_or(&declaration.path)
                    .to_owned()
            });
        sites.push(PackageSite {
            at,
            callee,
            entry,
            reach,
            package,
            ambient: declaration.path.starts_with(AMBIENT_PREFIX),
        });
    };
    for call in &file.ast.calls {
        // A dialect primitive is the catalog's; a reviewed test runner's registration (`describe`, `it`) runs the callback on its own stack and calls no action;
        // a reviewed reading of a package's API (ADR 0974) shows what it calls for the value it is handed.
        if lookup.primitive_at_call(file, call.span).is_some()
            || lookup.reviewed_test_runner_registration(file, call)
            || crate::reviewed_api::clears_call(ctx, file, call)
        {
            continue;
        }
        push(
            call.span,
            call.callee,
            Entry::Call,
            package_declaration(ctx, analyzed, file, &bound, call.callee, Resolve::Call),
        );
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
        let (Origin::Package(origin) | Origin::Lazy(origin)) =
            origin_of(ctx, analyzed, file, &bound, identifier.span, ORIGIN_DEPTH)
        else {
            continue;
        };
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
        if function && handed_as_value(ctx, file, identifier.span) {
            let at = handed_at(file, identifier.span);
            // A reviewed reading shows the assertion the value is handed to reads it as a primitive (ADR 0974).
            if crate::reviewed_api::clears_value_at(ctx, file, at) {
                continue;
            }
            push(at, identifier.span, Entry::Value, Some(origin));
        }
    }
    for member in &file.ast.members {
        // `[1].forEach(kernel.run)`: the object is an import of any kind, a binding that holds what an import makes, or a longer chain (`pkg.helpers.run`).
        if !followable_root(ctx, file, &bound, member.span) {
            continue;
        }
        let (Origin::Package(origin) | Origin::Lazy(origin)) =
            origin_of(ctx, analyzed, file, &bound, member.span, ORIGIN_DEPTH)
        else {
            continue;
        };
        // A member of a namespace import is a function the facts may say nothing of; a member of any other object is one where the compiler says it is callable
        // (a call's argument is typed: `[..].forEach(kernel.run)`), never by the kind of a property signature, which a function and a number share.
        let function =
            if root_import(file, &bound, member.span).is_some_and(|import| import.namespace) {
                may_be_function(ctx, file, member.span, member.span)
            } else {
                callable_fact(ctx, file, member.span) == Some(true)
            };
        if function && handed_as_value(ctx, file, member.span) {
            let at = handed_at(file, member.span);
            if !crate::reviewed_api::clears_value_at(ctx, file, at) {
                push(at, member.span, Entry::Value, Some(origin));
            }
            continue;
        }
        // The member is evaluated for its value where it stands (`void clock.now`): a getter or a setter the package's closure defines under a name of the chain
        // (or under a name nothing fixes, or in code nobody read) runs there. A global the project declares is not followed: its provider is not a package's closure.
        if !origin.path.starts_with(AMBIENT_PREFIX)
            && evaluated_member(file, member.span)
            && accessor_facts(ctx, &origin.path).may_run(&chain_names(file, member.span))
        {
            push(member.span, member.span, Entry::Read, Some(origin));
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
    let ast = &file.ast;
    if ast.calls_with_peeled_callee(span).next().is_some()
        || ast
            .tagged_templates
            .iter()
            .any(|template| ast.peel_ts_sugar_span(template.tag) == span)
        || ast.members_with_peeled_object(span).next().is_some()
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

struct DeclarationAt {
    path: String,
}

/// How many hops the origin of a value is followed (a call's callee, a member's object, a binding's initializer).
const ORIGIN_DEPTH: usize = 6;

/// Where the value an expression names came from.
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
/// function's result is what it returns; a host or runtime function's result is not followed, except a host constructor's fresh object), a member is read off
/// its object, an import names its package, an array literal holds its elements, and a project binding of one plain name to one initializer holds what that
/// makes. A global the standard library declares is the host's, whatever a package adds to its type.
fn origin_of(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    bound: &HashMap<Span, ImportBinding<'_>>,
    span: Span,
    depth: usize,
) -> Origin {
    if depth == 0 {
        return Origin::Unknown;
    }
    let lookup = ctx.semantic_lookup;
    let ast = &file.ast;
    let span = ast.peel_ts_sugar_span(span);
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
            // A host constructor makes a fresh host object; any other host or runtime function may hand back what it was given or a value a package made.
            Origin::Host
                if call.construct
                    && ast
                        .reference_declaration(ast.peel_ts_sugar_span(call.callee))
                        .is_none() =>
            {
                Origin::Host
            }
            Origin::Host | Origin::Runtime => Origin::Unknown,
            Origin::Unknown => function_return_origin(ctx, analyzed, file, call.callee, depth - 1),
        };
    }
    if let Some(member) = ast.member_at(span) {
        return match origin_of(ctx, analyzed, file, bound, member.object, depth - 1) {
            Origin::Package(declaration) => Origin::Package(declaration),
            // `Lazy.preload` is the dialect's wrapper, not the package's component.
            Origin::Lazy(_) | Origin::Runtime => Origin::Runtime,
            // A member of a global the standard library declares is the host's where the library alone declares the member, and code something else provides
            // where a package or a declaration file adds it (`window.__act`).
            Origin::Host => host_member_origin(ctx, analyzed, file, member.span, member.property),
            Origin::Unknown => namespace_member_origin(ctx, analyzed, file, bound, member),
        };
    }
    if let Some(array) = ast.array_literal_at(span) {
        for element in array.elements.iter().take(16) {
            if element.kind == solid_facts::ast::ArrayElementKind::Hole {
                continue;
            }
            if let Origin::Package(declaration) =
                origin_of(ctx, analyzed, file, bound, element.span, depth - 1)
            {
                return Origin::Package(declaration);
            }
        }
        return Origin::Unknown;
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
    if std::ptr::eq(next, file) {
        origin_of(ctx, analyzed, file, bound, initializer, depth - 1)
    } else {
        let next_bound = import_bindings(ctx, next);
        origin_of(ctx, analyzed, next, &next_bound, initializer, depth - 1)
    }
}

/// Where the component a `lazy(loader)` call makes comes from (ADR 0973): the modules its loader function literal imports dynamically (`() => import("pkg")`,
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
            // The project's own module, unless JavaScript the program does not analyze stands behind it (a shim); a rooted specifier names a path only the bundler resolves.
            Some(module) if module.starts_with('.') => unanalyzed_project_js(ctx, file, module),
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

/// Where a global (a name no binding of the file declares) came from, by where the compiler says it is declared: the standard library is the host's, whatever a
/// package also declares of it; an installed package's declaration is decided by that package; a project declaration file that no module of the program implements
/// (`declare function gtag(…)`) declares a global the host, a script or an installed package may provide (ADR 0973: a package that writes the global is code the
/// program loads, and Chrome ran it).
fn global_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    span: Span,
) -> Origin {
    // A name the compiler resolves to nothing at all is unknown; one it resolves to a symbol no file declares is the language's own (`globalThis`, `undefined`):
    // the host's.
    let Some(declarations) = ctx.semantic_lookup.alias_root_declarations(file, span) else {
        return Origin::Unknown;
    };
    let paths = declaration_paths_of(declarations);
    if paths.is_empty() || paths.iter().any(|path| path.starts_with("bundled:")) {
        return Origin::Host;
    }
    declarations_origin(ctx, analyzed, &paths).unwrap_or(Origin::Unknown)
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
) -> Origin {
    let lookup = ctx.semantic_lookup;
    let paths = match lookup.alias_root_declarations(file, property) {
        Some(declarations) => declaration_paths_of(declarations),
        None => match lookup
            .resolved_callee_call(file, member)
            .and_then(|resolved| resolved.declaration.as_ref())
        {
            Some(declaration) if declaration.standard_library => return Origin::Host,
            Some(declaration) => vec![declaration.location.path.to_string()],
            None => Vec::new(),
        },
    };
    if !paths.is_empty() && paths.iter().all(|path| path.starts_with("bundled:")) {
        return Origin::Host;
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

/// The origin of what a project function returns when it is called at `callee`: the first package origin among its return values (a `return` argument, an
/// arrow's expression body). Anything else is not followed.
fn function_return_origin(
    ctx: &AnalysisContext<'_>,
    analyzed: &HashSet<&str>,
    file: &FileFacts,
    callee: Span,
    depth: usize,
) -> Origin {
    let Some((function_file, function)) = ctx
        .semantic_lookup
        .function_called_at(file.path.as_str(), callee)
    else {
        return Origin::Unknown;
    };
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
        .take(8);
    for value in returned {
        if let Origin::Package(declaration) =
            origin_of(ctx, analyzed, function_file, &table, value, depth)
        {
            return Origin::Package(declaration);
        }
    }
    Origin::Unknown
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
        return origin_of(ctx, analyzed, next, &next_bound, initializer, depth - 1);
    }
    lookup
        .alias_root_declarations(file, symbol_at)
        .and_then(|declarations| {
            declarations
                .iter()
                .find_map(|declaration| installed_path(ctx, analyzed, &declaration.location.path))
        })
        .map_or(Origin::Unknown, |path| {
            Origin::Package(DeclarationAt { path })
        })
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
    match origin_of(ctx, analyzed, file, bound, callee, ORIGIN_DEPTH) {
        Origin::Package(declaration) | Origin::Lazy(declaration) => return Some(declaration),
        Origin::Host | Origin::Runtime => return None,
        Origin::Unknown => {}
    }
    let mut paths = Vec::<String>::new();
    if resolve == Resolve::Call
        && let Some(declaration) = lookup
            .resolved_callee_call(file, callee)
            .and_then(|resolved| resolved.declaration.as_ref())
    {
        if declaration.standard_library {
            return None;
        }
        if !declaration.origin_module.is_empty()
            && is_runtime_module(ctx, &declaration.origin_module)
        {
            return None;
        }
        paths.push(declaration.location.path.to_string());
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
        Entry::Value => "this hands on a function of",
        Entry::Read => "this reads a member of an object of",
    };
    crate::StaticViolation {
        id: "SC2002".into(),
        rule: "action-called-in-owned-scope".into(),
        message: format!(
            "{entered} `{}`, {}, where an owner may be active; {}, so whether it calls a Solid \
             action (which throws `ACTION_CALLED_IN_OWNED_SCOPE` in the dev build) is not proven",
            site.package,
            if site.ambient {
                "a declaration file whose global the host, a script or a package this build does not read as source may provide"
            } else {
                "a package this build does not read as source"
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
    use super::package_name;

    #[test]
    fn a_module_names_its_package() {
        assert_eq!(package_name("solid-js"), "solid-js");
        assert_eq!(package_name("solid-js/store"), "solid-js");
        assert_eq!(package_name("@solidjs/web/storage"), "@solidjs/web");
        assert_eq!(package_name("@solidjs/web"), "@solidjs/web");
        assert_eq!(package_name("fz-step"), "fz-step");
    }
}
