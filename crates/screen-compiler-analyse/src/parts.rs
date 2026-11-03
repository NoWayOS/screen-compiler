// SPDX-License-Identifier: MIT
//! A screen's own components and fragments, expanded at build time.
//!
//! A *part* is a function component of the same file that a screen, or
//! another part, uses as an element. Before a screen is walked, each use of
//! a part is replaced by what the part returns: each of its props by the
//! value written where it is used - a literal, a design token, one of the
//! screen's own props passed through - and `{children}` by the children
//! written between its tags. A fragment's children take its place. The walk
//! then checks and lowers the screen as if it had been written flat, so every
//! rule of the subset holds inside a part as anywhere else, and each error is
//! reported where it is written: inside the part, or where it is used.
//!
//! As in React, a prop the use does not give is `undefined`: an attribute
//! set from it is left out (so a kit prop keeps its default, and a required
//! one is reported missing), a child or a condition made of it shows
//! nothing, and a style property set from it is not set.
//!
//! Lists are expanded too. `.map` over an array literal or a top-level
//! `const` array - or one of these sliced with `.slice(0, N)` - gives one
//! copy of what the callback returns for each item, its parameter given the
//! item: a field of an object item as a part's prop is given, the item
//! itself to `(item) => …`, and `key` on what it returns is dropped. A
//! screen's prop sliced, `props.items.slice(0, N).map(item => …)`, gives `N`
//! rows the application fills: row `r` is the group `items_r`, each field
//! `item.name` the slot `items_r_name`, the item itself the slot
//! `items_r_item` - so the application shows as many rows as it has items,
//! and sets their text. A list from a prop with no most, a row of more than
//! one element, a list from a prop inside a row of another, or a callback
//! taking more than the item, is refused where written.
//!
//! Refused where written: a part that uses itself, directly or through
//! others; a spread on a part; a part's props taken other than destructured
//! or as one object; the whole props object passed on; `children` anywhere
//! but among children; a name a part uses that is neither its prop nor
//! declared in the file. Parts nest at
//! most [`MAX_PART_DEPTH`] deep, and a screen's expansion stops past
//! [`ELEMENTS_PER_NODE`] times as many elements as the walk takes
//! (`Limits::max_nodes`), which then reports the screen too large.

use std::collections::{BTreeMap, BTreeSet};

use screen_compiler_ir::{Code, Diagnostic, Loc};
use screen_compiler_tsx::syntax::{
    Attr, AttrValue, Body, Child, Element, ElementName, Expr, Function, LogicalOp, ObjectProp,
    Params, PropBinding, Stmt,
};

/// How deep parts may use parts.
pub(crate) const MAX_PART_DEPTH: usize = 16;

/// How many elements the expansion may make for each node the walk takes:
/// children given to a part are expanded where they are written, then
/// walked again inside the part, so they count more than once.
pub(crate) const ELEMENTS_PER_NODE: usize = 8;

/// The most rows a list from a prop may have: `.slice(0, N)` with `N` up
/// to this.
pub(crate) const MAX_LIST_ROWS: u16 = 32;

/// What starts the name of a prop the expansion makes - a list row's group
/// or slot - which the walk takes as the screen's prop of the name after it.
/// No name in a source file can hold it.
pub(crate) const MADE_PROP: char = '\u{1}';

/// How deep the expansion walks a tree: past the walk's own limit on
/// nesting, so a tree too deep is reported by the walk, where it is.
const MAX_TREE_DEPTH: usize = 64;

/// A part: a function component of the file that is used as an element.
pub(crate) struct Part<'s> {
    /// Its parameters.
    pub(crate) params: &'s Params,
    /// What it returns: `None` if that could not be told, which is
    /// reported where the part is declared.
    pub(crate) returned: Option<&'s Expr>,
}

/// What a part's parameters bind, for substitution.
#[derive(Clone, Copy)]
enum Bound<'s> {
    /// No parameters.
    None,
    /// `({ title, onSave: save })`.
    Destructured(&'s [PropBinding]),
    /// `(props)`.
    Object(&'s str),
}

/// A use of a part: what its props are given, and its children.
struct Use<'s> {
    /// The part's name.
    part: &'s str,
    /// What its parameters bind.
    bound: Bound<'s>,
    /// Each prop given, by name, as written where the part is used.
    given: BTreeMap<String, AttrValue>,
    /// The children written between its tags, already expanded.
    children: Vec<Child>,
    /// The value of the parameter itself, `(item) => item`: a list's item;
    /// a part's props object cannot be used whole.
    whole: Option<Expr>,
    /// Names a callback inside the part binds, which are not the part's.
    shadow: Vec<String>,
    /// Whether this is a list's callback, a closure: the names around it
    /// are its own as well, so any other name is left to the walk.
    closure: bool,
}

/// What a name in a part refers to.
enum Named<'n> {
    /// One of its props, by name.
    Prop(&'n str),
    /// Its whole props object.
    Props,
    /// Something declared in the file, or not a name of the part's.
    Other,
    /// A name a callback inside the part binds.
    Shadowed,
}

/// Expands the parts and fragments of one screen.
pub(crate) struct Expander<'p, 's> {
    parts: &'p BTreeMap<&'s str, Part<'s>>,
    /// The file's top-level `const` arrays, by name.
    lists: &'p BTreeMap<&'s str, &'s Expr>,
    /// The screen's parameters, which a list from a prop names.
    screen: Option<&'s Params>,
    /// Whether the expansion is inside a row of a list from a prop.
    in_row: bool,
    /// Every name declared in the file, which a part may use as it is.
    declared: &'p BTreeSet<String>,
    /// Problems found, each where it is written.
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// The parts being expanded, outermost first.
    stack: Vec<&'s str>,
    /// Elements made so far, and the most there may be.
    elements: usize,
    max_elements: usize,
    over: bool,
}

impl<'p, 's> Expander<'p, 's> {
    pub(crate) fn new(
        parts: &'p BTreeMap<&'s str, Part<'s>>,
        lists: &'p BTreeMap<&'s str, &'s Expr>,
        declared: &'p BTreeSet<String>,
        max_elements: usize,
    ) -> Self {
        Self {
            parts,
            lists,
            screen: None,
            in_row: false,
            declared,
            diagnostics: Vec::new(),
            stack: Vec::new(),
            elements: 0,
            max_elements: max_elements.saturating_mul(ELEMENTS_PER_NODE),
            over: false,
        }
    }

    fn error(&mut self, code: Code, loc: Loc, message: impl Into<String>, help: impl Into<String>) {
        self.diagnostics
            .push(Diagnostic::error(code, Some(loc), message).with_help(help));
    }

    /// What a screen component returns, its parts expanded: an element, or
    /// a fragment if a part at its root returns several.
    pub(crate) fn root(&mut self, returned: &'s Expr, screen: &'s Params) -> Expr {
        self.screen = Some(screen);
        match returned {
            Expr::Element(element) => self.single(element, returned.loc(), 0),
            other => other.clone(),
        }
    }

    /// `element`, expanded, as one expression: its element, or a fragment
    /// of what a part returns when that is not one element.
    fn single(&mut self, element: &Element, loc: Loc, depth: usize) -> Expr {
        let mut children = self.element(element, depth);
        if children.len() == 1
            && let Some(Child::Element(only)) = children.pop()
        {
            return Expr::Element(Box::new(only));
        }
        Expr::Fragment { children, loc }
    }

    /// Children, expanded: each fragment's children in its place, each
    /// part's what it returns.
    fn children(&mut self, children: &[Child], depth: usize) -> Vec<Child> {
        let mut out = Vec::new();
        if depth > MAX_TREE_DEPTH {
            // The walk reports a tree this deep where it is.
            return children.to_vec();
        }
        let inner = depth.saturating_add(1);
        for child in children {
            match child {
                Child::Element(element) => out.extend(self.element(element, inner)),
                Child::Expr(Expr::Element(element)) => out.extend(self.element(element, inner)),
                Child::Fragment { children, .. } | Child::Expr(Expr::Fragment { children, .. }) => {
                    out.extend(self.children(children, inner));
                }
                Child::Expr(Expr::Map {
                    list,
                    most,
                    callback,
                    loc,
                }) => out.extend(self.map(list, most.as_deref(), callback, *loc, inner)),
                Child::Expr(Expr::Logical {
                    op,
                    left,
                    right,
                    loc,
                }) => out.push(Child::Expr(Expr::Logical {
                    op: *op,
                    left: left.clone(),
                    right: Box::new(self.branch(right, inner)),
                    loc: *loc,
                })),
                Child::Expr(Expr::Conditional {
                    test,
                    consequent,
                    alternate,
                    loc,
                }) => out.push(Child::Expr(Expr::Conditional {
                    test: test.clone(),
                    consequent: Box::new(self.branch(consequent, inner)),
                    alternate: Box::new(self.branch(alternate, inner)),
                    loc: *loc,
                })),
                other => out.push(other.clone()),
            }
        }
        out
    }

    /// What a condition shows, expanded: one element, which a fragment of
    /// one element or a part returning one becomes.
    fn branch(&mut self, expr: &Expr, depth: usize) -> Expr {
        match expr {
            Expr::Element(element) => self.single(element, expr.loc(), depth),
            Expr::Fragment { children, loc } => {
                let mut children = self.children(children, depth);
                if children.len() == 1
                    && let Some(Child::Element(only)) = children.pop()
                {
                    return Expr::Element(Box::new(only));
                }
                Expr::Fragment {
                    children,
                    loc: *loc,
                }
            }
            other => other.clone(),
        }
    }

    /// An element, expanded: itself, with its children expanded, or, for a
    /// part, what the part returns.
    fn element(&mut self, element: &Element, depth: usize) -> Vec<Child> {
        self.elements = self.elements.saturating_add(1);
        if self.elements > self.max_elements {
            if !self.over {
                self.over = true;
                let most = self.max_elements;
                self.error(
                    Code::Limit,
                    element.loc,
                    format!("the screen expands to more than {most} elements"),
                    "use parts fewer times, or split the screen",
                );
            }
            return Vec::new();
        }
        if let ElementName::Component(name) = &element.name
            && let Some((&key, part)) = self.parts.get_key_value(name.as_str())
        {
            return self.call(key, part, element, depth);
        }
        let mut expanded = element.clone();
        expanded.children = self.children(&element.children, depth);
        vec![Child::Element(expanded)]
    }

    /// A use of `part`: what it returns, its props and children given.
    fn call(
        &mut self,
        name: &'s str,
        part: &Part<'s>,
        element: &Element,
        depth: usize,
    ) -> Vec<Child> {
        if self.stack.contains(&name) {
            let through = self.stack.join(" > ");
            self.error(
                Code::Unsupported,
                element.loc,
                format!("<{name}> uses itself ({through} > {name}): a part cannot be recursive"),
                "a screen is expanded in full at build time, so its parts must end",
            );
            return Vec::new();
        }
        if self.stack.len() >= MAX_PART_DEPTH {
            self.error(
                Code::Limit,
                element.loc,
                format!("parts nest more than {MAX_PART_DEPTH} deep here"),
                "flatten some of them",
            );
            return Vec::new();
        }
        let mut given = BTreeMap::new();
        for attr in &element.attrs {
            match attr {
                Attr::Named {
                    name: prop,
                    value,
                    loc,
                } => {
                    if prop == "children" {
                        self.error(
                            Code::Unsupported,
                            *loc,
                            format!("<{name}> is given `children` as a prop"),
                            format!("write the children between <{name}> and </{name}>"),
                        );
                    } else if given.insert(prop.clone(), value.clone()).is_some() {
                        self.error(
                            Code::DuplicateName,
                            *loc,
                            format!("prop `{prop}` is given more than once"),
                            "give each prop once",
                        );
                    }
                }
                Attr::Spread { loc } => self.error(
                    Code::Unsupported,
                    *loc,
                    format!("a spread on <{name}>: its props are known only when the code runs"),
                    format!("give <{name}>'s props one by one"),
                ),
            }
        }
        let Some(returned) = part.returned else {
            return Vec::new();
        };
        let bound = match part.params {
            Params::None => Bound::None,
            Params::Destructured(bindings) => Bound::Destructured(bindings),
            Params::Named { name: object, .. } => Bound::Object(object),
            // Reported where the part is declared.
            Params::Unsupported(_) => return Vec::new(),
        };
        let children = self.children(&element.children, depth);
        let using = Use {
            part: name,
            bound,
            given,
            children,
            whole: None,
            shadow: Vec::new(),
            closure: false,
        };
        let returned = match self.subst_expr(&using, returned, depth) {
            Some(returned) => returned,
            None => return Vec::new(),
        };
        self.stack.push(name);
        let expanded = match &returned {
            Expr::Element(inner) => self.element(inner, depth.saturating_add(1)),
            Expr::Fragment { children, .. } => self.children(children, depth.saturating_add(1)),
            Expr::Null { .. } => Vec::new(),
            other => {
                let what = other.describe();
                self.error(
                    Code::Unsupported,
                    other.loc(),
                    format!("part <{name}> returns {what}"),
                    "a part returns one element, or a fragment of several",
                );
                Vec::new()
            }
        };
        self.stack.pop();
        expanded
    }

    /// A list: one copy of what the callback returns for each item, or, for
    /// a screen's prop sliced, one row for each place.
    fn map(
        &mut self,
        list: &Expr,
        most: Option<&Expr>,
        callback: &Function,
        loc: Loc,
        depth: usize,
    ) -> Vec<Child> {
        let help = "write .map((item) => <Row>…</Row>), or .map(({ name }) => …)";
        let bound = match &callback.params {
            Params::Named { name, .. } => Bound::Object(name),
            Params::Destructured(bindings) => Bound::Destructured(bindings),
            Params::None => Bound::None,
            Params::Unsupported(other) => {
                self.error(
                    Code::Unsupported,
                    other.loc,
                    "a list's callback takes the item alone",
                    help,
                );
                return Vec::new();
            }
        };
        let returned = match returned_values(callback).as_slice() {
            [one] if returns_only(callback) => *one,
            _ => {
                self.error(
                    Code::Unsupported,
                    callback.loc,
                    "a list's callback only returns what each item shows",
                    help,
                );
                return Vec::new();
            }
        };
        let most = match most {
            None => None,
            Some(Expr::Num { value, raw, .. })
                if value.fract() == 0.0
                    && (1.0..=f64::from(MAX_LIST_ROWS)).contains(value)
                    && !raw.starts_with('0') =>
            {
                // Exact: a whole number from 1 to MAX_LIST_ROWS, so the
                // text is its digits.
                raw.parse::<usize>().ok()
            }
            Some(other) => {
                self.error(
                    Code::InvalidValue,
                    other.loc(),
                    format!("a list's most must be a whole number from 1 to {MAX_LIST_ROWS}"),
                    "write .slice(0, 5)",
                );
                return Vec::new();
            }
        };
        let items = match list {
            Expr::Array { elements, .. } => Some(elements.as_slice()),
            Expr::Ident { name, .. } => match self.lists.get(name.as_str()) {
                Some(Expr::Array { elements, .. }) => Some(elements.as_slice()),
                _ => None,
            },
            _ => None,
        };
        if let Some(items) = items {
            let take = most.unwrap_or(items.len());
            let mut out = Vec::new();
            for item in items.iter().take(take) {
                let given = match item {
                    Expr::Object { props, .. } => props
                        .iter()
                        .map(|prop| (prop.key.clone(), AttrValue::Expr(prop.value.clone())))
                        .collect(),
                    _ => BTreeMap::new(),
                };
                let using = Use {
                    part: "a list",
                    bound,
                    given,
                    children: Vec::new(),
                    whole: Some(item.clone()),
                    shadow: Vec::new(),
                    closure: true,
                };
                if let Some(shown) = self.subst_expr(&using, returned, depth) {
                    out.extend(self.shown(without_key(shown), depth));
                }
            }
            return out;
        }
        let Some(key) = self.screen_prop(list) else {
            self.error(
                Code::Unsupported,
                list.loc(),
                format!("{} cannot be listed", list.describe()),
                "list an array literal, a top-level const array, or a prop of the screen sliced: props.items.slice(0, 5).map(…)",
            );
            return Vec::new();
        };
        let Some(rows) = most else {
            self.error(
                Code::Unsupported,
                loc,
                format!(
                    "the list `{key}` has no most: the screen must know how many rows it may show"
                ),
                format!(
                    "write {key}.slice(0, 5).map(…): at most 5 rows, which the application fills"
                ),
            );
            return Vec::new();
        };
        if self.in_row {
            self.error(
                Code::Unsupported,
                loc,
                "a list from a prop inside a row of another",
                "list one level from props; build-time lists may nest",
            );
            return Vec::new();
        }
        let mut fields = BTreeSet::new();
        match bound {
            Bound::Object(name) => members_of(returned, name, &mut fields, 0),
            Bound::Destructured(bindings) => fields.extend(bindings.iter().map(|b| b.key.clone())),
            Bound::None => {}
        }
        let made = |name: String, at: Loc| Expr::Ident {
            name: format!("{MADE_PROP}{name}"),
            loc: at,
        };
        let mut out = Vec::new();
        self.in_row = true;
        for row in 1..=rows {
            let given = fields
                .iter()
                .map(|field| {
                    (
                        field.clone(),
                        AttrValue::Expr(made(format!("{key}_{row}_{field}"), loc)),
                    )
                })
                .collect();
            let using = Use {
                part: "a list",
                bound,
                given,
                children: Vec::new(),
                whole: Some(made(format!("{key}_{row}_item"), loc)),
                shadow: Vec::new(),
                closure: true,
            };
            let Some(shown) = self.subst_expr(&using, returned, depth) else {
                continue;
            };
            let mut shown = self.shown(without_key(shown), depth);
            let element = match (shown.len(), shown.pop()) {
                (1, Some(Child::Element(element))) => element,
                _ => {
                    self.error(
                        Code::Unsupported,
                        returned.loc(),
                        format!("each row of the list `{key}` must be one element"),
                        "wrap the row in a <Row> or a <Column>",
                    );
                    break;
                }
            };
            out.push(Child::Expr(Expr::Logical {
                op: LogicalOp::And,
                left: Box::new(made(format!("{key}_{row}"), loc)),
                right: Box::new(Expr::Element(Box::new(element))),
                loc,
            }));
        }
        self.in_row = false;
        out
    }

    /// A condition with the part's props substituted, decided where an
    /// omitted prop, `undefined`, decides it: `undefined` is false,
    /// `!undefined` true and `undefined === "review"` false.
    fn truth(&mut self, using: &Use<'_>, test: &Expr, depth: usize) -> Truth {
        let inner = depth.saturating_add(1);
        match test {
            Expr::Not { operand, loc } => match self.subst_expr(using, operand, inner) {
                None => Truth::Always,
                Some(operand) => Truth::Unknown(Expr::Not {
                    operand: Box::new(operand),
                    loc: *loc,
                }),
            },
            Expr::Equals { left, right, loc } => {
                let left_given = self.subst_expr(using, left, inner);
                let right_given = self.subst_expr(using, right, inner);
                match (left_given, right_given) {
                    (None, Some(Expr::Str { .. })) | (Some(Expr::Str { .. }), None) => Truth::Never,
                    // Anything else the walk judges, `undefined` as `null`.
                    (left_given, right_given) => Truth::Unknown(Expr::Equals {
                        left: Box::new(left_given.unwrap_or(Expr::Null { loc: left.loc() })),
                        right: Box::new(right_given.unwrap_or(Expr::Null { loc: right.loc() })),
                        loc: *loc,
                    }),
                }
            }
            other => self
                .subst_expr(using, other, inner)
                .map_or(Truth::Never, Truth::Unknown),
        }
    }

    /// What one item shows, expanded.
    fn shown(&mut self, shown: Expr, depth: usize) -> Vec<Child> {
        match shown {
            Expr::Element(element) => self.element(&element, depth),
            Expr::Fragment { children, .. } => self.children(&children, depth),
            Expr::Null { .. } => Vec::new(),
            other => vec![Child::Expr(other)],
        }
    }

    /// The screen's prop `list` names, `props.items` or a destructured
    /// `items`: the list's name for its rows.
    fn screen_prop(&self, list: &Expr) -> Option<String> {
        match (list, self.screen?) {
            (
                Expr::Member {
                    object, property, ..
                },
                Params::Named { name, .. },
            ) if object == name => Some(property.clone()),
            (Expr::Ident { name, .. }, Params::Destructured(bindings)) => bindings
                .iter()
                .find(|binding| binding.local == *name)
                .map(|binding| binding.key.clone()),
            _ => None,
        }
    }

    /// What `name` refers to inside the part `using` expands.
    fn named<'n>(using: &'n Use<'_>, name: &'n str) -> Named<'n> {
        if using.shadow.iter().any(|shadowed| shadowed == name) {
            return Named::Shadowed;
        }
        match using.bound {
            Bound::Destructured(bindings) => bindings
                .iter()
                .find(|binding| binding.local == name)
                .map_or(Named::Other, |binding| Named::Prop(binding.key.as_str())),
            Bound::Object(object) if object == name => Named::Props,
            Bound::Object(_) | Bound::None => Named::Other,
        }
    }

    /// The prop an expression names, if it names one of the part's: `title`
    /// or `props.title`.
    fn prop_of<'n>(&mut self, using: &'n Use<'_>, expr: &'n Expr) -> Result<Option<&'n str>, ()> {
        match expr {
            Expr::Ident { name, loc } => match Self::named(using, name) {
                Named::Prop(key) => Ok(Some(key)),
                Named::Props => {
                    self.error(
                        Code::Unsupported,
                        *loc,
                        format!("<{}> passes on its whole props object", using.part),
                        "pass each prop by name",
                    );
                    Err(())
                }
                Named::Other => {
                    self.declared_here(using, name, *loc);
                    Ok(None)
                }
                Named::Shadowed => Ok(None),
            },
            Expr::Member {
                object,
                property,
                loc,
            } => match Self::named(using, object) {
                Named::Props => Ok(Some(property.as_str())),
                Named::Prop(_) | Named::Shadowed => Ok(None),
                Named::Other => {
                    self.declared_here(using, object, *loc);
                    Ok(None)
                }
            },
            _ => Ok(None),
        }
    }

    /// Reports `name`, used in a part, if it is neither the part's prop nor
    /// declared in the file: it would be the screen's, which React does not
    /// give a part.
    fn declared_here(&mut self, using: &Use<'_>, name: &str, loc: Loc) {
        if !using.closure && !self.declared.contains(name) {
            let part = using.part;
            self.error(
                Code::UnknownName,
                loc,
                format!("`{name}` is not a prop of <{part}>, nor declared in this file"),
                format!("pass it to <{part}> as a prop"),
            );
        }
    }

    /// An attribute's value with the part's props given: `None` when it is
    /// a prop the use does not give, so the attribute is left out.
    fn subst_value(
        &mut self,
        using: &Use<'_>,
        value: &AttrValue,
        depth: usize,
    ) -> Option<AttrValue> {
        let AttrValue::Expr(expr) = value else {
            return Some(value.clone());
        };
        if let Some(whole) = Self::whole_of(using, expr) {
            return Some(match whole {
                Expr::Str { value, loc } => AttrValue::Str {
                    value: value.clone(),
                    loc: *loc,
                },
                other => AttrValue::Expr(other.clone()),
            });
        }
        match self.prop_of(using, expr) {
            Ok(Some("children")) => {
                self.error(
                    Code::Unsupported,
                    expr.loc(),
                    "`children` given as a prop's value",
                    "children go between the tags",
                );
                Some(value.clone())
            }
            Ok(Some(key)) => using.given.get(key).cloned(),
            Ok(None) => self.subst_expr(using, expr, depth).map(AttrValue::Expr),
            Err(()) => Some(value.clone()),
        }
    }

    /// An expression with the part's props given: `None` when it is
    /// `undefined`.
    /// The parameter itself, if `expr` names it and it has a value whole.
    fn whole_of<'u>(using: &'u Use<'_>, expr: &Expr) -> Option<&'u Expr> {
        match (expr, using.bound, &using.whole) {
            (Expr::Ident { name, .. }, Bound::Object(object), Some(whole)) if name == object => {
                Some(whole)
            }
            _ => None,
        }
    }

    fn subst_expr(&mut self, using: &Use<'_>, expr: &Expr, depth: usize) -> Option<Expr> {
        let inner = depth.saturating_add(1);
        if let Some(whole) = Self::whole_of(using, expr) {
            return Some(whole.clone());
        }
        match self.prop_of(using, expr) {
            Ok(Some("children")) => {
                self.error(
                    Code::Unsupported,
                    expr.loc(),
                    "`children` used as a value",
                    "children go between the tags of an element: {children}",
                );
                return Some(expr.clone());
            }
            Ok(Some(key)) => {
                return match using.given.get(key)? {
                    AttrValue::Str { value, loc } => Some(Expr::Str {
                        value: value.clone(),
                        loc: *loc,
                    }),
                    AttrValue::Expr(given) => Some(given.clone()),
                    AttrValue::Absent => {
                        self.error(
                            Code::Unsupported,
                            expr.loc(),
                            format!(
                                "`{key}` is given no value where <{}> is used, so it is `true`",
                                using.part
                            ),
                            format!("give it a value: {key}={{…}}"),
                        );
                        None
                    }
                    // Already reported, or a syntax error.
                    AttrValue::InvalidStr { .. } | AttrValue::Empty { .. } => None,
                };
            }
            Ok(None) => {}
            Err(()) => return Some(expr.clone()),
        }
        if depth > MAX_TREE_DEPTH {
            return Some(expr.clone());
        }
        Some(match expr {
            Expr::Element(element) => {
                Expr::Element(Box::new(self.subst_element(using, element, inner)))
            }
            Expr::Fragment { children, loc } => Expr::Fragment {
                children: self.subst_children(using, children, inner),
                loc: *loc,
            },
            Expr::Logical {
                op,
                left,
                right,
                loc,
            } => {
                let left = if *op == LogicalOp::And {
                    match self.truth(using, left, inner) {
                        Truth::Never => return None,
                        Truth::Always => return self.subst_expr(using, right, inner),
                        Truth::Unknown(left) => Some(left),
                    }
                } else {
                    self.subst_expr(using, left, inner)
                };
                let right = self.subst_expr(using, right, inner);
                match (op, left, right) {
                    // `undefined && x` and `x && undefined` show nothing.
                    (LogicalOp::And, None, _) | (LogicalOp::And, _, None) => return None,
                    (_, None, right) => return right,
                    (_, Some(left), None) => return Some(left),
                    (_, Some(left), Some(right)) => Expr::Logical {
                        op: *op,
                        left: Box::new(left),
                        right: Box::new(right),
                        loc: *loc,
                    },
                }
            }
            Expr::Conditional {
                test,
                consequent,
                alternate,
                loc,
            } => {
                let null = |at: Loc| Expr::Null { loc: at };
                let consequent = self
                    .subst_expr(using, consequent, inner)
                    .unwrap_or_else(|| null(consequent.loc()));
                let alternate = self
                    .subst_expr(using, alternate, inner)
                    .unwrap_or_else(|| null(alternate.loc()));
                // `undefined ? a : b` is `b`, `!undefined ? a : b` is `a`.
                let test = match self.truth(using, test, inner) {
                    Truth::Never => return Some(alternate),
                    Truth::Always => return Some(consequent),
                    Truth::Unknown(test) => test,
                };
                Expr::Conditional {
                    test: Box::new(test),
                    consequent: Box::new(consequent),
                    alternate: Box::new(alternate),
                    loc: *loc,
                }
            }
            Expr::Array { elements, loc } => Expr::Array {
                elements: elements
                    .iter()
                    .map(|element| {
                        self.subst_expr(using, element, inner)
                            .unwrap_or(Expr::Null { loc: element.loc() })
                    })
                    .collect(),
                loc: *loc,
            },
            Expr::Object { props, loc } => Expr::Object {
                props: props
                    .iter()
                    .filter_map(|prop| {
                        self.subst_expr(using, &prop.value, inner)
                            .map(|value| ObjectProp {
                                key: prop.key.clone(),
                                value,
                                loc: prop.loc,
                            })
                    })
                    .collect(),
                loc: *loc,
            },
            Expr::Map {
                list,
                most,
                callback,
                loc,
            } => {
                let list = self.subst_expr(using, list, inner)?;
                let most = match most {
                    Some(most) => Some(Box::new(self.subst_expr(using, most, inner)?)),
                    None => None,
                };
                // The callback's own parameters are not the part's.
                let shadowing = Use {
                    part: using.part,
                    bound: using.bound,
                    given: using.given.clone(),
                    children: using.children.clone(),
                    whole: using.whole.clone(),
                    closure: using.closure,
                    shadow: using
                        .shadow
                        .iter()
                        .cloned()
                        .chain(bound_names(&callback.params))
                        .collect(),
                };
                let mut function = (**callback).clone();
                function.body = match &callback.body {
                    Body::Expr(body) => Body::Expr(Box::new(
                        self.subst_expr(&shadowing, body, inner)
                            .unwrap_or(Expr::Null { loc: body.loc() }),
                    )),
                    Body::Block(statements) => Body::Block(
                        statements
                            .iter()
                            .map(|statement| match statement {
                                Stmt::Return {
                                    value: Some(value),
                                    loc,
                                } => Stmt::Return {
                                    value: self.subst_expr(&shadowing, value, inner),
                                    loc: *loc,
                                },
                                other => other.clone(),
                            })
                            .collect(),
                    ),
                };
                Expr::Map {
                    list: Box::new(list),
                    most,
                    callback: Box::new(function),
                    loc: *loc,
                }
            }
            // `!undefined` is true, which the subset cannot hold: the walk
            // refuses a `null` where it stands.
            // In a template literal an omitted prop is the text
            // "undefined", as JavaScript writes it.
            Expr::Template { pieces, exprs, loc } => Expr::Template {
                pieces: pieces.clone(),
                exprs: exprs
                    .iter()
                    .map(|expr| {
                        self.subst_expr(using, expr, inner)
                            .unwrap_or_else(|| Expr::Str {
                                value: "undefined".to_owned(),
                                loc: expr.loc(),
                            })
                    })
                    .collect(),
                loc: *loc,
            },
            Expr::Not { operand, loc } => Expr::Not {
                operand: Box::new(
                    self.subst_expr(using, operand, inner)
                        .unwrap_or(Expr::Null { loc: operand.loc() }),
                ),
                loc: *loc,
            },
            Expr::Equals { left, right, loc } => Expr::Equals {
                left: Box::new(
                    self.subst_expr(using, left, inner)
                        .unwrap_or(Expr::Null { loc: left.loc() }),
                ),
                right: Box::new(
                    self.subst_expr(using, right, inner)
                        .unwrap_or(Expr::Null { loc: right.loc() }),
                ),
                loc: *loc,
            },
            // Refused by the walk wherever it stands; nothing inside is
            // substituted.
            Expr::Function(_) => expr.clone(),
            Expr::Str { .. }
            | Expr::Num { .. }
            | Expr::Ident { .. }
            | Expr::Member { .. }
            | Expr::Null { .. }
            | Expr::Other(_) => expr.clone(),
        })
    }

    /// An element of the part, its props given.
    fn subst_element(&mut self, using: &Use<'_>, element: &Element, depth: usize) -> Element {
        let mut attrs = Vec::with_capacity(element.attrs.len());
        for attr in &element.attrs {
            match attr {
                Attr::Named { name, value, loc } => {
                    if let Some(value) = self.subst_value(using, value, depth) {
                        attrs.push(Attr::Named {
                            name: name.clone(),
                            value,
                            loc: *loc,
                        });
                    }
                }
                Attr::Spread { .. } => attrs.push(attr.clone()),
            }
        }
        Element {
            name: element.name.clone(),
            attrs,
            children: self.subst_children(using, &element.children, depth),
            loc: element.loc,
        }
    }

    /// Children of the part, its props given and `{children}` replaced by
    /// the use's.
    fn subst_children(&mut self, using: &Use<'_>, children: &[Child], depth: usize) -> Vec<Child> {
        let mut out = Vec::with_capacity(children.len());
        if depth > MAX_TREE_DEPTH {
            return children.to_vec();
        }
        let inner = depth.saturating_add(1);
        for child in children {
            match child {
                Child::Expr(expr) if Self::whole_of(using, expr).is_some() => {
                    if let Some(whole) = Self::whole_of(using, expr) {
                        out.push(Child::Expr(whole.clone()));
                    }
                }
                Child::Expr(expr) => match self.prop_of(using, expr) {
                    Ok(Some("children")) => out.extend(using.children.iter().cloned()),
                    _ => {
                        if let Some(expr) = self.subst_expr(using, expr, inner) {
                            out.push(Child::Expr(expr));
                        }
                    }
                },
                Child::Element(element) => {
                    out.push(Child::Element(self.subst_element(using, element, inner)));
                }
                Child::Fragment { children, loc } => out.push(Child::Fragment {
                    children: self.subst_children(using, children, inner),
                    loc: *loc,
                }),
                Child::Text { .. }
                | Child::InvalidText { .. }
                | Child::Empty { .. }
                | Child::Spread { .. } => out.push(child.clone()),
            }
        }
        out
    }
}

/// Every element name a function's returned values use, so the file's
/// components used as elements are known before any is expanded.
pub(crate) fn element_names<'e>(expr: &'e Expr, out: &mut BTreeSet<&'e str>, depth: usize) {
    if depth > MAX_TREE_DEPTH {
        return;
    }
    let inner = depth.saturating_add(1);
    match expr {
        Expr::Element(element) => {
            if let ElementName::Component(name) = &element.name {
                out.insert(name.as_str());
            }
            for attr in &element.attrs {
                if let Attr::Named {
                    value: AttrValue::Expr(value),
                    ..
                } = attr
                {
                    element_names(value, out, inner);
                }
            }
            child_names(&element.children, out, inner);
        }
        Expr::Fragment { children, .. } => child_names(children, out, inner),
        Expr::Logical { left, right, .. } => {
            element_names(left, out, inner);
            element_names(right, out, inner);
        }
        Expr::Conditional {
            test,
            consequent,
            alternate,
            ..
        } => {
            element_names(test, out, inner);
            element_names(consequent, out, inner);
            element_names(alternate, out, inner);
        }
        Expr::Array { elements, .. } => {
            for element in elements {
                element_names(element, out, inner);
            }
        }
        Expr::Object { props, .. } => {
            for prop in props {
                element_names(&prop.value, out, inner);
            }
        }
        Expr::Not { operand, .. } => element_names(operand, out, inner),
        Expr::Template { exprs, .. } => {
            for expr in exprs {
                element_names(expr, out, inner);
            }
        }
        Expr::Equals { left, right, .. } => {
            element_names(left, out, inner);
            element_names(right, out, inner);
        }
        Expr::Map { list, callback, .. } => {
            element_names(list, out, inner);
            for returned in returned_values(callback) {
                element_names(returned, out, inner);
            }
        }
        Expr::Function(_)
        | Expr::Str { .. }
        | Expr::Num { .. }
        | Expr::Ident { .. }
        | Expr::Member { .. }
        | Expr::Null { .. }
        | Expr::Other(_) => {}
    }
}

/// A condition as a part's use leaves it.
enum Truth {
    /// False at build time: an omitted prop, `undefined`.
    Never,
    /// True at build time: `!undefined`.
    Always,
    /// The application's to decide.
    Unknown(Expr),
}

/// The values a function returns, as written.
pub(crate) fn returned_values(function: &Function) -> Vec<&Expr> {
    match &function.body {
        Body::Expr(expr) => vec![expr],
        Body::Block(statements) => statements
            .iter()
            .filter_map(|statement| match statement {
                Stmt::Return {
                    value: Some(value), ..
                } => Some(value),
                _ => None,
            })
            .collect(),
    }
}

/// Whether a function's body is its returned value alone: an arrow's
/// expression, or a block that is one `return`.
fn returns_only(function: &Function) -> bool {
    match &function.body {
        Body::Expr(_) => true,
        Body::Block(statements) => {
            matches!(statements.as_slice(), [Stmt::Return { value: Some(_), .. }])
        }
    }
}

/// The names a function's parameters bind.
fn bound_names(params: &Params) -> Vec<String> {
    match params {
        Params::Named { name, .. } => vec![name.clone()],
        Params::Destructured(bindings) => bindings.iter().map(|b| b.local.clone()).collect(),
        Params::None | Params::Unsupported(_) => Vec::new(),
    }
}

/// Each property `name.property` reads in `expr`, for a row's fields.
fn members_of(expr: &Expr, name: &str, out: &mut BTreeSet<String>, depth: usize) {
    if depth > MAX_TREE_DEPTH {
        return;
    }
    let inner = depth.saturating_add(1);
    match expr {
        Expr::Member {
            object, property, ..
        } if object == name => {
            out.insert(property.clone());
        }
        Expr::Element(element) => members_in_element(element, name, out, inner),
        Expr::Fragment { children, .. } => members_in_children(children, name, out, inner),
        Expr::Logical { left, right, .. } => {
            members_of(left, name, out, inner);
            members_of(right, name, out, inner);
        }
        Expr::Conditional {
            test,
            consequent,
            alternate,
            ..
        } => {
            members_of(test, name, out, inner);
            members_of(consequent, name, out, inner);
            members_of(alternate, name, out, inner);
        }
        Expr::Array { elements, .. } => {
            for element in elements {
                members_of(element, name, out, inner);
            }
        }
        Expr::Object { props, .. } => {
            for prop in props {
                members_of(&prop.value, name, out, inner);
            }
        }
        Expr::Map { list, most, .. } => {
            members_of(list, name, out, inner);
            if let Some(most) = most {
                members_of(most, name, out, inner);
            }
        }
        _ => {}
    }
}

fn members_in_element(element: &Element, name: &str, out: &mut BTreeSet<String>, depth: usize) {
    for attr in &element.attrs {
        if let Attr::Named {
            value: AttrValue::Expr(value),
            ..
        } = attr
        {
            members_of(value, name, out, depth);
        }
    }
    members_in_children(&element.children, name, out, depth);
}

fn members_in_children(children: &[Child], name: &str, out: &mut BTreeSet<String>, depth: usize) {
    if depth > MAX_TREE_DEPTH {
        return;
    }
    for child in children {
        match child {
            Child::Element(element) => {
                members_in_element(element, name, out, depth.saturating_add(1));
            }
            Child::Expr(expr) => members_of(expr, name, out, depth),
            Child::Fragment { children, .. } => {
                members_in_children(children, name, out, depth.saturating_add(1));
            }
            Child::Text { .. }
            | Child::InvalidText { .. }
            | Child::Empty { .. }
            | Child::Spread { .. } => {}
        }
    }
}

/// `expr` without `key` on the element it is: React's list key, which
/// tells nothing here.
fn without_key(expr: Expr) -> Expr {
    match expr {
        Expr::Element(mut element) => {
            element
                .attrs
                .retain(|attr| !matches!(attr, Attr::Named { name, .. } if name == "key"));
            Expr::Element(element)
        }
        other => other,
    }
}

fn child_names<'e>(children: &'e [Child], out: &mut BTreeSet<&'e str>, depth: usize) {
    if depth > MAX_TREE_DEPTH {
        return;
    }
    for child in children {
        match child {
            Child::Element(element) => {
                if let ElementName::Component(name) = &element.name {
                    out.insert(name.as_str());
                }
                for attr in &element.attrs {
                    if let Attr::Named {
                        value: AttrValue::Expr(value),
                        ..
                    } = attr
                    {
                        element_names(value, out, depth);
                    }
                }
                child_names(&element.children, out, depth.saturating_add(1));
            }
            Child::Expr(expr) => element_names(expr, out, depth),
            Child::Fragment { children, .. } => child_names(children, out, depth.saturating_add(1)),
            Child::Text { .. }
            | Child::InvalidText { .. }
            | Child::Empty { .. }
            | Child::Spread { .. } => {}
        }
    }
}
