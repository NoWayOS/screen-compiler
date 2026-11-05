// SPDX-License-Identifier: MIT
//! The subset checks and the lowering to IR, in one walk.

use std::collections::{BTreeMap, BTreeSet};

use screen_compiler_ir::{
    AlignItems, ButtonVariant, Case, Code, Colour, Container, Diagnostic, Fit, Group, ICON_NAMES,
    InputMode, Justify, Length, Loc, Look, MAX_BORDER_WIDTH, MAX_INPUT_LEN, MAX_LABEL_BYTES,
    MAX_LINES, MAX_LONG_TEXT, MAX_RADIO_OPTIONS, MAX_RADIUS, MAX_SELECT_OPTIONS,
    MAX_TEMPLATE_BYTES, Module, Name, Node, NodeKind, SELECT_ROWS, Screen, Space, Surface,
    Template, TextAlign, TextRole, TextStyle, Token, Tone, Weight, has_errors, sort_diagnostics,
    special_char, validate,
};
use screen_compiler_tsx::SourceModule;
use screen_compiler_tsx::syntax::{
    Attr, AttrValue, Body, Child, DeclKeyword, Element, ElementName, Export, ExportSpecifier, Expr,
    Function, Import, ImportBinding, Item, LogicalOp, Params, PropBinding, Stmt, VarDecl,
};

use crate::files::{self, Load, LoadError, MAX_FILES};
use crate::parts::{Expander, Files, Part};
use crate::{Analysis, KitComponent, Options, parts};

/// Where a component or list is declared: its file and its name there.
type Origin = (u16, String);

/// What a name in scope refers to.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Binding {
    /// A kit component imported from the kit module.
    Kit(KitComponent),
    /// A design token (index into the token list).
    Token(usize),
    /// The theme's spacing tokens, `space` from the kit:
    /// `space.xs` to `space.xl`.
    Space,
    /// A component, by where it is declared.
    Component(Origin),
    /// A top-level `const` array, which `.map` expands, by where it is
    /// declared.
    List(Origin),
    /// Imported from a file of the screen's, until the import is resolved.
    Pending,
    /// `React` from `import React from "react"`.
    React,
    /// Something imported from another module (already reported).
    OtherImport,
    /// A destructured prop; holds the prop's name.
    Prop(String),
    /// The component's props object.
    PropsObject,
}

/// What the kit calls the theme's spacing tokens:
/// `import { space } from "nowayos:kit"`, then `gap={space.md}`.
const SPACE: &str = "space";

/// Names strict mode code (every module) may not bind: the strict-mode
/// reserved words the parser accepts, and `eval` and `arguments`.
const STRICT_RESERVED: [&str; 11] = [
    "arguments",
    "eval",
    "implements",
    "interface",
    "let",
    "package",
    "private",
    "protected",
    "public",
    "static",
    "yield",
];

/// How a module-scope name was declared, for redeclaration rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeclKind {
    /// `var`, which may be declared again by another `var`.
    Var,
    /// Imports, `const`, `let` and functions, which may not.
    Lexical,
}

/// Module-level state for one analysis.
struct Cx<'o> {
    options: &'o Options,
    diagnostics: Vec<Diagnostic>,
    /// The file being read: what each name at its top level refers to.
    bindings: BTreeMap<String, Binding>,
    /// Each name the file being read exports, and the name it exports.
    exports: BTreeMap<String, String>,
    /// Each file read so far, by number.
    scopes: Vec<FileScope>,
    /// The screen's files.
    read: &'o Read,
    /// Imports from the screen's files, resolved once every file is read.
    pending: Vec<Pending>,
    tokens: Vec<Token>,
    /// Module-scope value declarations: where each name is first declared.
    values: BTreeMap<String, (Loc, DeclKind)>,
    /// Module-scope type-level names: type aliases, interfaces, type-only
    /// imports and `declare` declarations.
    types: BTreeSet<String>,
    /// Exported names, and where each is first exported.
    exported: BTreeMap<String, Loc>,
}

/// A component found at the top level.
struct Component<'a> {
    name: Option<&'a str>,
    function: &'a Function,
    loc: Loc,
}

/// One of the screen's files, once read: the names at its top level.
struct FileScope {
    /// What each name refers to.
    bindings: BTreeMap<String, Binding>,
    /// Every name it declares or imports.
    declared: BTreeSet<String>,
    /// Each name it exports, and the name it exports.
    exports: BTreeMap<String, String>,
    /// Its type-level names, which an import may name but a screen cannot
    /// use.
    types: BTreeSet<String>,
}

/// An import from one of the screen's files.
struct Pending {
    /// The file the import is in.
    file: u16,
    /// The name it binds there.
    local: String,
    /// The name it imports: `default` for a default import.
    imported: String,
    /// The file it imports from.
    target: u16,
    /// What the import is written as, for messages.
    specifier: String,
    loc: Loc,
}

/// The screen's files: its own, file 0, and each it imports.
#[derive(Default)]
struct Read {
    /// Each file's path from the screen's folder, the screen's own first.
    paths: Vec<String>,
    /// The imported files' trees: file `n` is `modules[n - 1]`.
    modules: Vec<SourceModule>,
    /// The file each import reads, by where the import is written.
    targets: BTreeMap<Loc, u16>,
    /// Files that did not parse, whose names are not resolved.
    broken: BTreeSet<u16>,
}

impl Read {
    /// Reads every file `source` imports, directly or through another,
    /// reporting each that cannot be read where it is imported.
    fn files(
        source: &SourceModule,
        options: &Options,
        load: &mut dyn Load,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Self {
        let own = options
            .source_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(&options.source_name);
        let mut read = Self {
            paths: vec![own.to_owned()],
            ..Self::default()
        };
        let mut next = 0_usize;
        loop {
            let module = match next.checked_sub(1) {
                None => source,
                Some(index) => match read.modules.get(index) {
                    Some(module) => module,
                    None => break,
                },
            };
            let wanted: Vec<(Loc, String)> = module
                .items
                .iter()
                .filter_map(|item| match item {
                    Item::Import(import) if reads_file(import) => {
                        Some((import.loc, import.module.clone()))
                    }
                    _ => None,
                })
                .collect();
            let from = read.paths.get(next).cloned().unwrap_or_default();
            for (loc, specifier) in wanted {
                read.import(&from, &specifier, loc, load, diagnostics);
            }
            next = next.saturating_add(1);
        }
        read
    }

    /// Reads the file `specifier` names, imported at `loc` by the file at
    /// `from`, unless it has been read.
    fn import(
        &mut self,
        from: &str,
        specifier: &str,
        loc: Loc,
        load: &mut dyn Load,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let path = match files::resolve(from, specifier) {
            Ok(path) => path,
            Err((message, help)) => {
                diagnostics
                    .push(Diagnostic::error(Code::Unsupported, Some(loc), message).with_help(help));
                return;
            }
        };
        if let Some(index) = self.paths.iter().position(|read| *read == path) {
            if let Ok(file) = u16::try_from(index)
                && !self.broken.contains(&file)
            {
                self.targets.insert(loc, file);
            }
            return;
        }
        let file = match u16::try_from(self.paths.len()) {
            Ok(file) if self.paths.len() < MAX_FILES => file,
            _ => {
                diagnostics.push(
                    Diagnostic::error(
                        Code::Limit,
                        Some(loc),
                        format!("a screen reads at most {MAX_FILES} files, its own among them"),
                    )
                    .with_help("import from fewer files: put parts that go together in one"),
                );
                return;
            }
        };
        let text = match load.load(&path) {
            Ok(text) => text,
            Err(error) => {
                let code = match error {
                    LoadError::Missing | LoadError::Unreadable(_) => Code::UnknownName,
                    LoadError::TooLarge => Code::Limit,
                    LoadError::NotUtf8 => Code::Syntax,
                    LoadError::Outside | LoadError::NotGiven => Code::Unsupported,
                };
                diagnostics.push(Diagnostic::error(
                    code,
                    Some(loc),
                    format!(
                        "\"{specifier}\" ({path}) cannot be read: {}",
                        error.describe()
                    ),
                ));
                return;
            }
        };
        // The path is kept even when the file does not parse, so the
        // errors inside it name it.
        self.paths.push(path.clone());
        match screen_compiler_tsx::parse_file(&text, file) {
            Ok(parsed) => {
                diagnostics.extend(parsed.diagnostics.iter().cloned());
                self.modules.push(parsed);
                self.targets.insert(loc, file);
            }
            Err(errors) => {
                for mut error in errors {
                    if error.loc.is_none() {
                        error.loc = Some(loc);
                        error.message = format!("in {path}: {}", error.message);
                    }
                    diagnostics.push(error);
                }
                // Nothing more is read from it; its number stays its own.
                self.modules.push(SourceModule::default());
                self.broken.insert(file);
            }
        }
    }

    /// File `file`'s tree.
    fn module<'r>(&'r self, source: &'r SourceModule, file: u16) -> Option<&'r SourceModule> {
        match usize::from(file).checked_sub(1) {
            None => Some(source),
            Some(index) => self.modules.get(index),
        }
    }

    /// File `file`'s path from the screen's folder.
    fn path(&self, file: u16) -> &str {
        self.paths.get(usize::from(file)).map_or("", String::as_str)
    }
}

/// Whether `import` reads one of the screen's files: values imported from
/// a path. (Importing a path for its side effects alone is refused where
/// it is written, unread.)
fn reads_file(import: &Import) -> bool {
    files::is_path(&import.module)
        && !import.type_only
        && !import.side_effect_only
        && import.bindings.iter().any(|binding| {
            !matches!(
                binding,
                ImportBinding::Named {
                    type_only: true,
                    ..
                }
            )
        })
}

pub(crate) fn analyse(source: &SourceModule, options: &Options, load: &mut dyn Load) -> Analysis {
    let mut diagnostics = source.diagnostics.clone();
    let read = Read::files(source, options, load, &mut diagnostics);
    let mut cx = Cx {
        options,
        diagnostics,
        bindings: BTreeMap::new(),
        exports: BTreeMap::new(),
        scopes: Vec::new(),
        read: &read,
        pending: Vec::new(),
        tokens: Vec::new(),
        values: BTreeMap::new(),
        types: BTreeSet::new(),
        exported: BTreeMap::new(),
    };
    // Each file's top level, in its own scope; then the imports between them.
    let mut components: Vec<(u16, Component<'_>)> = Vec::new();
    let mut lists: BTreeMap<Origin, &Expr> = BTreeMap::new();
    for index in 0..read.paths.len() {
        let Ok(file) = u16::try_from(index) else {
            break;
        };
        let Some(module) = read.module(source, file) else {
            continue;
        };
        let (found, arrays) = cx.top_level(file, module);
        components.extend(found.into_iter().map(|component| (file, component)));
        lists.extend(
            arrays
                .into_iter()
                .map(|(name, list)| ((file, name.to_owned()), list)),
        );
    }
    cx.resolve_imports();
    // A component any file uses as an element is a part, expanded where it
    // is used (`parts`); every other of the screen's own file is a screen,
    // and one of another file is not converted.
    let mut used: BTreeSet<Origin> = BTreeSet::new();
    for (file, component) in &components {
        let mut names = BTreeSet::new();
        for returned in parts::returned_values(component.function) {
            parts::element_names(returned, &mut names, 0);
        }
        for name in names {
            if let Some(Binding::Component(origin)) = cx.binding(*file, name) {
                used.insert(origin.clone());
            }
        }
    }
    let mut files = Files::default();
    for (file, component) in &components {
        if let Some(name) = component.name
            && used.contains(&(*file, name.to_owned()))
        {
            let part = cx.part(name, component.function);
            files.parts.insert((*file, name), part);
        }
    }
    let keys: BTreeMap<Origin, (u16, &str)> = files
        .parts
        .keys()
        .map(|&(file, name)| ((file, name.to_owned()), (file, name)))
        .collect();
    for (file, scope) in (0_u16..).zip(&cx.scopes) {
        for (local, binding) in &scope.bindings {
            match binding {
                Binding::Component(origin) => {
                    if let Some(&key) = keys.get(origin) {
                        files.part_names.insert((file, local.clone()), key);
                    }
                }
                Binding::List(origin) => {
                    if let Some(&list) = lists.get(origin) {
                        files.lists.insert((file, local.clone()), list);
                    }
                }
                _ => {}
            }
        }
        files.declared.push(scope.declared.clone());
    }
    let screens: Vec<Screen> = components
        .iter()
        .filter(|(file, component)| {
            *file == 0
                && component
                    .name
                    .is_none_or(|name| !used.contains(&(0, name.to_owned())))
        })
        .filter_map(|(_, component)| cx.component(component, &files))
        .collect();

    let imported: Vec<String> = read.paths.iter().skip(1).cloned().collect();
    let module = Module {
        source_name: options.source_name.clone(),
        imported: imported.clone(),
        tokens: std::mem::take(&mut cx.tokens),
        screens,
    };
    let mut diagnostics = cx.diagnostics;
    diagnostics.extend(validate(&module));
    if module.screens.is_empty() && !has_errors(&diagnostics) {
        diagnostics.push(
            Diagnostic::error(Code::NoScreen, None, "no screen component found")
                .with_help("export a function whose body is `return <Screen>…</Screen>;`"),
        );
    }
    sort_diagnostics(&mut diagnostics);
    // A part used twice repeats what is wrong inside it: once is enough.
    diagnostics.dedup();
    let module = (!has_errors(&diagnostics)).then_some(module);
    Analysis {
        module,
        diagnostics,
        imported,
    }
}

impl<'o> Cx<'o> {
    /// Reads file `file`'s top level in a scope of its own, which is kept
    /// in [`Cx::scopes`]: its components, and its `const` arrays by name.
    fn top_level<'s>(
        &mut self,
        file: u16,
        module: &'s SourceModule,
    ) -> (Vec<Component<'s>>, Vec<(&'s str, &'s Expr)>) {
        let mut components = Vec::new();
        let mut lists = Vec::new();
        // Checked once every declaration is known: exports may come first.
        let mut export_specifiers: Vec<&ExportSpecifier> = Vec::new();
        let mut default_names: Vec<(&str, Loc)> = Vec::new();
        for item in &module.items {
            match item {
                Item::Import(import) => self.import(file, import),
                Item::Var(var) => {
                    let kind = if var.keyword == DeclKeyword::Var {
                        DeclKind::Var
                    } else {
                        DeclKind::Lexical
                    };
                    if !self.declare(&var.name, var.name_loc, kind) {
                        continue;
                    }
                    if var.export == Export::Named {
                        self.export(&var.name, &var.name, var.name_loc);
                    }
                    match &var.init {
                        Some(Expr::Function(function)) if var.keyword == DeclKeyword::Const => {
                            components.push(Component {
                                name: Some(&var.name),
                                function,
                                loc: var.loc,
                            });
                        }
                        // A list, which `.map` expands at build time.
                        Some(list @ Expr::Array { .. }) if var.keyword == DeclKeyword::Const => {
                            self.bindings
                                .insert(var.name.clone(), Binding::List((file, var.name.clone())));
                            lists.push((var.name.as_str(), list));
                        }
                        _ => self.token(var),
                    }
                }
                Item::Function(decl) => {
                    let loc = decl.function.loc;
                    if let Some(name) = decl.name.as_deref()
                        && !self.declare(name, loc, DeclKind::Lexical)
                    {
                        continue;
                    }
                    match (decl.export, decl.name.as_deref()) {
                        (Export::Default, name) => {
                            self.export("default", name.unwrap_or_default(), loc);
                        }
                        (Export::Named, Some(name)) => self.export(name, name, loc),
                        _ => {}
                    }
                    components.push(Component {
                        name: decl.name.as_deref(),
                        function: &decl.function,
                        loc,
                    });
                }
                Item::ExportDefaultExpr(expr) => match expr {
                    Expr::Ident { name, loc } => {
                        self.export("default", name, *loc);
                        default_names.push((name, *loc));
                    }
                    Expr::Function(function) => {
                        self.export("default", "", expr.loc());
                        components.push(Component {
                            name: None,
                            function,
                            loc: function.loc,
                        });
                    }
                    other => {
                        self.export("default", "", expr.loc());
                        self.error(
                            Code::Unsupported,
                            other.loc(),
                            format!("default export of {} is not supported", other.describe()),
                        );
                    }
                },
                Item::ExportList { specifiers, .. } => {
                    for specifier in specifiers {
                        self.export(&specifier.exported, &specifier.local, specifier.loc);
                        export_specifiers.push(specifier);
                    }
                }
                Item::TypeOnly { names, .. } => self.types.extend(names.iter().cloned()),
                Item::Other(other) => self.error(
                    Code::Unsupported,
                    other.loc,
                    format!("{} is not supported in subset v1", other.kind),
                ),
            }
        }
        for specifier in export_specifiers {
            let local = &specifier.local;
            if !self.values.contains_key(local) && !self.types.contains(local) {
                self.error_help(
                    Code::UnknownName,
                    specifier.loc,
                    format!("`{local}` is exported but not declared in this file"),
                    "a module can export only names it declares or imports (anything else is a syntax error)",
                );
            }
        }
        for (name, loc) in default_names {
            if self.values.contains_key(name) {
                continue;
            }
            if self.types.contains(name) {
                self.error(
                    Code::InvalidValue,
                    loc,
                    format!(
                        "`export default {name}` exports a type, but `export default` needs a value"
                    ),
                );
            } else {
                self.error(
                    Code::UnknownName,
                    loc,
                    format!("`{name}` is not declared in this file"),
                );
            }
        }
        for component in &components {
            if let Some(name) = component.name {
                self.bindings
                    .entry(name.to_owned())
                    .or_insert_with(|| Binding::Component((file, name.to_owned())));
            }
        }
        let bindings = std::mem::take(&mut self.bindings);
        let declared: BTreeSet<String> = std::mem::take(&mut self.values)
            .into_keys()
            .chain(bindings.keys().cloned())
            .collect();
        self.exported.clear();
        self.scopes.push(FileScope {
            bindings,
            declared,
            exports: std::mem::take(&mut self.exports),
            types: std::mem::take(&mut self.types),
        });
        (components, lists)
    }

    /// What `name` refers to at the top level of file `file`.
    fn binding(&self, file: u16, name: &str) -> Option<&Binding> {
        self.scopes
            .get(usize::from(file))
            .and_then(|scope| scope.bindings.get(name))
    }

    /// Binds each import from the screen's files to what it names, once
    /// every file is read: an import may name what another file imports.
    fn resolve_imports(&mut self) {
        let mut pending = std::mem::take(&mut self.pending);
        loop {
            let before = pending.len();
            pending.retain(|import| !self.bind_import(import));
            if pending.len() == before {
                break;
            }
        }
        for import in pending {
            self.error_help(
                Code::Unsupported,
                import.loc,
                format!(
                    "`{}` from \"{}\" is imported in a circle: each file imports it from another",
                    import.imported, import.specifier
                ),
                "import it from the file that declares it",
            );
            self.set_binding(import.file, &import.local, Binding::OtherImport);
        }
    }

    /// Binds `import` if what it names is known; `false` if that is an
    /// import still to be resolved.
    fn bind_import(&mut self, import: &Pending) -> bool {
        let path = self.read.path(import.target).to_owned();
        let Some(scope) = self.scopes.get(usize::from(import.target)) else {
            return true;
        };
        let local = match scope.exports.get(&import.imported) {
            Some(local) => Some(local),
            // A type, whose exports are not recorded: erased, as the
            // import's use of it is.
            None if scope.types.contains(&import.imported) => None,
            None => {
                self.no_export(import, &path);
                return true;
            }
        };
        match local.and_then(|local| scope.bindings.get(local)) {
            Some(Binding::Pending) => return false,
            Some(binding) => {
                let binding = binding.clone();
                self.set_binding(import.file, &import.local, binding);
            }
            // A type, an unnamed default export, or a declaration that is
            // reported where it is: nothing a screen can use, which is
            // reported wherever it is used.
            None => {
                if let Some(scope) = self.scopes.get_mut(usize::from(import.file)) {
                    scope.bindings.remove(&import.local);
                }
            }
        }
        true
    }

    /// Reports that the file `import` reads, at `path`, does not export
    /// what it names.
    fn no_export(&mut self, import: &Pending, path: &str) {
        let what = if import.imported == "default" {
            "default export".to_owned()
        } else {
            format!("export `{}`", import.imported)
        };
        self.error_help(
            Code::UnknownName,
            import.loc,
            format!("{path} has no {what}"),
            format!("export it from {path}, or import a name it exports"),
        );
        self.set_binding(import.file, &import.local, Binding::OtherImport);
    }

    fn set_binding(&mut self, file: u16, name: &str, binding: Binding) {
        if let Some(scope) = self.scopes.get_mut(usize::from(file)) {
            scope.bindings.insert(name.to_owned(), binding);
        }
    }
}

impl Cx<'_> {
    /// Reports `name` when strict mode code may not bind it.
    fn reserved_binding(&mut self, name: &str, loc: Loc) {
        if STRICT_RESERVED.contains(&name) {
            self.error(
                Code::Syntax,
                loc,
                format!("`{name}` cannot name a binding in a module (strict mode code)"),
            );
        }
    }

    /// Records a module-scope value declaration, reporting reserved names
    /// and second declarations (a syntax error unless both are `var`).
    /// Returns `false` for a second declaration, which is then left out.
    fn declare(&mut self, name: &str, loc: Loc, kind: DeclKind) -> bool {
        self.reserved_binding(name, loc);
        let Some(&(first, first_kind)) = self.values.get(name) else {
            self.values.insert(name.to_owned(), (loc, kind));
            return true;
        };
        if kind == DeclKind::Lexical || first_kind == DeclKind::Lexical {
            self.error_help(
                Code::DuplicateName,
                loc,
                format!("`{name}` is already declared at {first}"),
                "a module may declare each name only once (anything else is a syntax error)",
            );
        }
        false
    }

    /// Records that `name` is exported, as the file's `local`, reporting a
    /// name exported twice.
    fn export(&mut self, name: &str, local: &str, loc: Loc) {
        self.exports
            .entry(name.to_owned())
            .or_insert_with(|| local.to_owned());
        if let Some(&first) = self.exported.get(name) {
            let message = if name == "default" {
                format!("a module may have only one default export; the first is at {first}")
            } else {
                format!("`{name}` is exported twice; the first export is at {first}")
            };
            self.error(Code::DuplicateName, loc, message);
        } else {
            self.exported.insert(name.to_owned(), loc);
        }
    }

    fn error(&mut self, code: Code, loc: Loc, message: impl Into<String>) {
        self.diagnostics
            .push(Diagnostic::error(code, Some(loc), message));
    }

    fn error_help(
        &mut self,
        code: Code,
        loc: Loc,
        message: impl Into<String>,
        help: impl Into<String>,
    ) {
        self.diagnostics
            .push(Diagnostic::error(code, Some(loc), message).with_help(help));
    }

    fn import(&mut self, file: u16, import: &Import) {
        if import.has_attributes {
            self.error(
                Code::Unsupported,
                import.loc,
                "import attributes and import phases are not supported",
            );
        }
        let is_kit = import.module == self.options.kit_module;
        let is_react = import.module == "react";
        let is_path = files::is_path(&import.module);
        if import.type_only {
            for binding in &import.bindings {
                let (ImportBinding::Named { local, .. }
                | ImportBinding::Default { local, .. }
                | ImportBinding::Namespace { local, .. }) = binding;
                self.types.insert(local.clone());
            }
            return;
        }
        for binding in &import.bindings {
            match binding {
                ImportBinding::Named {
                    type_only: true,
                    local,
                    ..
                } => {
                    self.types.insert(local.clone());
                }
                ImportBinding::Named { local, loc, .. }
                | ImportBinding::Default { local, loc }
                | ImportBinding::Namespace { local, loc } => {
                    // Imports always bind, even when declared twice, so a
                    // duplicate is reported once and not again where used.
                    self.declare(local, *loc, DeclKind::Lexical);
                }
            }
        }
        for binding in &import.bindings {
            match binding {
                ImportBinding::Named {
                    type_only: true, ..
                } => {}
                ImportBinding::Named {
                    imported,
                    local,
                    loc,
                    ..
                } if is_kit && imported == SPACE => {
                    self.bindings.insert(local.clone(), Binding::Space);
                }
                ImportBinding::Named {
                    imported,
                    local,
                    loc,
                    ..
                } if is_kit => match KitComponent::from_name(imported) {
                    Some(component) => {
                        self.bindings.insert(local.clone(), Binding::Kit(component));
                    }
                    None => self.error_help(
                        Code::UnknownName,
                        *loc,
                        format!("`{imported}` is not a component of the kit"),
                        format!(
                            "subset v1 has {}, and `{SPACE}`: the theme's spacing tokens",
                            KitComponent::list("and")
                        ),
                    ),
                },
                ImportBinding::Named {
                    imported,
                    local,
                    loc,
                    ..
                } if is_react => {
                    self.bindings.insert(local.clone(), Binding::OtherImport);
                    self.error_help(
                        Code::Unsupported,
                        *loc,
                        format!("React API `{imported}` is not supported in subset v1"),
                        "hooks, state and effects are outside subset v1",
                    );
                }
                ImportBinding::Default { local, .. } | ImportBinding::Namespace { local, .. }
                    if is_react =>
                {
                    self.bindings.insert(local.clone(), Binding::React);
                }
                ImportBinding::Default { local, loc } | ImportBinding::Namespace { local, loc }
                    if is_kit =>
                {
                    self.bindings.insert(local.clone(), Binding::OtherImport);
                    self.error_help(
                        Code::Unsupported,
                        *loc,
                        "the kit must be imported by name",
                        format!(
                            "import {{ Screen, Column, Text }} from \"{}\";",
                            self.options.kit_module
                        ),
                    );
                }
                ImportBinding::Namespace { local, loc } if is_path => {
                    self.bindings.insert(local.clone(), Binding::OtherImport);
                    self.error_help(
                        Code::Unsupported,
                        *loc,
                        format!("\"{}\" must be imported by name", import.module),
                        format!("import {{ Header, Footer }} from \"{}\";", import.module),
                    );
                }
                ImportBinding::Named {
                    imported, local, ..
                } if is_path => self.import_from_file(file, import, imported, local),
                ImportBinding::Default { local, .. } if is_path => {
                    self.import_from_file(file, import, "default", local);
                }
                ImportBinding::Named { local, .. }
                | ImportBinding::Default { local, .. }
                | ImportBinding::Namespace { local, .. } => {
                    self.bindings.insert(local.clone(), Binding::OtherImport);
                }
            }
        }
        let has_value_bindings = import.bindings.iter().any(|b| {
            !matches!(
                b,
                ImportBinding::Named {
                    type_only: true,
                    ..
                }
            )
        });
        if is_kit || is_react || (is_path && has_value_bindings) {
            // A path that is not read was reported where it is imported.
            return;
        }
        // `import {} from "m"` keeps the import (Babel; TypeScript too under
        // verbatimModuleSyntax), so it runs "m" exactly like `import "m"`.
        let only_type_specifiers = !import.bindings.is_empty() && !has_value_bindings;
        if only_type_specifiers {
            self.error_help(
                Code::Unsupported,
                import.loc,
                format!(
                    "`import {{ type … }}` from \"{}\" can still load \"{}\" for its side effects",
                    import.module, import.module
                ),
                "write `import type { … }`, which every compiler removes; TypeScript's verbatimModuleSyntax keeps `import { type … }` as `import {}`",
            );
        } else if import.side_effect_only || import.bindings.is_empty() {
            let help = if [".css", ".scss", ".sass", ".less"]
                .iter()
                .any(|ext| import.module.ends_with(ext))
            {
                "Screen Compiler does not implement CSS files: write style={{ … }} with the declared properties, and layout props (gap, padding) and design tokens"
            } else {
                "side effects run at import time, which a static screen does not have"
            };
            self.error_help(
                Code::Unsupported,
                import.loc,
                format!(
                    "importing \"{}\" for its side effects is not supported",
                    import.module
                ),
                help,
            );
        } else if has_value_bindings {
            self.error_help(
                Code::Unsupported,
                import.loc,
                format!("imports from \"{}\" are not supported in subset v1", import.module),
                format!(
                    "import components from \"{}\", or your own from a .tsx file in the screen's folder (\"./header\"); declare design tokens as `const`",
                    self.options.kit_module
                ),
            );
        }
    }

    /// `local`, bound to what `imported` names in the file `import` reads,
    /// once every file is read; an import that read no file was reported.
    fn import_from_file(&mut self, file: u16, import: &Import, imported: &str, local: &str) {
        match self.read.targets.get(&import.loc) {
            Some(&target) => {
                self.bindings.insert(local.to_owned(), Binding::Pending);
                self.pending.push(Pending {
                    file,
                    local: local.to_owned(),
                    imported: imported.to_owned(),
                    target,
                    specifier: import.module.clone(),
                    loc: import.loc,
                });
            }
            None => {
                self.bindings.insert(local.to_owned(), Binding::OtherImport);
            }
        }
    }

    fn token(&mut self, var: &VarDecl) {
        if var.keyword != DeclKeyword::Const {
            let keyword = match var.keyword {
                DeclKeyword::Let => "let",
                DeclKeyword::Var => "var",
                DeclKeyword::Using => "using",
                DeclKeyword::Const => "const",
            };
            self.error_help(
                Code::Unsupported,
                var.loc,
                format!("`{keyword}` declarations are not supported"),
                "design tokens are `const` declarations",
            );
            return;
        }
        let value = match &var.init {
            Some(Expr::Num { value, raw, loc }) => {
                match self.pixels(*value, raw, *loc, &format!("design token `{}`", var.name)) {
                    Some(px) => px,
                    None => return,
                }
            }
            Some(other) => {
                self.error_help(
                    Code::Unsupported,
                    other.loc(),
                    format!(
                        "top-level constant `{}` must be a whole number (a design token) or a screen component, not {}",
                        var.name,
                        other.describe()
                    ),
                    "design tokens are written `const SPACE_MD = 16;`",
                );
                return;
            }
            None => {
                self.error(
                    Code::InvalidValue,
                    var.loc,
                    format!("`{}` has no value", var.name),
                );
                return;
            }
        };
        let name = match Name::parse(&var.name) {
            Ok(name) => name,
            Err(error) => {
                self.error(
                    Code::InvalidValue,
                    var.name_loc,
                    format!("`{}` cannot name a design token: {error}", var.name),
                );
                return;
            }
        };
        self.bindings
            .insert(var.name.clone(), Binding::Token(self.tokens.len()));
        self.tokens.push(Token {
            name,
            value,
            loc: Some(var.name_loc),
        });
    }

    /// A whole number of pixels in `0..=65535`, or an error.
    fn pixels(&mut self, value: f64, raw: &str, loc: Loc, what: &str) -> Option<u16> {
        let leading_zero =
            raw.starts_with('0') && raw.as_bytes().get(1).is_some_and(u8::is_ascii_digit);
        if leading_zero {
            self.error_help(
                Code::Syntax,
                loc,
                format!("`{raw}` has a leading zero: a legacy octal literal, which is not allowed in modules (strict mode code)"),
                "write the number without the leading zero",
            );
            return None;
        }
        if value.is_finite() && (0.0..=f64::from(u16::MAX)).contains(&value) && value.fract() == 0.0
        {
            // Exact: a whole number within u16's range.
            return Some(value as u16);
        }
        self.error(
            Code::InvalidValue,
            loc,
            format!("{what} must be a whole number of pixels from 0 to 65535, not {raw}"),
        );
        None
    }

    /// A part: a component the file uses as an element. Its parameters are
    /// checked as a screen's are; what it returns is expanded where it is
    /// used.
    fn part<'s>(&mut self, name: &str, function: &'s Function) -> Part<'s> {
        if function.is_async || function.is_generator {
            self.error(
                Code::Unsupported,
                function.loc,
                format!("component `{name}` must be a plain function, not async or a generator"),
            );
        }
        match &function.params {
            Params::Destructured(bindings) => {
                let mut seen: Vec<&str> = Vec::new();
                for binding in bindings {
                    self.reserved_binding(&binding.local, binding.loc);
                    if seen.contains(&binding.local.as_str()) {
                        self.error(
                            Code::DuplicateName,
                            binding.loc,
                            format!(
                                "`{}` is bound twice in the parameters of `{name}` (a syntax error)",
                                binding.local
                            ),
                        );
                    }
                    seen.push(&binding.local);
                }
            }
            Params::Named { name: object, loc } => self.reserved_binding(object, *loc),
            Params::None => {}
            Params::Unsupported(other) => self.error_help(
                Code::Unsupported,
                other.loc,
                format!("{} in component parameters is not supported", other.kind),
                format!("destructure its props: function {name}({{ title, children }}) {{ … }}"),
            ),
        }
        Part {
            params: &function.params,
            returned: self.returned(name, function),
        }
    }

    fn component<'s>(&mut self, component: &Component<'s>, files: &Files<'s>) -> Option<Screen> {
        let Some(source_name) = component.name else {
            self.error_help(
                Code::Unsupported,
                component.loc,
                "screen components must be named",
                "write `export default function WelcomeScreen() { … }`",
            );
            return None;
        };
        let name = match Name::parse(source_name) {
            Ok(name) => name,
            Err(error) => {
                self.error(
                    Code::InvalidValue,
                    component.loc,
                    format!("`{source_name}` cannot name a screen: {error}"),
                );
                return None;
            }
        };
        let function = component.function;
        if function.is_async || function.is_generator {
            self.error(
                Code::Unsupported,
                function.loc,
                format!(
                    "component `{source_name}` must be a plain function, not async or a generator"
                ),
            );
        }
        let scope = match &function.params {
            Params::None => Scope::None,
            Params::Destructured(bindings) => {
                let mut seen: Vec<&str> = Vec::new();
                for binding in bindings {
                    self.reserved_binding(&binding.local, binding.loc);
                    if seen.contains(&binding.local.as_str()) {
                        self.error(
                            Code::DuplicateName,
                            binding.loc,
                            format!(
                                "`{}` is bound twice in the parameters of `{source_name}` (a syntax error)",
                                binding.local
                            ),
                        );
                    }
                    seen.push(&binding.local);
                }
                Scope::Destructured(bindings)
            }
            Params::Named { name, loc } => {
                self.reserved_binding(name, *loc);
                Scope::Object(name)
            }
            Params::Unsupported(other) => {
                self.error_help(
                    Code::Unsupported,
                    other.loc,
                    format!("{} in component parameters is not supported", other.kind),
                    "destructure the callback props: function WelcomeScreen({ onStart }) { … }",
                );
                Scope::None
            }
        };
        let returned = self.returned(source_name, function)?;
        let mut expander = Expander::new(files, self.options.limits.max_nodes);
        let expanded = expander.root(returned, &function.params);
        self.diagnostics.append(&mut expander.diagnostics);
        let element = match &expanded {
            Expr::Element(element) => element,
            other => {
                self.error_help(
                    Code::Unsupported,
                    other.loc(),
                    format!(
                        "component `{source_name}` must return <Screen>…</Screen>, not {}",
                        other.describe()
                    ),
                    "every top-level component is a screen in subset v1",
                );
                return None;
            }
        };
        let mut lower = Lower {
            cx: self,
            scope,
            nodes: 0,
            too_many_nodes: false,
            too_deep: false,
        };
        match lower.resolve(element) {
            Some(KitComponent::Screen) => {}
            Some(other) => {
                lower.cx.error_help(
                    Code::Unsupported,
                    element.loc,
                    format!("component `{source_name}` returns <{other}>; a screen component returns <Screen>"),
                    "a component the file uses as an element is expanded where it is used; one used nowhere is a screen, which returns <Screen>",
                );
                // Still report what is inside, for a complete list.
                let _ = lower.node(element, 1);
                return None;
            }
            None => {
                lower.descend(&element.children, 1);
                return None;
            }
        }
        let root = lower.screen(element)?;
        Some(Screen {
            name,
            root,
            loc: Some(component.loc),
        })
    }

    /// The expression a component returns, reporting every other statement.
    fn returned<'f>(&mut self, name: &str, function: &'f Function) -> Option<&'f Expr> {
        let statements = match &function.body {
            Body::Expr(expr) => return Some(expr),
            Body::Block(statements) => statements,
        };
        let mut returned = None;
        let mut ok = true;
        for statement in statements {
            match statement {
                Stmt::Return {
                    value: Some(value),
                    loc,
                } => {
                    if returned.is_some() {
                        ok = false;
                        self.error(
                            Code::Unsupported,
                            *loc,
                            "a component may have only one `return`",
                        );
                    } else {
                        returned = Some(value);
                    }
                }
                Stmt::Return { value: None, loc } => {
                    ok = false;
                    self.error(
                        Code::Unsupported,
                        *loc,
                        format!("component `{name}` returns nothing"),
                    );
                }
                Stmt::Other(other) => {
                    ok = false;
                    let help = if other.kind.starts_with("React hook") {
                        "state and effects are outside subset v1: keep state in the application and pass callbacks as props"
                    } else {
                        "a screen component's body is only `return <Screen>…</Screen>;`"
                    };
                    self.error_help(
                        Code::Unsupported,
                        other.loc,
                        format!("{} in a component body is not supported", other.kind),
                        help,
                    );
                }
            }
        }
        if returned.is_none() && ok {
            self.error(
                Code::Unsupported,
                function.loc,
                format!("component `{name}` must return <Screen>…</Screen>"),
            );
        }
        returned
    }
}

/// The component's parameters, which shadow module-level names.
enum Scope<'a> {
    None,
    Destructured(&'a [PropBinding]),
    Object(&'a str),
}

/// What [`Lower::prop_of`] found.
enum PropRef {
    /// A prop of the component: its key, and where it is written.
    Prop(String, Loc),
    /// A name or member that is not a prop, already reported.
    Reported,
    /// Some other expression, not reported.
    NotAProp,
}

/// What a `<Text>`'s content is.
enum PropText {
    /// One prop alone: a slot named after it.
    Slot(Name),
    /// One prop between literal text (contract v14): a slot named after
    /// it, the literals compiled in.
    Template(Name, Template),
    /// One prop alone that cannot name a slot, or a name that is not a
    /// prop: already reported.
    Invalid,
    /// Anything else: literal text, checked by `literal_text`.
    Literal,
}

/// What an element's `style={{…}}` declares, each property parsed
/// as the component allows; everything else already reported.
#[derive(Default)]
struct StyleProps {
    gap: Option<Length>,
    padding: Option<Length>,
    width: Option<Length>,
    height: Option<Length>,
    min_width: Option<Length>,
    max_width: Option<Length>,
    align: Option<AlignItems>,
    justify: Option<Justify>,
    /// A container's fill, border and corners; a button's fill and label.
    look: Look,
    /// `<Text>`'s `textAlign`.
    text_align: Option<TextAlign>,
    /// `<Text>`'s `color`.
    colour: Option<Colour>,
    /// `<Image>`'s `objectFit`.
    fit: Option<Fit>,
    /// Where `style` is written.
    at: Option<Loc>,
}

/// The props of one element, as far as they were valid.
#[derive(Default)]
struct Props {
    id: Option<Name>,
    id_given: bool,
    gap: Option<Length>,
    padding: Option<Length>,
    size: Option<Length>,
    placeholder: Option<String>,
    on_press: Option<Name>,
    /// `onPress` or `onClick`, whichever is given first.
    on_press_given: Option<&'static str>,
    max_len: Option<Length>,
    /// Whether `maxLength` was given, valid or not.
    max_len_given: bool,
    mode: InputMode,
    label: Option<String>,
    label_given: bool,
    options: Option<Vec<String>>,
    options_given: bool,
    /// `<Text role="…">`.
    role: Option<TextRole>,
    /// `<Text weight="…">`.
    weight: Option<Weight>,
    /// Where `weight` is written.
    weight_at: Option<Loc>,
    /// `<Text lines={…}>`: the most lines it may wrap onto.
    lines: Option<u8>,
    /// `<Text align="…">`: where its lines sit.
    align: Option<TextAlign>,
    /// `<Icon name="…">`: one of [`ICON_NAMES`].
    icon: Option<String>,
    /// Whether `name` was given, valid or not.
    icon_given: bool,
    /// `<ProgressBar value={…}>`: the prop naming its slot.
    value: Option<Name>,
    /// Whether `value` was given, valid or not.
    value_given: bool,
    /// Whether `role`, `weight`, `lines` or `align` was given but invalid
    /// (already reported).
    style_invalid: bool,
    /// `<Button variant="…">`; primary when not given.
    variant: Option<ButtonVariant>,
    /// `<Notice tone="…">`.
    tone: Option<Tone>,
    /// Whether `tone` was given, valid or not.
    tone_given: bool,
    /// `style={{…}}`.
    style: StyleProps,
    /// `<Select rows={…}>`.
    rows: Option<u8>,
    /// Whether `rows` was given but invalid (already reported).
    rows_invalid: bool,
    /// `<FieldError for="…">`: the `id` of the control whose errors
    /// it shows.
    field_for: Option<Name>,
    /// `<Image src="…">`: one of the product's pictures.
    picture: Option<String>,
    /// Whether `src` was given, valid or not.
    picture_given: bool,
    /// `<Image alt="…">`.
    alt: Option<String>,
    /// Whether `alt` was given, valid or not.
    alt_given: bool,
    /// `<Image fit="…">`.
    fit: Option<Fit>,
    /// Whether `for` was given, valid or not.
    for_given: bool,
}

/// What a condition tests.
enum Test {
    /// A prop: `props.ready`.
    Truthy(Name),
    /// A prop negated: `!props.ready`.
    Falsy(Name),
    /// A prop compared with a string literal: `props.step === "review"`.
    Is {
        /// The prop, which names the switch.
        switch: Name,
        /// The literal, which names the case.
        when: Name,
    },
}

/// A piece of a `<Text>`'s content.
enum TextPiece<'e> {
    /// Literal text, and where it is written.
    Literal(&'e str, Loc),
    /// An expression.
    Expr(&'e Expr),
}

/// Lowering of one screen.
struct Lower<'c, 'o, 'a> {
    cx: &'c mut Cx<'o>,
    scope: Scope<'a>,
    nodes: usize,
    too_many_nodes: bool,
    too_deep: bool,
}

impl Lower<'_, '_, '_> {
    /// What `name`, written at `loc`, refers to: in the screen's own file a
    /// prop of the screen's or a name at its top level; in another file a
    /// name at that file's top level.
    fn lookup(&self, name: &str, loc: Loc) -> Option<Binding> {
        // A prop the expansion made: a list row's group or slot.
        if let Some(made) = name.strip_prefix(parts::MADE_PROP) {
            return Some(Binding::Prop(made.to_owned()));
        }
        if loc.file != 0 {
            return self.cx.binding(loc.file, name).cloned();
        }
        match &self.scope {
            Scope::Destructured(bindings) => {
                if let Some(binding) = bindings.iter().find(|b| b.local == name) {
                    return Some(Binding::Prop(binding.key.clone()));
                }
            }
            Scope::Object(object) if *object == name => return Some(Binding::PropsObject),
            _ => {}
        }
        self.cx.binding(0, name).cloned()
    }

    fn limits(&self) -> &crate::Limits {
        &self.cx.options.limits
    }

    /// The kit component an element names, or `None` after reporting why not.
    fn resolve(&mut self, element: &Element) -> Option<KitComponent> {
        let loc = element.loc;
        let kit = self.cx.options.kit_module.clone();
        match &element.name {
            ElementName::Component(name) => match self.lookup(name, loc) {
                Some(Binding::Kit(component)) => Some(component),
                // Reported where it is imported.
                Some(Binding::OtherImport) => None,
                Some(Binding::Component(_)) => {
                    self.cx.error_help(
                        Code::UnknownElement,
                        loc,
                        format!("custom component <{name}> is not supported in subset v1"),
                        "use the kit's elements directly",
                    );
                    None
                }
                _ if KitComponent::from_name(name).is_some() => {
                    self.cx.error_help(
                        Code::UnknownElement,
                        loc,
                        format!("<{name}> is not imported from \"{kit}\""),
                        format!("add `import {{ {name} }} from \"{kit}\";`"),
                    );
                    None
                }
                _ => {
                    self.cx.error_help(
                        Code::UnknownElement,
                        loc,
                        format!("<{name}> is not a component of subset v1"),
                        format!("use {} from \"{kit}\"", KitComponent::list("or")),
                    );
                    None
                }
            },
            ElementName::Intrinsic(tag) => {
                self.cx.error_help(
                    Code::UnknownElement,
                    loc,
                    format!("HTML element <{tag}> is not supported"),
                    format!(
                        "use the kit's elements: {}",
                        KitComponent::list("or").replacen("Screen, ", "", 1)
                    ),
                );
                None
            }
            ElementName::Other(text) => {
                self.cx.error_help(
                    Code::UnknownElement,
                    loc,
                    format!("<{text}> is not supported"),
                    format!("import the kit's components by name from \"{kit}\""),
                );
                None
            }
        }
    }

    /// `<Screen>`: no props, exactly one child element.
    fn screen(&mut self, element: &Element) -> Option<Node> {
        for attr in &element.attrs {
            let loc = attr_loc(attr);
            self.cx.error(
                Code::UnknownProp,
                loc,
                "<Screen> takes no props in subset v1",
            );
        }
        let mut roots: Vec<&Element> = Vec::new();
        for child in &element.children {
            match child {
                Child::Element(e) => roots.push(e),
                Child::Expr(Expr::Element(e)) => roots.push(e),
                other => self.bad_container_child(KitComponent::Screen, other, 0),
            }
        }
        match roots.as_slice() {
            [root] => self.node(root, 1),
            [] => {
                self.cx.error_help(
                    Code::InvalidChildren,
                    element.loc,
                    "<Screen> needs one child element",
                    "put the screen's content in a <Column>",
                );
                None
            }
            [_, second, ..] => {
                self.cx.error_help(
                    Code::InvalidChildren,
                    second.loc,
                    "<Screen> takes exactly one child element",
                    "wrap the elements in a <Column> or <Row>",
                );
                for root in &roots {
                    let _ = self.node(root, 1);
                }
                None
            }
        }
    }

    fn node(&mut self, element: &Element, depth: usize) -> Option<Node> {
        let Some(component) = self.resolve(element) else {
            self.descend(&element.children, depth);
            return None;
        };
        if component == KitComponent::Screen {
            self.cx.error(
                Code::InvalidChildren,
                element.loc,
                "<Screen> may only be the element a component returns",
            );
            return None;
        }
        if !self.admit(element.loc, depth) {
            return None;
        }
        let props = self.props(component, element);
        if matches!(
            component,
            KitComponent::TextInput
                | KitComponent::Checkbox
                | KitComponent::RadioGroup
                | KitComponent::Select
        ) {
            self.needs_id(component, element, &props);
        }
        let kind = match component {
            KitComponent::Column
            | KitComponent::Row
            | KitComponent::Card
            | KitComponent::Notice
            | KitComponent::HeaderBar => {
                let surface = match component {
                    KitComponent::Card => Some(Surface::Card),
                    KitComponent::HeaderBar => Some(Surface::Header),
                    KitComponent::Notice => {
                        if !props.tone_given {
                            self.cx.error_help(
                                Code::MissingProp,
                                element.loc,
                                "<Notice> needs a tone",
                                "write tone=\"success\", tone=\"caution\" or tone=\"warning\"",
                            );
                        }
                        props.tone.map(Surface::Notice)
                    }
                    _ => None,
                };
                let at = props.style.at;
                let gap = self.once("gap", props.gap, props.style.gap, at);
                let padding = self.once("padding", props.padding, props.style.padding, at);
                let container = Container {
                    gap,
                    padding,
                    children: self.container_children(component, &element.children, depth),
                    surface,
                    width: props.style.width,
                    height: props.style.height,
                    min_width: props.style.min_width,
                    max_width: props.style.max_width,
                    align: props.style.align,
                    justify: props.style.justify,
                    look: props.style.look,
                };
                if component == KitComponent::Notice && surface.is_none() {
                    return None;
                }
                if matches!(component, KitComponent::Row | KitComponent::HeaderBar) {
                    NodeKind::Row(container)
                } else {
                    NodeKind::Column(container)
                }
            }
            KitComponent::Text if props.max_len_given => {
                // Paragraphs of any length (contract v16): one prop alone,
                // wrapped onto as many lines as it needs.
                let help = "write <Text maxLength={10240}>{props.article}</Text>: one prop alone, wrapped onto as many lines as it needs";
                if props.lines.is_some() {
                    self.cx.error_help(
                        Code::InvalidValue,
                        element.loc,
                        "a <Text> with maxLength takes as many lines as it needs, not `lines`",
                        help,
                    );
                    return None;
                }
                let style = self.text_style(&props);
                let role = style.map_or(TextRole::Body, |style| style.role);
                if role == TextRole::Display {
                    self.cx.error_help(
                        Code::InvalidValue,
                        element.loc,
                        "a <Text> with maxLength cannot be in the display role",
                        help,
                    );
                    return None;
                }
                match self.prop_text(element, role) {
                    PropText::Slot(slot) => NodeKind::Slot {
                        slot,
                        style: style?,
                        template: None,
                        capacity: Some(props.max_len?),
                    },
                    PropText::Invalid => return None,
                    PropText::Template(..) | PropText::Literal => {
                        self.cx.error_help(
                            Code::InvalidValue,
                            element.loc,
                            "a <Text> with maxLength shows one prop the application sets, alone",
                            help,
                        );
                        return None;
                    }
                }
            }
            KitComponent::Text => {
                let style = self.text_style(&props);
                let role = style.map_or(TextRole::Body, |style| style.role);
                match self.prop_text(element, role) {
                    PropText::Slot(slot) => NodeKind::Slot {
                        slot,
                        style: style?,
                        template: None,
                        capacity: None,
                    },
                    PropText::Template(slot, template) => NodeKind::Slot {
                        slot,
                        style: style?,
                        template: Some(template),
                        capacity: None,
                    },
                    PropText::Invalid => return None,
                    PropText::Literal => {
                        let role = style.map_or(TextRole::Body, |style| style.role);
                        let text = self.literal_text(component, element, role);
                        NodeKind::Text {
                            text: text?,
                            style: style?,
                        }
                    }
                }
            }
            // One child shown through a view that scrolls; at most
            // one in a screen, never nested (checked by `validate`).
            KitComponent::ScrollView => {
                let children = self.container_children(component, &element.children, depth);
                if children.len() != 1 {
                    self.cx.error_help(
                        Code::InvalidChildren,
                        element.loc,
                        format!(
                            "<ScrollView> holds exactly one child element, not {}",
                            children.len()
                        ),
                        "put what scrolls in one <Column>: <ScrollView><Column>…</Column></ScrollView>",
                    );
                    return None;
                }
                NodeKind::ScrollView { children }
            }
            KitComponent::Icon => {
                self.no_children(component, element);
                if !props.icon_given {
                    self.cx.error_help(
                        Code::MissingProp,
                        element.loc,
                        "<Icon> needs a name",
                        format!("write name=\"fuel\": one of {}", ICON_NAMES.join(", ")),
                    );
                }
                NodeKind::Icon { name: props.icon? }
            }
            KitComponent::Image => {
                self.no_children(component, element);
                for (given, prop, help) in [
                    (
                        props.picture_given,
                        "src",
                        "write src=\"harbour\": the name of one of the product's pictures",
                    ),
                    (
                        props.alt_given,
                        "alt",
                        "write alt=\"…\" saying what it shows, or alt=\"\" for a picture that is only decoration",
                    ),
                ] {
                    if !given {
                        self.cx.error_help(
                            Code::MissingProp,
                            element.loc,
                            format!("<Image> needs {prop}"),
                            help,
                        );
                    }
                }
                if props.fit.is_some() && props.style.fit.is_some() {
                    self.cx.error_help(
                        Code::DuplicateName,
                        element.loc,
                        "<Image> is given both fit and a style's objectFit",
                        "give it one of them",
                    );
                    return None;
                }
                NodeKind::Image {
                    picture: props.picture?,
                    alt: props.alt?,
                    fit: props.fit.or(props.style.fit).unwrap_or_default(),
                    width: props.style.width,
                    height: props.style.height,
                }
            }
            KitComponent::ProgressBar => {
                self.no_children(component, element);
                if !props.value_given {
                    self.cx.error_help(
                        Code::MissingProp,
                        element.loc,
                        "<ProgressBar> needs a value prop",
                        "write value={props.fuelled}, where fuelled is a prop of the component",
                    );
                }
                NodeKind::Progress { slot: props.value? }
            }
            KitComponent::Button => {
                let label = self.literal_text(component, element, TextRole::Body);
                if props.on_press_given.is_none() {
                    self.cx.error_help(
                        Code::MissingProp,
                        element.loc,
                        "<Button> needs an onPress (or onClick) callback prop",
                        "write onPress={onContinue}, where onContinue is a prop of the component",
                    );
                }
                NodeKind::Button {
                    label: label?,
                    action: props.on_press?,
                    variant: props.variant.unwrap_or_default(),
                    look: props.style.look,
                }
            }
            KitComponent::TextInput => {
                self.no_children(component, element);
                props.id.as_ref()?;
                NodeKind::TextInput {
                    placeholder: props.placeholder,
                    max_len: props.max_len,
                    mode: props.mode,
                }
            }
            KitComponent::Checkbox => {
                self.no_children(component, element);
                if !props.label_given {
                    self.cx.error_help(
                        Code::MissingProp,
                        element.loc,
                        "<Checkbox> needs a label",
                        "write label=\"Remind me\"",
                    );
                }
                props.id.as_ref()?;
                NodeKind::Checkbox {
                    label: props.label?,
                }
            }
            KitComponent::RadioGroup => {
                self.no_children(component, element);
                if !props.options_given {
                    self.cx.error_help(
                        Code::MissingProp,
                        element.loc,
                        "<RadioGroup> needs its options",
                        "write options={[\"Morning\", \"Afternoon\"]}",
                    );
                }
                props.id.as_ref()?;
                NodeKind::RadioGroup {
                    options: props.options?,
                }
            }
            // A list picker.
            KitComponent::Select => {
                self.no_children(component, element);
                if !props.options_given {
                    self.cx.error_help(
                        Code::MissingProp,
                        element.loc,
                        "<Select> needs its options",
                        "write options={[\"Bag\", \"Phone\", \"Keys\"]}",
                    );
                }
                props.id.as_ref()?;
                if props.rows_invalid {
                    return None;
                }
                NodeKind::Select {
                    options: props.options?,
                    placeholder: props.placeholder,
                    rows: props.rows,
                }
            }
            KitComponent::Spacer => {
                self.no_children(component, element);
                NodeKind::Spacer { size: props.size }
            }
            // The on-screen keyboard; at most one in a screen, and
            // only in one with a text input (checked by `validate`).
            KitComponent::Keyboard => {
                self.no_children(component, element);
                NodeKind::Keyboard
            }
            // A field's error line; `for` names a control on the
            // screen, one line per field (checked by `validate`).
            KitComponent::FieldError => {
                self.no_children(component, element);
                if !props.for_given {
                    self.cx.error_help(
                        Code::MissingProp,
                        element.loc,
                        "<FieldError> needs the control it is for",
                        "write for=\"phone\", the id of the control whose errors it shows",
                    );
                }
                NodeKind::FieldError {
                    field: props.field_for?,
                }
            }
            KitComponent::Screen => return None,
        };
        Some(Node {
            kind,
            id: props.id,
            loc: Some(element.loc),
        })
    }

    /// Reports a value-holding control without an `id`: the application
    /// reads and sets its value through it, by field number.
    fn needs_id(&mut self, component: KitComponent, element: &Element, props: &Props) {
        if !props.id_given {
            self.cx.error_help(
                Code::MissingProp,
                element.loc,
                format!("<{component}> needs an id"),
                "write id=\"name\": the application reads the field through it",
            );
        }
    }

    /// Counts one more node at `depth`, reporting (once each) a screen
    /// nested too deeply or holding too many nodes; `false` if it is either.
    fn admit(&mut self, loc: Loc, depth: usize) -> bool {
        if depth > self.limits().max_depth {
            if !self.too_deep {
                self.too_deep = true;
                let max = self.limits().max_depth;
                self.cx.error(
                    Code::Limit,
                    loc,
                    format!("elements are nested more than {max} levels deep"),
                );
            }
            return false;
        }
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > self.limits().max_nodes {
            if !self.too_many_nodes {
                self.too_many_nodes = true;
                let max = self.limits().max_nodes;
                self.cx.error(
                    Code::Limit,
                    loc,
                    format!("the screen has more than {max} elements"),
                );
            }
            return false;
        }
        true
    }

    /// Conditional rendering (subset v1): `{props.NAME && <X/>}` becomes a
    /// group showing `<X/>` while the prop is true, and `{props.NAME ? <X/>
    /// : <Y/>}` (or `: null`) one choosing between the two. The group is a
    /// node of its own at `depth`, its content one level below. Anything
    /// else is reported where it is.
    fn conditional(&mut self, expr: &Expr, depth: usize) -> Option<Node> {
        let inner = depth.saturating_add(1);
        let (test, children, loc) = match expr {
            Expr::Logical {
                op: LogicalOp::And,
                left,
                right,
                loc,
            } => {
                let test = self.test(left);
                let shown = self.branch(right, inner);
                (test, vec![shown?], *loc)
            }
            Expr::Logical { op, loc, .. } => {
                let op = if *op == LogicalOp::Or { "||" } else { "??" };
                self.cx.error_help(
                    Code::Unsupported,
                    *loc,
                    format!("`{op}` in conditional rendering is not supported"),
                    "show or hide an element with {props.NAME && <Element />}, or choose with {props.NAME ? <A /> : <B />}",
                );
                return None;
            }
            Expr::Conditional {
                test,
                consequent,
                alternate,
                loc,
            } => {
                let test = self.test(test);
                if let Expr::Null { loc: at } = consequent.as_ref() {
                    // `p ? null : <X/>` is `!p && <X/>`.
                    let test = match test {
                        Some(Test::Truthy(name)) => Some(Test::Falsy(name)),
                        Some(Test::Falsy(name)) => Some(Test::Truthy(name)),
                        Some(Test::Is { switch, .. }) => {
                            self.cx.error_help(
                                Code::Unsupported,
                                *at,
                                format!("`null` while `{}` is on one case is not supported", switch.source()),
                                "show content on the cases that have it: {props.NAME === \"case\" && <Element />}",
                            );
                            None
                        }
                        None => None,
                    };
                    let shown = self.branch(alternate, inner);
                    return self.decided(test, vec![shown?], *loc, depth);
                }
                let shown = self.branch(consequent, inner);
                let otherwise = match alternate.as_ref() {
                    Expr::Null { .. } => None,
                    other => Some(self.branch(other, inner)),
                };
                let mut children = vec![shown?];
                if let Some(otherwise) = otherwise {
                    children.push(otherwise?);
                }
                (test, children, *loc)
            }
            _ => return None,
        };
        self.decided(test, children, loc, depth)
    }

    /// The node a condition and what it shows make.
    fn decided(
        &mut self,
        test: Option<Test>,
        children: Vec<Node>,
        loc: Loc,
        depth: usize,
    ) -> Option<Node> {
        let kind = match test? {
            Test::Truthy(name) => NodeKind::Group(Group {
                name,
                children,
                unless: false,
            }),
            // `!p ? <X/> : <Y/>` is `p ? <Y/> : <X/>`.
            Test::Falsy(name) if children.len() > 1 => NodeKind::Group(Group {
                name,
                children: children.into_iter().rev().collect(),
                unless: false,
            }),
            Test::Falsy(name) => NodeKind::Group(Group {
                name,
                children,
                unless: true,
            }),
            Test::Is { switch, when } => NodeKind::Case(Case {
                switch,
                when,
                children,
            }),
        };
        if !self.admit(loc, depth) {
            return None;
        }
        Some(Node {
            kind,
            id: None,
            loc: Some(loc),
        })
    }

    /// What a condition tests (contract v13 beyond one prop's truth): a
    /// prop, `!` a prop, or a prop `===` a string literal - one case of the
    /// prop's switch. Anything else is reported.
    fn test(&mut self, expr: &Expr) -> Option<Test> {
        match expr {
            Expr::Not { operand, loc } => {
                if matches!(operand.as_ref(), Expr::Not { .. }) {
                    self.cx.error_help(
                        Code::Unsupported,
                        *loc,
                        "`!!` in a condition is not supported",
                        "test the prop itself: {props.NAME && …}",
                    );
                    return None;
                }
                self.condition(operand).map(Test::Falsy)
            }
            Expr::Equals { left, right, loc } => {
                let literal = |side: &Expr| match side {
                    Expr::Str { value, loc } => Some((value.clone(), *loc)),
                    _ => None,
                };
                let found = match (literal(left), literal(right)) {
                    (Some(literal), None) => Some((right.as_ref(), literal)),
                    (None, Some(literal)) => Some((left.as_ref(), literal)),
                    (Some(_), Some(_)) | (None, None) => None,
                };
                let Some((prop, (value, at))) = found else {
                    self.cx.error_help(
                        Code::Unsupported,
                        *loc,
                        "a comparison in a condition is one prop `===` a string literal",
                        "write {props.step === \"review\" && …}; compare anything else in the application and pass the result as a prop",
                    );
                    return None;
                };
                let switch = self.condition(prop)?;
                match Name::parse(&value) {
                    Ok(when) => Some(Test::Is { switch, when }),
                    Err(error) => {
                        self.cx.error_help(
                            Code::InvalidValue,
                            at,
                            format!(
                                "\"{value}\" cannot name a case of `{}` in generated code: {error}",
                                switch.source()
                            ),
                            "a case is a word or two, such as \"review\" or \"step-2\"",
                        );
                        None
                    }
                }
            }
            other => self.condition(other).map(Test::Truthy),
        }
    }

    /// What a condition shows: exactly one element.
    fn branch(&mut self, expr: &Expr, depth: usize) -> Option<Node> {
        let help = "conditional rendering shows one element: wrap several in a <Column> or <Row>";
        match expr {
            Expr::Element(element) => self.node(element, depth),
            Expr::Fragment { children, loc } => {
                self.cx.error_help(
                    Code::Unsupported,
                    *loc,
                    "fragments (<>…</>) are not supported",
                    help,
                );
                self.descend(children, depth);
                None
            }
            Expr::Logical { loc, .. } | Expr::Conditional { loc, .. } => {
                self.cx.error_help(
                    Code::Unsupported,
                    *loc,
                    "a condition inside a condition is not supported",
                    "combine the conditions in the application and pass one prop, or nest them inside a <Column>",
                );
                None
            }
            Expr::Str { loc, .. } => {
                self.cx.error_help(
                    Code::InvalidChildren,
                    *loc,
                    "text shown by a condition must be in an element",
                    "wrap it in <Text>…</Text>",
                );
                None
            }
            other => {
                self.cx.error_help(
                    Code::Unsupported,
                    other.loc(),
                    format!("{} shown by a condition is not supported", other.describe()),
                    help,
                );
                None
            }
        }
    }

    /// The prop a condition tests, which names its group: `props.NAME` or a
    /// destructured `NAME`. Anything else is reported.
    fn condition(&mut self, expr: &Expr) -> Option<Name> {
        let help = "a condition is one prop of the component: work anything more out in the application and pass it as a prop, such as {props.canRetry && …}";
        match self.prop_of(expr, "a condition") {
            PropRef::Prop(key, loc) => self.prop_name(&key, loc, "show or hide content"),
            PropRef::Reported => None,
            PropRef::NotAProp => {
                self.cx.error_help(
                    Code::Unsupported,
                    expr.loc(),
                    format!("{} as a condition is not supported", expr.describe()),
                    help,
                );
                None
            }
        }
    }

    /// Whether `expr` names one prop: `props.NAME`, or `NAME` destructured
    /// from the props. A name or member that is not one is reported here.
    fn prop_of(&mut self, expr: &Expr, role: &str) -> PropRef {
        match expr {
            Expr::Member {
                object,
                property,
                loc,
            } => {
                if matches!(self.lookup(object, *loc), Some(Binding::PropsObject)) {
                    PropRef::Prop(property.clone(), *loc)
                } else {
                    self.cx.error_help(
                        Code::UnknownName,
                        *loc,
                        format!("`{object}.{property}` is not a prop of the component"),
                        format!(
                            "{role} must be a prop: props.NAME, or NAME destructured from the props"
                        ),
                    );
                    PropRef::Reported
                }
            }
            Expr::Ident { name, loc } => match self.lookup(name, *loc) {
                Some(Binding::Prop(key)) => PropRef::Prop(key, *loc),
                Some(Binding::PropsObject) => {
                    self.cx.error_help(
                        Code::InvalidValue,
                        *loc,
                        format!("{role} takes one prop, not the whole props object"),
                        format!("write {name}.NAME"),
                    );
                    PropRef::Reported
                }
                _ => {
                    self.cx.error_help(
                        Code::UnknownName,
                        *loc,
                        format!("`{name}` is not a prop of the component"),
                        format!("{role} must be a prop: take it as one, function Screen({{ {name} }}) {{ … }}"),
                    );
                    PropRef::Reported
                }
            },
            _ => PropRef::NotAProp,
        }
    }

    /// The name of the slot or group prop `key` controls.
    fn prop_name(&mut self, key: &str, loc: Loc, what: &str) -> Option<Name> {
        match Name::parse(key) {
            Ok(name) => Some(name),
            Err(error) => {
                self.cx.error(
                    Code::InvalidValue,
                    loc,
                    format!("prop `{key}` cannot {what} in generated code: {error}"),
                );
                None
            }
        }
    }

    /// A `<Text>` whose content is a prop (subset v1), alone or between
    /// literal text, written in JSX or as a template literal (contract
    /// v14): a slot the application fills. Text with no expression in it is
    /// literal text, and anything else is reported by
    /// [`literal_text`](Self::literal_text) - but a text with more than one
    /// prop, reported here.
    fn prop_text(&mut self, element: &Element, role: TextRole) -> PropText {
        // The content, piece by piece: literal text, or an expression.
        let mut pieces: Vec<TextPiece<'_>> = Vec::new();
        for child in &element.children {
            match child {
                Child::Empty { .. } => {}
                Child::Text { value, loc } | Child::Expr(Expr::Str { value, loc }) => {
                    pieces.push(TextPiece::Literal(value, *loc));
                }
                Child::Expr(Expr::Template {
                    pieces: texts,
                    exprs,
                    ..
                }) => {
                    for (at, text) in texts.iter().enumerate() {
                        let Some(value) = &text.text else {
                            return PropText::Literal;
                        };
                        pieces.push(TextPiece::Literal(value, text.loc));
                        match exprs.get(at) {
                            Some(Expr::Str { value, loc }) => {
                                pieces.push(TextPiece::Literal(value, *loc));
                            }
                            Some(expr) => pieces.push(TextPiece::Expr(expr)),
                            None => {}
                        }
                    }
                }
                Child::Expr(expr) => pieces.push(TextPiece::Expr(expr)),
                _ => return PropText::Literal,
            }
        }
        let exprs: Vec<&Expr> = pieces
            .iter()
            .filter_map(|piece| match piece {
                TextPiece::Expr(expr) => Some(*expr),
                TextPiece::Literal(..) => None,
            })
            .collect();
        // Literal text; or an expression only `literal_text` reports.
        if exprs.is_empty()
            || exprs
                .iter()
                .any(|expr| !matches!(expr, Expr::Ident { .. } | Expr::Member { .. }))
        {
            return PropText::Literal;
        }
        let mut props = Vec::new();
        for expr in &exprs {
            match self.prop_of(expr, "text from the application") {
                PropRef::Prop(key, loc) => props.push((key, loc)),
                PropRef::Reported | PropRef::NotAProp => return PropText::Invalid,
            }
        }
        if let Some((key, loc)) = props.get(1) {
            self.cx.error_help(
                Code::Unsupported,
                *loc,
                format!("`{key}` is a second prop in one <Text>; a text holds one prop at most"),
                "give each prop its text: <Row><Text>{props.a}</Text><Text> of {props.b}</Text></Row>, or join them in the application",
            );
            return PropText::Invalid;
        }
        let Some((key, loc)) = props.into_iter().next() else {
            return PropText::Literal;
        };
        let Some(slot) = self.prop_name(&key, loc, "name a slot") else {
            return PropText::Invalid;
        };
        // The literal text either side of the prop, checked as any text.
        let mut template = Template {
            before: String::new(),
            after: String::new(),
        };
        let mut ok = true;
        let mut after = false;
        for piece in &pieces {
            match piece {
                TextPiece::Expr(_) => after = true,
                TextPiece::Literal(value, at) => {
                    ok &= self.visible_text("<Text>", value, *at)
                        && self.drawn_text("<Text>", value, *at, role);
                    if after {
                        template.after.push_str(value);
                    } else {
                        template.before.push_str(value);
                    }
                }
            }
        }
        if !ok {
            return PropText::Invalid;
        }
        if template.before.is_empty() && template.after.is_empty() {
            return PropText::Slot(slot);
        }
        let bytes = template.before.len().saturating_add(template.after.len());
        if bytes > MAX_TEMPLATE_BYTES {
            self.cx.error_help(
                Code::Limit,
                element.loc,
                format!(
                    "the literal text around `{key}` is {bytes} bytes; at most {MAX_TEMPLATE_BYTES}"
                ),
                "put the rest in a <Text> of its own",
            );
            return PropText::Invalid;
        }
        PropText::Template(slot, template)
    }

    /// Children of an element that is not converted: report what is inside.
    fn descend(&mut self, children: &[Child], depth: usize) {
        let inner = depth.saturating_add(1);
        for child in children {
            match child {
                Child::Element(e) => {
                    let _ = self.node(e, inner);
                }
                Child::Expr(Expr::Element(e)) => {
                    let _ = self.node(e, inner);
                }
                Child::Fragment { children, .. } => self.descend(children, inner),
                Child::Expr(Expr::Fragment { children, .. }) => self.descend(children, inner),
                Child::Expr(expr @ (Expr::Logical { .. } | Expr::Conditional { .. })) => {
                    let _ = self.conditional(expr, inner);
                }
                Child::Text { .. }
                | Child::InvalidText { .. }
                | Child::Empty { .. }
                | Child::Expr(Expr::Str { .. }) => {}
                other => self.bad_container_child(KitComponent::Column, other, depth),
            }
        }
    }

    fn container_children(
        &mut self,
        parent: KitComponent,
        children: &[Child],
        depth: usize,
    ) -> Vec<Node> {
        let inner = depth.saturating_add(1);
        let mut nodes = Vec::new();
        for child in children {
            match child {
                Child::Element(e) => nodes.extend(self.node(e, inner)),
                Child::Expr(Expr::Element(e)) => nodes.extend(self.node(e, inner)),
                Child::Expr(expr @ (Expr::Logical { .. } | Expr::Conditional { .. })) => {
                    nodes.extend(self.conditional(expr, inner));
                }
                other => self.bad_container_child(parent, other, depth),
            }
        }
        nodes
    }

    /// Reports a child that a container cannot hold.
    fn bad_container_child(&mut self, parent: KitComponent, child: &Child, depth: usize) {
        match child {
            Child::Element(_) | Child::Expr(Expr::Element(_)) | Child::Empty { .. } => {}
            Child::Text { loc, .. }
            | Child::InvalidText { loc }
            | Child::Expr(Expr::Str { loc, .. }) => self.cx.error_help(
                Code::InvalidChildren,
                *loc,
                format!("text directly inside <{parent}> is not supported"),
                "wrap it in <Text>…</Text>",
            ),
            Child::Fragment { children, loc } | Child::Expr(Expr::Fragment { children, loc }) => {
                self.cx.error_help(
                    Code::Unsupported,
                    *loc,
                    "fragments (<>…</>) are not supported",
                    "use a <Column> or <Row>",
                );
                self.descend(children, depth);
            }
            Child::Expr(Expr::Ident { name, loc }) => self.cx.error_help(
                Code::Unsupported,
                *loc,
                format!("`{{{name}}}` directly inside <{parent}> is not supported"),
                format!("text from a prop goes in a <Text> of its own: <Text>{{{name}}}</Text>"),
            ),
            Child::Expr(expr @ (Expr::Logical { .. } | Expr::Conditional { .. })) => {
                // Only <Screen> gets here: every other container converts it.
                self.cx.error_help(
                    Code::InvalidChildren,
                    expr.loc(),
                    format!("<{parent}>'s one child must be an element, not a condition"),
                    "put the condition inside the screen's <Column>",
                );
            }
            Child::Expr(other) => self.cx.error_help(
                Code::Unsupported,
                other.loc(),
                format!("{} as a child is not supported", other.describe()),
                "lists and computed content are outside subset v1; conditional rendering is {props.NAME && <Element />} or {props.NAME ? <A /> : <B />}",
            ),
            Child::Spread { loc } => self.cx.error(
                Code::Unsupported,
                *loc,
                "spread children ({...children}) are not supported",
            ),
        }
    }

    /// Reports an element inside `<Text>` or `<Button>`.
    fn not_text(&mut self, component: KitComponent, element: &Element) {
        self.cx.error(
            Code::InvalidChildren,
            element.loc,
            format!(
                "<{component}> may contain only text, not <{}>",
                element.name.display()
            ),
        );
    }

    /// The literal text of a `<Text>` or `<Button>`, in `role`.
    fn literal_text(
        &mut self,
        component: KitComponent,
        element: &Element,
        role: TextRole,
    ) -> Option<String> {
        let mut text = String::new();
        let mut ok = true;
        // A prop left unreported here, as something beside it is.
        let mut beside = false;
        let reported = self.cx.diagnostics.len();
        for child in &element.children {
            match child {
                Child::Text { value, loc } | Child::Expr(Expr::Str { value, loc }) => {
                    if self.visible_text(&format!("<{component}>"), value, *loc)
                        && self.drawn_text(&format!("<{component}>"), value, *loc, role)
                    {
                        text.push_str(value);
                    } else {
                        ok = false;
                    }
                }
                // Already reported by the parser front end.
                Child::InvalidText { .. } => ok = false,
                Child::Empty { .. } => {}
                Child::Element(e) => {
                    ok = false;
                    self.not_text(component, e);
                }
                Child::Expr(Expr::Element(e)) => {
                    ok = false;
                    self.not_text(component, e);
                }
                // A prop beside something else that is reported: the text
                // with one prop converts once that goes (`prop_text`).
                Child::Expr(Expr::Ident { .. } | Expr::Member { .. })
                    if component == KitComponent::Text =>
                {
                    ok = false;
                    beside = true;
                }
                // A template literal with no expression is literal text; one
                // with any is reported here only beside something else.
                Child::Expr(Expr::Template { pieces, exprs, .. })
                    if component == KitComponent::Text =>
                {
                    for (at, piece) in pieces.iter().enumerate() {
                        // The piece, then the string or prop after it.
                        let literal = match exprs.get(at) {
                            Some(Expr::Str { value, loc }) => Some((value, *loc)),
                            _ => None,
                        };
                        for (value, loc) in piece
                            .text
                            .as_ref()
                            .map(|value| (value, piece.loc))
                            .into_iter()
                            .chain(literal)
                        {
                            if self.visible_text(&format!("<{component}>"), value, loc)
                                && self.drawn_text(&format!("<{component}>"), value, loc, role)
                            {
                                text.push_str(value);
                            } else {
                                ok = false;
                            }
                        }
                        if piece.text.is_none() {
                            ok = false;
                            self.cx.error(
                                Code::Unsupported,
                                piece.loc,
                                "a template literal's escape that is not text is not supported",
                            );
                        }
                        match exprs.get(at) {
                            None | Some(Expr::Str { .. }) => {}
                            Some(Expr::Ident { .. } | Expr::Member { .. }) => {
                                ok = false;
                                beside = true;
                            }
                            Some(expr) => {
                                ok = false;
                                self.cx.error_help(
                                    Code::Unsupported,
                                    expr.loc(),
                                    format!("{} inside <{component}> is not supported", expr.describe()),
                                    "text is literal, with at most one prop: <Text>Hello, {props.name}!</Text>",
                                );
                            }
                        }
                    }
                }
                Child::Expr(other) => {
                    ok = false;
                    let help = if component == KitComponent::Text {
                        "text is literal, with at most one prop: <Text>Hello, {props.name}!</Text>"
                    } else {
                        "a button's label is literal text"
                    };
                    self.cx.error_help(
                        Code::Unsupported,
                        other.loc(),
                        format!("{} inside <{component}> is not supported", other.describe()),
                        help,
                    );
                }
                Child::Fragment { loc, .. } | Child::Spread { loc } => {
                    ok = false;
                    self.cx.error(
                        Code::InvalidChildren,
                        *loc,
                        format!("<{component}> may contain only text"),
                    );
                }
            }
        }
        if !ok {
            // Never refused in silence: a prop is left to what beside it
            // is reported, and something always is.
            if beside && self.cx.diagnostics.len() == reported {
                self.cx.error(
                    Code::Internal,
                    element.loc,
                    format!("<{component}>'s text was refused with no reason given; please report this input"),
                );
            }
            return None;
        }
        if text.is_empty() {
            self.cx.error_help(
                Code::InvalidChildren,
                element.loc,
                format!("<{component}> needs text"),
                format!("write <{component}>Continue</{component}>"),
            );
            return None;
        }
        self.within_text_limit(&text, element.loc).then_some(text)
    }

    /// Whether `text` holds only characters shown as themselves; otherwise
    /// reports the first other one (a control, invisible formatting or
    /// separator character), which the device could not show as a browser
    /// does.
    fn visible_text(&mut self, what: &str, text: &str, loc: Loc) -> bool {
        let Some((c, kind)) = text
            .chars()
            .find_map(|c| special_char(c).map(|kind| (c, kind)))
        else {
            return true;
        };
        self.cx.error_help(
            Code::Unsupported,
            loc,
            format!(
                "{what} contains U+{:04X}, {}, which is not supported",
                u32::from(c),
                kind.describe()
            ),
            "remove it: text in subset v1 is visible characters and spaces only",
        );
        false
    }

    /// Whether the glyph atlases of `role` draw every character of `text`;
    /// otherwise reports the first they do not, and where it is in
    /// the text. Text the device cannot draw is refused here, where it is
    /// written, rather than when the runtime builds the screen.
    fn drawn_text(&mut self, what: &str, text: &str, loc: Loc, role: TextRole) -> bool {
        let Some((index, c)) = text.chars().enumerate().find(|&(_, c)| !role.draws(c)) else {
            return true;
        };
        let atlases = if role == TextRole::Display {
            "the display role's glyph atlases"
        } else {
            "the device's glyph atlases"
        };
        self.cx.error_help(
            Code::Unsupported,
            loc,
            format!(
                "{what} contains {c:?} (U+{:04X}), character {} of its text, which {atlases} do not draw",
                u32::from(c),
                index.saturating_add(1)
            ),
            role.describe_chars(),
        );
        false
    }

    /// The style of a `<Text>`: its `role`, body by default, in its
    /// own weight unless `weight` names another, on one line unless `lines`
    /// lets it wrap, against the start unless `align` names another
    /// place. `None` if any was invalid, or `weight="regular"`
    /// names a role drawn only in Bold (display, title and heading),
    /// reported.
    fn text_style(&mut self, props: &Props) -> Option<TextStyle> {
        if props.style_invalid {
            return None;
        }
        let role = props.role.unwrap_or_default();
        // `align` or the style's `textAlign`, never both.
        let align = match (props.align, props.style.text_align) {
            (Some(align), Some(_)) => {
                if let Some(at) = props.style.at {
                    self.cx.error_help(
                        Code::DuplicateName,
                        at,
                        "the text's alignment is given twice: as `align` and as the style's `textAlign`",
                        "keep one",
                    );
                }
                let _ = align;
                return None;
            }
            (align, styled) => align.or(styled).unwrap_or_default(),
        };
        let mut style = TextStyle::new(role)
            .lines(props.lines.unwrap_or(1))
            .align(align);
        if let Some(colour) = props.style.colour {
            style = style.colour(colour);
        }
        match (props.weight, props.weight_at) {
            (Some(Weight::Regular), Some(at)) if role.bold_only() => {
                self.cx.error_help(
                    Code::InvalidValue,
                    at,
                    format!("the {role} role is drawn only in Bold"),
                    "remove weight=\"regular\", or choose the body, label or caption role",
                );
                None
            }
            (Some(Weight::Bold), _) => Some(style.bold()),
            _ => Some(style),
        }
    }

    /// `name="…"` on an `<Icon>`: a string literal, one of
    /// [`ICON_NAMES`].
    fn icon_name(&mut self, value: &AttrValue, loc: Loc) -> Option<String> {
        let help = format!("the icons are {}", ICON_NAMES.join(", "));
        let Some((text, at)) = literal_string(value) else {
            self.cx.error_help(
                Code::InvalidValue,
                loc,
                "`name` must be a string literal",
                help,
            );
            return None;
        };
        if ICON_NAMES.contains(&text) {
            Some(text.to_owned())
        } else {
            self.cx.error_help(
                Code::InvalidValue,
                at,
                format!("there is no icon named \"{text}\""),
                help,
            );
            None
        }
    }

    /// `src="…"` on an `<Image>`: a string literal, one of the product's
    /// pictures that `--pictures` names.
    fn picture_name(&mut self, value: &AttrValue, loc: Loc) -> Option<String> {
        let Some((text, at)) = literal_string(value) else {
            self.cx.error_help(
                Code::InvalidValue,
                loc,
                "`src` must be a string literal",
                "write src=\"harbour\": the name of one of the product's pictures",
            );
            return None;
        };
        let Some(names) = &self.cx.options.pictures else {
            self.cx.error_help(
                Code::InvalidValue,
                at,
                format!("there is no picture named \"{text}\": no pictures were given"),
                "convert with --pictures FILE, the names file the product's picture tool writes beside its pictures",
            );
            return None;
        };
        if names.iter().any(|name| name == text) {
            Some(text.to_owned())
        } else {
            let help = format!("the pictures are {}", names.join(", "));
            self.cx.error_help(
                Code::InvalidValue,
                at,
                format!("there is no picture named \"{text}\""),
                help,
            );
            None
        }
    }

    /// `value={…}` on a `<ProgressBar>`: a prop of the component,
    /// which names the bar's slot.
    fn progress_value(&mut self, value: &AttrValue, loc: Loc) -> Option<Name> {
        let help = "write value={props.fuelled}: how far the bar shows comes from the application";
        match value {
            AttrValue::Expr(expr) => match self.prop_of(expr, "a progress bar's value") {
                PropRef::Prop(key, at) => self.prop_name(&key, at, "name a progress bar"),
                PropRef::Reported => None,
                PropRef::NotAProp => {
                    self.cx.error_help(
                        Code::InvalidValue,
                        expr.loc(),
                        "`value` must be a prop of the component",
                        help,
                    );
                    None
                }
            },
            AttrValue::Str { loc, .. } | AttrValue::Empty { loc } => {
                self.cx
                    .error_help(Code::InvalidValue, *loc, "`value` must be a prop", help);
                None
            }
            AttrValue::Absent => {
                self.cx
                    .error_help(Code::InvalidValue, loc, "`value` needs a value", help);
                None
            }
            // Already reported by the parser front end.
            AttrValue::InvalidStr { .. } => None,
        }
    }

    /// `lines={…}` on a `<Text>`: a whole number from 1 to
    /// [`MAX_LINES`], written as a number.
    fn lines(&mut self, value: &AttrValue, loc: Loc) -> Option<u8> {
        let help = format!(
            "write lines={{3}}: the most lines the text may wrap onto, from 1 to {MAX_LINES}"
        );
        match value {
            AttrValue::Expr(Expr::Num { value, raw, loc }) => {
                let whole = value.is_finite()
                    && value.fract() == 0.0
                    && (1.0..=f64::from(MAX_LINES)).contains(value)
                    && !raw.starts_with('0');
                if whole {
                    // Exact: a whole number from 1 to 8.
                    Some(*value as u8)
                } else {
                    self.cx.error_help(
                        Code::InvalidValue,
                        *loc,
                        format!("`lines` must be a whole number from 1 to {MAX_LINES}, not {raw}"),
                        help,
                    );
                    None
                }
            }
            AttrValue::Expr(other) => {
                self.cx.error_help(
                    Code::InvalidValue,
                    other.loc(),
                    format!("`lines` must be a number, not {}", other.describe()),
                    help,
                );
                None
            }
            AttrValue::Str { loc, .. } | AttrValue::Empty { loc } => {
                self.cx
                    .error_help(Code::InvalidValue, *loc, "`lines` must be a number", help);
                None
            }
            AttrValue::Absent => {
                self.cx
                    .error_help(Code::InvalidValue, loc, "`lines` needs a value", help);
                None
            }
            // Already reported by the parser front end.
            AttrValue::InvalidStr { .. } => None,
        }
    }

    /// `role="…"` on a `<Text>`.
    fn text_role(&mut self, value: &AttrValue, loc: Loc) -> Option<TextRole> {
        let help =
            "role is \"display\", \"title\", \"heading\", \"body\", \"label\" or \"caption\"";
        let Some((text, at)) = literal_string(value) else {
            self.cx.error_help(
                Code::InvalidValue,
                loc,
                "`role` must be a string literal",
                help,
            );
            return None;
        };
        let role = TextRole::parse(text);
        if role.is_none() {
            self.cx.error_help(
                Code::InvalidValue,
                at,
                format!("\"{text}\" is not a text role"),
                help,
            );
        }
        role
    }

    /// `variant="…"` on a `<Button>`.
    fn variant(&mut self, value: &AttrValue, loc: Loc) -> Option<ButtonVariant> {
        let help = "variant is \"primary\" (the default), \"secondary\" or \"quiet\"";
        let Some((text, at)) = literal_string(value) else {
            self.cx.error_help(
                Code::InvalidValue,
                loc,
                "`variant` must be a string literal",
                help,
            );
            return None;
        };
        let variant = ButtonVariant::parse(text);
        if variant.is_none() {
            self.cx.error_help(
                Code::InvalidValue,
                at,
                format!("\"{text}\" is not a button variant"),
                help,
            );
        }
        variant
    }

    /// `tone="…"` on a `<Notice>`.
    fn tone(&mut self, value: &AttrValue, loc: Loc) -> Option<Tone> {
        let help = "tone is \"success\", \"caution\" or \"warning\"";
        let Some((text, at)) = literal_string(value) else {
            self.cx.error_help(
                Code::InvalidValue,
                loc,
                "`tone` must be a string literal",
                help,
            );
            return None;
        };
        let tone = Tone::parse(text);
        if tone.is_none() {
            self.cx.error_help(
                Code::InvalidValue,
                at,
                format!("\"{text}\" is not a tone"),
                help,
            );
        }
        tone
    }

    /// `weight="…"` on a `<Text>`.
    /// `prop` given as a prop or in the style at `at`, never both.
    fn once(
        &mut self,
        prop: &str,
        given: Option<Length>,
        styled: Option<Length>,
        at: Option<Loc>,
    ) -> Option<Length> {
        if given.is_some()
            && styled.is_some()
            && let Some(at) = at
        {
            self.cx.error_help(
                Code::DuplicateName,
                at,
                format!("`{prop}` is given twice: as a prop and in the style"),
                "keep one",
            );
            return None;
        }
        given.or(styled)
    }

    /// `style={{ … }}`: an object literal of the properties the
    /// component's style may name ([`KitComponent::style_properties`]),
    /// each with a static value - a theme colour's name, a keyword, a
    /// number of layout units, a design token or one of the theme's
    /// spacing tokens. Anything else is reported where written: another
    /// property, a colour value (`#fff`, `rgb(…)`, `red`), a value from a
    /// prop or computed, a spread, a computed key, or a style that is not an
    /// object literal. There is no cascade: a style is its element's alone.
    fn style_object(&mut self, component: KitComponent, value: &AttrValue, loc: Loc) -> StyleProps {
        let mut style = StyleProps {
            at: Some(loc),
            ..StyleProps::default()
        };
        let props = match value {
            AttrValue::Expr(Expr::Object { props, .. }) => props,
            // Already reported by the parser front end.
            AttrValue::InvalidStr { .. } => return style,
            other => {
                let at = match other {
                    AttrValue::Str { loc, .. } | AttrValue::Empty { loc } => *loc,
                    AttrValue::Expr(expr) => expr.loc(),
                    _ => loc,
                };
                self.cx.error_help(
                    Code::InvalidValue,
                    at,
                    "`style` must be an object literal of static properties",
                    "write style={{ gap: 8, backgroundColor: \"input\" }}: Screen Compiler does not implement CSS text",
                );
                return style;
            }
        };
        let allowed = component.style_names();
        let mut seen: Vec<&str> = Vec::new();
        for prop in props {
            let key = prop.key.as_str();
            if key.is_empty() {
                self.cx.error_help(
                    Code::Unsupported,
                    prop.loc,
                    format!("{} is not supported in a style", prop.value.describe()),
                    "write each property as name: value",
                );
                continue;
            }
            if seen.contains(&key) {
                self.cx.error(
                    Code::DuplicateName,
                    prop.loc,
                    format!("style property `{key}` is given more than once"),
                );
                continue;
            }
            seen.push(key);
            if !allowed.contains(&key) {
                self.cx.error_help(
                    Code::UnknownProp,
                    prop.loc,
                    format!("`{key}` is not a style property of <{component}> in subset v1"),
                    format!("<{component}>'s style takes {}", allowed.join(", ")),
                );
                continue;
            }
            let value = AttrValue::Expr(prop.value.clone());
            let at = prop.value.loc();
            match key {
                "gap" => style.gap = self.length(key, &value, at),
                "padding" => style.padding = self.length(key, &value, at),
                "width" => style.width = self.style_size(key, &value, at),
                "height" => style.height = self.style_size(key, &value, at),
                "minWidth" => style.min_width = self.style_size(key, &value, at),
                "maxWidth" => style.max_width = self.style_size(key, &value, at),
                "alignItems" => {
                    style.align =
                        self.keyword(key, &prop.value, &AlignItems::NAMES, AlignItems::parse);
                }
                "justifyContent" => {
                    style.justify = self.keyword(key, &prop.value, &Justify::NAMES, Justify::parse);
                }
                "textAlign" => {
                    style.text_align = self.keyword(
                        key,
                        &prop.value,
                        &TextAlign::STYLE_NAMES,
                        TextAlign::parse_style,
                    );
                }
                "backgroundColor" => style.look.fill = self.theme_colour(key, &prop.value),
                "borderColor" => style.look.border = self.theme_colour(key, &prop.value),
                "color" => {
                    let colour = self.theme_colour(key, &prop.value);
                    if component == KitComponent::Button {
                        style.look.text = colour;
                    } else {
                        style.colour = colour;
                    }
                }
                "borderWidth" => {
                    style.look.border_width = self.style_number(key, &prop.value, MAX_BORDER_WIDTH);
                }
                "borderRadius" => {
                    style.look.radius = self.style_number(key, &prop.value, MAX_RADIUS);
                }
                "objectFit" => {
                    style.fit = self.keyword(key, &prop.value, &Fit::NAMES, Fit::parse);
                }
                _ => {}
            }
        }
        style
    }

    /// A size in a style: a number of layout units or a design
    /// token the screen declares - not one of the theme's spacing tokens,
    /// which measure gaps and padding.
    fn style_size(&mut self, key: &str, value: &AttrValue, at: Loc) -> Option<Length> {
        match self.length(key, value, at)? {
            Length::Space(_) => {
                self.cx.error_help(
                    Code::InvalidValue,
                    at,
                    format!("`{key}` cannot be a spacing token"),
                    "the theme's spacing tokens measure gaps and padding: write a number of layout units or a design token",
                );
                None
            }
            length => Some(length),
        }
    }

    /// A whole number of layout units in a style, at most `most`:
    /// a border's width or a corner's radius.
    fn style_number(&mut self, key: &str, value: &Expr, most: u16) -> Option<u16> {
        let Expr::Num { value, raw, loc } = value else {
            self.cx.error_help(
                Code::InvalidValue,
                value.loc(),
                format!("`{key}` must be a number, not {}", value.describe()),
                format!("write {key}: 2, in layout units from 0 to {most}"),
            );
            return None;
        };
        let units = self.cx.pixels(*value, raw, *loc, &format!("`{key}`"))?;
        if units > most {
            self.cx.error(
                Code::InvalidValue,
                *loc,
                format!("`{key}` may be at most {most}, not {units}"),
            );
            return None;
        }
        Some(units)
    }

    /// One of `names`, as a string, in a style.
    fn keyword<T>(
        &mut self,
        key: &str,
        value: &Expr,
        names: &[&str],
        parse: impl Fn(&str) -> Option<T>,
    ) -> Option<T> {
        let help = format!("`{key}` is one of {}", quoted(names));
        let Expr::Str { value: text, loc } = value else {
            self.cx.error_help(
                Code::InvalidValue,
                value.loc(),
                format!("`{key}` must be a string literal, not {}", value.describe()),
                help,
            );
            return None;
        };
        let found = parse(text);
        if found.is_none() {
            self.cx.error_help(
                Code::InvalidValue,
                *loc,
                format!("\"{text}\" is not a value of `{key}`"),
                help,
            );
        }
        found
    }

    /// One of the theme's colours, by its name, in a style: never
    /// a colour value.
    fn theme_colour(&mut self, key: &str, value: &Expr) -> Option<Colour> {
        let help = format!(
            "colours are the theme's, so its large-text, high-contrast version recolours the screen: {}",
            quoted(&Colour::NAMES)
        );
        let Expr::Str { value: text, loc } = value else {
            self.cx.error_help(
                Code::InvalidValue,
                value.loc(),
                format!(
                    "`{key}` must name one of the theme's colours, not {}",
                    value.describe()
                ),
                help,
            );
            return None;
        };
        // CSS's custom property, `var(--text-muted)`, names it too; a
        // fallback would be a colour value.
        let named = match text
            .strip_prefix("var(--")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            Some(name) if name.contains(',') => {
                self.cx.error_help(
                    Code::InvalidValue,
                    *loc,
                    format!("\"{text}\" has a fallback, which would be a colour value"),
                    help,
                );
                return None;
            }
            Some(name) => name,
            None => text.as_str(),
        };
        let colour = Colour::parse(named);
        if colour.is_none() {
            self.cx.error_help(
                Code::InvalidValue,
                *loc,
                format!("\"{text}\" is not one of the theme's colours"),
                help,
            );
        }
        colour
    }

    /// `align="…"` on a `<Text>`: `start`, `center` or `end`.
    fn text_align(&mut self, value: &AttrValue, loc: Loc) -> Option<TextAlign> {
        let help = "align is \"start\", \"center\" or \"end\"";
        let Some((text, at)) = literal_string(value) else {
            self.cx.error_help(
                Code::InvalidValue,
                loc,
                "`align` must be a string literal",
                help,
            );
            return None;
        };
        let align = TextAlign::parse(text);
        if align.is_none() {
            self.cx.error_help(
                Code::InvalidValue,
                at,
                format!("\"{text}\" is not an alignment"),
                help,
            );
        }
        align
    }

    fn weight(&mut self, value: &AttrValue, loc: Loc) -> Option<Weight> {
        let help = "weight is \"regular\" or \"bold\"";
        let Some((text, at)) = literal_string(value) else {
            self.cx.error_help(
                Code::InvalidValue,
                loc,
                "`weight` must be a string literal",
                help,
            );
            return None;
        };
        let weight = Weight::parse(text);
        if weight.is_none() {
            self.cx.error_help(
                Code::InvalidValue,
                at,
                format!("\"{text}\" is not a weight"),
                help,
            );
        }
        weight
    }

    fn within_text_limit(&mut self, text: &str, loc: Loc) -> bool {
        let max = self.limits().max_text_bytes;
        if text.len() > max {
            self.cx.error(
                Code::Limit,
                loc,
                format!("text is {} bytes long; the limit is {max}", text.len()),
            );
            return false;
        }
        true
    }

    fn no_children(&mut self, component: KitComponent, element: &Element) {
        for child in &element.children {
            let loc = match child {
                Child::Empty { .. } => continue,
                Child::Text { loc, .. }
                | Child::InvalidText { loc }
                | Child::Fragment { loc, .. }
                | Child::Spread { loc } => *loc,
                Child::Element(e) => e.loc,
                Child::Expr(e) => e.loc(),
            };
            self.cx.error(
                Code::InvalidChildren,
                loc,
                format!("<{component}> takes no children"),
            );
        }
    }

    fn props(&mut self, component: KitComponent, element: &Element) -> Props {
        let mut props = Props::default();
        let mut seen: Vec<&str> = Vec::new();
        for attr in &element.attrs {
            let (name, value, loc) = match attr {
                Attr::Named { name, value, loc } => (name.as_str(), value, *loc),
                Attr::Spread { loc } => {
                    self.cx.error_help(
                        Code::Unsupported,
                        *loc,
                        "spread props ({...props}) are not supported",
                        "write each prop explicitly",
                    );
                    continue;
                }
            };
            if seen.contains(&name) {
                self.cx.error(
                    Code::DuplicateName,
                    loc,
                    format!("prop `{name}` is given more than once"),
                );
                continue;
            }
            seen.push(name);
            if component.prop(name).is_none() {
                let help = component.unknown_prop_help(name);
                self.cx.error_help(
                    Code::UnknownProp,
                    loc,
                    format!("<{component}> has no prop `{name}` in subset v1"),
                    help,
                );
                continue;
            }
            if let AttrValue::InvalidStr { .. } = value {
                // Already reported by the parser front end; the prop is
                // still given, so it is not reported missing as well.
                props.id_given |= name == "id";
                if matches!(name, "onPress" | "onClick") {
                    props.on_press_given = Some("onPress");
                }
                props.label_given |= name == "label";
                props.options_given |= name == "options";
                props.style_invalid |= matches!(name, "role" | "weight" | "lines");
                props.value_given |= name == "value";
                props.icon_given |= name == "name";
                props.tone_given |= name == "tone";
                props.for_given |= name == "for";
                props.picture_given |= name == "src";
                props.alt_given |= name == "alt";
                continue;
            }
            match name {
                "id" => {
                    props.id_given = true;
                    props.id = self.id(value, loc);
                }
                "for" => {
                    props.for_given = true;
                    props.field_for = self.id_named("for", value, loc);
                }
                "gap" => props.gap = self.length(name, value, loc),
                "padding" => props.padding = self.length(name, value, loc),
                "size" => props.size = self.length(name, value, loc),
                "placeholder" => props.placeholder = self.string(name, value, loc),
                "maxLength" => {
                    // A text input's most, or a long text's (contract v16).
                    let most = if component == KitComponent::Text {
                        MAX_LONG_TEXT
                    } else {
                        MAX_INPUT_LEN
                    };
                    props.max_len = self.max_len(value, loc, most);
                    props.max_len_given = true;
                }
                "inputMode" => props.mode = self.input_mode(value, loc).unwrap_or_default(),
                "label" => {
                    props.label_given = true;
                    props.label = self
                        .string(name, value, loc)
                        .filter(|label| self.within_label_limit("label", label, loc));
                }
                "options" => {
                    props.options_given = true;
                    props.options = self.options(component, value, loc);
                }
                "rows" => {
                    props.rows = self.select_rows(value, loc);
                    props.rows_invalid = props.rows.is_none();
                }
                "onPress" | "onClick" => {
                    let prop = if name == "onPress" {
                        "onPress"
                    } else {
                        "onClick"
                    };
                    if let Some(first) = props.on_press_given {
                        self.cx.error_help(
                            Code::DuplicateName,
                            loc,
                            format!("<{component}> is given both {first} and {prop}"),
                            "give it one of them: onClick is React DOM's name for onPress",
                        );
                        continue;
                    }
                    props.on_press_given = Some(prop);
                    props.on_press = self.action(prop, value, loc);
                }
                "variant" => props.variant = self.variant(value, loc),
                "tone" => {
                    props.tone_given = true;
                    props.tone = self.tone(value, loc);
                }
                "role" => {
                    props.role = self.text_role(value, loc);
                    props.style_invalid |= props.role.is_none();
                }
                "weight" => {
                    props.weight = self.weight(value, loc);
                    props.weight_at = Some(literal_string(value).map_or(loc, |(_, at)| at));
                    props.style_invalid |= props.weight.is_none();
                }
                "lines" => {
                    props.lines = self.lines(value, loc);
                    props.style_invalid |= props.lines.is_none();
                }
                "align" => {
                    props.align = self.text_align(value, loc);
                    props.style_invalid |= props.align.is_none();
                }
                "style" => props.style = self.style_object(component, value, loc),
                "value" => {
                    props.value_given = true;
                    props.value = self.progress_value(value, loc);
                }
                "name" => {
                    props.icon_given = true;
                    props.icon = self.icon_name(value, loc);
                }
                "src" => {
                    props.picture_given = true;
                    props.picture = self.picture_name(value, loc);
                }
                "alt" => {
                    props.alt_given = true;
                    props.alt = self
                        .string(name, value, loc)
                        .filter(|alt| self.within_label_limit("alt", alt, loc));
                }
                "fit" => {
                    props.fit = literal_string(value).and_then(|(text, _)| Fit::parse(text));
                    if props.fit.is_none() {
                        self.cx.error_help(
                            Code::InvalidValue,
                            loc,
                            "`fit` must be \"fill\", \"contain\" or \"cover\"",
                            "write fit=\"cover\": CSS's object-fit",
                        );
                    }
                }
                _ => {}
            }
        }
        props
    }

    fn id(&mut self, value: &AttrValue, loc: Loc) -> Option<Name> {
        self.id_named("id", value, loc)
    }

    /// An `id`, or a `for` naming one: a string of ASCII letters,
    /// digits, `-` and `_`, starting with a letter.
    fn id_named(&mut self, prop: &str, value: &AttrValue, loc: Loc) -> Option<Name> {
        let Some((text, at)) = literal_string(value) else {
            self.cx.error_help(
                Code::InvalidValue,
                loc,
                format!("`{prop}` must be a string"),
                format!("write {prop}=\"username\""),
            );
            return None;
        };
        let valid = text.starts_with(|c: char| c.is_ascii_alphabetic())
            && text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        match Name::parse(text) {
            Ok(name) if valid => Some(name),
            _ => {
                self.cx.error(
                    Code::InvalidValue,
                    at,
                    format!("id \"{text}\" must start with a letter and contain only ASCII letters, digits, `-` and `_`"),
                );
                None
            }
        }
    }

    fn string(&mut self, prop: &str, value: &AttrValue, loc: Loc) -> Option<String> {
        match literal_string(value) {
            Some((text, at)) => (self.visible_text(&format!("`{prop}`"), text, at)
                && self.drawn_text(&format!("`{prop}`"), text, at, TextRole::Body)
                && self.within_text_limit(text, at))
            .then(|| text.to_owned()),
            None => {
                self.cx.error_help(
                    Code::InvalidValue,
                    loc,
                    format!("`{prop}` must be a string literal"),
                    format!("write {prop}=\"…\""),
                );
                None
            }
        }
    }

    fn length(&mut self, prop: &str, value: &AttrValue, loc: Loc) -> Option<Length> {
        let expr = match value {
            // Already reported by the parser front end.
            AttrValue::InvalidStr { .. } => return None,
            AttrValue::Absent | AttrValue::Empty { .. } => {
                self.cx.error_help(
                    Code::InvalidValue,
                    loc,
                    format!("`{prop}` needs a value"),
                    format!("write {prop}={{16}} or {prop}={{SPACE_MD}}"),
                );
                return None;
            }
            AttrValue::Str { value, loc } => {
                return self.css_length(prop, value, *loc).map(Length::Px);
            }
            AttrValue::Expr(expr) => expr,
        };
        match expr {
            Expr::Num { value, raw, loc } => self
                .cx
                .pixels(*value, raw, *loc, &format!("`{prop}`"))
                .map(Length::Px),
            Expr::Ident { name, loc } => match self.lookup(name, *loc) {
                Some(Binding::Token(index)) => self
                    .cx
                    .tokens
                    .get(index)
                    .map(|t| Length::Token(t.name.clone())),
                Some(Binding::Prop(_) | Binding::PropsObject) => {
                    self.cx.error_help(
                        Code::Unsupported,
                        *loc,
                        format!("`{prop}` cannot come from a prop"),
                        "layout is static in subset v1: use a number or a design token",
                    );
                    None
                }
                Some(Binding::Space) => {
                    self.cx.error_help(
                        Code::InvalidValue,
                        *loc,
                        format!("`{name}` holds the theme's spacing tokens: name one"),
                        format!("write {prop}={{{name}.md}}: {}", space_names(name)),
                    );
                    None
                }
                Some(_) => {
                    self.cx.error(
                        Code::InvalidValue,
                        *loc,
                        format!("`{name}` is not a design token"),
                    );
                    None
                }
                None => {
                    self.cx.error_help(
                        Code::UnknownName,
                        *loc,
                        format!("unknown design token `{name}`"),
                        format!("declare it in this file: const {name} = 16;"),
                    );
                    None
                }
            },
            // The theme's spacing token: `space.md`.
            Expr::Member {
                object,
                property,
                loc,
            } if matches!(self.lookup(object, *loc), Some(Binding::Space)) => {
                match Space::parse(property) {
                    Some(space) => Some(Length::Space(space)),
                    None => {
                        self.cx.error_help(
                            Code::UnknownName,
                            *loc,
                            format!(
                                "`{object}.{property}` is not one of the theme's spacing tokens"
                            ),
                            space_names(object),
                        );
                        None
                    }
                }
            }
            Expr::Member {
                object,
                property,
                loc,
            } => {
                self.cx.error_help(
                    Code::Unsupported,
                    *loc,
                    format!(
                        "`{object}.{property}`: design tokens must be plain constants in subset v1"
                    ),
                    format!(
                        "declare `const SPACE_MD = 16;` and write gap={{SPACE_MD}}, or name one of the theme's spacing tokens: import {{ {SPACE} }} from the kit and write gap={{{SPACE}.md}}"
                    ),
                );
                None
            }
            Expr::Str { value, loc } => self.css_length(prop, value, *loc).map(Length::Px),
            other => {
                self.cx.error(
                    Code::Unsupported,
                    other.loc(),
                    format!(
                        "`{prop}` must be a number or a design token, not {}",
                        other.describe()
                    ),
                );
                None
            }
        }
    }

    /// `maxLength={N}`: a whole number, or a design token, from 1 to
    /// [`MAX_INPUT_LEN`] bytes.
    fn max_len(&mut self, value: &AttrValue, loc: Loc, most: u16) -> Option<Length> {
        let help = format!(
            "write maxLength={{32}}: a whole number of bytes from 1 to {most}, or a design token"
        );
        let (length, bytes, at) = match value {
            AttrValue::Expr(Expr::Num { value, raw, loc }) => {
                let whole = value.is_finite()
                    && value.fract() == 0.0
                    && (1.0..=f64::from(most)).contains(value)
                    && !raw.starts_with('0');
                if !whole {
                    self.cx.error_help(
                        Code::InvalidValue,
                        *loc,
                        format!("`maxLength` must be from 1 to {most} bytes, not {raw}"),
                        help,
                    );
                    return None;
                }
                // Exact: a whole number from 1 to `most`.
                let bytes = *value as u16;
                (Length::Px(bytes), bytes, *loc)
            }
            AttrValue::Expr(Expr::Ident { name, loc }) => match self.lookup(name, *loc) {
                Some(Binding::Token(index)) => {
                    let token = self.cx.tokens.get(index)?;
                    (Length::Token(token.name.clone()), token.value, *loc)
                }
                _ => {
                    self.cx.error_help(
                        Code::InvalidValue,
                        *loc,
                        format!("`{name}` is not a design token"),
                        help,
                    );
                    return None;
                }
            },
            AttrValue::Expr(other) => {
                self.cx.error_help(
                    Code::InvalidValue,
                    other.loc(),
                    format!(
                        "`maxLength` must be a number or a design token, not {}",
                        other.describe()
                    ),
                    help,
                );
                return None;
            }
            AttrValue::Str { loc, .. } | AttrValue::Empty { loc } => {
                self.cx.error_help(
                    Code::InvalidValue,
                    *loc,
                    "`maxLength` must be a number or a design token",
                    help,
                );
                return None;
            }
            AttrValue::Absent => {
                self.cx
                    .error_help(Code::InvalidValue, loc, "`maxLength` needs a value", help);
                return None;
            }
            // Already reported by the parser front end.
            AttrValue::InvalidStr { .. } => return None,
        };
        if (1..=most).contains(&bytes) {
            Some(length)
        } else {
            self.cx.error_help(
                Code::InvalidValue,
                at,
                format!("`maxLength` must be from 1 to {most} bytes, not {bytes}"),
                help,
            );
            None
        }
    }

    /// `inputMode="…"`: `text`, `numeric` or `tel` (digits), or `email`.
    fn input_mode(&mut self, value: &AttrValue, loc: Loc) -> Option<InputMode> {
        let help = "inputMode is \"text\", \"numeric\" or \"tel\" (digits only) or \"email\"";
        let Some((mode, at)) = literal_string(value) else {
            self.cx.error_help(
                Code::InvalidValue,
                loc,
                "`inputMode` must be a string literal",
                help,
            );
            return None;
        };
        let parsed = InputMode::parse(mode);
        if parsed.is_none() {
            self.cx.error_help(
                Code::InvalidValue,
                at,
                format!("inputMode \"{mode}\" is not supported"),
                help,
            );
        }
        parsed
    }

    /// `options={["…", …]}`: 1 to [`MAX_RADIO_OPTIONS`] string literals.
    /// `rows={N}` on a `<Select>`: a whole number within
    /// [`SELECT_ROWS`].
    fn select_rows(&mut self, value: &AttrValue, loc: Loc) -> Option<u8> {
        let (least, most) = (*SELECT_ROWS.start(), *SELECT_ROWS.end());
        let help =
            format!("write rows={{5}}: how many options show at once, from {least} to {most}");
        match value {
            AttrValue::Expr(Expr::Num { value, raw, loc }) => {
                let whole = value.is_finite()
                    && value.fract() == 0.0
                    && (f64::from(least)..=f64::from(most)).contains(value)
                    && !raw.starts_with('0');
                if whole {
                    // Exact: a whole number from 3 to 8.
                    Some(*value as u8)
                } else {
                    self.cx.error_help(
                        Code::InvalidValue,
                        *loc,
                        format!("`rows` must be a whole number from {least} to {most}, not {raw}"),
                        help,
                    );
                    None
                }
            }
            AttrValue::Expr(other) => {
                self.cx.error_help(
                    Code::InvalidValue,
                    other.loc(),
                    format!("`rows` must be a number, not {}", other.describe()),
                    help,
                );
                None
            }
            AttrValue::Str { loc, .. } | AttrValue::Empty { loc } => {
                self.cx
                    .error_help(Code::InvalidValue, *loc, "`rows` must be a number", help);
                None
            }
            AttrValue::Absent => {
                self.cx
                    .error_help(Code::InvalidValue, loc, "`rows` needs a value", help);
                None
            }
            // Already reported by the parser front end.
            AttrValue::InvalidStr { .. } => None,
        }
    }

    /// `options={[…]}`: string literals, 1 to the component's most - a
    /// radio group's [`MAX_RADIO_OPTIONS`], a list picker's
    /// [`MAX_SELECT_OPTIONS`].
    fn options(
        &mut self,
        component: KitComponent,
        value: &AttrValue,
        loc: Loc,
    ) -> Option<Vec<String>> {
        let (most, what) = match component {
            KitComponent::Select => (MAX_SELECT_OPTIONS, "a list picker"),
            _ => (MAX_RADIO_OPTIONS, "a radio group"),
        };
        let help = "write options={[\"Morning\", \"Afternoon\"]}: string literals only";
        let (elements, at) = match value {
            AttrValue::Expr(Expr::Array { elements, loc }) => (elements, *loc),
            AttrValue::Expr(other) => {
                self.cx.error_help(
                    Code::InvalidValue,
                    other.loc(),
                    format!(
                        "`options` must be an array of strings, not {}",
                        other.describe()
                    ),
                    help,
                );
                return None;
            }
            _ => {
                self.cx.error_help(
                    Code::InvalidValue,
                    loc,
                    "`options` must be an array of strings",
                    help,
                );
                return None;
            }
        };
        if elements.is_empty() {
            self.cx.error_help(
                Code::InvalidValue,
                at,
                format!("{what} needs at least one option"),
                help,
            );
            return None;
        }
        if elements.len() > most {
            let advice = match component {
                KitComponent::Select => "split the choice, or ask for typed text".to_owned(),
                _ => format!(
                    "for more choices than a radio group holds, use a list picker, <Select>, which offers up to {MAX_SELECT_OPTIONS}"
                ),
            };
            self.cx.error_help(
                Code::Limit,
                at,
                format!(
                    "{what} offers at most {most} options, not {}",
                    elements.len()
                ),
                advice,
            );
            return None;
        }
        let mut options = Vec::new();
        let mut ok = true;
        for element in elements {
            match element {
                Expr::Str { value, loc } => {
                    if value.is_empty() {
                        ok = false;
                        self.cx
                            .error(Code::InvalidValue, *loc, "an option may not be empty");
                    } else if self.visible_text("an option", value, *loc)
                        && self.drawn_text("an option", value, *loc, TextRole::Body)
                        && self.within_label_limit("option", value, *loc)
                    {
                        options.push(value.clone());
                    } else {
                        ok = false;
                    }
                }
                other => {
                    ok = false;
                    self.cx.error_help(
                        Code::Unsupported,
                        other.loc(),
                        format!("{} as an option is not supported", other.describe()),
                        help,
                    );
                }
            }
        }
        ok.then_some(options)
    }

    /// Whether a checkbox's label or a radio option is within
    /// [`MAX_LABEL_BYTES`]; otherwise reports it.
    fn within_label_limit(&mut self, what: &str, text: &str, loc: Loc) -> bool {
        if text.len() > MAX_LABEL_BYTES {
            self.cx.error(
                Code::Limit,
                loc,
                format!(
                    "the {what} is {} bytes long; the limit is {MAX_LABEL_BYTES}",
                    text.len()
                ),
            );
            return false;
        }
        true
    }

    /// A CSS length as a string: `"16px"`, a whole number of px, is that
    /// many layout units, as a number is. Other units - `rem`, `em`, `%`,
    /// `vh`, `vw` and the rest - and anything else are reported.
    fn css_length(&mut self, prop: &str, value: &str, loc: Loc) -> Option<u16> {
        let help = format!("write {prop}=\"16px\", {prop}={{16}} or a design token");
        let Some(number) = value.strip_suffix("px") else {
            let unit = value.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
            let units = [
                "rem", "em", "%", "vh", "vw", "vmin", "vmax", "ch", "ex", "pt", "pc", "cm", "mm",
                "in", "q",
            ];
            if !unit.is_empty() && unit.len() < value.len() && units.contains(&unit) {
                self.cx.error_help(
                    Code::Unsupported,
                    loc,
                    format!("`{prop}` in `{unit}` is not supported: lengths are layout units, written in px"),
                    help,
                );
            } else {
                self.cx.error_help(
                    Code::InvalidValue,
                    loc,
                    format!(
                        "`{prop}` must be a length in px, a number or a design token, not the string \"{value}\""
                    ),
                    help,
                );
            }
            return None;
        };
        let whole = !number.is_empty()
            && number.bytes().all(|b| b.is_ascii_digit())
            && (number == "0" || !number.starts_with('0'));
        match number.parse::<u16>() {
            Ok(units) if whole => Some(units),
            _ => {
                self.cx.error_help(
                    Code::InvalidValue,
                    loc,
                    format!(
                        "`{prop}` must be a whole number of px from 0 to 65535, not \"{value}\""
                    ),
                    help,
                );
                None
            }
        }
    }

    fn action(&mut self, prop: &str, value: &AttrValue, loc: Loc) -> Option<Name> {
        let help = format!(
            "pass a callback prop of the component: {prop}={{onContinue}}, or {prop}={{() => props.onContinue()}}"
        );
        match value {
            // A function that only calls one: the callback it calls.
            AttrValue::Expr(Expr::Handler { calls, .. }) => {
                self.action(prop, &AttrValue::Expr((**calls).clone()), loc)
            }
            AttrValue::Expr(Expr::Ident { name, loc }) => match self.lookup(name, *loc) {
                Some(Binding::Prop(key)) => self.action_name(&key, *loc),
                Some(Binding::PropsObject) => {
                    self.cx.error_help(
                        Code::InvalidValue,
                        *loc,
                        format!("`{prop}` takes one callback prop, not the whole props object"),
                        format!("write {prop}={{{name}.onContinue}}"),
                    );
                    None
                }
                Some(_) => {
                    self.cx.error_help(
                        Code::InvalidValue,
                        *loc,
                        format!("`{name}` is not a callback prop"),
                        help.clone(),
                    );
                    None
                }
                None => {
                    self.cx.error_help(
                        Code::UnknownName,
                        *loc,
                        format!("unknown callback `{name}`"),
                        format!("take it as a prop: function Screen({{ {name} }}) {{ … }}"),
                    );
                    None
                }
            },
            AttrValue::Expr(Expr::Member {
                object,
                property,
                loc,
            }) => {
                if matches!(self.lookup(object, *loc), Some(Binding::PropsObject)) {
                    self.action_name(property, *loc)
                } else {
                    self.cx.error_help(
                        Code::InvalidValue,
                        *loc,
                        format!("`{object}.{property}` is not a callback prop"),
                        help.clone(),
                    );
                    None
                }
            }
            AttrValue::Expr(Expr::Function(function)) => {
                self.cx.error_help(
                    Code::Unsupported,
                    function.loc,
                    "a function here may only call a callback prop, with no arguments",
                    help.clone(),
                );
                None
            }
            AttrValue::Expr(other) => {
                self.cx.error_help(
                    Code::Unsupported,
                    other.loc(),
                    format!(
                        "`{prop}` must name a callback prop, not {}",
                        other.describe()
                    ),
                    help.clone(),
                );
                None
            }
            AttrValue::Str { loc, .. } | AttrValue::Empty { loc } => {
                self.cx.error_help(
                    Code::InvalidValue,
                    *loc,
                    format!("`{prop}` must name a callback prop"),
                    help.clone(),
                );
                None
            }
            AttrValue::Absent => {
                self.cx.error_help(
                    Code::InvalidValue,
                    loc,
                    format!("`{prop}` needs a value"),
                    help,
                );
                None
            }
            // Already reported by the parser front end.
            AttrValue::InvalidStr { .. } => None,
        }
    }

    /// The action for callback prop `key`: `onContinue` becomes `continue`.
    fn action_name(&mut self, key: &str, loc: Loc) -> Option<Name> {
        match Name::parse(key) {
            Ok(name) => Some(name.without_leading("on")),
            Err(error) => {
                self.cx.error(
                    Code::InvalidValue,
                    loc,
                    format!("callback prop `{key}` cannot name an action: {error}"),
                );
                None
            }
        }
    }
}

fn attr_loc(attr: &Attr) -> Loc {
    match attr {
        Attr::Named { loc, .. } | Attr::Spread { loc } => *loc,
    }
}

/// A string literal value: `a="…"` or `a={"…"}`.
/// `names`, each quoted, the last after "or": "\"a\", \"b\" or \"c\"".
fn quoted(names: &[&str]) -> String {
    let names: Vec<String> = names.iter().map(|name| format!("\"{name}\"")).collect();
    match names.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
        Some((last, _)) => last.clone(),
        None => String::new(),
    }
}

/// The help line listing the theme's spacing tokens, as `object` names
/// them: "the theme has space.xs, space.sm, space.md, space.lg and
/// space.xl".
fn space_names(object: &str) -> String {
    let names: Vec<String> = Space::NAMES
        .iter()
        .map(|name| format!("{object}.{name}"))
        .collect();
    match names.split_last() {
        Some((last, rest)) => format!("the theme has {} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

fn literal_string(value: &AttrValue) -> Option<(&str, Loc)> {
    match value {
        AttrValue::Str { value, loc } | AttrValue::Expr(Expr::Str { value, loc }) => {
            Some((value.as_str(), *loc))
        }
        _ => None,
    }
}
