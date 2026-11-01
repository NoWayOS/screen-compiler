// SPDX-License-Identifier: MIT
//! Screen Compiler's own view of a TSX module.
//!
//! This tree is owned and parser-independent: no oxc type appears in it, so
//! the analyser never depends on the parser's API (which changes between
//! releases). It keeps the constructs the analyser needs in detail and
//! reduces everything else to [`Other`] with a readable description and a
//! position, so unsupported constructs can be reported precisely.
//!
//! JSX semantics are already applied: JSX text has React's whitespace rules
//! and HTML character references applied, and JSX attribute strings have
//! their character references decoded. Text or an attribute string that
//! cannot be converted exactly is reported in [`SourceModule::diagnostics`]
//! and appears in the tree as [`Child::InvalidText`] or
//! [`AttrValue::InvalidStr`], never as a stand-in value.

use screen_compiler_ir::{Diagnostic, Loc};

/// A parsed module: its top-level items in source order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourceModule {
    /// Top-level items.
    pub items: Vec<Item>,
    /// Problems found while converting JSX text and attribute strings (for
    /// example an unsupported character reference). The tree is still
    /// complete enough to analyse, but any error here means the module must
    /// not be converted.
    pub diagnostics: Vec<Diagnostic>,
}

/// A construct outside what this tree models in detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Other {
    /// What it is, for messages: "class declaration", "call to `useState`".
    pub kind: String,
    /// Where it starts.
    pub loc: Loc,
}

/// A top-level item.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// `import … from "…"`.
    Import(Import),
    /// One declarator of a `const`, `let` or `var` declaration.
    Var(VarDecl),
    /// A function declaration.
    Function(FunctionDecl),
    /// `export { a, b as c }`: no runtime content of its own.
    ExportList {
        /// The exported bindings, in order. Type-only specifiers
        /// (`export { type T }`) are erased and left out.
        specifiers: Vec<ExportSpecifier>,
        /// Where the statement starts.
        loc: Loc,
    },
    /// `export default <expression>;`, including a plain name.
    ExportDefaultExpr(Expr),
    /// A TypeScript declaration that is erased at compile time.
    TypeOnly {
        /// The type-level names it declares: type aliases, interfaces,
        /// `declare` declarations and overload signatures. Empty for
        /// `export type { … }`.
        names: Vec<String>,
        /// Where the declaration starts.
        loc: Loc,
    },
    /// Anything else.
    Other(Other),
}

/// One binding of an `export { … }` list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportSpecifier {
    /// The local name it refers to.
    pub local: String,
    /// The name it is exported as.
    pub exported: String,
    /// Where the specifier starts.
    pub loc: Loc,
}

/// An import declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    /// The module specifier, e.g. `nowayos:kit`.
    pub module: String,
    /// `import type …`: erased at compile time.
    pub type_only: bool,
    /// The names it binds; empty for `import "./styles.css";`.
    pub bindings: Vec<ImportBinding>,
    /// `import "…";` with no bindings, run for its side effects.
    pub side_effect_only: bool,
    /// Import attributes (`with { … }`) or an import phase are present.
    pub has_attributes: bool,
    /// Where the declaration starts.
    pub loc: Loc,
}

/// One name bound by an import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportBinding {
    /// `{ imported as local }`.
    Named {
        /// The exported name.
        imported: String,
        /// The local name.
        local: String,
        /// `{ type X }`: erased at compile time.
        type_only: bool,
        /// Where the specifier starts.
        loc: Loc,
    },
    /// `import local from …`.
    Default {
        /// The local name.
        local: String,
        /// Where the specifier starts.
        loc: Loc,
    },
    /// `import * as local from …`.
    Namespace {
        /// The local name.
        local: String,
        /// Where the specifier starts.
        loc: Loc,
    },
}

/// The keyword of a variable declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclKeyword {
    /// `const`
    Const,
    /// `let`
    Let,
    /// `var`
    Var,
    /// `using` / `await using`
    Using,
}

/// Whether and how a declaration is exported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Export {
    /// Not exported.
    None,
    /// `export …`
    Named,
    /// `export default …`
    Default,
}

/// One declarator of a top-level variable declaration with a plain name.
#[derive(Clone, Debug, PartialEq)]
pub struct VarDecl {
    /// `const`, `let`, `var` or `using`.
    pub keyword: DeclKeyword,
    /// The declared name.
    pub name: String,
    /// Where the name is written.
    pub name_loc: Loc,
    /// The initialiser, if any.
    pub init: Option<Expr>,
    /// Whether it is exported.
    pub export: Export,
    /// Where the declarator starts.
    pub loc: Loc,
}

/// A function declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionDecl {
    /// The function's name (`export default function () {}` has none).
    pub name: Option<String>,
    /// The function itself.
    pub function: Function,
    /// Whether it is exported.
    pub export: Export,
}

/// A function or arrow function.
#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    /// Its parameters.
    pub params: Params,
    /// Its body.
    pub body: Body,
    /// `async`.
    pub is_async: bool,
    /// `function*`.
    pub is_generator: bool,
    /// Where it starts.
    pub loc: Loc,
}

/// A function's parameter list, as far as components use it.
#[derive(Clone, Debug, PartialEq)]
pub enum Params {
    /// `()`.
    None,
    /// `({ a, b: local })`: destructured props.
    Destructured(Vec<PropBinding>),
    /// `(props)`: the props object under one name.
    Named {
        /// The parameter's name.
        name: String,
        /// Where it is written.
        loc: Loc,
    },
    /// Anything else: several parameters, defaults, rest, nested patterns.
    Unsupported(Other),
}

/// One destructured prop: `{ key }` or `{ key: local }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropBinding {
    /// The prop's name.
    pub key: String,
    /// The local variable it is bound to.
    pub local: String,
    /// Where the binding is written.
    pub loc: Loc,
}

/// A function body.
#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    /// An arrow function's expression body.
    Expr(Box<Expr>),
    /// A block of statements.
    Block(Vec<Stmt>),
}

/// A statement inside a function body.
#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    /// `return value;`
    Return {
        /// The returned value.
        value: Option<Expr>,
        /// Where the statement starts.
        loc: Loc,
    },
    /// Anything else.
    Other(Other),
}

/// An expression, as far as the subset uses them.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    /// A JavaScript string literal (escapes already processed).
    Str {
        /// The string's value.
        value: String,
        /// Where it starts.
        loc: Loc,
    },
    /// A numeric literal.
    Num {
        /// Its value.
        value: f64,
        /// The literal as written.
        raw: String,
        /// Where it starts.
        loc: Loc,
    },
    /// A plain identifier.
    Ident {
        /// The name.
        name: String,
        /// Where it starts.
        loc: Loc,
    },
    /// `object.property` where `object` is a plain identifier.
    Member {
        /// The object's name.
        object: String,
        /// The property's name.
        property: String,
        /// Where it starts.
        loc: Loc,
    },
    /// An arrow function or function expression.
    Function(Box<Function>),
    /// A JSX element.
    Element(Box<Element>),
    /// A JSX fragment `<>…</>`.
    Fragment {
        /// Its children.
        children: Vec<Child>,
        /// Where it starts.
        loc: Loc,
    },
    /// `null`.
    Null {
        /// Where it starts.
        loc: Loc,
    },
    /// An array literal `[a, b]`. A spread element (`...xs`) or a hole
    /// (`[a, , b]`) is an [`Expr::Other`] in its place.
    Array {
        /// Its elements, in order.
        elements: Vec<Expr>,
        /// Where it starts (its `[`).
        loc: Loc,
    },
    /// An object literal `{ key: value, … }`: what `style={{…}}`
    /// holds. A property whose key is computed, a spread, a method, a
    /// getter or setter, or a shorthand (`{ a }`) is kept with an empty key
    /// and an [`Expr::Other`] value, so it is reported where written.
    Object {
        /// Its properties, in order.
        props: Vec<ObjectProp>,
        /// Where it starts (its `{`).
        loc: Loc,
    },
    /// `left && right`, `left || right` or `left ?? right`.
    Logical {
        /// The operator.
        op: LogicalOp,
        /// The left operand.
        left: Box<Expr>,
        /// The right operand.
        right: Box<Expr>,
        /// Where the expression starts (its left operand).
        loc: Loc,
    },
    /// `test ? consequent : alternate`.
    Conditional {
        /// The condition.
        test: Box<Expr>,
        /// The value when the condition holds.
        consequent: Box<Expr>,
        /// The value when it does not.
        alternate: Box<Expr>,
        /// Where the expression starts (its condition).
        loc: Loc,
    },
    /// Anything else.
    Other(Other),
}

/// The operator of a [`Expr::Logical`] expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogicalOp {
    /// `&&`
    And,
    /// `||`
    Or,
    /// `??`
    Coalesce,
}

impl Expr {
    /// Where the expression starts.
    #[must_use]
    pub fn loc(&self) -> Loc {
        match self {
            Self::Str { loc, .. }
            | Self::Num { loc, .. }
            | Self::Ident { loc, .. }
            | Self::Member { loc, .. }
            | Self::Fragment { loc, .. }
            | Self::Null { loc }
            | Self::Array { loc, .. }
            | Self::Object { loc, .. }
            | Self::Logical { loc, .. }
            | Self::Conditional { loc, .. } => *loc,
            Self::Function(f) => f.loc,
            Self::Element(e) => e.loc,
            Self::Other(o) => o.loc,
        }
    }

    /// A short description for messages.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Str { .. } => "a string".to_owned(),
            Self::Num { .. } => "a number".to_owned(),
            Self::Ident { name, .. } => format!("`{name}`"),
            Self::Member {
                object, property, ..
            } => format!("`{object}.{property}`"),
            Self::Function(_) => "a function".to_owned(),
            Self::Element(e) => format!("a <{}> element", e.name.display()),
            Self::Fragment { .. } => "a fragment".to_owned(),
            Self::Null { .. } => "`null`".to_owned(),
            Self::Array { .. } => "an array literal".to_owned(),
            Self::Object { .. } => "an object literal".to_owned(),
            Self::Logical { .. } => "a logical (`&&`, `||`, `??`) expression".to_owned(),
            Self::Conditional { .. } => "a conditional (`?:`) expression".to_owned(),
            Self::Other(o) => with_article(&o.kind),
        }
    }
}

/// One property of an object literal.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectProp {
    /// Its key: an identifier's name or a string's value; empty for any
    /// other, whose value is then an [`Expr::Other`].
    pub key: String,
    /// Its value.
    pub value: Expr,
    /// Where the property starts (its key).
    pub loc: Loc,
}

/// `kind` with "a" or "an" in front: "an object literal", "a `new` expression".
fn with_article(kind: &str) -> String {
    let first = kind.trim_start_matches('`').chars().next();
    // No kind starts with a "u" pronounced as a vowel ("unary" takes "a").
    let article = if matches!(first, Some('a' | 'e' | 'i' | 'o')) {
        "an"
    } else {
        "a"
    };
    format!("{article} {kind}")
}

/// A JSX element.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    /// The tag name.
    pub name: ElementName,
    /// Attributes, in order.
    pub attrs: Vec<Attr>,
    /// Children, in order. Text children that React drops are not included.
    pub children: Vec<Child>,
    /// Where the element starts (its `<`).
    pub loc: Loc,
}

/// A JSX tag name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ElementName {
    /// A capitalised name: a component such as `Column`.
    Component(String),
    /// A lowercase name: an HTML element such as `div`.
    Intrinsic(String),
    /// A member or namespaced name such as `Kit.Column` or `svg:path`.
    Other(String),
}

impl ElementName {
    /// The name as written.
    #[must_use]
    pub fn display(&self) -> &str {
        match self {
            Self::Component(n) | Self::Intrinsic(n) | Self::Other(n) => n,
        }
    }
}

/// A JSX attribute.
#[derive(Clone, Debug, PartialEq)]
pub enum Attr {
    /// `name`, `name="…"` or `name={…}`.
    Named {
        /// The attribute's name.
        name: String,
        /// Its value.
        value: AttrValue,
        /// Where the attribute starts.
        loc: Loc,
    },
    /// `{...props}`.
    Spread {
        /// Where it starts.
        loc: Loc,
    },
}

/// The value of a JSX attribute.
#[derive(Clone, Debug, PartialEq)]
pub enum AttrValue {
    /// No value: `<X disabled />`.
    Absent,
    /// `name="…"`, character references decoded. Never contains a control,
    /// invisible formatting or separator character.
    Str {
        /// The decoded value.
        value: String,
        /// Where the string starts.
        loc: Loc,
    },
    /// `name="…"` that could not be converted exactly. The problem is
    /// already in [`SourceModule::diagnostics`], which stops conversion.
    InvalidStr {
        /// Where the string starts.
        loc: Loc,
    },
    /// `name={expression}` (or an element used directly as the value).
    Expr(Expr),
    /// `name={}` or `name={/* comment */}`.
    Empty {
        /// Where it starts.
        loc: Loc,
    },
}

/// A child of a JSX element or fragment.
#[derive(Clone, Debug, PartialEq)]
pub enum Child {
    /// Text, after React's whitespace rules; never empty, and never contains
    /// a control, invisible formatting or separator character.
    Text {
        /// The text React would render.
        value: String,
        /// Where the visible text starts.
        loc: Loc,
    },
    /// Text that could not be converted exactly. The problem is already in
    /// [`SourceModule::diagnostics`], which stops conversion; the child is
    /// kept so the analyser can still check where text is allowed.
    InvalidText {
        /// Where the visible text starts.
        loc: Loc,
    },
    /// `{expression}`.
    Expr(Expr),
    /// `{}` or `{/* comment */}`: renders nothing.
    Empty {
        /// Where it starts.
        loc: Loc,
    },
    /// A nested element.
    Element(Element),
    /// A nested fragment.
    Fragment {
        /// Its children.
        children: Vec<Child>,
        /// Where it starts.
        loc: Loc,
    },
    /// `{...children}`.
    Spread {
        /// Where it starts.
        loc: Loc,
    },
}
