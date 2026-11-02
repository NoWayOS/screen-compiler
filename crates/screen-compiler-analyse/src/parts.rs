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
    Attr, AttrValue, Child, Element, ElementName, Expr, LogicalOp, ObjectProp, Params, PropBinding,
};

/// How deep parts may use parts.
pub(crate) const MAX_PART_DEPTH: usize = 16;

/// How many elements the expansion may make for each node the walk takes:
/// children given to a part are expanded where they are written, then
/// walked again inside the part, so they count more than once.
pub(crate) const ELEMENTS_PER_NODE: usize = 8;

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
}

/// What a name in a part refers to.
enum Named<'n> {
    /// One of its props, by name.
    Prop(&'n str),
    /// Its whole props object.
    Props,
    /// Something declared in the file, or not a name of the part's.
    Other,
}

/// Expands the parts and fragments of one screen.
pub(crate) struct Expander<'p, 's> {
    parts: &'p BTreeMap<&'s str, Part<'s>>,
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
        declared: &'p BTreeSet<String>,
        max_elements: usize,
    ) -> Self {
        Self {
            parts,
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
    pub(crate) fn root(&mut self, returned: &'s Expr) -> Expr {
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

    /// What `name` refers to inside the part `using` expands.
    fn named<'n>(using: &'n Use<'_>, name: &'n str) -> Named<'n> {
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
            },
            Expr::Member {
                object,
                property,
                loc,
            } => match Self::named(using, object) {
                Named::Props => Ok(Some(property.as_str())),
                Named::Prop(_) => Ok(None),
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
        if !self.declared.contains(name) {
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
    fn subst_expr(&mut self, using: &Use<'_>, expr: &Expr, depth: usize) -> Option<Expr> {
        let inner = depth.saturating_add(1);
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
                let left = self.subst_expr(using, left, inner);
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
                // `undefined ? a : b` is `b`.
                let Some(test) = self.subst_expr(using, test, inner) else {
                    return Some(alternate);
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
        Expr::Function(_)
        | Expr::Str { .. }
        | Expr::Num { .. }
        | Expr::Ident { .. }
        | Expr::Member { .. }
        | Expr::Null { .. }
        | Expr::Other(_) => {}
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
