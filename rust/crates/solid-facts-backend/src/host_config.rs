//! Non-evaluating, closed application-config grammar. Plugin names identify
//! audited export bindings; installed byte identity is checked separately.

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, BindingPattern, ExportDefaultDeclarationKind, Expression,
    ImportDeclarationSpecifier, ImportOrExportKind, ModuleExportName, ObjectExpression,
    ObjectPropertyKind, PropertyKey, PropertyKind, Statement, VariableDeclarationKind,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EntryMode {
    Html,
    ClientStart,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Config {
    pub mode: EntryMode,
    pub plugins: BTreeSet<String>,
    pub routes: Option<String>,
    pub tailwind: bool,
    pub aliases: BTreeMap<String, Option<String>>,
    /// Closed config with Vite's ordinary extension probing. No plugin answer
    /// or tsconfig-path resolver is substituted for the Type Facts join.
    pub default_extensions: bool,
    /// Either resolver may discover configs other than the analyzed project.
    pub tsconfig_paths: bool,
    /// Package deduplication changes importer-relative lookup; default package
    /// load proofs are withheld until that resolver branch is implemented.
    pub package_dedupe: bool,
    /// Excluded CommonJS dependencies cannot use the optimizeDeps premise.
    pub optimize_exclude: BTreeSet<String>,
    /// A lazy route manifest has no eager picked route-config dependencies.
    pub lazy_routes: bool,
}

struct Bindings<'a> {
    plugins: BTreeMap<&'a str, &'a str>,
    data: BTreeSet<&'a str>,
    paths: BTreeSet<&'a str>,
    define: Option<(&'a str, &'a str)>,
    directory: &'a Path,
    path_modules: BTreeSet<&'a str>,
    resolve_functions: BTreeSet<&'a str>,
    refusal: &'a RefCell<ConfigRefusal>,
}

#[derive(Debug)]
pub(super) struct ConfigRefusal {
    pub offset: usize,
    pub reason: String,
}

impl Bindings<'_> {
    fn refusing(&self, offset: u32, reason: impl Into<String>) {
        *self.refusal.borrow_mut() = ConfigRefusal {
            offset: offset as usize,
            reason: reason.into(),
        };
    }
}

fn alias_literal(value: &Expression<'_>, bindings: &Bindings<'_>) -> Option<String> {
    if let Expression::StringLiteral(value) = value.get_inner_expression() {
        return Some(value.value.to_string());
    }
    let Expression::CallExpression(call) = value.get_inner_expression() else {
        return None;
    };
    let resolves = match call.callee.get_inner_expression() {
        Expression::Identifier(id) => bindings.resolve_functions.contains(id.name.as_str()),
        Expression::StaticMemberExpression(member) => {
            !member.optional
                && member.property.name == "resolve"
                && matches!(member.object.get_inner_expression(), Expression::Identifier(id) if bindings.path_modules.contains(id.name.as_str()))
        }
        _ => false,
    };
    let [Argument::Identifier(base), Argument::StringLiteral(target)] = call.arguments.as_slice()
    else {
        return None;
    };
    if !resolves || call.optional || base.name != "__dirname" {
        return None;
    }
    bindings
        .directory
        .join(target.value.as_str())
        .to_str()
        .map(str::to_owned)
}

fn properties<'a, 'b>(
    object: &'b ObjectExpression<'a>,
) -> Option<BTreeMap<&'b str, &'b Expression<'a>>> {
    let mut fields = BTreeMap::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            let ObjectPropertyKind::SpreadProperty(spread) = property else {
                return None;
            };
            let Expression::ObjectExpression(object) = spread.argument.get_inner_expression()
            else {
                return None;
            };
            for (key, value) in properties(object)? {
                if fields.insert(key, value).is_some() {
                    return None;
                }
            }
            continue;
        };
        let key = match &property.key {
            PropertyKey::StaticIdentifier(key) => key.name.as_str(),
            PropertyKey::StringLiteral(key) => key.value.as_str(),
            _ => return None,
        };
        if property.computed
            || property.method
            || property.shorthand
            || property.kind != PropertyKind::Init
            || fields.insert(key, &property.value).is_some()
        {
            return None;
        }
    }
    Some(fields)
}

fn bool_value(value: &Expression<'_>, expected: bool) -> bool {
    matches!(value.get_inner_expression(), Expression::BooleanLiteral(value) if value.value == expected)
}

// Only inert literal data: no callbacks, getters, computed keys, spreads or
// imported option objects can run a server-side app importer during config.
fn literal(value: &Expression<'_>) -> bool {
    match value.get_inner_expression() {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_) => true,
        Expression::ArrayExpression(array) => array
            .elements
            .iter()
            .all(|element| element.as_expression().is_some_and(literal)),
        Expression::ObjectExpression(object) => {
            properties(object).is_some_and(|fields| fields.values().all(|value| literal(value)))
        }
        _ => false,
    }
}

// Scalar environment reads and exact Node path helpers are permitted in
// non-host configuration. They never establish a target or resolve an alias.
fn data(value: &Expression<'_>, bindings: &Bindings<'_>) -> bool {
    if literal(value) {
        return true;
    }
    match value.get_inner_expression() {
        Expression::Identifier(id) => {
            bindings.data.contains(id.name.as_str())
                || ["process", "__dirname", "undefined"].contains(&id.name.as_str())
        }
        Expression::StaticMemberExpression(member) => {
            !member.optional && data(&member.object, bindings)
        }
        Expression::MetaProperty(meta) => {
            meta.meta.name == "import" && meta.property.name == "meta"
        }
        Expression::BinaryExpression(binary) => {
            data(&binary.left, bindings) && data(&binary.right, bindings)
        }
        Expression::LogicalExpression(binary) => {
            data(&binary.left, bindings) && data(&binary.right, bindings)
        }
        Expression::ConditionalExpression(branch) => {
            data(&branch.test, bindings)
                && data(&branch.consequent, bindings)
                && data(&branch.alternate, bindings)
        }
        Expression::UnaryExpression(unary) => data(&unary.argument, bindings),
        Expression::CallExpression(call) => {
            let allowed = match call.callee.get_inner_expression() {
                Expression::Identifier(id) => bindings.paths.contains(id.name.as_str()),
                Expression::StaticMemberExpression(member) => {
                    matches!(
                        member.object.get_inner_expression(), Expression::Identifier(id)
                        if bindings.paths.contains(id.name.as_str())
                    ) && ["resolve", "join", "dirname"].contains(&member.property.name.as_str())
                }
                _ => false,
            };
            allowed
                && !call.optional
                && call.arguments.iter().all(|arg| {
                    arg.as_expression()
                        .is_some_and(|value| data(value, bindings))
                })
        }
        Expression::NewExpression(call) => {
            matches!(call.callee.get_inner_expression(),
            Expression::Identifier(id) if id.name == "URL")
                && call.arguments.iter().all(|arg| {
                    arg.as_expression()
                        .is_some_and(|value| data(value, bindings))
                })
        }
        Expression::ObjectExpression(object) => properties(object)
            .is_some_and(|fields| fields.values().all(|value| data(value, bindings))),
        Expression::ArrayExpression(array) => array.elements.iter().all(|element| {
            element
                .as_expression()
                .is_some_and(|value| data(value, bindings))
        }),
        _ => false,
    }
}

fn plugin(
    call: &oxc_ast::ast::CallExpression<'_>,
    bindings: &Bindings<'_>,
    config: &mut Config,
) -> Option<()> {
    bindings.refusing(
        call.span.start,
        "plugin must be an exact imported callable with inert literal options",
    );
    let Expression::Identifier(callee) = call.callee.get_inner_expression() else {
        return None;
    };
    let name = *bindings.plugins.get(callee.name.as_str())?;
    bindings.refusing(
        call.span.start,
        format!("unsupported host-sensitive options for {name}"),
    );
    if call.optional || call.type_arguments.is_some() {
        return None;
    }
    let fields = match call.arguments.as_slice() {
        [] => BTreeMap::new(),
        [Argument::ObjectExpression(object)] => properties(object)?,
        _ => return None,
    };
    if !fields.values().all(|value| literal(value)) {
        return None;
    }
    match name {
        "@solidjs/vite-plugin" | "vite-plugin-solid" => {
            if config.plugins.iter().any(|name| ["@solidjs/vite-plugin", "vite-plugin-solid"].contains(&name.as_str()))
                || fields.keys().any(|key| !["start", "ssr", "serverFunctions", "extensions", "diagnostics"].contains(key))
                || ["ssr", "serverFunctions"].iter().any(|key|
                    fields.get(key).is_some_and(|value| !bool_value(value, false)))
            { return None; }
            if let Some(start) = fields.get("start") {
                if bool_value(start, true) {
                    config.mode = EntryMode::ClientStart;
                } else if let Expression::ObjectExpression(options) = start.get_inner_expression() {
                    let options = properties(options)?;
                    // Root overrides, middleware, document/render selectors,
                    // transforms and server features are outside this audit.
                    if options.keys().any(|key| *key != "devtools")
                        || options.get("devtools").is_some_and(|value| !bool_value(value, false))
                    { return None; }
                    config.mode = EntryMode::ClientStart;
                } else if !bool_value(start, false) { return None; }
            }
        }
        "filesystem-routing/vite" => {
            if config.routes.is_some() || fields.keys().any(|key|
                !["dir", "types", "codeSplitting", "httpMethods"].contains(key))
            { return None; }
            let dir = match fields.get("dir") {
                None => "src/routes",
                Some(Expression::StringLiteral(value)) => value.value.as_str(),
                _ => return None,
            };
            if dir.is_empty() || Path::new(dir).is_absolute()
                || Path::new(dir).components().any(|part| !matches!(part, std::path::Component::Normal(_)))
            { return None; }
            config.routes = Some(dir.into());
            config.lazy_routes = fields.get("codeSplitting").is_none_or(|value| bool_value(value, true));
        }
        "@tailwindcss/vite" => {
            if fields.keys().any(|key| *key != "optimize") { return None; }
            config.tailwind = true;
        }
        "unocss/vite" | "@unocss/vite" => {
            // Default Uno config loading can execute arbitrary app imports.
            if fields.len() != 1 || !bool_value(fields.get("configFile")?, false) { return None; }
        }
        "vite-plugin-pwa" => {
            if fields.keys().any(|key| !["strategies", "registerType", "injectRegister", "manifest", "disable"].contains(key))
                || fields.get("strategies").is_some_and(|value|
                    !matches!(value, Expression::StringLiteral(value) if value.value == "generateSW"))
            { return None; }
        }
        "vite-plugin-solid-svg" => {
            // svgo.loadConfig() executes JS unless optimization is disabled.
            if fields.keys().any(|key| !["svgo", "defaultAsComponent", "compilerOptions"].contains(key)) { return None; }
            let Expression::ObjectExpression(svgo) = *fields.get("svgo")? else { return None; };
            let svgo = properties(svgo)?;
            if svgo.len() != 1 || !bool_value(svgo.get("enabled")?, false) { return None; }
        }
        "unplugin-icons/vite" => {
            // @iconify/utils falls back from absent icons.json to importing
            // the collection's executable entry. Options cannot disable it.
            return None;
        }
        "vite-tsconfig-paths" => {
            // Only default discovery is reviewed. Non-default project scope
            // and parseNative/compiler execution remain outside this grammar.
            if !fields.is_empty() { return None; }
        }
        "@tanstack/router-plugin/vite" => {
            if fields.keys().any(|key| !["target", "autoCodeSplitting"].contains(key))
                || !matches!(fields.get("target"), Some(Expression::StringLiteral(value)) if value.value == "solid")
            { return None; }
        }
        _ => return None,
    }
    if !config.plugins.insert(name.into()) {
        return None;
    }
    Some(())
}

fn object(object: &ObjectExpression<'_>, bindings: &Bindings<'_>) -> Option<Config> {
    bindings.refusing(
        object.span.start,
        "computed, duplicate, accessor, or nonliteral Vite config properties",
    );
    let fields = properties(object)?;
    let mut config = Config {
        mode: EntryMode::Html,
        plugins: BTreeSet::new(),
        routes: None,
        tailwind: false,
        aliases: BTreeMap::new(),
        default_extensions: true,
        tsconfig_paths: false,
        package_dedupe: false,
        optimize_exclude: BTreeSet::new(),
        lazy_routes: false,
    };
    for (key, value) in fields {
        bindings.refusing(
            value.span().start,
            format!("unsupported Vite config field or value: {key}"),
        );
        if key == "plugins" {
            let Expression::ArrayExpression(plugins) = value.get_inner_expression() else {
                return None;
            };
            for element in &plugins.elements {
                let ArrayExpressionElement::CallExpression(call) = element else {
                    return None;
                };
                plugin(call, bindings, &mut config)?;
            }
        } else {
            // Closed fields, with a closed nested host-sensitive grammar.
            let allowed: &[&str] = match key {
                "server" | "preview" => &[
                    "port",
                    "strictPort",
                    "host",
                    "proxy",
                    "headers",
                    "allowedHosts",
                    "open",
                    "cors",
                ],
                "build" => &[
                    "target",
                    "outDir",
                    "assetsDir",
                    "emptyOutDir",
                    "assetsInlineLimit",
                    "chunkSizeWarningLimit",
                    "sourcemap",
                    "minify",
                    "cssMinify",
                    "manifest",
                ],
                // Type Facts honors tsconfig paths. A Vite-only alias can
                // redirect a target after that attestation, so it needs a
                // separate congruence proof before admission.
                "resolve" => &["dedupe", "tsconfigPaths", "alias"],
                "optimizeDeps" => &["include", "exclude"],
                // Vitest metadata supplies no application runtime entries.
                "test" => &[
                    "environment",
                    "globals",
                    "setupFiles",
                    "isolate",
                    "include",
                    "exclude",
                ],
                "base" | "publicDir" | "envPrefix" => {
                    if !data(value, bindings) {
                        return None;
                    }
                    continue;
                }
                _ => return None,
            };
            let Expression::ObjectExpression(options) = value.get_inner_expression() else {
                return None;
            };
            let options = properties(options)?;
            if key == "resolve"
                && let Some(value) = options.get("dedupe")
            {
                let Expression::ArrayExpression(values) = value.get_inner_expression() else {
                    return None;
                };
                if !values.elements.iter().all(|value| {
                    matches!(value.as_expression(), Some(Expression::StringLiteral(_)))
                }) {
                    return None;
                }
                config.package_dedupe = !values.elements.is_empty();
            }
            if key == "optimizeDeps"
                && let Some(value) = options.get("exclude")
            {
                let Expression::ArrayExpression(values) = value.get_inner_expression() else {
                    return None;
                };
                for value in &values.elements {
                    let Some(Expression::StringLiteral(value)) = value.as_expression() else {
                        return None;
                    };
                    config.optimize_exclude.insert(value.value.to_string());
                }
            }
            if key == "resolve"
                && options
                    .get("tsconfigPaths")
                    .is_some_and(|value| !bool_value(value, false))
            {
                config.default_extensions = false;
                config.tsconfig_paths = true;
            }
            if key == "resolve"
                && let Some(value) = options.get("alias")
            {
                let Expression::ObjectExpression(aliases) = value.get_inner_expression() else {
                    return None;
                };
                for (find, replacement) in properties(aliases)? {
                    if find.is_empty() {
                        return None;
                    }
                    if !data(replacement, bindings) {
                        return None;
                    }
                    let replacement = alias_literal(replacement, bindings);
                    config.aliases.insert(find.into(), replacement);
                }
            }
            if options.keys().any(|key| !allowed.contains(key))
                || options.values().any(|value| !data(value, bindings))
            {
                return None;
            }
        }
    }
    Some(config)
}

fn expression(value: &Expression<'_>, bindings: &Bindings<'_>) -> Option<Config> {
    match value.get_inner_expression() {
        Expression::ObjectExpression(value) => object(value, bindings),
        Expression::ConditionalExpression(branch) if data(&branch.test, bindings) => {
            let mut left = expression(&branch.consequent, bindings)?;
            let right = expression(&branch.alternate, bindings)?;
            if left.mode != right.mode
                || left.routes != right.routes
                || left.aliases != right.aliases
                || left.lazy_routes != right.lazy_routes
            {
                return None;
            }
            left.plugins.extend(right.plugins);
            left.tailwind |= right.tailwind;
            left.default_extensions &= right.default_extensions;
            left.tsconfig_paths |= right.tsconfig_paths;
            left.package_dedupe |= right.package_dedupe;
            left.optimize_exclude.extend(right.optimize_exclude);
            Some(left)
        }
        _ => None,
    }
}

fn statements(body: &[Statement<'_>], bindings: &Bindings<'_>) -> Option<Config> {
    match body {
        [Statement::ReturnStatement(value)] => expression(value.argument.as_ref()?, bindings),
        [Statement::BlockStatement(value)] => statements(&value.body, bindings),
        [Statement::IfStatement(branch)] if data(&branch.test, bindings) => {
            let mut left = statements(std::slice::from_ref(&branch.consequent), bindings)?;
            let right = statements(std::slice::from_ref(branch.alternate.as_ref()?), bindings)?;
            if left.mode != right.mode
                || left.routes != right.routes
                || left.aliases != right.aliases
                || left.lazy_routes != right.lazy_routes
            {
                return None;
            }
            left.plugins.extend(right.plugins);
            left.tailwind |= right.tailwind;
            left.default_extensions &= right.default_extensions;
            left.tsconfig_paths |= right.tsconfig_paths;
            left.package_dedupe |= right.package_dedupe;
            left.optimize_exclude.extend(right.optimize_exclude);
            Some(left)
        }
        _ => None,
    }
}

#[cfg(test)]
pub(super) fn parse(path: &Path, source: &str) -> Option<Config> {
    parse_detailed(path, source).ok()
}

pub(super) fn parse_detailed(path: &Path, source: &str) -> Result<Config, ConfigRefusal> {
    let refusal = RefCell::new(ConfigRefusal {
        offset: 0,
        reason: "invalid Vite config syntax, source type, or program directive".into(),
    });
    parse_grammar(path, source, &refusal).ok_or_else(|| refusal.into_inner())
}

fn parse_grammar(path: &Path, source: &str, refusal: &RefCell<ConfigRefusal>) -> Option<Config> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::from_path(path).ok()?).parse();
    if !parsed.errors.is_empty() || !parsed.program.directives.is_empty() {
        return None;
    }
    let mut bindings = Bindings {
        plugins: BTreeMap::new(),
        data: BTreeSet::new(),
        paths: BTreeSet::new(),
        define: None,
        directory: path.parent()?,
        path_modules: BTreeSet::new(),
        resolve_functions: BTreeSet::new(),
        refusal,
    };
    let mut names = BTreeSet::new();
    let (last, preceding) = parsed.program.body.split_last()?;
    for statement in preceding {
        bindings.refusing(
            statement.span().start,
            "Vite config statement may execute unaudited code",
        );
        match statement {
            Statement::ImportDeclaration(import) => {
                if import.import_kind == ImportOrExportKind::Type {
                    continue;
                }
                if import.phase.is_some() || import.with_clause.is_some() {
                    return None;
                }
                let source = import.source.value.as_str();
                bindings.refusing(
                    import.span.start,
                    format!("unsupported config import binding from {source}"),
                );
                let specifiers = import.specifiers.as_ref()?;
                if specifiers.is_empty() {
                    return None;
                }
                for specifier in specifiers {
                    let (local, exported) = match specifier {
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(value) => {
                            (value.local.name.as_str(), "default")
                        }
                        ImportDeclarationSpecifier::ImportSpecifier(value) => {
                            if value.import_kind == ImportOrExportKind::Type {
                                continue;
                            }
                            let ModuleExportName::IdentifierName(exported) = &value.imported else {
                                return None;
                            };
                            (value.local.name.as_str(), exported.name.as_str())
                        }
                        _ => return None,
                    };
                    if local == "__dirname" || !names.insert(local) {
                        return None;
                    }
                    match (source, exported) {
                        ("vite" | "vitest/config", "defineConfig") if bindings.define.is_none() => {
                            bindings.define = Some((local, source))
                        }
                        ("node:path" | "path", "default" | "resolve" | "join" | "dirname")
                        | ("node:url", "fileURLToPath") => {
                            bindings.paths.insert(local);
                            if source != "node:url" && exported == "default" {
                                bindings.path_modules.insert(local);
                            }
                            if source != "node:url" && exported == "resolve" {
                                bindings.resolve_functions.insert(local);
                            }
                        }
                        ("node:url", "URL") => {
                            if local != "URL" {
                                return None;
                            }
                        }
                        ("filesystem-routing/vite", "fileRoutes")
                        | ("@tanstack/router-plugin/vite", "tanstackRouter")
                        | ("vite-plugin-pwa", "VitePWA") => {
                            bindings.plugins.insert(local, source);
                        }
                        (
                            "@solidjs/vite-plugin"
                            | "vite-plugin-solid"
                            | "@tailwindcss/vite"
                            | "unocss/vite"
                            | "@unocss/vite"
                            | "vite-plugin-solid-svg"
                            | "unplugin-icons/vite"
                            | "vite-tsconfig-paths",
                            "default",
                        ) => {
                            bindings.plugins.insert(local, source);
                        }
                        _ => return None,
                    }
                }
            }
            Statement::VariableDeclaration(declaration)
                if declaration.kind == VariableDeclarationKind::Const =>
            {
                for declaration in &declaration.declarations {
                    let BindingPattern::BindingIdentifier(id) = &declaration.id else {
                        return None;
                    };
                    if id.name == "__dirname"
                        || !names.insert(id.name.as_str())
                        || !data(declaration.init.as_ref()?, &bindings)
                    {
                        return None;
                    }
                    bindings.data.insert(id.name.as_str());
                }
            }
            _ => return None,
        }
    }
    bindings.refusing(
        last.span().start,
        "Vite config needs one final default object or exact defineConfig call",
    );
    let Statement::ExportDefaultDeclaration(export) = last else {
        return None;
    };
    let mut config = match &export.declaration {
        ExportDefaultDeclarationKind::ObjectExpression(value) => object(value, &bindings),
        ExportDefaultDeclarationKind::CallExpression(call) => {
            if !matches!(call.callee.get_inner_expression(), Expression::Identifier(id)
                if Some(id.name.as_str()) == bindings.define.map(|(local, _)| local))
                || call.optional
                || call.type_arguments.is_some()
            {
                return None;
            }
            let [arg] = call.arguments.as_slice() else {
                return None;
            };
            let value = arg.as_expression()?;
            if let Expression::ArrowFunctionExpression(arrow) = value.get_inner_expression() {
                if arrow.r#async || !arrow.body.directives.is_empty() || arrow.params.rest.is_some()
                {
                    return None;
                }
                // Only a plain identifier or inert destructuring of the Vite
                // mode/command input; no defaults, bindings shadowing imports,
                // computed keys or rest effects.
                for parameter in &arrow.params.items {
                    if parameter.initializer.is_some() || !parameter.decorators.is_empty() {
                        return None;
                    }
                    match &parameter.pattern {
                        BindingPattern::BindingIdentifier(id) => {
                            if id.name == "__dirname" || names.contains(id.name.as_str()) {
                                return None;
                            }
                            bindings.data.insert(id.name.as_str());
                        }
                        BindingPattern::ObjectPattern(object) => {
                            if object.rest.is_some() {
                                return None;
                            }
                            for property in &object.properties {
                                let PropertyKey::StaticIdentifier(key) = &property.key else {
                                    return None;
                                };
                                let BindingPattern::BindingIdentifier(id) = &property.value else {
                                    return None;
                                };
                                if property.computed
                                    || !["mode", "command", "isSsrBuild", "isPreview"]
                                        .contains(&key.name.as_str())
                                    || names.contains(id.name.as_str())
                                    || id.name == "__dirname"
                                {
                                    return None;
                                }
                                bindings.data.insert(id.name.as_str());
                            }
                        }
                        _ => return None,
                    }
                }
                if arrow.expression {
                    let [Statement::ExpressionStatement(value)] = arrow.body.statements.as_slice()
                    else {
                        return None;
                    };
                    expression(&value.expression, &bindings)
                } else {
                    statements(&arrow.body.statements, &bindings)
                }
            } else {
                expression(value, &bindings)
            }
        }
        _ => export
            .declaration
            .as_expression()
            .and_then(|value| expression(value, &bindings)),
    }?;
    // Even an unused plugin import runs in Node while loading the config.
    config
        .plugins
        .extend(bindings.plugins.values().map(|name| (*name).into()));
    if let Some((_, source)) = bindings.define {
        config.plugins.insert(source.into());
    }
    config.default_extensions &= !config.plugins.contains("vite-tsconfig-paths");
    config.tsconfig_paths |= config.plugins.contains("vite-tsconfig-paths");
    Some(config)
}

pub(super) fn jsonc(source: &str) -> Option<serde_json::Value> {
    // Preserve strings verbatim. Strip only JSONC comments and trailing
    // commas; serde still rejects executable expressions and invalid JSON.
    let mut bytes = source.as_bytes().to_vec();
    let mut i = 0;
    let mut string = false;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if string => {
                i += 2;
                continue;
            }
            b'"' => string = !string,
            b'/' if !string && bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    bytes[i] = b' ';
                    i += 1;
                }
                continue;
            }
            b'/' if !string && bytes.get(i + 1) == Some(&b'*') => {
                bytes[i] = b' ';
                bytes[i + 1] = b' ';
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    bytes[i] = b' ';
                    i += 1;
                }
                if i + 1 >= bytes.len() {
                    return None;
                }
                bytes[i] = b' ';
                bytes[i + 1] = b' ';
                i += 2;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    string = false;
    i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if string => {
                i += 2;
                continue;
            }
            b'"' => string = !string,
            b',' if !string => {
                let next = bytes[i + 1..]
                    .iter()
                    .find(|byte| !byte.is_ascii_whitespace());
                if matches!(next, Some(b'}' | b']')) {
                    bytes[i] = b' ';
                }
            }
            _ => {}
        }
        i += 1;
    }
    serde_json::from_slice(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_resolver_controls_and_branch_exclusions_remain_closed() {
        let path = Path::new("vite.config.ts");
        let default = parse(
            path,
            "export default {plugins:[],resolve:{dedupe:[]},optimizeDeps:{exclude:[]}}",
        )
        .unwrap();
        assert!(!default.package_dedupe);
        assert!(default.optimize_exclude.is_empty());
        let changed = parse(path, "export default {plugins:[],resolve:{dedupe:['solid-js']},optimizeDeps:{exclude:['commonjs-package']}}").unwrap();
        assert!(changed.package_dedupe);
        assert!(changed.optimize_exclude.contains("commonjs-package"));
        let branch = parse(path, "import {defineConfig} from 'vite';export default defineConfig(({mode}) => mode === 'a' ? {plugins:[],optimizeDeps:{exclude:['a']}} : {plugins:[],optimizeDeps:{exclude:['b']}})").unwrap();
        assert_eq!(
            branch.optimize_exclude,
            BTreeSet::from(["a".into(), "b".into()])
        );
        for source in [
            "export default {plugins:[],resolve:{conditions:['node']}}",
            "export default {plugins:[],resolve:{mainFields:['main']}}",
            "export default {plugins:[],optimizeDeps:{disabled:true}}",
            "export default {plugins:[],resolve:{dedupe:true}}",
        ] {
            assert!(parse(path, source).is_none(), "{source}");
        }
    }

    #[test]
    fn admission_fixture_pairs_are_a_closed_config_grammar() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/inferred-host-spa/admission-cases.json");
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in cases.as_array().unwrap() {
            assert_eq!(
                parse(
                    Path::new("vite.config.ts"),
                    case["source"].as_str().unwrap()
                )
                .is_some(),
                case["accepted"].as_bool().unwrap(),
                "{}",
                case["name"]
            );
        }
        assert_eq!(
            jsonc("{ /* */ \"paths\": {\"~/*\":[\"src/*\",],}, // eof\n}"),
            Some(serde_json::json!({"paths":{"~/*":["src/*"]}}))
        );
        assert!(jsonc("{\"x\": import('server')}").is_none());
        assert!(jsonc("{/*unterminated").is_none());
    }

    #[test]
    fn allowlisted_calls_are_literal_in_every_branch() {
        let prefix = "import {defineConfig} from 'vite'; import solid from '@solidjs/vite-plugin'; import {fileRoutes} from 'filesystem-routing/vite';";
        let source = format!(
            "{prefix} export default defineConfig(({{mode}}) => mode === 'production' ? {{plugins:[solid({{start:true}}),fileRoutes()]}} : {{plugins:[solid({{start:true}}),fileRoutes()]}});"
        );
        assert_eq!(
            parse(Path::new("vite.config.ts"), &source).unwrap().mode,
            EntryMode::ClientStart
        );
        for tail in [
            "({plugins:[solid({start:true}), ...extra]})",
            "({plugins:[solid({start:true,ssr:true})]})",
            "mode === 'production' ? {plugins:[solid({start:true})]} : {plugins:[unknown()]}",
            "({plugins:[solid({start:true}), {configureServer(s){s.ssrLoadModule('./src/App.tsx')}}]})",
            "({plugins:[solid({start:true})], build:{ssr:'src/App.tsx'}})",
        ] {
            assert!(
                parse(
                    Path::new("vite.config.ts"),
                    &format!("{prefix} export default defineConfig(({{mode}}) => {tail});")
                )
                .is_none(),
                "{tail}"
            );
        }
    }

    #[test]
    fn optional_plugins_require_closed_nonexecuting_options() {
        for (module, exported, good, bad) in [
            ("@unocss/vite", "default", "{configFile:false}", "{}"),
            (
                "vite-plugin-solid-svg",
                "default",
                "{svgo:{enabled:false}}",
                "{}",
            ),
            (
                "vite-plugin-pwa",
                "VitePWA",
                "{strategies:'generateSW'}",
                "{strategies:'injectManifest'}",
            ),
            (
                "@tanstack/router-plugin/vite",
                "tanstackRouter",
                "{target:'solid'}",
                "{target:'solid',plugins:[]}",
            ),
            (
                "filesystem-routing/vite",
                "fileRoutes",
                "{types:true}",
                "{buildInputs:['ssr']}",
            ),
            ("vite-tsconfig-paths", "default", "{}", "{parseNative:true}"),
        ] {
            let import = if exported == "default" {
                "import p".to_owned()
            } else {
                format!("import {{{exported} as p}}")
            };
            for (options, accepted) in [(good, true), (bad, false)] {
                let source =
                    format!("{import} from '{module}'; export default {{plugins:[p({options})]}};");
                assert_eq!(
                    parse(Path::new("vite.config.ts"), &source).is_some(),
                    accepted,
                    "{source}"
                );
            }
        }
    }
}
