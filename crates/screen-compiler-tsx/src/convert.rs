// SPDX-License-Identifier: MIT
//! oxc AST to Screen Compiler's syntax tree. The only module that names oxc types.

use oxc_ast::ast::{
    self as js, ArrayExpressionElement, BindingPattern, ExportDefaultDeclarationKind, Expression,
    FormalParameters, FunctionBody, ImportDeclarationSpecifier, JSXAttributeItem, JSXAttributeName,
    JSXAttributeValue, JSXChild, JSXElementName, JSXExpression, JSXMemberExpressionObject,
    LogicalOperator, ModuleExportName, PropertyKey, Statement, UnaryOperator,
    VariableDeclarationKind,
};
use oxc_span::{GetSpan, Span};

use screen_compiler_ir::{Code, Diagnostic, Loc};

use crate::jsx_text::{self, TextError};
use crate::lines::LineIndex;
use crate::syntax::{
    Attr, AttrValue, Body, Child, DeclKeyword, Element, ElementName, Export, ExportSpecifier, Expr,
    Function, FunctionDecl, Import, ImportBinding, Item, LogicalOp, ObjectProp, Other, Params,
    PropBinding, SourceModule, Stmt, TemplatePiece, VarDecl,
};

/// JSX nesting deeper than this is not converted (the stack stays bounded);
/// the analyser's own limit is far lower.
pub const MAX_NESTING: usize = 256;

pub struct Converter<'s> {
    source: &'s str,
    /// The file's number, which every location carries.
    file: u16,
    lines: LineIndex,
    diagnostics: Vec<Diagnostic>,
    nesting_reported: bool,
}

impl<'s> Converter<'s> {
    pub fn new(source: &'s str, file: u16) -> Self {
        Self {
            source,
            file,
            lines: LineIndex::new(source),
            diagnostics: Vec::new(),
            nesting_reported: false,
        }
    }

    fn at(&self, offset: usize) -> Loc {
        self.lines.loc(self.source, offset).in_file(self.file)
    }

    fn loc(&self, span: Span) -> Loc {
        self.at(offset(span))
    }

    fn other(&self, kind: impl Into<String>, span: Span) -> Other {
        Other {
            kind: kind.into(),
            loc: self.loc(span),
        }
    }

    pub fn module(&mut self, program: &js::Program<'_>) -> SourceModule {
        let mut items = Vec::new();
        for directive in &program.directives {
            items.push(Item::Other(self.other(
                format!("directive \"{}\"", directive.directive.as_str()),
                directive.span,
            )));
        }
        for statement in &program.body {
            self.top_level(statement, &mut items);
        }
        SourceModule {
            items,
            diagnostics: std::mem::take(&mut self.diagnostics),
        }
    }

    fn top_level(&mut self, statement: &Statement<'_>, items: &mut Vec<Item>) {
        let span = statement.span();
        match statement {
            Statement::ImportDeclaration(decl) => items.push(Item::Import(self.import(decl))),
            Statement::ExportNamedDeclaration(decl) => {
                if decl.source.is_some() {
                    items.push(Item::Other(
                        self.other("re-export from another module", span),
                    ));
                } else if let Some(declaration) = &decl.declaration {
                    self.declaration(declaration, Export::Named, items);
                } else if decl.export_kind.is_type() {
                    items.push(Item::TypeOnly {
                        names: Vec::new(),
                        loc: self.loc(span),
                    });
                } else {
                    let mut specifiers = Vec::new();
                    for specifier in &decl.specifiers {
                        if specifier.export_kind.is_type() {
                            continue;
                        }
                        specifiers.push(ExportSpecifier {
                            local: self.export_name(&specifier.local),
                            exported: self.export_name(&specifier.exported),
                            loc: self.loc(specifier.span),
                        });
                    }
                    items.push(Item::ExportList {
                        specifiers,
                        loc: self.loc(span),
                    });
                }
            }
            Statement::ExportDefaultDeclaration(decl) => match &decl.declaration {
                ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                    items.push(self.function_declaration(function, Export::Default));
                }
                ExportDefaultDeclarationKind::ClassDeclaration(_) => {
                    items.push(Item::Other(self.other("class declaration", span)));
                }
                ExportDefaultDeclarationKind::TSInterfaceDeclaration(interface) => {
                    items.push(Item::TypeOnly {
                        names: vec![interface.id.name.as_str().to_owned()],
                        loc: self.loc(span),
                    });
                }
                other => match other.as_expression() {
                    Some(expression) => {
                        let expr = self.expr(expression, 0);
                        items.push(Item::ExportDefaultExpr(expr));
                    }
                    None => items.push(Item::Other(self.other("default export", span))),
                },
            },
            Statement::ExportAllDeclaration(_) => {
                items.push(Item::Other(self.other("`export * from`", span)));
            }
            Statement::EmptyStatement(_) => {}
            other => {
                if let Some(declaration) = other.as_declaration() {
                    self.declaration(declaration, Export::None, items);
                } else {
                    items.push(Item::Other(self.other(statement_kind(other), span)));
                }
            }
        }
    }

    fn declaration(
        &mut self,
        declaration: &js::Declaration<'_>,
        export: Export,
        items: &mut Vec<Item>,
    ) {
        let span = declaration.span();
        match declaration {
            js::Declaration::VariableDeclaration(decl) => {
                if decl.declare {
                    let names = decl
                        .declarations
                        .iter()
                        .filter_map(|d| match &d.id {
                            BindingPattern::BindingIdentifier(id) => {
                                Some(id.name.as_str().to_owned())
                            }
                            _ => None,
                        })
                        .collect();
                    items.push(Item::TypeOnly {
                        names,
                        loc: self.loc(span),
                    });
                    return;
                }
                let keyword = match decl.kind {
                    VariableDeclarationKind::Const => DeclKeyword::Const,
                    VariableDeclarationKind::Let => DeclKeyword::Let,
                    VariableDeclarationKind::Var => DeclKeyword::Var,
                    VariableDeclarationKind::Using | VariableDeclarationKind::AwaitUsing => {
                        DeclKeyword::Using
                    }
                };
                for declarator in &decl.declarations {
                    match &declarator.id {
                        BindingPattern::BindingIdentifier(id) => {
                            let init = declarator.init.as_ref().map(|e| self.expr(e, 0));
                            items.push(Item::Var(VarDecl {
                                keyword,
                                name: id.name.as_str().to_owned(),
                                name_loc: self.loc(id.span),
                                init,
                                export,
                                loc: self.loc(declarator.span),
                            }));
                        }
                        _ => items.push(Item::Other(
                            self.other("destructuring declaration", declarator.span),
                        )),
                    }
                }
            }
            js::Declaration::FunctionDeclaration(function) => {
                items.push(self.function_declaration(function, export));
            }
            js::Declaration::TSTypeAliasDeclaration(alias) => items.push(Item::TypeOnly {
                names: vec![alias.id.name.as_str().to_owned()],
                loc: self.loc(span),
            }),
            js::Declaration::TSInterfaceDeclaration(interface) => items.push(Item::TypeOnly {
                names: vec![interface.id.name.as_str().to_owned()],
                loc: self.loc(span),
            }),
            js::Declaration::ClassDeclaration(_) => {
                items.push(Item::Other(self.other("class declaration", span)));
            }
            js::Declaration::TSEnumDeclaration(_) => {
                items.push(Item::Other(self.other("enum declaration", span)));
            }
            js::Declaration::TSModuleDeclaration(_) | js::Declaration::TSGlobalDeclaration(_) => {
                items.push(Item::Other(
                    self.other("namespace or module declaration", span),
                ));
            }
            js::Declaration::TSImportEqualsDeclaration(_) => {
                items.push(Item::Other(self.other("`import … = require(…)`", span)));
            }
        }
    }

    fn function_declaration(&mut self, function: &js::Function<'_>, export: Export) -> Item {
        if function.declare || function.body.is_none() {
            return Item::TypeOnly {
                names: function
                    .id
                    .iter()
                    .map(|id| id.name.as_str().to_owned())
                    .collect(),
                loc: self.loc(function.span),
            };
        }
        Item::Function(FunctionDecl {
            name: function.id.as_ref().map(|id| id.name.as_str().to_owned()),
            function: self.function(&FnParts::function(function), 0),
            export,
        })
    }

    fn import(&mut self, decl: &js::ImportDeclaration<'_>) -> Import {
        self.check_string_escapes(decl.source.span);
        let mut bindings = Vec::new();
        for specifier in decl.specifiers.iter().flatten() {
            bindings.push(match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(s) => ImportBinding::Named {
                    imported: self.export_name(&s.imported),
                    local: s.local.name.as_str().to_owned(),
                    type_only: s.import_kind.is_type(),
                    loc: self.loc(s.span),
                },
                ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => ImportBinding::Default {
                    local: s.local.name.as_str().to_owned(),
                    loc: self.loc(s.span),
                },
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                    ImportBinding::Namespace {
                        local: s.local.name.as_str().to_owned(),
                        loc: self.loc(s.span),
                    }
                }
            });
        }
        Import {
            module: decl.source.value.as_str().to_owned(),
            type_only: decl.import_kind.is_type(),
            bindings,
            side_effect_only: decl.specifiers.is_none(),
            has_attributes: decl.with_clause.is_some() || decl.phase.is_some(),
            loc: self.loc(decl.span),
        }
    }

    /// Converts a function whose body sits at nesting `depth`.
    fn function(&mut self, parts: &FnParts<'_, '_>, depth: usize) -> Function {
        let params = self.params(parts.params);
        let body = match parts.body {
            Some(body) if parts.expression_body => match body.statements.first() {
                Some(Statement::ExpressionStatement(statement)) => {
                    Body::Expr(Box::new(self.expr(&statement.expression, depth)))
                }
                _ => Body::Block(Vec::new()),
            },
            Some(body) => {
                let mut statements: Vec<Stmt> = body
                    .directives
                    .iter()
                    .map(|d| Stmt::Other(self.other("directive", d.span)))
                    .collect();
                for statement in &body.statements {
                    if let Some(converted) = self.statement(statement, depth) {
                        statements.push(converted);
                    }
                }
                Body::Block(statements)
            }
            None => Body::Block(Vec::new()),
        };
        let (is_async, is_generator, span) = (parts.is_async, parts.is_generator, parts.span);
        Function {
            params,
            body,
            is_async,
            is_generator,
            loc: self.loc(span),
        }
    }

    fn params(&mut self, params: &FormalParameters<'_>) -> Params {
        if let Some(rest) = &params.rest {
            return Params::Unsupported(self.other("rest parameter", rest.span));
        }
        let mut items = params.items.iter();
        let (Some(param), None) = (items.next(), items.next()) else {
            return if params.items.is_empty() {
                Params::None
            } else {
                Params::Unsupported(self.other("more than one parameter", params.span))
            };
        };
        if param.initializer.is_some() {
            return Params::Unsupported(self.other("default parameter value", param.span));
        }
        match &param.pattern {
            BindingPattern::BindingIdentifier(id) => Params::Named {
                name: id.name.as_str().to_owned(),
                loc: self.loc(id.span),
            },
            BindingPattern::ObjectPattern(object) => {
                if let Some(rest) = &object.rest {
                    return Params::Unsupported(self.other("rest element in props", rest.span));
                }
                let mut bindings = Vec::new();
                for property in &object.properties {
                    let key = match &property.key {
                        PropertyKey::StaticIdentifier(id) if !property.computed => {
                            id.name.as_str().to_owned()
                        }
                        PropertyKey::StringLiteral(s) if !property.computed => {
                            self.check_string_escapes(s.span);
                            s.value.as_str().to_owned()
                        }
                        _ => {
                            return Params::Unsupported(
                                self.other("computed prop name", property.span),
                            );
                        }
                    };
                    let local =
                        match &property.value {
                            BindingPattern::BindingIdentifier(id) => id.name.as_str().to_owned(),
                            BindingPattern::AssignmentPattern(_) => {
                                return Params::Unsupported(self.other(
                                    format!("default value for prop `{key}`"),
                                    property.span,
                                ));
                            }
                            _ => {
                                return Params::Unsupported(
                                    self.other("nested destructuring", property.span),
                                );
                            }
                        };
                    bindings.push(PropBinding {
                        key,
                        local,
                        loc: self.loc(property.span),
                    });
                }
                Params::Destructured(bindings)
            }
            BindingPattern::ArrayPattern(_) => {
                Params::Unsupported(self.other("array destructuring", param.span))
            }
            BindingPattern::AssignmentPattern(_) => {
                Params::Unsupported(self.other("default parameter value", param.span))
            }
        }
    }

    fn statement(&mut self, statement: &Statement<'_>, depth: usize) -> Option<Stmt> {
        let span = statement.span();
        Some(match statement {
            Statement::ReturnStatement(ret) => Stmt::Return {
                value: ret.argument.as_ref().map(|e| self.expr(e, depth)),
                loc: self.loc(span),
            },
            Statement::EmptyStatement(_) => return None,
            Statement::ExpressionStatement(statement) => Stmt::Other(
                self.other(hook_or(&statement.expression, "expression statement"), span),
            ),
            Statement::VariableDeclaration(decl) => {
                let kind = decl
                    .declarations
                    .iter()
                    .filter_map(|d| d.init.as_ref())
                    .map(|init| hook_or(init, ""))
                    .find(|kind| !kind.is_empty())
                    .unwrap_or_else(|| "local variable".to_owned());
                Stmt::Other(self.other(kind, span))
            }
            other => Stmt::Other(self.other(statement_kind(other), span)),
        })
    }

    /// Reports (once) that the nesting limit was reached.
    fn too_deep(&mut self, span: Span) {
        if !self.nesting_reported {
            self.nesting_reported = true;
            self.diagnostics.push(Diagnostic::error(
                Code::Limit,
                Some(self.loc(span)),
                format!("elements and functions are nested more than {MAX_NESTING} levels deep"),
            ));
        }
    }

    /// Converts an expression at nesting `depth`. Every construct that nests
    /// (functions, elements, fragments) adds one level, so the recursion and
    /// the tree it builds stay bounded by [`MAX_NESTING`].
    /// The parts of `list.map(callback)` or `list.slice(0, most).map(callback)`,
    /// `callback` one arrow or function: the list, `most`, the callback.
    fn map_call<'c, 'a>(
        &self,
        call: &'c js::CallExpression<'a>,
    ) -> Option<(
        &'c Expression<'a>,
        Option<&'c Expression<'a>>,
        FnParts<'c, 'a>,
    )> {
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return None;
        };
        if member.optional || call.optional || member.property.name.as_str() != "map" {
            return None;
        }
        let [argument] = call.arguments.as_slice() else {
            return None;
        };
        let callback = match argument.as_expression()? {
            Expression::ArrowFunctionExpression(arrow) => FnParts::arrow(arrow),
            Expression::FunctionExpression(function) => FnParts::function(function),
            _ => return None,
        };
        let mut list = &member.object;
        while let Expression::ParenthesizedExpression(inner) = list {
            list = &inner.expression;
        }
        if let Expression::CallExpression(slice) = list
            && let Expression::StaticMemberExpression(sliced) = &slice.callee
            && sliced.property.name.as_str() == "slice"
            && !sliced.optional
            && !slice.optional
            && let [from, most] = slice.arguments.as_slice()
            && let Some(Expression::NumericLiteral(from)) = from.as_expression()
            && from.value == 0.0
            && let Some(most) = most.as_expression()
        {
            return Some((&sliced.object, Some(most), callback));
        }
        Some((list, None, callback))
    }

    fn expr(&mut self, expression: &Expression<'_>, depth: usize) -> Expr {
        let mut expression = expression;
        while let Expression::ParenthesizedExpression(inner) = expression {
            expression = &inner.expression;
        }
        let span = expression.span();
        if depth >= MAX_NESTING {
            self.too_deep(span);
            return Expr::Other(self.other("expression nested too deeply", span));
        }
        let inner = depth.saturating_add(1);
        if let Expression::StringLiteral(s) = expression {
            self.check_string_escapes(s.span);
        }
        match expression {
            Expression::StringLiteral(s) if !s.lone_surrogates => Expr::Str {
                value: s.value.as_str().to_owned(),
                loc: self.loc(span),
            },
            Expression::NumericLiteral(n) => Expr::Num {
                value: n.value,
                raw: n.raw.map(|r| r.as_str().to_owned()).unwrap_or_default(),
                loc: self.loc(span),
            },
            Expression::Identifier(id) => Expr::Ident {
                name: id.name.as_str().to_owned(),
                loc: self.loc(span),
            },
            Expression::StaticMemberExpression(member) if !member.optional => {
                match &member.object {
                    Expression::Identifier(object) => Expr::Member {
                        object: object.name.as_str().to_owned(),
                        property: member.property.name.as_str().to_owned(),
                        loc: self.loc(span),
                    },
                    _ => Expr::Other(self.other("nested member expression", span)),
                }
            }
            Expression::ArrowFunctionExpression(arrow) => {
                Expr::Function(Box::new(self.function(&FnParts::arrow(arrow), inner)))
            }
            Expression::FunctionExpression(function) => {
                Expr::Function(Box::new(self.function(&FnParts::function(function), inner)))
            }
            Expression::JSXElement(element) => match self.element(element, depth) {
                Some(element) => Expr::Element(Box::new(element)),
                None => Expr::Other(self.other("expression nested too deeply", span)),
            },
            Expression::JSXFragment(fragment) => Expr::Fragment {
                children: self.children(&fragment.children, inner),
                loc: self.loc(span),
            },
            Expression::TemplateLiteral(template) => Expr::Template {
                pieces: template
                    .quasis
                    .iter()
                    .map(|quasi| TemplatePiece {
                        text: quasi
                            .value
                            .cooked
                            .as_ref()
                            .filter(|_| !quasi.lone_surrogates)
                            .map(|cooked| cooked.as_str().to_owned()),
                        loc: self.loc(quasi.span),
                    })
                    .collect(),
                exprs: template
                    .expressions
                    .iter()
                    .map(|expr| self.expr(expr, inner))
                    .collect(),
                loc: self.loc(span),
            },
            Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => {
                Expr::Not {
                    operand: Box::new(self.expr(&unary.argument, inner)),
                    loc: self.loc(span),
                }
            }
            Expression::BinaryExpression(binary)
                if binary.operator == js::BinaryOperator::StrictEquality =>
            {
                Expr::Equals {
                    left: Box::new(self.expr(&binary.left, inner)),
                    right: Box::new(self.expr(&binary.right, inner)),
                    loc: self.loc(span),
                }
            }
            Expression::UnaryExpression(unary)
                if unary.operator == UnaryOperator::UnaryNegation
                    && matches!(unary.argument, Expression::NumericLiteral(_)) =>
            {
                Expr::Other(self.other("negative number", span))
            }
            Expression::NullLiteral(_) => Expr::Null {
                loc: self.loc(span),
            },
            Expression::ArrayExpression(array) => Expr::Array {
                elements: array
                    .elements
                    .iter()
                    .map(|element| match element {
                        ArrayExpressionElement::SpreadElement(spread) => {
                            Expr::Other(self.other("spread element", spread.span))
                        }
                        ArrayExpressionElement::Elision(hole) => {
                            Expr::Other(self.other("hole in an array", hole.span))
                        }
                        other => match other.as_expression() {
                            Some(expression) => self.expr(expression, inner),
                            None => Expr::Other(self.other("array element", other.span())),
                        },
                    })
                    .collect(),
                loc: self.loc(span),
            },
            // An object literal: plain keys and their values; any
            // other property kept as an Other, reported where written.
            Expression::ObjectExpression(object) => Expr::Object {
                props: object
                    .properties
                    .iter()
                    .map(|property| match property {
                        js::ObjectPropertyKind::SpreadProperty(spread) => ObjectProp {
                            key: String::new(),
                            value: Expr::Other(self.other("spread property", spread.span)),
                            loc: self.loc(spread.span),
                        },
                        js::ObjectPropertyKind::ObjectProperty(property) => {
                            let loc = self.loc(property.span);
                            let plain = property.kind == js::PropertyKind::Init
                                && !property.method
                                && !property.shorthand
                                && !property.computed;
                            let key = match &property.key {
                                PropertyKey::StaticIdentifier(id) if plain => {
                                    Some(id.name.as_str().to_owned())
                                }
                                PropertyKey::StringLiteral(s) if plain => {
                                    self.check_string_escapes(s.span);
                                    Some(s.value.as_str().to_owned())
                                }
                                _ => None,
                            };
                            match key {
                                Some(key) => ObjectProp {
                                    key,
                                    value: self.expr(&property.value, inner),
                                    loc,
                                },
                                None => ObjectProp {
                                    key: String::new(),
                                    value: Expr::Other(self.other(
                                        "computed, shorthand or method property",
                                        property.span,
                                    )),
                                    loc,
                                },
                            }
                        }
                    })
                    .collect(),
                loc: self.loc(span),
            },
            Expression::CallExpression(call) if self.map_call(call).is_some() => {
                match self.map_call(call) {
                    Some((list, most, callback)) => Expr::Map {
                        list: Box::new(self.expr(list, inner)),
                        most: most.map(|most| Box::new(self.expr(most, inner))),
                        callback: Box::new(self.function(&callback, inner)),
                        loc: self.loc(span),
                    },
                    None => Expr::Other(self.other("call expression", span)),
                }
            }
            Expression::LogicalExpression(logical) => Expr::Logical {
                op: match logical.operator {
                    LogicalOperator::And => LogicalOp::And,
                    LogicalOperator::Or => LogicalOp::Or,
                    LogicalOperator::Coalesce => LogicalOp::Coalesce,
                },
                left: Box::new(self.expr(&logical.left, inner)),
                right: Box::new(self.expr(&logical.right, inner)),
                loc: self.loc(span),
            },
            Expression::ConditionalExpression(conditional) => Expr::Conditional {
                test: Box::new(self.expr(&conditional.test, inner)),
                consequent: Box::new(self.expr(&conditional.consequent, inner)),
                alternate: Box::new(self.expr(&conditional.alternate, inner)),
                loc: self.loc(span),
            },
            other => Expr::Other(self.other(expression_kind(other), span)),
        }
    }

    /// Converts an element at nesting `depth`; `None` past [`MAX_NESTING`].
    fn element(&mut self, element: &js::JSXElement<'_>, depth: usize) -> Option<Element> {
        if depth >= MAX_NESTING {
            self.too_deep(element.span);
            return None;
        }
        let depth = depth.saturating_add(1);
        let opening = &element.opening_element;
        let name = match &opening.name {
            JSXElementName::Identifier(id) => ElementName::Intrinsic(id.name.as_str().to_owned()),
            JSXElementName::IdentifierReference(id) => {
                ElementName::Component(id.name.as_str().to_owned())
            }
            JSXElementName::NamespacedName(n) => ElementName::Other(format!(
                "{}:{}",
                n.namespace.name.as_str(),
                n.name.name.as_str()
            )),
            JSXElementName::MemberExpression(m) => ElementName::Other(jsx_member_name(m)),
            JSXElementName::ThisExpression(_) => ElementName::Other("this".to_owned()),
        };
        let mut attrs = Vec::new();
        for item in &opening.attributes {
            match item {
                JSXAttributeItem::Attribute(attr) => {
                    let name = match &attr.name {
                        JSXAttributeName::Identifier(id) => id.name.as_str().to_owned(),
                        JSXAttributeName::NamespacedName(n) => {
                            format!("{}:{}", n.namespace.name.as_str(), n.name.name.as_str())
                        }
                    };
                    let value = self.attr_value(attr.value.as_ref(), depth);
                    attrs.push(Attr::Named {
                        name,
                        value,
                        loc: self.loc(attr.span),
                    });
                }
                JSXAttributeItem::SpreadAttribute(spread) => attrs.push(Attr::Spread {
                    loc: self.loc(spread.span),
                }),
            }
        }
        Some(Element {
            name,
            attrs,
            children: self.children(&element.children, depth),
            loc: self.loc(element.span),
        })
    }

    fn attr_value(&mut self, value: Option<&JSXAttributeValue<'_>>, depth: usize) -> AttrValue {
        match value {
            None => AttrValue::Absent,
            Some(JSXAttributeValue::StringLiteral(s)) => {
                let raw = s.value.as_str();
                // The value starts one byte after the opening quote.
                let start = offset(s.span).saturating_add(1);
                let loc = self.loc(s.span);
                match self.decoded(raw, start, jsx_text::attribute_string(raw)) {
                    Some(value) => AttrValue::Str { value, loc },
                    None => AttrValue::InvalidStr { loc },
                }
            }
            Some(JSXAttributeValue::ExpressionContainer(container)) => {
                match &container.expression {
                    JSXExpression::EmptyExpression(_) => AttrValue::Empty {
                        loc: self.loc(container.span),
                    },
                    expression => match expression.as_expression() {
                        Some(e) => AttrValue::Expr(self.expr(e, depth)),
                        None => AttrValue::Empty {
                            loc: self.loc(container.span),
                        },
                    },
                }
            }
            Some(JSXAttributeValue::Element(element)) => match self.element(element, depth) {
                Some(element) => AttrValue::Expr(Expr::Element(Box::new(element))),
                None => AttrValue::Expr(Expr::Other(
                    self.other("expression nested too deeply", element.span),
                )),
            },
            Some(JSXAttributeValue::Fragment(fragment)) => AttrValue::Expr(Expr::Fragment {
                children: self.children(&fragment.children, depth),
                loc: self.loc(fragment.span),
            }),
        }
    }

    fn children(&mut self, children: &[JSXChild<'_>], depth: usize) -> Vec<Child> {
        let mut out = Vec::new();
        for child in children {
            match child {
                JSXChild::Text(text) => {
                    let raw = text.value.as_str();
                    let start = offset(text.span);
                    let visible = raw
                        .find(|c: char| !matches!(c, ' ' | '\t' | '\n' | '\r'))
                        .unwrap_or(0);
                    let loc = self.at(start.saturating_add(visible));
                    match self.decoded(raw, start, jsx_text::jsx_text(raw)) {
                        Some(Some(value)) => out.push(Child::Text { value, loc }),
                        Some(None) => {}
                        // Kept so the analyser can still check where text is
                        // allowed; the error already stops conversion.
                        None => out.push(Child::InvalidText { loc }),
                    }
                }
                JSXChild::Element(element) => match self.element(element, depth) {
                    Some(element) => out.push(Child::Element(element)),
                    None => out.push(Child::Expr(Expr::Other(
                        self.other("expression nested too deeply", element.span),
                    ))),
                },
                JSXChild::Fragment(fragment) => {
                    let children = if depth >= MAX_NESTING {
                        self.too_deep(fragment.span);
                        Vec::new()
                    } else {
                        self.children(&fragment.children, depth.saturating_add(1))
                    };
                    out.push(Child::Fragment {
                        children,
                        loc: self.loc(fragment.span),
                    });
                }
                JSXChild::ExpressionContainer(container) => match &container.expression {
                    JSXExpression::EmptyExpression(_) => out.push(Child::Empty {
                        loc: self.loc(container.span),
                    }),
                    expression => match expression.as_expression() {
                        Some(e) => out.push(Child::Expr(self.expr(e, depth))),
                        None => out.push(Child::Empty {
                            loc: self.loc(container.span),
                        }),
                    },
                },
                JSXChild::Spread(spread) => out.push(Child::Spread {
                    loc: self.loc(spread.span),
                }),
            }
        }
        out
    }

    /// The name in an import or export specifier.
    fn export_name(&mut self, name: &ModuleExportName<'_>) -> String {
        match name {
            ModuleExportName::IdentifierName(n) => n.name.as_str().to_owned(),
            ModuleExportName::IdentifierReference(n) => n.name.as_str().to_owned(),
            ModuleExportName::StringLiteral(s) => {
                self.check_string_escapes(s.span);
                s.value.as_str().to_owned()
            }
        }
    }

    /// Reports a legacy octal escape (`\01`, `\7`, `\08`) or `\8`/`\9` in
    /// the string literal at `span`. They are syntax errors in strict mode
    /// code, which every module is, but the parser accepts them.
    fn check_string_escapes(&mut self, span: Span) {
        let start = offset(span);
        let end = usize::try_from(span.end).unwrap_or(usize::MAX);
        let Some((at, escape)) = self.source.get(start..end).and_then(legacy_escape) else {
            return;
        };
        self.diagnostics.push(
            Diagnostic::error(
                Code::Syntax,
                Some(self.at(start.saturating_add(at))),
                format!("the escape `{escape}` is not allowed in modules (strict mode code)"),
            )
            .with_help("write the character itself, or a \\x or \\u escape such as \\u0008"),
        );
    }

    /// Records a text error (at its exact position) and returns `None`, or
    /// passes the decoded value through.
    fn decoded<T>(&mut self, raw: &str, start: usize, result: Result<T, TextError>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                let at = start.saturating_add(error.offset().min(raw.len()));
                self.diagnostics.push(
                    Diagnostic::error(Code::Unsupported, Some(self.at(at)), error.to_string())
                        .with_help(error.help()),
                );
                None
            }
        }
    }
}

/// The parts of a function declaration, function expression or arrow
/// function that the converter needs.
struct FnParts<'b, 'a> {
    params: &'b FormalParameters<'a>,
    body: Option<&'b FunctionBody<'a>>,
    /// An arrow function whose body is a single expression.
    expression_body: bool,
    is_async: bool,
    is_generator: bool,
    span: Span,
}

impl<'b, 'a> FnParts<'b, 'a> {
    fn function(function: &'b js::Function<'a>) -> Self {
        Self {
            params: &function.params,
            body: function.body.as_deref(),
            expression_body: false,
            is_async: function.r#async,
            is_generator: function.generator,
            span: function.span,
        }
    }

    fn arrow(arrow: &'b js::ArrowFunctionExpression<'a>) -> Self {
        Self {
            params: &arrow.params,
            body: Some(&arrow.body),
            expression_body: arrow.expression,
            is_async: arrow.r#async,
            is_generator: false,
            span: arrow.span,
        }
    }
}

fn offset(span: Span) -> usize {
    usize::try_from(span.start).unwrap_or(usize::MAX)
}

/// The byte offset and text of the first legacy octal escape (`\01`, `\7`,
/// `\08`) or `\8`/`\9` in the source text of a JavaScript string literal.
/// `\0` not followed by a digit is the NUL escape, which strict mode allows.
fn legacy_escape(raw: &str) -> Option<(usize, &str)> {
    let bytes = raw.as_bytes();
    let mut i = 0_usize;
    while let Some(&byte) = bytes.get(i) {
        if byte != b'\\' {
            i = i.saturating_add(1);
            continue;
        }
        let after = bytes.get(i.saturating_add(1)..).unwrap_or(&[]);
        let digits = after
            .iter()
            .take(3)
            .take_while(|d| d.is_ascii_digit())
            .count();
        let nul = after.first() == Some(&b'0') && digits == 1;
        if digits > 0 && !nul {
            let end = i.saturating_add(1).saturating_add(digits);
            return Some((i, raw.get(i..end).unwrap_or("\\")));
        }
        // Skip the backslash and the byte it escapes. A UTF-8 continuation
        // byte is never a backslash, so a multi-byte escaped character is safe
        // to step into.
        i = i.saturating_add(2);
    }
    None
}

fn jsx_member_name(member: &js::JSXMemberExpression<'_>) -> String {
    let object = match &member.object {
        JSXMemberExpressionObject::IdentifierReference(id) => id.name.as_str().to_owned(),
        JSXMemberExpressionObject::MemberExpression(_) => "…".to_owned(),
        JSXMemberExpressionObject::ThisExpression(_) => "this".to_owned(),
    };
    format!("{object}.{}", member.property.name.as_str())
}

/// "React hook `useX`" when the expression calls a `use…` function, else
/// `fallback`.
fn hook_or(expression: &Expression<'_>, fallback: &str) -> String {
    if let Expression::CallExpression(call) = expression
        && let Expression::Identifier(callee) = &call.callee
    {
        let name = callee.name.as_str();
        if name.starts_with("use") && name.chars().nth(3).is_some_and(char::is_uppercase) {
            return format!("React hook `{name}`");
        }
    }
    fallback.to_owned()
}

fn statement_kind(statement: &Statement<'_>) -> &'static str {
    match statement {
        Statement::BlockStatement(_) => "block statement",
        Statement::BreakStatement(_) => "`break` statement",
        Statement::ContinueStatement(_) => "`continue` statement",
        Statement::DebuggerStatement(_) => "`debugger` statement",
        Statement::DoWhileStatement(_) | Statement::WhileStatement(_) => "loop",
        Statement::ForInStatement(_)
        | Statement::ForOfStatement(_)
        | Statement::ForStatement(_) => "loop",
        Statement::ExpressionStatement(_) => "expression statement",
        Statement::IfStatement(_) => "`if` statement",
        Statement::LabeledStatement(_) => "labelled statement",
        Statement::ReturnStatement(_) => "`return` statement",
        Statement::SwitchStatement(_) => "`switch` statement",
        Statement::ThrowStatement(_) => "`throw` statement",
        Statement::TryStatement(_) => "`try` statement",
        Statement::WithStatement(_) => "`with` statement",
        Statement::FunctionDeclaration(_) => "nested function",
        Statement::ClassDeclaration(_) => "class declaration",
        Statement::VariableDeclaration(_) => "variable declaration",
        Statement::TSEnumDeclaration(_) => "enum declaration",
        Statement::TSModuleDeclaration(_) | Statement::TSGlobalDeclaration(_) => {
            "namespace or module declaration"
        }
        Statement::TSExportAssignment(_) => "`export =`",
        Statement::TSNamespaceExportDeclaration(_) => "`export as namespace`",
        _ => "statement",
    }
}

fn expression_kind(expression: &Expression<'_>) -> String {
    let kind = match expression {
        Expression::BooleanLiteral(_) => "boolean",
        Expression::NullLiteral(_) => "`null`",
        Expression::BigIntLiteral(_) => "BigInt literal",
        Expression::RegExpLiteral(_) => "regular expression",
        Expression::StringLiteral(_) => "string with unpaired surrogates",
        Expression::TemplateLiteral(_) => "template literal",
        Expression::ArrayExpression(_) => "array literal",
        Expression::AssignmentExpression(_) => "assignment",
        Expression::AwaitExpression(_) => "`await` expression",
        Expression::BinaryExpression(_) => "binary expression",
        Expression::CallExpression(call) => {
            return match &call.callee {
                Expression::Identifier(callee) => format!("call to `{}`", callee.name.as_str()),
                Expression::StaticMemberExpression(member) => {
                    format!("call to `.{}()`", member.property.name.as_str())
                }
                _ => "call expression".to_owned(),
            };
        }
        Expression::ChainExpression(_) => "optional chaining",
        Expression::ClassExpression(_) => "class expression",
        Expression::ConditionalExpression(_) => "conditional (`?:`) expression",
        Expression::ImportExpression(_) => "dynamic import",
        Expression::LogicalExpression(_) => "logical (`&&`, `||`, `??`) expression",
        Expression::NewExpression(_) => "`new` expression",
        Expression::ObjectExpression(_) => "object literal",
        Expression::SequenceExpression(_) => "comma expression",
        Expression::TaggedTemplateExpression(_) => "tagged template",
        Expression::ThisExpression(_) => "`this`",
        Expression::UnaryExpression(_) => "unary expression",
        Expression::UpdateExpression(_) => "`++`/`--` expression",
        Expression::YieldExpression(_) => "`yield` expression",
        Expression::TSAsExpression(_) => "`as` type assertion",
        Expression::TSSatisfiesExpression(_) => "`satisfies` expression",
        Expression::TSTypeAssertion(_) => "type assertion",
        Expression::TSNonNullExpression(_) => "non-null assertion (`!`)",
        Expression::TSInstantiationExpression(_) => "generic instantiation",
        Expression::ComputedMemberExpression(_) => "computed member access (`a[b]`)",
        Expression::StaticMemberExpression(_) => "optional member access",
        Expression::PrivateFieldExpression(_) => "private field access",
        _ => "expression",
    };
    kind.to_owned()
}
