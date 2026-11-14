//! Async and loading-cover context: Loading boundaries, async reads, and non-component function containment.

use super::*;

/// Whether a `Loading` boundary above a render position is proven, proven
/// absent, or cannot be resolved from the analyzed project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LoadingCover {
    /// A boundary encloses the position, directly or through the call sites
    /// that render the function containing it.
    Covered,
    /// Every call-site chain was followed to a mount root (a `render` or
    /// `hydrate` argument) and none passed through a boundary.
    Uncovered,
    /// A chain ends at a function nothing in the project renders -- an
    /// exported component, a route handed to a router, a lazy page -- so the
    /// boundary above it is decided by code the analysis cannot see. Missing
    /// is not absent.
    Unresolved,
}

pub(crate) fn read_loading_cover(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    span: Span,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> LoadingCover {
    loading_cover_at(lookup, file, span, symbol_names, &mut HashSet::default())
}

pub(super) fn loading_cover_at(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    span: Span,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    visited: &mut HashSet<(String, Span)>,
) -> LoadingCover {
    loading_cover_from(lookup, file, span, None, symbol_names, visited)
}

/// [`loading_cover_at`], where `outside` is a `Loading` element the walk has already left through its `fallback`: that element does not hold
/// the position, which is where it stands.
fn loading_cover_from(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    span: Span,
    outside: Option<Span>,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    visited: &mut HashSet<(String, Span)>,
) -> LoadingCover {
    let entities = lookup.entities();
    // ADR 0978: a boundary holds what it renders as `children` and under its `on`, not its `fallback`, which renders outside the hold (rc.21
    // `Loading`: `createLoadingBoundary(() => props.children, () => props.fallback, ..)`). A read in the `fallback` of a `Loading` the program
    // writes goes on outward from the element, as one in a hand-called boundary's `fallback` getter does.
    let mut fallback_of = None;
    for element in file.ast.jsx_containing(span) {
        if Some(element.span) == outside
            || !jsx_element_is_loading(file, element, entities, symbol_names, lookup.dialect)
        {
            continue;
        }
        let fallback_prop = lookup
            .dialect
            .async_boundary_shape()
            .map(|shape| shape.fallback_prop);
        let in_fallback = element.attributes.iter().any(|attribute| {
            attribute.namespace.is_none()
                && fallback_prop
                    .is_some_and(|prop| file.source_text(attribute.local_name) == Some(prop))
                && attribute.span.contains(span)
        });
        if !in_fallback {
            return LoadingCover::Covered;
        }
        // Elements come outermost first: the innermost boundary the position is the fallback of is left first, and the walk from it meets
        // any other (a `Loading` written in that fallback holds it as a child).
        fallback_of = Some(element.span);
    }
    if let Some(element) = fallback_of {
        return loading_cover_from(lookup, file, element, Some(element), symbol_names, visited);
    }
    // ADR 0668: a component covers a site only as its child, and only where the
    // `Loading` holds what it renders of `children`: `<App />` covers neither its
    // own tag nor a read beside the `Loading` in its body.
    let mut beside_loading = false;
    for element in file.ast.jsx_containing(span) {
        if !element.children.iter().any(|child| child.contains(span)) {
            continue;
        }
        let Some((target_file, target)) = jsx_target_function(lookup, file, element) else {
            continue;
        };
        let loadings = target_file
            .ast
            .jsx_within(target.body)
            .filter(|candidate| {
                jsx_element_is_loading(
                    target_file,
                    candidate,
                    entities,
                    symbol_names,
                    lookup.dialect,
                )
            })
            .collect::<Vec<_>>();
        if loadings.is_empty() {
            continue;
        }
        // ADR 0668: `children` of the component's own props, the parameter or
        // an exact `const` alias of it. A `children` member of any other object
        // (`other.children`) is a spelling, not what the component renders.
        let children_in_loading = target_file.ast.members.iter().any(|member| {
            target.body.contains(member.span)
                && target_file.source_text(member.property) == Some("children")
                && is_props_of(target_file, target, member.object)
                && loadings
                    .iter()
                    .any(|loading| loading.span.contains(member.span))
        });
        if children_in_loading {
            return LoadingCover::Covered;
        }
        beside_loading = true;
    }
    if beside_loading {
        return LoadingCover::Unresolved;
    }
    // A wrapper the site is a child of, with no `Loading` of its own, that
    // reads its `children`: it may hand them to another component that has
    // one (`<Layout><Loading>{props.children}</Loading></Layout>` behind a
    // `Shell`), which this walk does not follow. Not proven uncovered (review
    // round 4).
    if file.ast.jsx_containing(span).any(|element| {
        element.children.iter().any(|child| child.contains(span))
            && jsx_target_function(lookup, file, element).is_some_and(|(target_file, target)| {
                target_file.ast.members.iter().any(|member| {
                    target.body.contains(member.span)
                        && target_file.source_text(member.property) == Some("children")
                })
            })
    }) {
        return LoadingCover::Unresolved;
    }
    // Walk outward from the innermost function. A closure nothing in the
    // project calls (a `<For>` row callback, a render prop) is part of the
    // function around it; the first function the project does render or call
    // is the one whose call sites decide.
    // ADR 0454: a site in a parameter list is first the list's function's.
    let mut enclosing = file.ast.functions_owning(span).collect::<Vec<_>>();
    enclosing.sort_by_key(|function| function.span.end - function.span.start);
    for function in enclosing {
        if !visited.insert((file.path.as_str().to_owned(), function.span)) {
            // Already being resolved on this chain: it adds no information.
            return LoadingCover::Uncovered;
        }
        let call_sites = lookup.function_call_sites(file.path.as_str(), function.span);
        if !call_sites.is_empty() {
            // ADR 0668: a `Loading` in the body that does not hold the site (the
            // first check) proves nothing about it: a read beside one still warns.
            // It stays open, because what the site's value reaches may be inside.
            let beside = !file.ast.any_jsx_containing(span)
                && file.ast.jsx_within(function.body).any(|candidate| {
                    jsx_element_is_loading(file, candidate, entities, symbol_names, lookup.dialect)
                });
            let mut result = if beside {
                LoadingCover::Unresolved
            } else {
                LoadingCover::Uncovered
            };
            for (caller_file, callee) in call_sites {
                match loading_cover_at(lookup, caller_file, callee, symbol_names, visited) {
                    LoadingCover::Covered => return LoadingCover::Covered,
                    LoadingCover::Unresolved => result = LoadingCover::Unresolved,
                    LoadingCover::Uncovered => {}
                }
            }
            return result;
        }
        if function_is_mount_root(lookup, file, function) {
            return LoadingCover::Uncovered;
        }
        // ADR 0978: a layer no call edge places. A getter of a hand-called boundary's props or an argument of `createLoadingBoundary` is
        // run by the boundary (`children` is the hold, `fallback` renders outside it); every other layer a call does not hand (an object's
        // member, an array element, a class element, a tag function, a closure handed to a call the analysis does not resolve) is run where
        // the analysis does not see, so a boundary may hold it: never read as uncovered.
        if let Some(layer) = crate::loading_entries::layer_cover(lookup, file, function) {
            return match layer {
                crate::loading_entries::LayerCover::Covered => LoadingCover::Covered,
                crate::loading_entries::LayerCover::Unresolved => LoadingCover::Unresolved,
                crate::loading_entries::LayerCover::Outward(at) => {
                    loading_cover_from(lookup, file, at, None, symbol_names, visited)
                }
            };
        }
    }
    LoadingCover::Unresolved
}

/// ADR 0668: whether `object`, the object of a member read in `component`'s
/// body, is the component's own props: its first parameter (a plain
/// identifier, never reassigned) or an exact `const` alias of it
/// (`const p = props`), each resolved by the binder. A destructured or
/// defaulted parameter, an object built in the body and any other binding are
/// not: the caller keeps the answer it gives without a proof.
fn is_props_of(
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
    object: Span,
) -> bool {
    /// Aliases followed before the answer is no.
    const ALIAS_DEPTH: usize = 3;
    let Some(parameter) = component.parameters.first() else {
        return false;
    };
    let [name] = parameter.names.as_slice() else {
        return false;
    };
    if parameter.shape != solid_facts::ast::BindingShape::Identifier
        || parameter.initializer.is_some()
        || crate::indexes::binding_written(file, name.span)
    {
        return false;
    }
    let ast = &file.ast;
    let mut declaration = ast.reference_declaration(ast.peel_ts_sugar_span(object));
    for _ in 0..=ALIAS_DEPTH {
        let Some(declared) = declaration else {
            return false;
        };
        if declared == name.span {
            return true;
        }
        // One exact alias hop: `const p = props`, `p` never reassigned.
        let alias = ast.bindings.iter().find(|binding| {
            binding.immutable
                && binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding.names.len() == 1
                && binding.names[0].span == declared
        });
        declaration = alias
            .and_then(|binding| binding.initializer_identifier.as_ref())
            .and_then(|initializer| ast.reference_declaration(initializer.span));
    }
    false
}

/// ADR 0503: `component` is rendered by a call of a synchronous mount
/// (`render`, `mount_runs_component_synchronously`) written at a module's top
/// level, whose first argument is a function literal whose whole body is one
/// self-closing JSX element (no attribute, no child) whose tag resolves exactly
/// to `component`. Such a call runs the component's body on its own stack,
/// outside every flush, lane, transition and boundary (`@solidjs/web@2.0.0-rc.17`
/// `dist/web.dev.js:542-568`). One such mount is enough for a claim about a
/// run that happens in it.
///
/// ADR 0507: or a parent so mounted renders it ([`MountSite::parents`]).
pub(crate) fn mounted_by_module_render(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
) -> bool {
    !module_render_sites(lookup, file, component).is_empty()
}

/// A mount [`mounted_by_module_render`] accepts: the module-scope `render`
/// call and the file it is written in, and the parents between it and the
/// component, outermost first: each parent's file, function and the tag in it
/// that renders the next one (or the component).
pub(crate) struct MountSite<'a> {
    pub(crate) entry: &'a solid_facts::FileFacts,
    pub(crate) render: Span,
    pub(crate) parents: Vec<(
        &'a solid_facts::FileFacts,
        &'a solid_facts::ast::FunctionFact,
        Span,
    )>,
}

/// How many parents a mount may pass through.
const MOUNT_DEPTH: usize = 4;

/// The mounts of `component`: its own module-scope renders (ADR 0503), and
/// those of each parent that renders it as a static child (ADR 0507,
/// [`static_child_tag`]).
pub(crate) fn module_render_sites<'a>(
    lookup: &SemanticLookup<'a>,
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
) -> Vec<MountSite<'a>> {
    mount_sites_within(lookup, file, component, MOUNT_DEPTH)
}

fn mount_sites_within<'a>(
    lookup: &SemanticLookup<'a>,
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
    depth: usize,
) -> Vec<MountSite<'a>> {
    let mut sites = direct_render_sites(lookup, file, component)
        .into_iter()
        .map(|(entry, render)| MountSite {
            entry,
            render,
            parents: Vec::new(),
        })
        .collect::<Vec<_>>();
    if depth == 0 || !lookup.function_value_is_current(file, component) {
        return sites;
    }
    for (caller, tag) in lookup.function_call_sites(file.path.as_str(), component.span) {
        let Some(parent) = static_child_tag(lookup, caller, tag) else {
            continue;
        };
        for mut site in mount_sites_within(lookup, caller, parent, depth - 1) {
            site.parents.push((caller, parent, tag));
            sites.push(site);
        }
    }
    sites
}

/// ADR 0507: the parent whose body renders the component at `tag` while it
/// runs, synchronously: the tag is a self-closing JSX element with no
/// attribute, spread or child, written in the parent's own body (not in a
/// function nested there), inside the argument of the parent's one return,
/// which is unconditional and in its straight-line flow (or its expression
/// body), reached from that argument through intrinsic elements only, each the
/// child of the next (no expression container, fragment or component between).
/// The compiler evaluates such a tag (`createComponent`) while it builds the
/// returned tree, so the child's body runs inside the parent's, outside any
/// computation. `<Show>`, `<For>` and other components run their children
/// later or inside a computation, and are not this.
pub(crate) fn static_child_tag<'a>(
    lookup: &SemanticLookup<'a>,
    file: &'a solid_facts::FileFacts,
    tag: Span,
) -> Option<&'a solid_facts::ast::FunctionFact> {
    let ast = &file.ast;
    let element = ast
        .jsx_elements
        .iter()
        .find(|element| element.name.span == tag)?;
    if !element.self_closing || !element.spreads.is_empty() || !element.children.is_empty() {
        return None;
    }
    // ADR 0508: a `<For>` row (`{() => <C />}`) runs the tag in the row
    // function, which the climb steps over to the `<For>` element.
    let mut at = element.span;
    let mut holders = Vec::new();
    let mut owner = crate::owners::containing_ast_function(ast, element.span)?;
    if let Some((row, list)) = for_row(lookup, file, owner, element.span) {
        holders.push(list.span);
        holders.push(row.span);
        at = list.span;
        owner = crate::owners::containing_ast_function(ast, list.span)?;
    }
    let parent = owner;
    if parent.r#async
        || parent.generator
        || !lookup.function_may_be_component(file, parent)
        || !lookup.function_value_is_current(file, parent)
    {
        return None;
    }
    // ADR 0510: each attribute hands the child a node the parent created.
    if !element
        .attributes
        .iter()
        .all(|attribute| handed_attribute(lookup, file, parent, attribute))
    {
        return None;
    }
    let own = |span: Span| {
        crate::owners::containing_ast_function(ast, span)
            .is_some_and(|function| function.span == parent.span)
    };
    let returns = ast
        .returns
        .iter()
        .filter(|fact| own(fact.span))
        .collect::<Vec<_>>();
    let root = if parent.expression_body {
        if !returns.is_empty() {
            return None;
        }
        ast.peel_ts_sugar_span(parent.body)
    } else {
        let [only] = returns.as_slice() else {
            return None;
        };
        if only.conditional || !only.straight_line {
            return None;
        }
        ast.peel_ts_sugar_span(only.argument?)
    };
    if !root.contains(element.span)
        || ast
            .jsx_fragments
            .iter()
            .any(|fragment| root.contains(*fragment) && fragment.contains(element.span))
    {
        return None;
    }
    // Climb from the tag to the returned element, each step the child of an
    // intrinsic element, or the one child of a `<Show when={true}>` (ADR 0508).
    for _ in 0..64 {
        if at == root {
            return inert_parent(lookup, file, parent, element, &holders).then_some(parent);
        }
        let holder = ast
            .jsx_elements
            .iter()
            .filter(|candidate| candidate.span != at && candidate.span.contains(at))
            .min_by_key(|candidate| candidate.span.end - candidate.span.start)?;
        if !holder.children.contains(&at) || !own(holder.span) {
            return None;
        }
        if !intrinsic_element(file, holder) {
            if !shown_child(lookup, file, holder, at)
                && !provided_child(lookup, file, parent, holder, at)
                && !loading_child(lookup, file, holder, at)
            {
                return None;
            }
            holders.push(holder.span);
        }
        at = holder.span;
    }
    None
}

/// ADR 0570: where the component tag at `tag` (its name span) is created
/// while the code around it runs, synchronously: the outermost element of
/// the chain from the tag up through holders that build their children when
/// they are created, each holding the next as a direct child (no expression
/// container, attribute value or fragment between): intrinsic elements (the
/// compiler creates a child component as `insert`'s argument), ADR 0508's
/// `<Show when={true}>` and literal `<For>` row, ADR 0511's literal-fallback
/// `<Loading>`, and a context provider (ADR 0510; any value, which is read
/// by a getter). The answer is evaluated where the code holding it runs (the
/// innermost function whose body holds it, or module scope): an answer
/// inside a callback (`<Show when={on()}>{() => <C />}</Show>`, a row of a
/// non-literal `<For>`) runs when that callback does. `None` for any other
/// holder (`<Show when={on()}>`, `<Switch>`, a project component's children)
/// or a tag inside a class body.
pub(crate) fn synchronous_tag_position(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    tag: Span,
) -> Option<Span> {
    let ast = &file.ast;
    let element = ast
        .jsx_elements
        .iter()
        .find(|element| element.name.span == tag)?;
    let mut at = element.span;
    let mut owner = crate::owners::containing_ast_function(ast, element.span);
    if let Some(row_function) = owner
        && let Some((_, list)) = for_row(lookup, file, row_function, element.span)
    {
        at = list.span;
        owner = crate::owners::containing_ast_function(ast, list.span);
    }
    let owner = owner.map(|function| function.span);
    let own = |span: Span| {
        crate::owners::containing_ast_function(ast, span).map(|function| function.span) == owner
    };
    for _ in 0..64 {
        let Some(holder) = ast
            .jsx_elements
            .iter()
            .filter(|candidate| candidate.span != at && candidate.span.contains(at))
            .min_by_key(|candidate| candidate.span.end - candidate.span.start)
        else {
            break;
        };
        if !own(holder.span) {
            break;
        }
        if !holder.children.contains(&at)
            || !(intrinsic_element(file, holder)
                || shown_child(lookup, file, holder, at)
                || loading_child(lookup, file, holder, at)
                || (holder.member_object.is_none()
                    && holder.spreads.is_empty()
                    && context_declaration(lookup, file, holder.name.span).is_some()))
        {
            return None;
        }
        at = holder.span;
    }
    let inside = |span: Span| span != at && span.contains(at) && own(span);
    (!ast.jsx_fragments.iter().any(|fragment| inside(*fragment))
        && !ast.classes.iter().any(|class| class.span.contains(at)))
    .then_some(at)
}

/// ADR 0661: the code at `at` runs whenever its own function does: on no arm of
/// a conditional, not to the right of a logical, and under JSX holders that
/// build it when created. An intrinsic's attribute and `<Show when>` itself run
/// with their element.
pub(super) fn unconditional_position(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    at: Span,
) -> bool {
    let ast = &file.ast;
    let owner = crate::owners::containing_ast_function(ast, at).map(|function| function.span);
    let own = |span: Span| {
        crate::owners::containing_ast_function(ast, span).map(|function| function.span) == owner
    };
    if ast.conditional_expressions.iter().any(|conditional| {
        own(conditional.span)
            && (conditional.consequent.contains(at) || conditional.alternate.contains(at))
    }) || ast
        .logical_expressions
        .iter()
        .any(|logical| own(logical.span) && logical.right.contains(at))
        || ast.if_regions.iter().any(|region| {
            own(region.consequent)
                && (region.consequent.contains(at)
                    || region
                        .alternate
                        .is_some_and(|alternate| alternate.contains(at)))
        })
    {
        return false;
    }
    ast.jsx_elements
        .iter()
        .filter(|holder| holder.span != at && holder.span.contains(at) && own(holder.span))
        .all(|holder| {
            if let Some(child) = holder
                .children
                .iter()
                .copied()
                .find(|child| child.contains(at))
            {
                return intrinsic_element(file, holder)
                    || shown_child(lookup, file, holder, child)
                    || loading_child(lookup, file, holder, child)
                    || (holder.member_object.is_none()
                        && holder.spreads.is_empty()
                        && context_declaration(lookup, file, holder.name.span).is_some());
            }
            intrinsic_element(file, holder)
                || holder.attributes.iter().any(|attribute| {
                    attribute.expression.is_some_and(|value| value.contains(at))
                        && file.source_text(attribute.name) == Some("when")
                        && element_primitive(lookup, file, holder)
                            == Some(solid_dialect::Primitive::Show)
                })
        })
}

/// A lowercase, non-member tag of letters and digits.
fn intrinsic_element(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> bool {
    element.member_object.is_none()
        && file.source_text(element.name.span).is_some_and(|name| {
            name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                && name.chars().all(|c| c.is_ascii_alphanumeric())
        })
}

/// The dialect primitive `element` renders, by its index in the file's JSX.
pub(crate) fn element_primitive(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> Option<solid_dialect::Primitive> {
    let index = file
        .ast
        .jsx_elements
        .iter()
        .position(|candidate| candidate.span == element.span)?;
    crate::known_primitive(lookup.primitives(file).jsx.get(index)?)
}

/// The children of `element` other than whitespace text that holds a line
/// break: the compiler and TypeScript drop such text, but keep whitespace on
/// one line (`<For> {() => <C />} </For>` hands `For` an array; ADR 0508
/// review).
fn significant_children<'f>(
    file: &'f solid_facts::FileFacts,
    element: &'f solid_facts::ast::JsxElementFact,
) -> Vec<Span> {
    element
        .children
        .iter()
        .copied()
        .filter(|child| {
            file.source_text(*child)
                .is_none_or(|text| !(text.trim().is_empty() && text.contains('\n')))
        })
        .collect()
}

/// ADR 0511: `holder` is the dialect's `<Loading fallback={<literal>}>` (one
/// attribute, `fallback`, a primitive literal; no `on`, no spread) whose one
/// child is `child`. `Loading` returns `createLoadingBoundary(() =>
/// props.children, …)` (`solid-js@2.0.0-rc.17` `dist/solid.dev.js:1654-1659`),
/// whose boundary builds the children in `computed(fn, { name: "children" })`
/// when it is created (`@solidjs/signals@2.0.0-rc.17` `dist/dev.js:9463-9468`;
/// `computed` runs a node that is not `lazy` at creation), so the child
/// renders during the parent's tree, under that owner. A render effect
/// created there runs its first apply on the call, which `runEffect`'s hold
/// behind a fallback does not stop (only a queued run is held,
/// `dist/dev.js:1947-1955`). A user effect's queued run is held while the
/// boundary shows its fallback: ADR 0505's premises (a static tree, nothing
/// else in the mount) leave nothing under it to make it show one.
fn loading_child(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    holder: &solid_facts::ast::JsxElementFact,
    child: Span,
) -> bool {
    let fallback = match holder.attributes.as_slice() {
        [attribute]
            if attribute.namespace.is_none()
                && file.source_text(attribute.name) == Some("fallback") =>
        {
            attribute.expression
        }
        _ => return false,
    };
    element_primitive(lookup, file, holder) == Some(solid_dialect::Primitive::Loading)
        && holder.spreads.is_empty()
        && holder.boolean_properties.is_empty()
        && fallback.is_some_and(|fallback| {
            file.source_text(file.ast.peel_ts_sugar_span(fallback))
                .is_some_and(crate::execution_role::primitive_literal_body)
        })
        && significant_children(file, holder) == [child]
}

/// ADR 0508: `holder` is the dialect's `<Show when={true}>` (one attribute,
/// `when`, whose value is the literal `true`; no spread) whose one child is
/// `child`. Its value memo computes when `Show` is created (`computed` runs
/// a node that is not `lazy` at creation, `@solidjs/signals@2.0.0-rc.17`
/// `dist/dev-shared.js:4913-4932`) and reads `props.children` when the
/// condition holds (`solid-js@2.0.0-rc.17` `dist/solid.dev.js:1571-1600`), so
/// the child renders inside that compute, during the parent's tree.
fn shown_child(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    holder: &solid_facts::ast::JsxElementFact,
    child: Span,
) -> bool {
    let when = match holder.attributes.as_slice() {
        [attribute] if file.source_text(attribute.name) == Some("when") => attribute.expression,
        _ => return false,
    };
    // `when={true}` is a boolean property too.
    element_primitive(lookup, file, holder) == Some(solid_dialect::Primitive::Show)
        && holder.spreads.is_empty()
        && holder
            .boolean_properties
            .iter()
            .all(|property| property.value && file.source_text(property.name) == Some("when"))
        && when.is_some_and(|when| {
            literally_truthy(file.source_text(file.ast.peel_ts_sugar_span(when)))
        })
        && significant_children(file, holder) == [child]
}

/// `true`, or a number or string literal that is not falsy.
fn literally_truthy(text: Option<&str>) -> bool {
    text.is_some_and(|text| {
        let quoted = text.len() > 2
            && (text.starts_with('"') || text.starts_with('\''))
            && text.ends_with(&text[..1]);
        text == "true"
            || quoted
            || text
                .parse::<f64>()
                .is_ok_and(|value| value != 0.0 && !value.is_nan())
    })
}

/// ADR 0508: `row` is the one child of the dialect's `<For each={[…]}>` (one
/// attribute, `each`, an array literal of primitive literals with at least one
/// element; no spread): a function literal with no parameter whose expression
/// body is `element`. `For` builds its `mapArray` on the first read
/// of its result (`solid-js@2.0.0-rc.17` `dist/solid.dev.js:1548-1562`), which
/// the parent's insert makes on its first pass, so the row runs during the
/// parent's tree.
fn for_row<'a>(
    lookup: &SemanticLookup<'_>,
    file: &'a solid_facts::FileFacts,
    row: &'a solid_facts::ast::FunctionFact,
    element: Span,
) -> Option<(
    &'a solid_facts::ast::FunctionFact,
    &'a solid_facts::ast::JsxElementFact,
)> {
    let ast = &file.ast;
    if row.r#async
        || row.generator
        || !row.parameters.is_empty()
        || row.rest_parameter
        || !row.expression_body
        || ast.peel_ts_sugar_span(row.body) != element
    {
        return None;
    }
    let list = ast
        .jsx_elements
        .iter()
        .filter(|candidate| candidate.span.contains(row.span))
        .min_by_key(|candidate| candidate.span.end - candidate.span.start)?;
    let [container] = significant_children(file, list)[..] else {
        return None;
    };
    let inner = file.source_text(container)?.trim();
    let held = inner
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .map(str::trim)?;
    let each = match list.attributes.as_slice() {
        [attribute] if file.source_text(attribute.name) == Some("each") => attribute.expression?,
        _ => return None,
    };
    let each = file.source_text(ast.peel_ts_sugar_span(each))?.trim();
    let elements = each
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))?;
    let literals = elements.split(',').map(str::trim).collect::<Vec<_>>();
    let primitive = |text: &str| {
        matches!(text, "true" | "false" | "null")
            || (!text.is_empty()
                && text
                    .trim_start_matches('-')
                    .chars()
                    .all(|c| c.is_ascii_digit())
                && !text.trim_start_matches('-').is_empty())
    };
    (element_primitive(lookup, file, list) == Some(solid_dialect::Primitive::For)
        && list.spreads.is_empty()
        && list.boolean_properties.is_empty()
        && file.source_text(row.span).map(str::trim) == Some(held)
        && !literals.is_empty()
        && literals.iter().all(|literal| primitive(literal)))
    .then_some((row, list))
}

/// ADR 0507: the parent runs nothing but the render of `child`: its body is
/// one `return <tree>` (or the tree as its expression body), with no other
/// statement (an implicit coercion, getter, iteration or loop runs code no
/// syntax names; ADR 0507 review), no parameter but plain ones, and in its
/// span no call, member read, tagged template, class, `throw`, nested function
/// or fragment, and no JSX element but `child` and intrinsic elements with no
/// attribute, spread or expression child. An earlier pass of the same mount
/// that went pending would stage the child's passes
/// (`@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:1690-1708`), and code
/// that throws ends the mount.
fn inert_parent(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    parent: &solid_facts::ast::FunctionFact,
    child: &solid_facts::ast::JsxElementFact,
    holders: &[Span],
) -> bool {
    let ast = &file.ast;
    let inside = |span: Span| parent.span.contains(span);
    // ADR 0510: the `const` creations written before the return.
    let Some((creations, after_creations)) = parent_creations(lookup, file, parent) else {
        return false;
    };
    let computes = creations
        .iter()
        .filter_map(|call| call.arguments.first())
        .map(|argument| ast.peel_ts_sugar_span(argument.span))
        .collect::<Vec<_>>();
    let plain_parameters = parent.parameters.iter().all(|parameter| {
        parameter.shape == solid_facts::ast::BindingShape::Identifier
            && parameter.initializer.is_none()
    });
    let elements_static = ast
        .jsx_elements
        .iter()
        .filter(|element| {
            inside(element.span) && element.span != child.span && !holders.contains(&element.span)
        })
        .all(|element| {
            let intrinsic = element.member_object.is_none()
                && file.source_text(element.name.span).is_some_and(|name| {
                    name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                        && name.chars().all(|c| c.is_ascii_alphanumeric())
                });
            let children = element.children.iter().all(|span| {
                let text = file.source_text(*span).unwrap_or("{");
                if text.starts_with('<') {
                    ast.jsx_elements.iter().any(|inner| inner.span == *span)
                } else if text.starts_with('{') {
                    element.empty_children.contains(span)
                } else {
                    true
                }
            });
            intrinsic && element.attributes.is_empty() && element.spreads.is_empty() && children
        });
    // The body is the return and nothing else: between the braces, `return`
    // and the returned expression (`ReturnFact::span`), in parentheses or not.
    let own_returns = ast
        .returns
        .iter()
        .filter(|fact| {
            crate::owners::containing_ast_function(ast, fact.span)
                .is_some_and(|function| function.span == parent.span)
        })
        .collect::<Vec<_>>();
    let only_return = (parent.expression_body && creations.is_empty())
        || match own_returns.as_slice() {
            [only] => {
                let returned = file
                    .source
                    .get(after_creations as usize..parent.body.end as usize)
                    .and_then(|body| body.trim().strip_suffix('}'))
                    .and_then(|inner| inner.trim().strip_prefix("return"))
                    .map(|rest| {
                        let mut rest = rest.trim().trim_end_matches(';').trim_end();
                        while let Some(inside) =
                            rest.strip_prefix('(').and_then(|r| r.strip_suffix(')'))
                        {
                            rest = inside.trim();
                        }
                        rest
                    });
                returned.is_some_and(|returned| {
                    file.source_text(only.span).map(str::trim) == Some(returned)
                })
            }
            _ => false,
        };
    only_return
        && plain_parameters
        && elements_static
        && !ast.calls.iter().any(|call| {
            inside(call.span) && !creations.iter().any(|created| created.span == call.span)
        })
        && !ast.members.iter().any(|member| inside(member.span))
        && !ast.computed_members.iter().any(|member| inside(*member))
        && !ast.tagged_template_tags.iter().any(|tag| inside(*tag))
        && !ast.classes.iter().any(|class| inside(class.span))
        && !ast
            .throw_statements
            .iter()
            .any(|statement| inside(*statement))
        && !ast.jsx_fragments.iter().any(|fragment| inside(*fragment))
        && !ast.functions.iter().any(|function| {
            function.span != parent.span
                && inside(function.span)
                && !holders.contains(&function.span)
                && !computes.contains(&function.span)
        })
}

/// ADR 0510: the statements of `parent`'s block body before its return, each
/// `const <pattern> = <creation>;` with an inert creation
/// ([`crate::execution_role::inert_creation`]: a memo of one primitive
/// literal, a signal of at most one, a context), and the offset where the
/// rest of the body starts. An expression body has none.
fn parent_creations<'f>(
    lookup: &SemanticLookup<'_>,
    file: &'f solid_facts::FileFacts,
    parent: &solid_facts::ast::FunctionFact,
) -> Option<(Vec<&'f solid_facts::ast::CallFact>, u32)> {
    let ast = &file.ast;
    if parent.expression_body {
        return Some((Vec::new(), parent.body.start));
    }
    let source = file.source.as_bytes();
    let skip = |mut at: usize| {
        while source.get(at).is_some_and(u8::is_ascii_whitespace) {
            at += 1;
        }
        at
    };
    if source.get(parent.body.start as usize) != Some(&b'{') {
        return None;
    }
    let mut at = skip(parent.body.start as usize + 1);
    let mut creations = Vec::new();
    for _ in 0..16 {
        let rest = file.source.get(at..parent.body.end as usize)?;
        let Some(after) = rest.strip_prefix("const") else {
            break;
        };
        if !after.starts_with(|c: char| c.is_ascii_whitespace()) {
            break;
        }
        let pattern = skip(at + "const".len());
        let binding = ast.bindings.iter().find(|binding| {
            binding.pattern.start as usize == pattern
                && crate::owners::containing_ast_function(ast, binding.declaration)
                    .is_some_and(|function| function.span == parent.span)
        })?;
        let initializer = ast.peel_ts_sugar_span(binding.initializer?);
        let call = ast.call_at(initializer)?;
        let between = file
            .source
            .get(binding.pattern.end as usize..initializer.start as usize)?;
        if between.trim() != "=" || !crate::execution_role::inert_creation(file, call, lookup) {
            return None;
        }
        creations.push(call);
        at = skip(initializer.end as usize);
        if source.get(at) == Some(&b';') {
            at = skip(at + 1);
        }
    }
    Some((creations, u32::try_from(at).ok()?))
}

/// ADR 0510: `expression` names a node `parent` created in its own body: an
/// identifier bound by a `const` there (a name, or a slot of an array
/// pattern) whose initializer is an inert creation. Reading it runs nothing.
fn handed_name(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    parent: &solid_facts::ast::FunctionFact,
    expression: Span,
) -> bool {
    let ast = &file.ast;
    let expression = ast.peel_ts_sugar_span(expression);
    ast.identifiers.iter().any(|identifier| {
        identifier.span == expression
            && identifier.role == solid_facts::ast::IdentifierRole::Reference
    }) && ast
        .reference_declaration(expression)
        .and_then(|declaration| {
            ast.bindings.iter().find(|binding| {
                binding.immutable && binding.names.iter().any(|name| name.span == declaration)
            })
        })
        .is_some_and(|binding| {
            crate::owners::containing_ast_function(ast, binding.declaration)
                .is_some_and(|function| function.span == parent.span)
                && binding
                    .call_initializer
                    .and_then(|call| ast.call_at(call))
                    .is_some_and(|call| crate::execution_role::inert_creation(file, call, lookup))
        })
}

/// ADR 0510: a child's attribute `name={x}`, `x` a node the parent created
/// ([`handed_name`]): the compiler hands an identifier over as the prop's
/// value, and nothing runs when the child reads it.
fn handed_attribute(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    parent: &solid_facts::ast::FunctionFact,
    attribute: &solid_facts::ast::JsxAttributeFact,
) -> bool {
    attribute.namespace.is_none()
        && attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
        && file.source_text(attribute.name) != Some("ref")
        && attribute
            .expression
            .is_some_and(|expression| handed_name(lookup, file, parent, expression))
}

/// The module-scope `const` holding a context the dialect's `createContext`
/// created ([`crate::execution_role::inert_creation`]) that `name` names.
fn context_declaration(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    name: Span,
) -> Option<Span> {
    let ast = &file.ast;
    let declaration = ast.reference_declaration(name)?;
    let binding = ast.bindings.iter().find(|binding| {
        binding.immutable && binding.names.iter().any(|named| named.span == declaration)
    })?;
    let call = ast.call_at(binding.call_initializer?)?;
    (crate::indexes::at_module_scope(ast, binding.declaration)
        && lookup.primitive_at_call(file, call.span)
            == Some(solid_dialect::Primitive::CreateContext)
        && crate::execution_role::inert_creation(file, call, lookup))
    .then_some(declaration)
}

/// The object literal `holder`'s one `value` attribute holds, when `holder`
/// is a context's provider (`<K value={{ … }}>`, [`context_declaration`]) with
/// no other attribute or spread: each property a plain `key: x` or `{ x }`
/// whose value names a node `parent` created ([`handed_name`]).
fn provider_value<'f>(
    lookup: &SemanticLookup<'_>,
    file: &'f solid_facts::FileFacts,
    parent: &solid_facts::ast::FunctionFact,
    holder: &solid_facts::ast::JsxElementFact,
) -> Option<(Span, Vec<&'f solid_facts::ast::ObjectPropertyFact>)> {
    let ast = &file.ast;
    if holder.member_object.is_some()
        || !holder.spreads.is_empty()
        || !holder.boolean_properties.is_empty()
    {
        return None;
    }
    let context = context_declaration(lookup, file, holder.name.span)?;
    let [attribute] = holder.attributes.as_slice() else {
        return None;
    };
    if attribute.namespace.is_some()
        || file.source_text(attribute.name) != Some("value")
        || attribute.value_kind != solid_facts::ast::JsxAttributeValueKind::Expression
    {
        return None;
    }
    let value = ast.peel_ts_sugar_span(attribute.expression?);
    let text = file.source_text(value)?;
    if !text.starts_with('{')
        || ast.spreads.iter().any(|spread| value.contains(spread.span))
        || ast
            .functions
            .iter()
            .any(|function| value.contains(function.span))
        || ast.calls.iter().any(|call| value.contains(call.span))
        || ast.members.iter().any(|member| value.contains(member.span))
    {
        return None;
    }
    let properties = ast
        .object_properties
        .iter()
        .filter(|property| value.contains(property.span))
        .collect::<Vec<_>>();
    properties
        .iter()
        .all(|property| {
            !property.computed
                && !property.method
                && handed_name(lookup, file, parent, property.value)
        })
        .then_some((context, properties))
}

/// ADR 0510: `holder` is a context's provider ([`provider_value`]) whose one
/// child is `child`. The provider sets its value on a root it creates and
/// returns `children(() => props.children)` (`solid-js@2.0.0-rc.17`
/// `dist/solid.dev.js:11-15`), lazy memos the parent's insert reads on its
/// first pass (ADR 0508's `<For>` argument), so the child renders during the
/// parent's tree, under an owner that carries the value.
fn provided_child(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    parent: &solid_facts::ast::FunctionFact,
    holder: &solid_facts::ast::JsxElementFact,
    child: Span,
) -> bool {
    provider_value(lookup, file, parent, holder).is_some()
        && significant_children(file, holder) == [child]
}

/// ADR 0510: the tag at `tag` in `file` passes `memo` (a name of the
/// parent's) as its prop `name`.
pub(crate) fn tag_passes_prop(
    file: &solid_facts::FileFacts,
    tag: Span,
    name: &str,
    memo: Option<Span>,
) -> bool {
    let ast = &file.ast;
    ast.jsx_elements
        .iter()
        .find(|element| element.name.span == tag)
        .is_some_and(|element| {
            element.attributes.iter().any(|attribute| {
                attribute.namespace.is_none()
                    && file.source_text(attribute.name) == Some(name)
                    && attribute.expression.is_some_and(|expression| {
                        memo.is_some()
                            && ast.reference_declaration(ast.peel_ts_sugar_span(expression)) == memo
                    })
            })
        })
}

/// ADR 0510: the tag at `tag` in `file` is the one child of a provider of
/// the context declared at `context` whose value object holds `memo` (a name
/// of the parent's) under the key `name`; the nearest provider of that context
/// above the child is that one.
pub(crate) fn tag_provided(
    file: &solid_facts::FileFacts,
    tag: Span,
    context: Span,
    name: &str,
    memo: Option<Span>,
) -> bool {
    let ast = &file.ast;
    let Some(element) = ast
        .jsx_elements
        .iter()
        .find(|element| element.name.span == tag)
    else {
        return false;
    };
    let Some(holder) = ast
        .jsx_elements
        .iter()
        .filter(|candidate| candidate.span != element.span && candidate.span.contains(element.span))
        .min_by_key(|candidate| candidate.span.end - candidate.span.start)
    else {
        return false;
    };
    let Some(value) = holder
        .attributes
        .iter()
        .find(|attribute| file.source_text(attribute.name) == Some("value"))
        .and_then(|attribute| attribute.expression)
        .map(|expression| ast.peel_ts_sugar_span(expression))
    else {
        return false;
    };
    memo.is_some()
        && ast.reference_declaration(holder.name.span) == Some(context)
        && ast
            .object_properties
            .iter()
            .filter(|property| value.contains(property.span))
            .any(|property| {
                file.source_text(property.key) == Some(name)
                    && ast.reference_declaration(ast.peel_ts_sugar_span(property.value)) == memo
            })
}

/// The module-scope renders that mount `component` itself (ADR 0503).
pub(crate) fn direct_render_sites<'a>(
    lookup: &SemanticLookup<'a>,
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
) -> Vec<(&'a solid_facts::FileFacts, Span)> {
    // The binding holds this function where it is rendered (a `let`
    // reassigned before the render mounts another; ADR 0503 review).
    if !lookup.function_value_is_current(file, component) {
        return Vec::new();
    }
    lookup
        .function_call_sites(file.path.as_str(), component.span)
        .into_iter()
        .filter_map(|(caller, tag)| {
            let ast = &caller.ast;
            let element = ast
                .jsx_elements
                .iter()
                .find(|element| element.name.span == tag)?;
            if !element.self_closing
                || !element.attributes.is_empty()
                || !element.spreads.is_empty()
                || !element.children.is_empty()
            {
                return None;
            }
            let arrow = crate::owners::containing_ast_function(ast, element.span)?;
            if ast.peel_ts_sugar_span(arrow.body) != element.span {
                return None;
            }
            mount_call(lookup, caller, arrow).map(|call| (caller, call))
        })
        .collect()
}

/// `arrow` is the mount function of a module-scope `render`: argument 0 of a
/// call that runs whenever the module is evaluated, outside every function and
/// class, in the module's straight-line flow (no condition, loop, or exit
/// before it; ADR 0503 review).
fn mount_call(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    arrow: &solid_facts::ast::FunctionFact,
) -> Option<Span> {
    let ast = &file.ast;
    if arrow.r#async || arrow.generator {
        return None;
    }
    let (call, _) = ast.arguments_containing(arrow.span).find(|(call, index)| {
        *index == 0 && ast.peel_ts_sugar_span(call.arguments[*index].span) == arrow.span
    })?;
    let runs = ast.functions_owning(call.span).next().is_none()
        && !ast
            .classes
            .iter()
            .any(|class| class.span.contains(call.span))
        && ast.completion_sites.binary_search(&call.span).is_ok()
        && lookup
            .primitive_at_call(file, call.span)
            .is_some_and(|primitive| lookup.dialect.mount_runs_component_synchronously(primitive));
    runs.then_some(call.span)
}

/// ADR 0661: `component` is built while a module-scope `render` runs: a tag at a
/// synchronous position of the mount function, whatever holders surround it
/// (`render(() => <div><X /></div>)`). A provider, a fragment and an intrinsic
/// are holders, which [`direct_render_sites`]'s bare form does not take.
pub(super) fn mounted_by_root(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
) -> bool {
    lookup.function_value_is_current(file, component)
        && lookup
            .function_call_sites(file.path.as_str(), component.span)
            .into_iter()
            .any(|(caller, tag)| {
                let ast = &caller.ast;
                ast.jsx_elements
                    .iter()
                    .any(|element| element.name.span == tag)
                    && synchronous_tag_position(lookup, caller, tag).is_some_and(|position| {
                        crate::owners::containing_ast_function(ast, position)
                            .is_some_and(|arrow| mount_call(lookup, caller, arrow).is_some())
                    })
            })
}

/// Whether `function` is what a `render`/`hydrate` call mounts: the function
/// literal passed as its first argument, or a function it names.
pub(super) fn function_is_mount_root(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    file.ast.calls.iter().any(|call| {
        lookup
            .primitive_at_call(file, call.span)
            .is_some_and(|primitive| lookup.dialect.mounts_component_tree(primitive))
            && call.arguments.first().is_some_and(|argument| {
                let argument = file.ast.peel_ts_sugar_span(argument.span);
                argument == function.span
                    || lookup
                        .function_called_at(file.path.as_str(), argument)
                        .is_some_and(|(target_file, target)| {
                            target_file.path == file.path && target.span == function.span
                        })
            })
    })
}

/// Whether a compiler ref/directive application role at `span` belongs to an
/// intrinsic element. The runtime applies an intrinsic element's `ref` through
/// `ref()` with no owner; a component (a capitalized or member tag) receives
/// `ref` as an ordinary prop and invokes it whenever its own code does, often
/// in its owned body. By JSX semantics a lowercase, non-member tag is
/// intrinsic, so a callback written as an attribute value of any other element
/// is not a proven ownerless application.
pub(crate) fn ref_application_on_intrinsic(file: &solid_facts::FileFacts, span: Span) -> bool {
    !file.ast.jsx_elements.iter().any(|element| {
        is_component_element(file, element)
            && element.attributes.iter().any(|attribute| {
                attribute
                    .expression
                    .is_some_and(|value| file.ast.peel_ts_sugar_span(value).contains(span))
            })
    })
}

/// Whether a JSX element is a component, by the tag's form as the JSX transform
/// reads it: a member tag whatever its head's case (`<props.Comp>`), or any name
/// that does not start with a lowercase letter (`<Comp>`, `<_Comp>`). A plain
/// lowercase name and a namespaced or hyphenated one are intrinsic. Its children
/// and spreads are not the intrinsic element's own, so a ref written inside them
/// is not placed by the intrinsic element's owner.
pub(crate) fn is_component_element(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> bool {
    let tag = file
        .source
        .get(element.name.span.start as usize..element.name.span.end as usize)
        .unwrap_or_default();
    !tag.is_empty()
        && (tag.contains('.')
            || (!tag.contains([':', '-']) && !tag.starts_with(|c: char| c.is_ascii_lowercase())))
}

/// Whether `span` lies inside a component element, in its children, spreads
/// or attributes. A component's children run inside the component's own memo
/// (`<Show>`'s children, ADR 0345), so an owner read from the code around the
/// element says nothing about what runs in them.
pub(crate) fn inside_component_element(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.ast
        .jsx_elements
        .iter()
        .any(|element| element.span.contains(span) && is_component_element(file, element))
}

/// The role a pending async read plays for the missing-boundary claim: a
/// tracked read inside a non-render computation (a memo, a user effect's
/// compute, a derived signal) is not a render of the value, so it is not
/// [`ExecutionRole::TrackedJsx`] for that claim. The pending state flows on to
/// whichever render effect consumes the computation, and that consumer is the
/// one whose boundary matters.
pub(crate) fn async_read_role(
    file: &solid_facts::FileFacts,
    span: Span,
    role: crate::ExecutionRole,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> crate::ExecutionRole {
    if role != crate::ExecutionRole::TrackedJsx {
        return role;
    }
    // A function that is a JSX attribute's value (an event handler, a callback
    // prop) runs when something calls it, not while the JSX renders. Nothing
    // proves a read inside it is rendered, so it is not a render read. A
    // `children` attribute is the element's own children and keeps its role.
    if file.ast.functions_owning(span).any(|function| {
        file.ast.jsx_containing(function.span).any(|element| {
            element.attributes.iter().any(|attribute| {
                attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
                    && attribute
                        .expression
                        .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == function.span)
                    && file
                        .source
                        .get(attribute.name.start as usize..attribute.name.end as usize)
                        != Some("children")
            })
        })
    }) {
        return crate::ExecutionRole::Unknown;
    }
    if tracked_computation_read(file, span, entities, symbol_names, lookup)
        || handed_computation_read(file, span, lookup)
    {
        crate::ExecutionRole::Unknown
    } else {
        role
    }
}

/// ADR 0637: a read in a function a computation runs by a name or a member
/// (`createMemo(cb)`, `createEffect(bag.compute, …)`), through `const` aliases:
/// no render, as a literal compute's is not. Nothing here proves which of its
/// other uses runs it, so its async row is unproven. A leaf's callback keeps its
/// own proof.
fn handed_computation_read(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use solid_dialect::Primitive;
    let ast = &file.ast;
    let holds = |value: Span| {
        let mut at = ast.peel_ts_sugar_span(value);
        for _ in 0..4 {
            while let Some(member) = ast.member_at(at) {
                at = ast.peel_ts_sugar_span(member.object);
            }
            let Some(declaration) = ast.reference_declaration(at) else {
                return false;
            };
            if ast.functions.iter().any(|function| {
                function.span.contains(span)
                    && function
                        .name
                        .as_ref()
                        .is_some_and(|name| name.span == declaration)
            }) {
                return true;
            }
            let Some(initializer) = ast
                .bindings
                .iter()
                .find(|binding| binding.names.iter().any(|name| name.span == declaration))
                .and_then(|binding| binding.initializer)
            else {
                return false;
            };
            if initializer.contains(span) {
                return true;
            }
            at = ast.peel_ts_sugar_span(initializer);
        }
        false
    };
    !ast.any_jsx_containing(span)
        && ast.calls.iter().any(|call| {
            lookup
                .primitive_at_call(file, call.span)
                .is_some_and(|primitive| {
                    !matches!(
                        primitive,
                        Primitive::CreateTrackedEffect | Primitive::OnSettled
                    ) && !lookup.dialect.computation_read_is_render(primitive)
                })
                && call.arguments.iter().any(|argument| {
                    ast.function_at(ast.peel_ts_sugar_span(argument.span))
                        .is_none()
                        && holds(argument.span)
                })
        })
}

/// ADR 0623: a read in the tracked callback of a computation that is no render
/// (a memo's compute, a user effect's compute, a derived signal, an effect's
/// apply, or a cleanup it returns, when its role is tracked). A pending source
/// suspends such a read: it is no render, and no unsuspendable read either.
pub(crate) fn tracked_computation_read(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let dialect = lookup.dialect;
    file.ast.arguments_containing(span).any(|(call, index)| {
        let argument = &call.arguments[index];
        matches!(
            argument.value,
            solid_facts::ast::ArgumentValueKind::Identifier
                | solid_facts::ast::ArgumentValueKind::Function
                | solid_facts::ast::ArgumentValueKind::AsyncFunction
        ) && crate::call_primitive_name_cached(lookup, file, call, entities, symbol_names, dialect)
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| {
                // ADR 0614: an effect's apply, or a cleanup it returns, is no
                // render either, whichever JSX its effect is written in.
                // An inline callback runs in its caller's pass (`latest`,
                // `isPending`, Chrome rc.21: the boundary claim stands).
                callback_execution_at_call(file, call, primitive, index, lookup)
                    .is_some_and(|execution| execution != solid_dialect::Execution::Inline)
                    && !dialect.computation_read_is_render(primitive)
            })
            // A JSX region written inside the computation is a render again.
            && !file
                .compiler
                .tracked_regions
                .iter()
                .any(|region| argument.span.contains(region.span) && region.span.contains(span))
    })
}

/// ADR 0892: the read at `span` is written in the value of a prop that an
/// exactly resolved flow component reads only in a computation of its own
/// (`<Show when={a()}>`, `<For each={a()}>`, `<Match when={a()}>` as a direct
/// child of a `Switch`): a pending value suspends that computation, so no
/// strict-read window sees it. Review D20: no element or function may stand
/// between the read and the attribute (it evaluates elsewhere), no comment may
/// sit in the value (`/*@static*/` drops the getter), and no spread may stand
/// on the tag.
pub(crate) fn read_in_flow_prop(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> bool {
    let holding = |inner: Span| {
        file.ast
            .jsx_elements
            .iter()
            .filter(|element| element.span.contains(inner) && element.span != inner)
            .min_by_key(|element| element.span.end - element.span.start)
    };
    let Some(element) = holding(span) else {
        return false;
    };
    if !element.spreads.is_empty() {
        return false;
    }
    let Some(attribute) = element.attributes.iter().find(|attribute| {
        attribute.namespace.is_none()
            && attribute
                .expression
                .is_some_and(|value| value.contains(span))
    }) else {
        return false;
    };
    if file
        .source_text(attribute.span)
        .is_none_or(|text| text.contains("/*") || text.contains("//"))
    {
        return false;
    }
    let function = |at: Span| containing_ast_function(&file.ast, at).map(|function| function.span);
    if function(span) != function(element.span) {
        return false;
    }
    // Review R40: a call the read is an argument of may run it elsewhere:
    // `untrack(p, "x")` sets a strict window inside the computation.
    if file.ast.arguments_containing(span).any(|(call, _)| {
        attribute
            .expression
            .is_some_and(|value| value.contains(call.span))
    }) {
        return false;
    }
    let tag = |element: &solid_facts::ast::JsxElementFact| {
        jsx_primitive_name(file, element, entities, symbol_names, dialect)
    };
    let Some(name) = tag(element) else {
        return false;
    };
    let name: &str = &name;
    if !file
        .source_text(attribute.name)
        .is_some_and(|prop| dialect.flow_prop_read_in_own_computation(name, prop))
    {
        return false;
    }
    // `Match` returns its props: only `Switch` reads them in a computation.
    name != "Match"
        || holding(element.span).is_some_and(|parent| {
            parent.children.contains(&element.span)
                && tag(parent).is_some_and(|parent| &*parent == "Switch")
        })
}

/// ADR 0892: `Match` returns its props, so outside a `Switch` a read in its
/// `when` runs wherever that object is read: no tracked role speaks for it.
pub(crate) fn read_in_loose_match(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> bool {
    let Some(element) = file
        .ast
        .jsx_elements
        .iter()
        .filter(|element| element.span.contains(span) && element.span != span)
        .min_by_key(|element| element.span.end - element.span.start)
    else {
        return false;
    };
    element.attributes.iter().any(|attribute| {
        attribute
            .expression
            .is_some_and(|value| value.contains(span))
            && file.source_text(attribute.name) == Some("when")
    }) && jsx_primitive_name(file, element, entities, symbol_names, dialect).as_deref()
        == Some("Match")
        && !read_in_flow_prop(file, span, entities, symbol_names, dialect)
}

/// ADR 0892: a read in a flow prop warns from the render that reads the
/// component's result. An element that is no JSX child and no return of a
/// component may be dropped (`const f = () => <Show when={a()}>...`, `f()`):
/// Chrome rc.21 is silent there, so the boundary claim is not proved.
pub(crate) fn flow_prop_element_unrendered(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if !read_in_flow_prop(file, span, entities, symbol_names, lookup.dialect) {
        return false;
    }
    let Some(element) = file
        .ast
        .jsx_elements
        .iter()
        .filter(|element| element.span.contains(span) && element.span != span)
        .min_by_key(|element| element.span.end - element.span.start)
    else {
        return false;
    };
    let child = file
        .ast
        .jsx_elements
        .iter()
        .any(|parent| parent.children.contains(&element.span));
    let returned = containing_ast_function(&file.ast, element.span).is_some_and(|function| {
        lookup.function_may_be_component(file, function)
            && crate::local_access::returned_values(file, function)
                .iter()
                .flatten()
                .any(|value| file.ast.peel_ts_sugar_span(*value) == element.span)
    });
    !(child || returned)
}

pub(crate) fn jsx_element_is_loading(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> bool {
    jsx_primitive_name(file, element, entities, symbol_names, dialect)
        .as_deref()
        .is_some_and(|tag| dialect.is_async_boundary(tag))
}

pub(crate) fn jsx_target_function<'a>(
    lookup: &SemanticLookup<'a>,
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> Option<(
    &'a solid_facts::FileFacts,
    &'a solid_facts::ast::FunctionFact,
)> {
    lookup.function_called_at(file.path.as_str(), element.name.span)
}

pub(crate) fn computation_is_async(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    argument: Span,
) -> bool {
    // The computation is the argument's own function. An async function that
    // is merely nested inside it (`createMemo(() => async () => ...)`, an async
    // IIFE) is a value the computation returns or runs, not the computation:
    // the memo's result is a function and can never be pending.
    let argument = file.ast.peel_ts_sugar_span(argument);
    if lookup
        .typescript_file(file.path.as_str())
        .is_some_and(|typescript_file| {
            typescript_file.async_functions.iter().any(|function| {
                function.can_return_async
                    && u64::from(argument.start) == function.expression.start_byte
                    && function.expression.end_byte == u64::from(argument.end)
            })
        })
    {
        return true;
    }
    file.ast
        .function_at(argument)
        .is_some_and(|function| function.r#async)
}

/// ADR 0665: what a source's first argument says about a first flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComputationProvenance {
    /// A pass of the computation returns a Promise.
    Async,
    /// No pass does: a data value, or a function whose return is closed.
    Sync,
    /// Nothing proves either: an `any` return, a value no function fact holds,
    /// a compute that may answer without a flight, or one that may throw
    /// `NotReadyError` from a sync body.
    Open,
}

/// ADR 0665: [`computation_is_async`] is a proof of a Promise, and static rules
/// read it as one, so "not proven" is this function's third answer.
pub(crate) fn computation_provenance(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    argument: &solid_facts::ast::ArgumentFact,
) -> ComputationProvenance {
    use solid_facts::ast::{ArgumentValueKind, RuntimeValueKind};
    if argument.spread {
        return ComputationProvenance::Open;
    }
    let value = file.ast.peel_ts_sugar_span(argument.span);
    if computation_is_async(lookup, file, argument.span) {
        return if computation_may_answer_sync(lookup, file, argument.span) {
            ComputationProvenance::Open
        } else {
            ComputationProvenance::Async
        };
    }
    if argument.runtime_value_kind.is_data_literal()
        || matches!(
            argument.runtime_value_kind,
            RuntimeValueKind::Primitive | RuntimeValueKind::Nullish
        )
        || lookup
            .entity_at(file.path.as_str(), value)
            .and_then(|entity| entity.callability)
            == Some(typefacts::Callability::NonCallable)
    {
        return ComputationProvenance::Sync;
    }
    let function = match argument.value {
        ArgumentValueKind::Function => file.ast.function_at(value),
        ArgumentValueKind::Identifier => lookup
            .function_called_at(file.path.as_str(), value)
            .filter(|(target, _)| target.path == file.path)
            .map(|(_, function)| function),
        _ => None,
    };
    let Some(function) = function else {
        return ComputationProvenance::Open;
    };
    let closed = lookup
        .typescript_file(file.path.as_str())
        .is_some_and(|typescript_file| {
            typescript_file.async_functions.iter().any(|fact| {
                !fact.return_open
                    && !fact.can_return_async
                    && u64::from(function.span.start) == fact.expression.start_byte
                    && u64::from(function.span.end) == fact.expression.end_byte
            })
        });
    if closed && !may_throw_not_ready(lookup, file, function) {
        ComputationProvenance::Sync
    } else {
        ComputationProvenance::Open
    }
}

/// A reference inside `region` to the export `name` of a Solid package, through
/// a named import or a namespace's member.
fn solid_import_named(file: &solid_facts::FileFacts, name: &str, region: Span) -> bool {
    let bindings = || {
        file.ast
            .imports
            .iter()
            .filter(|import| {
                import.module == "solid-js"
                    || import.module.starts_with("solid-js/")
                    || import.module.starts_with("@solidjs/")
            })
            .flat_map(|import| import.bindings.iter())
    };
    let named = bindings()
        .filter(|binding| binding.imported.as_deref() == Some(name))
        .map(|binding| binding.local.span)
        .collect::<Vec<_>>();
    let namespaces = bindings()
        .filter(|binding| binding.kind == solid_facts::ast::ImportKind::Namespace)
        .map(|binding| binding.local.span)
        .collect::<Vec<_>>();
    if named.is_empty() && namespaces.is_empty() {
        return false;
    }
    let declared_by = |span: Span, spans: &[Span]| {
        file.ast
            .reference_declaration(span)
            .is_some_and(|declaration| spans.contains(&declaration))
    };
    file.ast
        .identifiers
        .iter()
        .any(|identifier| region.contains(identifier.span) && declared_by(identifier.span, &named))
        || file.ast.members.iter().any(|member| {
            region.contains(member.span)
                && file.source_text(member.property) == Some(name)
                && declared_by(file.ast.peel_ts_sugar_span(member.object), &namespaces)
        })
}

const NOT_READY_WALK_BUDGET: usize = 2048;

/// A sync compute that throws `NotReadyError` is pending while it does, and no
/// return type says so. Its own body, or a function it calls, names the export.
/// A helper no exact call names is beyond this walk; a project that names the
/// error in one is what the walk is for.
fn may_throw_not_ready(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    may_throw_not_ready_within(lookup, file, function, &mut HashSet::default())
}

/// Each function once: a cycle adds nothing, and past [`NOT_READY_WALK_BUDGET`]
/// a call may reach anything.
fn may_throw_not_ready_within(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    seen: &mut HashSet<(String, Span)>,
) -> bool {
    if !seen.insert((file.path.to_string(), function.span)) {
        return false;
    }
    if seen.len() > NOT_READY_WALK_BUDGET
        || solid_import_named(file, "NotReadyError", function.span)
    {
        return true;
    }
    file.ast.calls_within(function.span).any(|call| {
        lookup
            .function_called_at(file.path.as_str(), call.callee)
            .is_some_and(|(target, called)| {
                may_throw_not_ready_within(lookup, target, called, seen)
            })
    })
}

/// ADR 0631: the computation may return without a pending flight
/// (`cache.get(k) ?? load()`, a conditional, a custom thenable): nothing proves
/// its source is pending at a first read, or that it ever is.
pub(crate) fn computation_may_answer_sync(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    argument: Span,
) -> bool {
    let argument = file.ast.peel_ts_sugar_span(argument);
    lookup
        .typescript_file(file.path.as_str())
        .is_some_and(|typescript_file| {
            typescript_file.async_functions.iter().any(|function| {
                function.may_answer_sync
                    && u64::from(argument.start) == function.expression.start_byte
                    && function.expression.end_byte == u64::from(argument.end)
            })
        })
}

pub(crate) fn inside_non_component_function(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if file
        .compiler
        .callback_roles
        .iter()
        .any(|callback| callback.span.contains(span))
    {
        return false;
    }
    // An argument to a primitive that takes a callback is not "some helper
    // function's body", whatever the helper is called. Asked of the dialect
    // rather than listed: the seven names this had were 2.0's, three of which
    // 1.x does not have, and it was missing every 1.x callback-taker outside
    // the four it shares.
    if file.ast.arguments_containing(span).any(|(call, index)| {
        lookup
            .primitive_at_call(file, call.span)
            .is_some_and(|primitive| {
                lookup
                    .dialect
                    .callback_semantics_at(primitive, index, call.arguments.len())
                    .execution
                    .is_some()
            })
    }) {
        return false;
    }
    file.ast.functions_body_containing(span).any(|function| {
        function_binding_name(file, function).is_some()
            && !object_method_escapes(file, function, lookup)
            && !lookup.function_may_be_component(file, function)
    })
}

/// ADR 0320: whether `function` is an object-literal method whose literal
/// goes anywhere other than a member dispatch the analysis resolves: handed
/// to unknown code, enumerated, returned, stored. Such a method is a stored
/// callback like a function-expression property, not a named helper whose
/// reads are claimed where it is called: it may be called where nothing
/// claims them.
pub(crate) fn object_method_escapes(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(name) = own_method_name(file, function) else {
        return false;
    };
    // An object literal's own method; a class's is selected by its instance.
    if !file
        .ast
        .object_properties
        .iter()
        .any(|property| property.key == name.span && property.method)
    {
        return false;
    }
    let Some(literal) = file
        .ast
        .object_get_shapes
        .iter()
        .filter(|shape| shape.span.contains(name.span))
        .min_by_key(|shape| shape.span.end - shape.span.start)
    else {
        return true;
    };
    !object_literal_only_dispatched(file, literal.span, lookup)
}

/// Whether every way the object literal at `literal` is used is a member
/// call on it: it is the receiver of one, or a `const` in this module whose
/// every reference is, or an argument of an exact callee in this file that
/// only reads members of its parameter.
pub(super) fn object_literal_only_dispatched(
    file: &solid_facts::FileFacts,
    literal: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let called_member_of = |value: Span| {
        file.ast.members.iter().any(|member| {
            file.ast.peel_ts_sugar_span(member.object) == value
                && file
                    .ast
                    .computed_members
                    .binary_search(&member.span)
                    .is_err()
                && file
                    .ast
                    .calls_with_peeled_callee(member.span)
                    .next()
                    .is_some()
        })
    };
    let closed_argument = |value: Span| {
        file.ast
            .arguments_containing(value)
            .find(|(call, index)| file.ast.peel_ts_sugar_span(call.arguments[*index].span) == value)
            .is_some_and(|(call, index)| {
                let mut consulted = Vec::new();
                !call.arguments[index].spread
                    && lookup.parameter_only_member_accessed_consulting(
                        file,
                        call,
                        index,
                        &mut consulted,
                    )
                    && consulted
                        .iter()
                        .all(|(path, _)| path.as_str() == file.path.as_str())
            })
    };
    if called_member_of(literal) || closed_argument(literal) {
        return true;
    }
    let Some(binding) = file.ast.bindings.iter().find(|binding| {
        binding
            .initializer
            .is_some_and(|initializer| file.ast.peel_ts_sugar_span(initializer) == literal)
    }) else {
        return false;
    };
    let [name] = binding.names.as_slice() else {
        return false;
    };
    binding.immutable
        && binding.shape == solid_facts::ast::BindingShape::Identifier
        && crate::execution_role::file_is_module(file)
        && !crate::execution_role::flow_binding_exported(file, name.span)
        && file
            .ast
            .reference_declarations
            .iter()
            .filter(|(reference, declaration)| *declaration == name.span && *reference != name.span)
            .all(|(reference, _)| called_member_of(*reference) || closed_argument(*reference))
}

/// Whether `span` is inside a syntactically direct function argument that a
/// known primitive provably never invokes.
///
/// Two positive proofs qualify, and nothing else: the dialect declares the
/// position a stored value (Solid 1.x `createSignal(() => value)`), or the
/// position's modelled callback belongs to a returned lazy adapter that this
/// call never consumes (`mapArray` whose result is discarded). A primitive
/// with no callback model at all — 2.0 `children`, `onCleanup` — proves
/// nothing: those callbacks do run, so their reads must not be treated as
/// dormant. Requiring the argument itself to be a function keeps an IIFE
/// passed as a value from being mistaken for dormant code.
pub(crate) fn inside_known_value_function_argument(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    known_value_function_argument_call(file, span, lookup).is_some()
}

/// The call whose function argument [`inside_known_value_function_argument`]
/// found around `span`: the primitive that never invokes it. ADR 0933 names it
/// as the fact behind the silence.
pub(crate) fn known_value_function_argument_call(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    // Both proofs are dialect answers that Solid 2.0 leaves at their negative
    // default, so under 2.0 this question is provably always "no" and the
    // containment query below need not run at all.
    if !lookup.models_stored_function_arguments() && !lookup.models_returned_callbacks() {
        return None;
    }
    // Only the calls whose arguments actually contain `span` can answer. The
    // former whole-file scan asked every call in the file, once per call being
    // classified, which is quadratic in a file's call count.
    file.ast
        .arguments_containing(span)
        .find_map(|(call, argument_index)| {
            let primitive = lookup.primitive_at_call(file, call.span)?;
            let argument = &call.arguments[argument_index];
            (matches!(
                argument.value,
                solid_facts::ast::ArgumentValueKind::Function
                    | solid_facts::ast::ArgumentValueKind::AsyncFunction
            ) && ({
                let semantics = lookup.dialect.callback_semantics_at(
                    primitive,
                    argument_index,
                    call.arguments.len(),
                );
                semantics.stores_as_value
                    || (semantics.requires_return_invocation
                        && callback_execution_at_call(
                            file,
                            call,
                            primitive,
                            argument_index,
                            lookup,
                        )
                        .is_none())
            }))
            .then_some(call.span)
        })
}
