// SPDX-License-Identifier: MIT
//! The subset checks and the lowering to IR, in one walk.

use std::collections::{BTreeMap, BTreeSet};

use screen_compiler_ir::{
    AlignItems, ButtonVariant, Code, Colour, Container, Diagnostic, Group, ICON_NAMES, InputMode,
    Justify, Length, Loc, Look, MAX_BORDER_WIDTH, MAX_INPUT_LEN, MAX_LABEL_BYTES, MAX_LINES,
    MAX_RADIO_OPTIONS, MAX_RADIUS, MAX_SELECT_OPTIONS, Module, Name, Node, NodeKind, SELECT_ROWS,
    Screen, Space, Surface, TextAlign, TextRole, TextStyle, Token, Tone, Weight, has_errors,
    sort_diagnostics, special_char, validate,
};
use screen_compiler_tsx::SourceModule;
use screen_compiler_tsx::syntax::{
    Attr, AttrValue, Body, Child, DeclKeyword, Element, ElementName, Export, ExportSpecifier, Expr,
    Function, Import, ImportBinding, Item, LogicalOp, Params, PropBinding, Stmt, VarDecl,
};

use crate::{Analysis, KitComponent, Options};

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
    /// A component declared in this file.
    Component,
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
    bindings: BTreeMap<String, Binding>,
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

pub(crate) fn analyse(source: &SourceModule, options: &Options) -> Analysis {
    let mut cx = Cx {
        options,
        diagnostics: source.diagnostics.clone(),
        bindings: BTreeMap::new(),
        tokens: Vec::new(),
        values: BTreeMap::new(),
        types: BTreeSet::new(),
        exported: BTreeMap::new(),
    };
    let mut components = Vec::new();
    // Checked once every declaration is known: exports may come first.
    let mut export_specifiers: Vec<&ExportSpecifier> = Vec::new();
    let mut default_names: Vec<(&str, Loc)> = Vec::new();
    for item in &source.items {
        match item {
            Item::Import(import) => cx.import(import),
            Item::Var(var) => {
                let kind = if var.keyword == DeclKeyword::Var {
                    DeclKind::Var
                } else {
                    DeclKind::Lexical
                };
                if !cx.declare(&var.name, var.name_loc, kind) {
                    continue;
                }
                if var.export == Export::Named {
                    cx.export(&var.name, var.name_loc);
                }
                match &var.init {
                    Some(Expr::Function(function)) if var.keyword == DeclKeyword::Const => {
                        components.push(Component {
                            name: Some(&var.name),
                            function,
                            loc: var.loc,
                        });
                    }
                    _ => cx.token(var),
                }
            }
            Item::Function(decl) => {
                let loc = decl.function.loc;
                if let Some(name) = decl.name.as_deref()
                    && !cx.declare(name, loc, DeclKind::Lexical)
                {
                    continue;
                }
                match (decl.export, decl.name.as_deref()) {
                    (Export::Default, _) => cx.export("default", loc),
                    (Export::Named, Some(name)) => cx.export(name, loc),
                    _ => {}
                }
                components.push(Component {
                    name: decl.name.as_deref(),
                    function: &decl.function,
                    loc,
                });
            }
            Item::ExportDefaultExpr(expr) => {
                cx.export("default", expr.loc());
                match expr {
                    Expr::Function(function) => components.push(Component {
                        name: None,
                        function,
                        loc: function.loc,
                    }),
                    Expr::Ident { name, loc } => default_names.push((name, *loc)),
                    other => cx.error(
                        Code::Unsupported,
                        other.loc(),
                        format!("default export of {} is not supported", other.describe()),
                    ),
                }
            }
            Item::ExportList { specifiers, .. } => {
                for specifier in specifiers {
                    cx.export(&specifier.exported, specifier.loc);
                    export_specifiers.push(specifier);
                }
            }
            Item::TypeOnly { names, .. } => cx.types.extend(names.iter().cloned()),
            Item::Other(other) => cx.error(
                Code::Unsupported,
                other.loc,
                format!("{} is not supported in subset v1", other.kind),
            ),
        }
    }
    for specifier in export_specifiers {
        let local = &specifier.local;
        if !cx.values.contains_key(local) && !cx.types.contains(local) {
            cx.error_help(
                Code::UnknownName,
                specifier.loc,
                format!("`{local}` is exported but not declared in this file"),
                "a module can export only names it declares or imports (anything else is a syntax error)",
            );
        }
    }
    for (name, loc) in default_names {
        if cx.values.contains_key(name) {
            continue;
        }
        if cx.types.contains(name) {
            cx.error(
                Code::InvalidValue,
                loc,
                format!(
                    "`export default {name}` exports a type, but `export default` needs a value"
                ),
            );
        } else {
            cx.error(
                Code::UnknownName,
                loc,
                format!("`{name}` is not declared in this file"),
            );
        }
    }
    for component in &components {
        if let Some(name) = component.name {
            cx.bindings
                .entry(name.to_owned())
                .or_insert(Binding::Component);
        }
    }
    let screens: Vec<Screen> = components
        .iter()
        .filter_map(|component| cx.component(component))
        .collect();

    let module = Module {
        source_name: options.source_name.clone(),
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
    let module = (!has_errors(&diagnostics)).then_some(module);
    Analysis {
        module,
        diagnostics,
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

    /// Records an exported name, reporting one exported twice.
    fn export(&mut self, name: &str, loc: Loc) {
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

    fn import(&mut self, import: &Import) {
        if import.has_attributes {
            self.error(
                Code::Unsupported,
                import.loc,
                "import attributes and import phases are not supported",
            );
        }
        let is_kit = import.module == self.options.kit_module;
        let is_react = import.module == "react";
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
        if is_kit || is_react {
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
                    "import components only from \"{}\" and declare design tokens as `const` in this file",
                    self.options.kit_module
                ),
            );
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

    fn component(&mut self, component: &Component<'_>) -> Option<Screen> {
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
        let element = match returned {
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
                    "custom components are outside subset v1: wrap the elements in <Screen>",
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
    on_press_given: bool,
    max_len: Option<Length>,
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
    /// Whether `for` was given, valid or not.
    for_given: bool,
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
    fn lookup(&self, name: &str) -> Option<Binding> {
        match &self.scope {
            Scope::Destructured(bindings) => {
                if let Some(binding) = bindings.iter().find(|b| b.local == name) {
                    return Some(Binding::Prop(binding.key.clone()));
                }
            }
            Scope::Object(object) if *object == name => return Some(Binding::PropsObject),
            _ => {}
        }
        self.cx.bindings.get(name).cloned()
    }

    fn limits(&self) -> &crate::Limits {
        &self.cx.options.limits
    }

    /// The kit component an element names, or `None` after reporting why not.
    fn resolve(&mut self, element: &Element) -> Option<KitComponent> {
        let loc = element.loc;
        let kit = self.cx.options.kit_module.clone();
        match &element.name {
            ElementName::Component(name) => match self.lookup(name) {
                Some(Binding::Kit(component)) => Some(component),
                Some(Binding::Component) => {
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
            KitComponent::Text => {
                let style = self.text_style(&props);
                match self.prop_text(element) {
                    PropText::Slot(slot) => NodeKind::Slot {
                        slot,
                        style: style?,
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
                if !props.on_press_given {
                    self.cx.error_help(
                        Code::MissingProp,
                        element.loc,
                        "<Button> needs an onPress callback prop",
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
        let (name, children, loc) = match expr {
            Expr::Logical {
                op: LogicalOp::And,
                left,
                right,
                loc,
            } => {
                let name = self.condition(left);
                let shown = self.branch(right, inner);
                (name, vec![shown?], *loc)
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
                let name = self.condition(test);
                if let Expr::Null { loc } = consequent.as_ref() {
                    self.cx.error_help(
                        Code::Unsupported,
                        *loc,
                        "`null` while the condition holds is not supported",
                        "pass the opposite as a prop and write {props.NAME && <Element />}",
                    );
                    return None;
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
                (name, children, *loc)
            }
            _ => return None,
        };
        let name = name?;
        if !self.admit(loc, depth) {
            return None;
        }
        Some(Node {
            kind: NodeKind::Group(Group { name, children }),
            id: None,
            loc: Some(loc),
        })
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
                if matches!(self.lookup(object), Some(Binding::PropsObject)) {
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
            Expr::Ident { name, loc } => match self.lookup(name) {
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

    /// A `<Text>` whose only content is a prop (subset v1): a slot the
    /// application fills. Anything else is literal text, or reported by
    /// [`literal_text`](Self::literal_text).
    fn prop_text(&mut self, element: &Element) -> PropText {
        let mut content = element
            .children
            .iter()
            .filter(|child| !matches!(child, Child::Empty { .. }));
        let (Some(Child::Expr(expr)), None) = (content.next(), content.next()) else {
            return PropText::Literal;
        };
        match self.prop_of(expr, "text from the application") {
            PropRef::Prop(key, loc) => match self.prop_name(&key, loc, "name a slot") {
                Some(slot) => PropText::Slot(slot),
                None => PropText::Invalid,
            },
            PropRef::Reported => PropText::Invalid,
            PropRef::NotAProp => PropText::Literal,
        }
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
                Child::Expr(expr @ (Expr::Ident { .. } | Expr::Member { .. }))
                    if component == KitComponent::Text =>
                {
                    ok = false;
                    self.cx.error_help(
                        Code::Unsupported,
                        expr.loc(),
                        format!("text mixed with {} is not supported", expr.describe()),
                        "a prop's text is a <Text> of its own: <Row><Text>Label:</Text><Text>{props.value}</Text></Row>",
                    );
                }
                Child::Expr(other) => {
                    ok = false;
                    let help = if component == KitComponent::Text {
                        "text is literal, or one prop alone: <Text>{props.value}</Text>"
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
        let allowed = component.style_properties();
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
                        &["left", "start", "center", "right", "end"],
                        |v| match v {
                            "left" => Some(TextAlign::Start),
                            "right" => Some(TextAlign::End),
                            other => TextAlign::parse(other),
                        },
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
        let colour = Colour::parse(text);
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
            if !component.props().contains(&name) {
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
                props.on_press_given |= name == "onPress";
                props.label_given |= name == "label";
                props.options_given |= name == "options";
                props.style_invalid |= matches!(name, "role" | "weight" | "lines");
                props.value_given |= name == "value";
                props.icon_given |= name == "name";
                props.tone_given |= name == "tone";
                props.for_given |= name == "for";
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
                "maxLength" => props.max_len = self.max_len(value, loc),
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
                "onPress" => {
                    props.on_press_given = true;
                    props.on_press = self.action(value, loc);
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
                self.string_length(prop, value, *loc);
                return None;
            }
            AttrValue::Expr(expr) => expr,
        };
        match expr {
            Expr::Num { value, raw, loc } => self
                .cx
                .pixels(*value, raw, *loc, &format!("`{prop}`"))
                .map(Length::Px),
            Expr::Ident { name, loc } => match self.lookup(name) {
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
            } if matches!(self.lookup(object), Some(Binding::Space)) => {
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
            Expr::Str { value, loc } => {
                self.string_length(prop, value, *loc);
                None
            }
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
    fn max_len(&mut self, value: &AttrValue, loc: Loc) -> Option<Length> {
        let help = format!(
            "write maxLength={{32}}: a whole number of bytes from 1 to {MAX_INPUT_LEN}, or a design token"
        );
        let (length, bytes, at) = match value {
            AttrValue::Expr(Expr::Num { value, raw, loc }) => {
                let whole = value.is_finite()
                    && value.fract() == 0.0
                    && (1.0..=f64::from(MAX_INPUT_LEN)).contains(value)
                    && !raw.starts_with('0');
                if !whole {
                    self.cx.error_help(
                        Code::InvalidValue,
                        *loc,
                        format!("`maxLength` must be from 1 to {MAX_INPUT_LEN} bytes, not {raw}"),
                        help,
                    );
                    return None;
                }
                // Exact: a whole number from 1 to 64.
                let bytes = *value as u16;
                (Length::Px(bytes), bytes, *loc)
            }
            AttrValue::Expr(Expr::Ident { name, loc }) => match self.lookup(name) {
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
        if (1..=MAX_INPUT_LEN).contains(&bytes) {
            Some(length)
        } else {
            self.cx.error_help(
                Code::InvalidValue,
                at,
                format!("`maxLength` must be from 1 to {MAX_INPUT_LEN} bytes, not {bytes}"),
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
        match mode {
            "text" => Some(InputMode::Text),
            "numeric" | "tel" => Some(InputMode::Digits),
            "email" => Some(InputMode::Email),
            other => {
                self.cx.error_help(
                    Code::InvalidValue,
                    at,
                    format!("inputMode \"{other}\" is not supported"),
                    help,
                );
                None
            }
        }
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

    fn string_length(&mut self, prop: &str, value: &str, loc: Loc) {
        self.cx.error_help(
            Code::InvalidValue,
            loc,
            format!(
                "`{prop}` must be a number of pixels or a design token, not the string \"{value}\""
            ),
            format!("CSS values are not supported: write {prop}={{16}} or use a design token"),
        );
    }

    fn action(&mut self, value: &AttrValue, loc: Loc) -> Option<Name> {
        let help = "pass a callback prop of the component: onPress={onContinue}";
        match value {
            AttrValue::Expr(Expr::Ident { name, loc }) => match self.lookup(name) {
                Some(Binding::Prop(key)) => self.action_name(&key, *loc),
                Some(Binding::PropsObject) => {
                    self.cx.error_help(
                        Code::InvalidValue,
                        *loc,
                        "`onPress` takes one callback prop, not the whole props object",
                        format!("write onPress={{{name}.onContinue}}"),
                    );
                    None
                }
                Some(_) => {
                    self.cx.error_help(
                        Code::InvalidValue,
                        *loc,
                        format!("`{name}` is not a callback prop"),
                        help,
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
                if matches!(self.lookup(object), Some(Binding::PropsObject)) {
                    self.action_name(property, *loc)
                } else {
                    self.cx.error_help(
                        Code::InvalidValue,
                        *loc,
                        format!("`{object}.{property}` is not a callback prop"),
                        help,
                    );
                    None
                }
            }
            AttrValue::Expr(Expr::Function(function)) => {
                self.cx.error_help(
                    Code::Unsupported,
                    function.loc,
                    "inline event handlers are not supported in subset v1",
                    help,
                );
                None
            }
            AttrValue::Expr(other) => {
                self.cx.error_help(
                    Code::Unsupported,
                    other.loc(),
                    format!(
                        "`onPress` must name a callback prop, not {}",
                        other.describe()
                    ),
                    help,
                );
                None
            }
            AttrValue::Str { loc, .. } | AttrValue::Empty { loc } => {
                self.cx.error_help(
                    Code::InvalidValue,
                    *loc,
                    "`onPress` must name a callback prop",
                    help,
                );
                None
            }
            AttrValue::Absent => {
                self.cx
                    .error_help(Code::InvalidValue, loc, "`onPress` needs a value", help);
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
