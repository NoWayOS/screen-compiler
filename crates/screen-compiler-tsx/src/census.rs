// SPDX-License-Identifier: MIT
//! A census of a TSX module: every construct written in it, counted, for
//! the coverage report (`screen-compiler audit`).
//!
//! The census says what is written, never whether it converts: the
//! analyser's audit decides that. It walks oxc's whole tree, so it sees
//! what Screen Compiler's own syntax tree reduces to a description - the
//! elements inside a `.map` callback, a hook called before the `return`.
//! It records names (of elements, props, style properties, hooks, called
//! functions and imported packages) and operators, never code.

use std::collections::{BTreeMap, BTreeSet};

use oxc_ast::ast::{
    BinaryExpression, BindingPattern, CallExpression, Class, ConditionalExpression, Declaration,
    DoWhileStatement, ExportDefaultDeclarationKind, Expression, ForInStatement, ForOfStatement,
    ForStatement, FunctionBody, IfStatement, ImportDeclaration, ImportOrExportKind,
    JSXAttributeItem, JSXAttributeName, JSXAttributeValue, JSXChild, JSXElement, JSXElementName,
    JSXExpression, JSXFragment, JSXOpeningElement, LogicalExpression, ObjectPropertyKind, Program,
    PropertyKey, Statement, SwitchStatement, TemplateLiteral, UnaryExpression, VariableDeclaration,
    VariableDeclarationKind, WhileStatement,
};
use oxc_ast_visit::{Visit, walk};

/// One kind of construct a census counts.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Construct {
    /// `<Name …>`: a capitalised element - a component.
    Component(String),
    /// `<name …>`: a lowercase element - an HTML or SVG element.
    Html(String),
    /// `<a.b>`, `<a:b>` or `<this>`: a member or namespaced element name.
    OtherElement,
    /// `<>…</>`.
    Fragment,
    /// A prop on an element: the element's name as written and the prop's.
    Prop {
        /// The element's name.
        element: String,
        /// The prop's name.
        prop: String,
    },
    /// `{...spread}` on an element.
    SpreadProp,
    /// A property of a `style={{…}}` object literal.
    Style {
        /// The element's name.
        element: String,
        /// The property's name.
        property: String,
    },
    /// A `style` that is not an object literal, or one of its properties
    /// whose key is computed or spread.
    ComputedStyle,
    /// An element whose children mix text with `{…}` expressions.
    MixedText,
    /// A call to a hook: a function whose name is `use` and a capital.
    Hook(String),
    /// `.map(…)` on a list with no most: a prop's, or anything else.
    Map,
    /// `.map(…)` on a list whose length is known at build time: an array
    /// literal, a top-level `const` array, or one of these or a prop sliced
    /// with `.slice(0, N)`.
    BoundedMap,
    /// Any other call: the callee's name, or `.name` for a method.
    Call(String),
    /// A template literal.
    Template,
    /// A binary operator, such as `===` or `+`.
    Binary(&'static str),
    /// A unary operator, such as `!` or `typeof`.
    Unary(&'static str),
    /// `&&`, `||` or `??`.
    Logical(&'static str),
    /// `test ? a : b`.
    Conditional,
    /// An arrow or function expression given as a prop.
    InlineFunction {
        /// The element's name.
        element: String,
        /// The prop's name.
        prop: String,
    },
    /// An import, by where it comes from.
    Import(ImportKind),
    /// A class.
    Class,
    /// A statement inside a function other than its `return`: `if`,
    /// `switch`, a loop, or a local variable.
    Statement(&'static str),
}

/// Where an import comes from.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImportKind {
    /// `nowayos:kit`.
    Kit,
    /// `react` or one of its subpaths.
    React,
    /// A `.module.css` file.
    CssModule,
    /// Any other stylesheet: `.css`, `.scss`, `.sass`, `.less`.
    Stylesheet,
    /// A file of the project's own: a path starting `.` or `/`.
    Local,
    /// A package, by name.
    Package(String),
}

/// What a module holds: each construct and how many times it is written.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Census {
    /// Each construct, and how often.
    pub counts: BTreeMap<Construct, u32>,
    /// The module's own function components, by name: its top-level
    /// function declarations and `const` declarations of an arrow or a
    /// function, exported or not. An element of one of these names is the
    /// module's own component, not one from elsewhere.
    pub own_components: BTreeSet<String>,
    /// The module's top-level `const` arrays, by name.
    pub own_lists: BTreeSet<String>,
}

impl Census {
    fn add(&mut self, construct: Construct) {
        let count = self.counts.entry(construct).or_insert(0);
        *count = count.saturating_add(1);
    }
}

/// Counts every construct `program` holds.
pub(crate) fn count(program: &Program<'_>) -> Census {
    let mut counter = Counter {
        census: Census {
            counts: BTreeMap::new(),
            own_components: own_components(program),
            own_lists: own_lists(program),
        },
        functions: 0,
        bounded_slices: BTreeSet::new(),
    };
    counter.visit_program(program);
    counter.census
}

/// The names of `program`'s top-level `const` arrays ([`Census::own_lists`]).
fn own_lists(program: &Program<'_>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for statement in &program.body {
        let declaration = match statement {
            Statement::ExportNamedDeclaration(decl) if decl.source.is_none() => {
                decl.declaration.as_ref()
            }
            other => other.as_declaration(),
        };
        if let Some(Declaration::VariableDeclaration(decl)) = declaration
            && decl.kind == VariableDeclarationKind::Const
        {
            for declarator in &decl.declarations {
                if let (
                    BindingPattern::BindingIdentifier(id),
                    Some(Expression::ArrayExpression(_)),
                ) = (&declarator.id, &declarator.init)
                {
                    names.insert(id.name.as_str().to_owned());
                }
            }
        }
    }
    names
}

/// The names of `program`'s own function components ([`Census::own_components`]).
fn own_components(program: &Program<'_>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for statement in &program.body {
        let declaration = match statement {
            Statement::ExportNamedDeclaration(decl) if decl.source.is_none() => {
                decl.declaration.as_ref()
            }
            Statement::ExportDefaultDeclaration(decl) => {
                if let ExportDefaultDeclarationKind::FunctionDeclaration(function) =
                    &decl.declaration
                    && let Some(id) = &function.id
                {
                    names.insert(id.name.as_str().to_owned());
                }
                continue;
            }
            other => other.as_declaration(),
        };
        match declaration {
            Some(Declaration::FunctionDeclaration(function)) => {
                if let Some(id) = &function.id {
                    names.insert(id.name.as_str().to_owned());
                }
            }
            Some(Declaration::VariableDeclaration(decl))
                if decl.kind == VariableDeclarationKind::Const =>
            {
                for declarator in &decl.declarations {
                    if let (
                        BindingPattern::BindingIdentifier(id),
                        Some(
                            Expression::ArrowFunctionExpression(_)
                            | Expression::FunctionExpression(_),
                        ),
                    ) = (&declarator.id, &declarator.init)
                    {
                        names.insert(id.name.as_str().to_owned());
                    }
                }
            }
            _ => {}
        }
    }
    names
}

struct Counter {
    census: Census,
    /// How many functions the walk is inside.
    functions: usize,
    /// Where each `.slice(0, N)` a `.map` lists starts: part of that list,
    /// not a call of its own.
    bounded_slices: BTreeSet<u32>,
}

impl Counter {
    /// What `.map` on `list` is: bounded if its length is known at build
    /// time; a `.slice(0, N)` it lists is noted, so it is not counted again.
    fn map_construct(&mut self, list: &Expression<'_>) -> Construct {
        let mut list = list;
        while let Expression::ParenthesizedExpression(inner) = list {
            list = &inner.expression;
        }
        match list {
            Expression::ArrayExpression(_) => Construct::BoundedMap,
            Expression::Identifier(id) if self.census.own_lists.contains(id.name.as_str()) => {
                Construct::BoundedMap
            }
            Expression::CallExpression(slice)
                if matches!(&slice.callee, Expression::StaticMemberExpression(member)
                    if member.property.name.as_str() == "slice")
                    && matches!(slice.arguments.first().and_then(|a| a.as_expression()),
                        Some(Expression::NumericLiteral(from)) if from.value == 0.0)
                    && slice.arguments.len() == 2 =>
            {
                self.bounded_slices.insert(slice.span.start);
                Construct::BoundedMap
            }
            _ => Construct::Map,
        }
    }

    /// What a module specifier names.
    fn import_kind(module: &str) -> ImportKind {
        if module == "nowayos:kit" {
            ImportKind::Kit
        } else if module == "react" || module.starts_with("react/") {
            ImportKind::React
        } else if module.ends_with(".module.css") {
            ImportKind::CssModule
        } else if [".css", ".scss", ".sass", ".less"]
            .iter()
            .any(|ending| module.ends_with(ending))
        {
            ImportKind::Stylesheet
        } else if module.starts_with('.') || module.starts_with('/') {
            ImportKind::Local
        } else {
            ImportKind::Package(module.to_owned())
        }
    }

    /// The element's name as written, and what it is.
    fn element(name: &JSXElementName<'_>) -> (String, Construct) {
        match name {
            JSXElementName::Identifier(id) => {
                let name = id.name.as_str().to_owned();
                (name.clone(), Construct::Html(name))
            }
            JSXElementName::IdentifierReference(id) => {
                let name = id.name.as_str().to_owned();
                (name.clone(), Construct::Component(name))
            }
            JSXElementName::NamespacedName(_)
            | JSXElementName::MemberExpression(_)
            | JSXElementName::ThisExpression(_) => (String::new(), Construct::OtherElement),
        }
    }

    /// Counts a `style` attribute's properties.
    fn style(&mut self, element: &str, value: Option<&JSXAttributeValue<'_>>) {
        let object = match value {
            Some(JSXAttributeValue::ExpressionContainer(container)) => {
                match &container.expression {
                    JSXExpression::ObjectExpression(object) => Some(object),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some(object) = object else {
            self.census.add(Construct::ComputedStyle);
            return;
        };
        for property in &object.properties {
            let key = match property {
                ObjectPropertyKind::ObjectProperty(property) if !property.computed => {
                    match &property.key {
                        PropertyKey::StaticIdentifier(id) => Some(id.name.as_str().to_owned()),
                        PropertyKey::StringLiteral(s) => Some(s.value.as_str().to_owned()),
                        _ => None,
                    }
                }
                _ => None,
            };
            self.census.add(
                key.map_or(Construct::ComputedStyle, |property| Construct::Style {
                    element: element.to_owned(),
                    property,
                }),
            );
        }
    }
}

impl<'a> Visit<'a> for Counter {
    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
        // A type-only import is erased: nothing of it reaches the screen.
        if it.import_kind != ImportOrExportKind::Type {
            self.census.add(Construct::Import(Self::import_kind(
                it.source.value.as_str(),
            )));
        }
        walk::walk_import_declaration(self, it);
    }

    fn visit_jsx_element(&mut self, it: &JSXElement<'a>) {
        let text = it.children.iter().any(
            |child| matches!(child, JSXChild::Text(text) if !text.value.as_str().trim().is_empty()),
        );
        let expression = it.children.iter().any(|child| {
            matches!(child, JSXChild::ExpressionContainer(container)
                if !matches!(container.expression, JSXExpression::EmptyExpression(_)))
        });
        if text && expression {
            self.census.add(Construct::MixedText);
        }
        walk::walk_jsx_element(self, it);
    }

    fn visit_jsx_opening_element(&mut self, it: &JSXOpeningElement<'a>) {
        let (element, construct) = Self::element(&it.name);
        self.census.add(construct);
        for attribute in &it.attributes {
            match attribute {
                JSXAttributeItem::Attribute(attribute) => {
                    let prop = match &attribute.name {
                        JSXAttributeName::Identifier(id) => id.name.as_str().to_owned(),
                        JSXAttributeName::NamespacedName(n) => {
                            format!("{}:{}", n.namespace.name.as_str(), n.name.name.as_str())
                        }
                    };
                    if prop == "style" {
                        self.style(&element, attribute.value.as_ref());
                    }
                    if let Some(JSXAttributeValue::ExpressionContainer(container)) =
                        &attribute.value
                        && matches!(
                            container.expression,
                            JSXExpression::ArrowFunctionExpression(_)
                                | JSXExpression::FunctionExpression(_)
                        )
                    {
                        self.census.add(Construct::InlineFunction {
                            element: element.clone(),
                            prop: prop.clone(),
                        });
                    }
                    self.census.add(Construct::Prop {
                        element: element.clone(),
                        prop,
                    });
                }
                JSXAttributeItem::SpreadAttribute(_) => self.census.add(Construct::SpreadProp),
            }
        }
        walk::walk_jsx_opening_element(self, it);
    }

    fn visit_jsx_fragment(&mut self, it: &JSXFragment<'a>) {
        self.census.add(Construct::Fragment);
        walk::walk_jsx_fragment(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        let construct = match &it.callee {
            Expression::Identifier(id) => {
                let name = id.name.as_str();
                let hook = name
                    .strip_prefix("use")
                    .and_then(|rest| rest.chars().next())
                    .is_some_and(char::is_uppercase);
                if hook {
                    Construct::Hook(name.to_owned())
                } else {
                    Construct::Call(name.to_owned())
                }
            }
            Expression::StaticMemberExpression(member)
                if member.property.name.as_str() == "map" =>
            {
                self.map_construct(&member.object)
            }
            Expression::StaticMemberExpression(_)
                if self.bounded_slices.contains(&it.span.start) =>
            {
                walk::walk_call_expression(self, it);
                return;
            }
            Expression::StaticMemberExpression(member) => {
                Construct::Call(format!(".{}", member.property.name.as_str()))
            }
            _ => Construct::Call(String::new()),
        };
        self.census.add(construct);
        walk::walk_call_expression(self, it);
    }

    fn visit_template_literal(&mut self, it: &TemplateLiteral<'a>) {
        self.census.add(Construct::Template);
        walk::walk_template_literal(self, it);
    }

    fn visit_binary_expression(&mut self, it: &BinaryExpression<'a>) {
        self.census.add(Construct::Binary(it.operator.as_str()));
        walk::walk_binary_expression(self, it);
    }

    fn visit_unary_expression(&mut self, it: &UnaryExpression<'a>) {
        self.census.add(Construct::Unary(it.operator.as_str()));
        walk::walk_unary_expression(self, it);
    }

    fn visit_logical_expression(&mut self, it: &LogicalExpression<'a>) {
        self.census.add(Construct::Logical(it.operator.as_str()));
        walk::walk_logical_expression(self, it);
    }

    fn visit_conditional_expression(&mut self, it: &ConditionalExpression<'a>) {
        self.census.add(Construct::Conditional);
        walk::walk_conditional_expression(self, it);
    }

    fn visit_class(&mut self, it: &Class<'a>) {
        self.census.add(Construct::Class);
        walk::walk_class(self, it);
    }

    fn visit_function_body(&mut self, it: &FunctionBody<'a>) {
        self.functions = self.functions.saturating_add(1);
        walk::walk_function_body(self, it);
        self.functions = self.functions.saturating_sub(1);
    }

    fn visit_variable_declaration(&mut self, it: &VariableDeclaration<'a>) {
        // A module's own constants are design tokens; inside a function a
        // variable is computed while the screen is drawn.
        if self.functions > 0 {
            self.census.add(Construct::Statement("local variable"));
        }
        walk::walk_variable_declaration(self, it);
    }

    fn visit_if_statement(&mut self, it: &IfStatement<'a>) {
        self.census.add(Construct::Statement("if"));
        walk::walk_if_statement(self, it);
    }

    fn visit_switch_statement(&mut self, it: &SwitchStatement<'a>) {
        self.census.add(Construct::Statement("switch"));
        walk::walk_switch_statement(self, it);
    }

    fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
        self.census.add(Construct::Statement("loop"));
        walk::walk_for_statement(self, it);
    }

    fn visit_for_in_statement(&mut self, it: &ForInStatement<'a>) {
        self.census.add(Construct::Statement("loop"));
        walk::walk_for_in_statement(self, it);
    }

    fn visit_for_of_statement(&mut self, it: &ForOfStatement<'a>) {
        self.census.add(Construct::Statement("loop"));
        walk::walk_for_of_statement(self, it);
    }

    fn visit_while_statement(&mut self, it: &WhileStatement<'a>) {
        self.census.add(Construct::Statement("loop"));
        walk::walk_while_statement(self, it);
    }

    fn visit_do_while_statement(&mut self, it: &DoWhileStatement<'a>) {
        self.census.add(Construct::Statement("loop"));
        walk::walk_do_while_statement(self, it);
    }
}
