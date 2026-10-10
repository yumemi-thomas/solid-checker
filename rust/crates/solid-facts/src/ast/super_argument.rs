//! Where a module-level function runs when its only use is an argument of a
//! subclass's `super(…)` (ADR 0139 § 3): the syntax half. Which exports own
//! it is the backend's; what the base does with it is the base's accepted
//! contract's.

use crate::core::Span;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, BindingPattern, Class, ClassElement, ComputedMemberExpression, Declaration,
    Expression, IdentifierReference, ImportDeclarationSpecifier, ImportOrExportKind,
    MethodDefinitionKind, ModuleExportName, PrivateFieldExpression, Program, Statement,
    StaticMemberExpression, ThisExpression,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder};
use oxc_span::SourceType;
use oxc_syntax::reference::ReferenceId;
use oxc_syntax::symbol::SymbolId;
use std::collections::HashMap;
use std::path::Path;

/// One position of a function as argument `argument_index` of the top-level
/// `super(…)` statement of a module-level class's constructor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuperArgumentSite {
    pub argument_index: usize,
    /// The `super(…)` call.
    pub super_call: Span,
    /// The class's heritage, when it is a bare identifier naming a named
    /// import: the module specifier and the imported name.
    pub base: Option<(String, String)>,
    /// The class declares an element other than its constructor: a method a
    /// base constructor's `this.m(…)` would run in place of its own.
    pub other_members: bool,
    /// The constructor names a member of `this` or `super` outside the
    /// `super(…)` call: construction may invoke a base member.
    pub touches_instance: bool,
    /// A `this` in the class body is not the object of a member access, or the
    /// class nests a class: the instance may reach code that keeps it.
    pub escapes: bool,
}

/// The module-level function an obligation sits in (or whose one user is the
/// import binding it sits on), when every reference to it in this module is an
/// export specifier or a [`SuperArgumentSite`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuperArgumentFunction {
    /// The names this module's own `export { … }` publishes the function
    /// under; other modules may import it by them.
    pub published: Vec<String>,
    pub sites: Vec<SuperArgumentSite>,
}

/// See [`SuperArgumentFunction`]. `None` unless the module parses with no
/// error, has no semantic error and no `eval`; `location` is inside a
/// module-level function declaration or function-valued declarator, or is a
/// value import binding every reference of which is inside one; that binding
/// is declared once and never written; and every reference to the function is
/// an export specifier or a super-argument site.
#[must_use]
pub fn super_argument_function(
    path: &Path,
    source: &str,
    location: Span,
) -> Option<SuperArgumentFunction> {
    with_module(path, source, |program, scoping, sites, walk| {
        let functions = module_functions(program);
        let at = |span: oxc_span::Span| {
            functions
                .iter()
                .find(|(_, function)| function.start <= span.start && span.end <= function.end)
                .map(|(symbol, _)| *symbol)
        };
        let function = at(oxc_span::Span::new(location.start, location.end)).or_else(|| {
            // An import binding whose every use is inside one module-level
            // function.
            let binding = import_binding_at(program, location)?;
            let references = scoping.get_resolved_reference_ids(binding);
            let mut owner = None;
            for reference in references {
                let span = reference_span(walk, *reference)?;
                let found = at(span)?;
                if owner.is_some_and(|owner| owner != found) {
                    return None;
                }
                owner = Some(found);
            }
            owner
        })?;
        if !scoping.symbol_redeclarations(function).is_empty() {
            return None;
        }
        let publications = module_publications(program);
        let mut published = Vec::new();
        let mut found = Vec::new();
        for reference in scoping.get_resolved_reference_ids(function) {
            if scoping.get_reference(*reference).is_write() {
                return None;
            }
            if let Some(name) = publications.get(reference) {
                published.push(name.clone());
                continue;
            }
            found.push(sites.get(reference)?.clone());
        }
        published.sort();
        published.dedup();
        Some(SuperArgumentFunction {
            published,
            sites: found,
        })
    })
}

/// Every reference of the value import binding at `binding` in this module,
/// when each is a [`SuperArgumentSite`] -- or `None`.
#[must_use]
pub fn super_argument_sites_of_binding(
    path: &Path,
    source: &str,
    binding: Span,
) -> Option<Vec<SuperArgumentSite>> {
    with_module(path, source, |program, scoping, sites, _| {
        let symbol = import_binding_at(program, binding)?;
        let references = scoping.get_resolved_reference_ids(symbol);
        if references.is_empty() {
            return None;
        }
        references
            .iter()
            .map(|reference| sites.get(reference).cloned())
            .collect()
    })
}

fn with_module<T>(
    path: &Path,
    source: &str,
    answer: impl FnOnce(
        &Program<'_>,
        &Scoping,
        &HashMap<ReferenceId, SuperArgumentSite>,
        &ModuleWalk,
    ) -> Option<T>,
) -> Option<T> {
    if source.len() > 4 * 1024 * 1024 {
        return None;
    }
    let source_type = SourceType::from_path(path).ok()?.with_module(true);
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let built = SemanticBuilder::new().build(&parsed.program);
    if !built.errors.is_empty() {
        return None;
    }
    let scoping = built.semantic.scoping();
    let mut walk = ModuleWalk::default();
    walk.visit_program(&parsed.program);
    if walk.eval {
        return None;
    }
    let imports = named_imports(&parsed.program);
    let mut sites = HashMap::new();
    for class in module_classes(&parsed.program) {
        let Some(constructor) = class.body.body.iter().find_map(|element| match element {
            ClassElement::MethodDefinition(method)
                if method.kind == MethodDefinitionKind::Constructor =>
            {
                Some(&method.value)
            }
            _ => None,
        }) else {
            continue;
        };
        let Some(body) = &constructor.body else {
            continue;
        };
        let base = match class
            .super_class
            .as_ref()
            .map(Expression::get_inner_expression)
        {
            Some(Expression::Identifier(identifier)) => identifier
                .reference_id
                .get()
                .and_then(|reference| scoping.get_reference(reference).symbol_id())
                .and_then(|symbol| imports.get(&symbol).cloned()),
            _ => None,
        };
        let other_members = class.body.body.iter().any(|element| {
            !matches!(
                element,
                ClassElement::MethodDefinition(method)
                    if method.kind == MethodDefinitionKind::Constructor
            )
        });
        let mut instance = InstanceWalk::default();
        instance.visit_class(class);
        let escapes = instance.nested_classes > 0
            || instance
                .this_sites
                .iter()
                .any(|start| !instance.member_this.contains(start));
        for statement in &body.statements {
            let Statement::ExpressionStatement(statement) = statement else {
                continue;
            };
            let Expression::CallExpression(call) = statement.expression.get_inner_expression()
            else {
                continue;
            };
            if !matches!(call.callee, Expression::Super(_)) {
                continue;
            }
            let touches_instance = instance.members.iter().any(|member| {
                constructor.span.start <= member.start
                    && member.end <= constructor.span.end
                    && !(call.span.start <= member.start && member.end <= call.span.end)
            });
            for (index, argument) in call.arguments.iter().enumerate() {
                let Argument::Identifier(identifier) = argument else {
                    continue;
                };
                let Some(reference) = identifier.reference_id.get() else {
                    continue;
                };
                sites.insert(
                    reference,
                    SuperArgumentSite {
                        argument_index: index,
                        super_call: Span {
                            start: call.span.start,
                            end: call.span.end,
                        },
                        base: base.clone(),
                        other_members,
                        touches_instance,
                        escapes,
                    },
                );
            }
        }
    }
    answer(&parsed.program, scoping, &sites, &walk)
}

fn reference_span(walk: &ModuleWalk, reference: ReferenceId) -> Option<oxc_span::Span> {
    walk.references.get(&reference).copied()
}

/// Module-level function bindings and the spans of the functions they bind.
fn function_declarator(
    id: &BindingPattern<'_>,
    init: Option<&Expression<'_>>,
) -> Option<(SymbolId, oxc_span::Span)> {
    let BindingPattern::BindingIdentifier(identifier) = id else {
        return None;
    };
    let span = match init.map(Expression::get_inner_expression) {
        Some(Expression::ArrowFunctionExpression(arrow)) => arrow.span,
        Some(Expression::FunctionExpression(function)) => function.span,
        _ => return None,
    };
    Some((identifier.symbol_id.get()?, span))
}

fn module_functions(program: &Program<'_>) -> Vec<(SymbolId, oxc_span::Span)> {
    let mut functions = Vec::new();
    for statement in &program.body {
        match statement {
            Statement::FunctionDeclaration(function) => {
                if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
                    functions.push((symbol, function.span));
                }
            }
            Statement::VariableDeclaration(declaration) => {
                functions.extend(
                    declaration
                        .declarations
                        .iter()
                        .filter_map(|item| function_declarator(&item.id, item.init.as_ref())),
                );
            }
            Statement::ExportNamedDeclaration(export) => match &export.declaration {
                Some(Declaration::FunctionDeclaration(function)) => {
                    if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
                        functions.push((symbol, function.span));
                    }
                }
                Some(Declaration::VariableDeclaration(declaration)) => {
                    functions.extend(
                        declaration
                            .declarations
                            .iter()
                            .filter_map(|item| function_declarator(&item.id, item.init.as_ref())),
                    );
                }
                _ => {}
            },
            _ => {}
        }
    }
    functions
}

/// `export { local as name }` without `from`: reference id -> exported name.
fn module_publications(program: &Program<'_>) -> HashMap<ReferenceId, String> {
    let mut publications = HashMap::new();
    for statement in &program.body {
        let Statement::ExportNamedDeclaration(export) = statement else {
            continue;
        };
        if export.export_kind == ImportOrExportKind::Type || export.source.is_some() {
            continue;
        }
        for specifier in &export.specifiers {
            if specifier.export_kind == ImportOrExportKind::Type {
                continue;
            }
            if let ModuleExportName::IdentifierReference(local) = &specifier.local
                && let Some(reference) = local.reference_id.get()
            {
                publications.insert(reference, specifier.exported.name().to_string());
            }
        }
    }
    publications
}

/// Named value imports: binding symbol -> (specifier, imported name).
fn named_imports(program: &Program<'_>) -> HashMap<SymbolId, (String, String)> {
    let mut imports = HashMap::new();
    for statement in &program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        if import.import_kind == ImportOrExportKind::Type {
            continue;
        }
        for specifier in import.specifiers.iter().flatten() {
            if let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier
                && specifier.import_kind != ImportOrExportKind::Type
                && let Some(symbol) = specifier.local.symbol_id.get()
            {
                imports.insert(
                    symbol,
                    (
                        import.source.value.to_string(),
                        specifier.imported.name().to_string(),
                    ),
                );
            }
        }
    }
    imports
}

/// The value import binding whose local identifier is exactly `location`.
fn import_binding_at(program: &Program<'_>, location: Span) -> Option<SymbolId> {
    for statement in &program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        if import.import_kind == ImportOrExportKind::Type {
            continue;
        }
        for specifier in import.specifiers.iter().flatten() {
            let local = match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    if specifier.import_kind == ImportOrExportKind::Type {
                        continue;
                    }
                    &specifier.local
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => &specifier.local,
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => &specifier.local,
            };
            if local.span.start == location.start && local.span.end == location.end {
                return local.symbol_id.get();
            }
        }
    }
    None
}

fn class_initializer<'c, 'a>(init: Option<&'c Expression<'a>>) -> Option<&'c Class<'a>> {
    match init.map(Expression::get_inner_expression) {
        Some(Expression::ClassExpression(class)) => Some(class),
        _ => None,
    }
}

fn module_classes<'c, 'a>(program: &'c Program<'a>) -> Vec<&'c Class<'a>> {
    let mut classes = Vec::new();
    for statement in &program.body {
        match statement {
            Statement::ClassDeclaration(class) => classes.push(&**class),
            Statement::VariableDeclaration(declaration) => classes.extend(
                declaration
                    .declarations
                    .iter()
                    .filter_map(|item| class_initializer(item.init.as_ref())),
            ),
            Statement::ExportNamedDeclaration(export) => match &export.declaration {
                Some(Declaration::ClassDeclaration(class)) => classes.push(&**class),
                Some(Declaration::VariableDeclaration(declaration)) => classes.extend(
                    declaration
                        .declarations
                        .iter()
                        .filter_map(|item| class_initializer(item.init.as_ref())),
                ),
                _ => {}
            },
            _ => {}
        }
    }
    classes
}

/// Every identifier reference's span, and whether the module references
/// `eval`.
#[derive(Default)]
struct ModuleWalk {
    eval: bool,
    references: HashMap<ReferenceId, oxc_span::Span>,
}

impl<'a> Visit<'a> for ModuleWalk {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.eval |= identifier.name == "eval";
        if let Some(reference) = identifier.reference_id.get() {
            self.references.insert(reference, identifier.span);
        }
        walk::walk_identifier_reference(self, identifier);
    }
}

/// `this` and member accesses of `this`/`super` in one class.
#[derive(Default)]
struct InstanceWalk {
    depth: usize,
    nested_classes: usize,
    this_sites: Vec<u32>,
    member_this: std::collections::HashSet<u32>,
    members: Vec<oxc_span::Span>,
}

impl<'a> Visit<'a> for InstanceWalk {
    fn visit_class(&mut self, class: &Class<'a>) {
        if self.depth > 0 {
            self.nested_classes += 1;
        }
        self.depth += 1;
        walk::walk_class(self, class);
        self.depth -= 1;
    }

    fn visit_this_expression(&mut self, this: &ThisExpression) {
        self.this_sites.push(this.span.start);
    }

    fn visit_static_member_expression(&mut self, expression: &StaticMemberExpression<'a>) {
        if let Expression::ThisExpression(this) = &expression.object {
            self.member_this.insert(this.span.start);
        }
        if matches!(
            expression.object,
            Expression::ThisExpression(_) | Expression::Super(_)
        ) {
            self.members.push(expression.span);
        }
        walk::walk_static_member_expression(self, expression);
    }

    fn visit_computed_member_expression(&mut self, expression: &ComputedMemberExpression<'a>) {
        if let Expression::ThisExpression(this) = &expression.object {
            self.member_this.insert(this.span.start);
        }
        if matches!(
            expression.object,
            Expression::ThisExpression(_) | Expression::Super(_)
        ) {
            self.members.push(expression.span);
        }
        walk::walk_computed_member_expression(self, expression);
    }

    fn visit_private_field_expression(&mut self, expression: &PrivateFieldExpression<'a>) {
        if let Expression::ThisExpression(this) = &expression.object {
            self.member_this.insert(this.span.start);
        }
        self.members.push(expression.span);
        walk::walk_private_field_expression(self, expression);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(source: &str, needle: &str) -> Span {
        let start = u32::try_from(source.find(needle).unwrap()).unwrap();
        Span {
            start,
            end: start + u32::try_from(needle.len()).unwrap(),
        }
    }

    // The shape of `@tanstack/solid-router@2.0.0-rc.8`'s `routerStores.js`
    // and `router.js`, joined into one module and split into two.
    const STORES: &str = "import { createMutable } from \"core\";\n\
var getStoreFactory = (opts) => {\n\treturn { createMutableStore: createMutable };\n};\n\
export { getStoreFactory };\n";
    const ROUTER: &str = "import { getStoreFactory } from \"./routerStores.js\";\n\
import { RouterCore } from \"core\";\n\
var createRouter = (options) => new Router(options);\n\
var Router = class extends RouterCore {\n\tconstructor(options) {\n\t\tsuper(options, getStoreFactory);\n\t}\n};\n\
export { Router, createRouter };\n";

    #[test]
    fn a_function_used_only_as_a_super_argument_names_its_sites() {
        // In the module that declares it, through its import binding.
        let found = super_argument_function(
            Path::new("routerStores.js"),
            STORES,
            at(STORES, "createMutable"),
        )
        .unwrap();
        assert_eq!(found.published, vec!["getStoreFactory".to_string()]);
        assert!(found.sites.is_empty());
        let sites = super_argument_sites_of_binding(
            Path::new("router.js"),
            ROUTER,
            at(ROUTER, "getStoreFactory"),
        )
        .unwrap();
        let [site] = sites.as_slice() else {
            panic!("{sites:?}");
        };
        assert_eq!(site.argument_index, 1);
        assert_eq!(site.base, Some(("core".into(), "RouterCore".into())));
        assert!(!site.other_members && !site.touches_instance && !site.escapes);
        // Joined in one module, the function is found by the obligation inside
        // it.
        let joined = "import { RouterCore } from \"core\";\n\
var getStoreFactory = (opts) => ({ opts });\n\
var Router = class extends RouterCore {\n\tconstructor(options) {\n\t\tsuper(options, getStoreFactory);\n\t}\n};\n\
export { Router };\n";
        let found = super_argument_function(Path::new("router.js"), joined, at(joined, "{ opts }"))
            .unwrap();
        assert!(found.published.is_empty());
        assert_eq!(found.sites.len(), 1);
    }

    #[test]
    fn the_constructor_around_the_site_is_described() {
        for (replacement, touches, escapes, other) in [
            (
                "super(options, getStoreFactory);\n\t\tprimeRouterFromRegistry(this);",
                false,
                true,
                false,
            ),
            (
                "super(options, getStoreFactory);\n\t\tif (!this.isServer) {}",
                true,
                false,
                false,
            ),
            (
                "super(options, getStoreFactory);\n\t}\n\tupdate() {",
                false,
                false,
                true,
            ),
        ] {
            let source = ROUTER.replace("super(options, getStoreFactory);", replacement);
            let sites = super_argument_sites_of_binding(
                Path::new("router.js"),
                &source,
                at(&source, "getStoreFactory"),
            )
            .unwrap();
            assert_eq!(
                (
                    sites[0].touches_instance,
                    sites[0].escapes,
                    sites[0].other_members
                ),
                (touches, escapes, other),
                "{source}"
            );
        }
    }

    #[test]
    fn any_other_use_answers_nothing() {
        for source in [
            // Called as well.
            ROUTER.replace(
                "export { Router, createRouter };",
                "getStoreFactory({});\nexport { Router, createRouter };",
            ),
            // Handed to something other than `super`.
            ROUTER.replace(
                "super(options, getStoreFactory);",
                "super(options, wrap(getStoreFactory));",
            ),
            // Not a top-level statement of the constructor.
            ROUTER.replace(
                "super(options, getStoreFactory);",
                "if (options) super(options, getStoreFactory);",
            ),
        ] {
            assert_eq!(
                super_argument_sites_of_binding(
                    Path::new("router.js"),
                    &source,
                    at(&source, "getStoreFactory")
                ),
                None,
                "{source}"
            );
        }
        // A binding used outside the function is not the function's.
        let stores = STORES.replace(
            "export { getStoreFactory };",
            "export const direct = createMutable;\nexport { getStoreFactory };",
        );
        assert_eq!(
            super_argument_function(
                Path::new("routerStores.js"),
                &stores,
                at(&stores, "createMutable")
            ),
            None
        );
    }
}
