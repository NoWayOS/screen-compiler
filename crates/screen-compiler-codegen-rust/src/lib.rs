// SPDX-License-Identifier: MIT
//! Rust code generation for Screen Compiler: UI IR to Rust source for NoWayOS's UI
//! runtime.
//!
//! [`generate`] turns a validated [`Module`] into one Rust source file,
//! written against the target API [`contract`]. The output is deterministic:
//! the same IR always gives byte-for-byte the same text, with no timestamps,
//! absolute paths or tool versions in it, and it is pure printable ASCII.
//!
//! Generated code is `no_std`-friendly: no allocation, no `unsafe`, no
//! panics. Every constructor call propagates the builder's error with `?`.
//!

#![deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

pub mod contract;
mod rust;

pub use rust::string_literal;
use screen_compiler_ir::{
    AnimationName, ButtonVariant, Code, Diagnostic, InputMode, Length, Loc, Look, Module, Name,
    Node, NodeKind, Screen, Template, TextAlign, TextStyle, has_errors, sort_diagnostics, validate,
};

/// Module names the generated file uses itself. The crate `API_MODULE`
/// starts with (`nowayos_ui`) is reserved too: a screen module of that name
/// would shadow it in the file's `use` line.
const RESERVED_MODULES: [&str; 2] = ["tokens", "ui"];

/// Whether a screen module may not have this name.
fn is_reserved_module(name: &str) -> bool {
    RESERVED_MODULES.contains(&name)
        || contract::API_MODULE.split("::").next() == Some(name)
        || rust::is_keyword(name)
}

/// The field of `Nodes` that holds the root node.
const ROOT_FIELD: &str = "root";

/// Generates Rust source for `module`.
///
/// # Errors
///
/// Returns diagnostics (sorted by position) when the IR is invalid (see
/// [`screen_compiler_ir::validate`]), when a name would be a Rust keyword or clash
/// with a name the generated code reserves, or when any identifier it would
/// emit is not a valid Rust identifier.
pub fn generate(module: &Module) -> Result<String, Vec<Diagnostic>> {
    let mut diagnostics = validate(module);
    diagnostics.extend(check_rust_names(module));
    if has_errors(&diagnostics) {
        sort_diagnostics(&mut diagnostics);
        return Err(diagnostics);
    }
    Ok(Emitter::new(module).module())
}

fn check_rust_names(module: &Module) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    // Defence in depth: names are validated when they enter the IR, so these
    // never fire unless that validation has a bug. Keywords are reported
    // below, as reserved names.
    let mut identifier = |ident: String, what: String, loc: Option<Loc>| {
        if !rust::is_identifier(&ident) && !rust::is_keyword(&ident) {
            diagnostics.push(Diagnostic::error(
                Code::Internal,
                loc,
                format!("{what} would become `{ident}`, which is not a Rust identifier; please report this input"),
            ));
        }
    };
    for token in &module.tokens {
        identifier(
            token.name.screaming_snake_case(),
            format!("design token `{}`", token.name),
            token.loc,
        );
    }
    for screen in &module.screens {
        identifier(
            screen.name.snake_case(),
            format!("screen `{}`", screen.name),
            screen.loc,
        );
        for action in screen.actions() {
            identifier(
                action.screaming_snake_case(),
                format!("callback prop `{action}`"),
                screen.loc,
            );
        }
        for name in screen.groups().into_iter().chain(screen.slots()) {
            identifier(
                name.screaming_snake_case(),
                format!("prop `{name}`"),
                screen.loc,
            );
        }
        for (switch, cases) in screen.switches() {
            identifier(
                switch.screaming_snake_case(),
                format!("prop `{switch}`"),
                screen.loc,
            );
            identifier(switch.snake_case(), format!("prop `{switch}`"), screen.loc);
            for case in cases {
                identifier(
                    case.screaming_snake_case(),
                    format!("case \"{case}\" of `{switch}`"),
                    screen.loc,
                );
            }
        }
        for node in screen.nodes() {
            if let Some(id) = &node.id {
                identifier(id.snake_case(), format!("id \"{id}\""), node.loc);
            }
        }
    }
    for screen in &module.screens {
        let snake = screen.name.snake_case();
        if is_reserved_module(&snake) {
            diagnostics.push(
                Diagnostic::error(
                    Code::ReservedName,
                    screen.loc,
                    format!(
                        "screen `{}` would become module `{snake}`, which is reserved in generated Rust",
                        screen.name
                    ),
                )
                .with_help("rename the component"),
            );
        }
        // Each switch's cases are a module beside `use super::ui;`.
        for (switch, _) in screen.switches() {
            let cases = switch.snake_case();
            if cases == "ui" || rust::is_keyword(&cases) {
                let at = screen
                    .nodes()
                    .into_iter()
                    .find(|node| {
                        matches!(&node.kind, NodeKind::Case(case) if case.switch.same_words(switch))
                    })
                    .and_then(|node| node.loc);
                diagnostics.push(
                    Diagnostic::error(
                        Code::ReservedName,
                        at,
                        format!(
                            "prop `{switch}` would name module `{cases}` of its cases, which is reserved in generated Rust"
                        ),
                    )
                    .with_help("rename the prop"),
                );
            }
        }
        for node in screen.nodes() {
            let Some(id) = &node.id else { continue };
            let field = id.snake_case();
            if field == ROOT_FIELD || rust::is_keyword(&field) {
                diagnostics.push(
                    Diagnostic::error(
                        Code::ReservedName,
                        node.loc,
                        format!(
                            "id \"{id}\" would become `{field}`, which is reserved in generated Rust"
                        ),
                    )
                    .with_help("choose another id"),
                );
            }
        }
    }
    diagnostics
}

/// Writes the source text.
struct Emitter<'m> {
    module: &'m Module,
    source: String,
    out: String,
}

/// A statement of a screen's `build` body, and the handles with ids.
#[derive(Default)]
struct Body {
    lines: Vec<String>,
    /// `(field, variable, id as written, location)` in document order.
    handles: Vec<(String, String, String, Option<Loc>)>,
    next: usize,
}

impl<'m> Emitter<'m> {
    fn new(module: &'m Module) -> Self {
        Self {
            module,
            source: rust::file_label(&module.source_name),
            out: String::new(),
        }
    }

    fn line(&mut self, indent: usize, text: &str) {
        for _ in 0..indent {
            self.out.push_str("    ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn blank(&mut self) {
        self.out.push('\n');
    }

    /// ` (file.tsx:3:14)` for a location, or nothing.
    fn at(&self, loc: Option<Loc>) -> String {
        loc.map(|l| format!(" ({}:{l})", self.file(l)))
            .unwrap_or_default()
    }

    /// The name of the file `loc` is in, as comments show it.
    fn file(&self, loc: Loc) -> String {
        if loc.file == 0 {
            self.source.clone()
        } else {
            rust::file_label(self.module.file_name(loc.file))
        }
    }

    fn module(mut self) -> String {
        let source = self.source.clone();
        self.line(
            0,
            &format!("// @generated by screen-compiler from {source}. Do not edit by hand."),
        );
        self.line(
            0,
            &format!(
                "// Target: nowayos-ui builder contract v{} (provisional). Regenerate with `screen-compiler convert`.",
                contract::VERSION
            ),
        );
        if !self.module.screens.is_empty() {
            self.blank();
            self.line(0, &format!("use {} as ui;", contract::API_MODULE));
        }
        if !self.module.tokens.is_empty() {
            self.blank();
            self.tokens();
        }
        for screen in &self.module.screens {
            self.blank();
            self.screen(screen);
        }
        self.out
    }

    fn tokens(&mut self) {
        let source = self.source.clone();
        let doc = if self.module.imported.is_empty() {
            format!("/// Design tokens declared in `{source}`, in logical pixels.")
        } else {
            format!(
                "/// Design tokens declared in `{source}` and the files it imports, in logical pixels."
            )
        };
        self.line(0, &doc);
        self.line(0, "pub mod tokens {");
        for token in &self.module.tokens {
            let doc = format!("/// `{}`{}.", token.name, self.at(token.loc));
            self.line(1, &doc);
            self.line(
                1,
                &format!(
                    "pub const {}: u16 = {};",
                    token.name.screaming_snake_case(),
                    token.value
                ),
            );
        }
        self.line(0, "}");
    }

    fn screen(&mut self, screen: &Screen) {
        let doc = format!("/// Screen `{}`{}.", screen.name, self.at(screen.loc));
        self.line(0, &doc);
        self.line(0, &format!("pub mod {} {{", screen.name.snake_case()));
        self.line(1, "use super::ui;");

        let actions = screen.actions();
        if !actions.is_empty() {
            self.blank();
            self.actions(&actions);
        }
        let fields = screen.fields();
        if !fields.is_empty() {
            self.blank();
            self.fields(screen, &fields);
        }
        let groups = screen.groups();
        if !groups.is_empty() {
            self.blank();
            self.numbered(
                screen,
                &groups,
                "groups",
                "GroupId",
                "Groups of content the application shows or hides, one per prop, numbered from 1 in document order.",
            );
        }
        let switches = screen.switches();
        if !switches.is_empty() {
            self.blank();
            self.switches(screen, &switches);
        }
        let layers = screen.layers();
        if !layers.is_empty() {
            self.blank();
            self.numbered(
                screen,
                &layers,
                "layers",
                "LayerId",
                "Layers over the screen, one per prop that opens one, numbered from 1 in document order: the application opens and closes each.",
            );
        }
        let lists = screen.lists();
        if !lists.is_empty() {
            self.blank();
            self.numbered(
                screen,
                &lists,
                "lists",
                "ListId",
                "Virtual lists, one per prop, numbered from 1 in document order: the application sets each one's count.",
            );
        }
        let slots = screen.slots();
        if !slots.is_empty() {
            self.blank();
            self.numbered(
                screen,
                &slots,
                "slots",
                "SlotId",
                "Slots for text the application sets, one per prop, numbered from 1 in document order.",
            );
        }

        let mut body = Body::default();
        let root = self.node(&screen.root, &mut body);

        self.blank();
        self.line(
            1,
            "/// Handles to the screen's root node and to each element with an `id`.",
        );
        self.line(1, "#[derive(Clone, Copy, Debug, PartialEq, Eq)]");
        self.line(1, "pub struct Nodes<N> {");
        self.line(2, "/// The root node: install it as the screen's root.");
        self.line(2, &format!("pub {ROOT_FIELD}: N,"));
        for (field, _, id, loc) in &body.handles {
            let doc = format!("/// `id=\"{id}\"`{}.", self.at(*loc));
            self.line(2, &doc);
            self.line(2, &format!("pub {field}: N,"));
        }
        self.line(1, "}");

        self.blank();
        self.line(
            1,
            "/// Builds the screen with `b`, children before their parents.",
        );
        self.line(1, "///");
        self.line(1, "/// # Errors");
        self.line(1, "///");
        self.line(
            1,
            "/// Returns the builder's error, for example when its node arena is full.",
        );
        self.line(
            1,
            "pub fn build<B: ui::UiBuilder>(b: &mut B) -> Result<Nodes<B::Node>, B::Error> {",
        );
        for line in &body.lines {
            self.line(2, line);
        }
        let mut fields = vec![format!("{ROOT_FIELD}: {root}")];
        fields.extend(body.handles.iter().map(|(field, var, _, _)| {
            if field == var {
                field.clone()
            } else {
                format!("{field}: {var}")
            }
        }));
        self.line(2, &format!("Ok(Nodes {{ {} }})", fields.join(", ")));
        self.line(1, "}");
        self.line(0, "}");
    }

    fn actions(&mut self, actions: &[&Name]) {
        self.line(
            1,
            "/// Actions the screen emits, numbered from 1 in document order.",
        );
        self.line(1, "pub mod actions {");
        self.line(2, "use super::ui;");
        self.blank();
        for (number, action) in (1_u32..).zip(actions) {
            self.line(2, &format!("/// Callback prop `{}`.", action.source()));
            self.line(
                2,
                &format!(
                    "pub const {}: ui::ActionId = ui::ActionId::new({number});",
                    action.screaming_snake_case()
                ),
            );
        }
        self.line(1, "}");
    }

    /// A screen's `fields` module: each value-holding control's field
    /// number, named after its id, from 1 in document order.
    fn fields(&mut self, screen: &Screen, fields: &[&Name]) {
        self.line(
            1,
            "/// Fields of the controls that hold a value, named after their ids, numbered from 1 in document order.",
        );
        self.line(1, "pub mod fields {");
        self.line(2, "use super::ui;");
        self.blank();
        for (number, id) in (1_u32..).zip(fields) {
            let node = screen
                .nodes()
                .into_iter()
                .find(|node| node.kind.is_field() && node.id.as_ref() == Some(*id));
            let what = node.map(|node| node.component()).unwrap_or_default();
            let at = self.at(node.and_then(|node| node.loc));
            self.line(2, &format!("/// `{what}` `id=\"{}\"`{at}.", id.source()));
            self.line(
                2,
                &format!(
                    "pub const {}: ui::FieldId = ui::FieldId::new({number});",
                    id.screaming_snake_case()
                ),
            );
        }
        self.line(1, "}");
    }

    /// The `switches` module: each switch a constant named after its prop,
    /// numbered from 1, and beside it a module of its cases, each a `u8`
    /// named after its value, numbered from 0 (contract v13).
    fn switches(&mut self, screen: &Screen, switches: &[(&Name, Vec<&Name>)]) {
        self.line(
            1,
            "/// Switches the application sets to one case, one per prop, numbered from 1 in document order; each switch's cases, numbered from 0, in the module of its name.",
        );
        self.line(1, "pub mod switches {");
        self.line(2, "use super::ui;");
        for (number, (switch, cases)) in (1_u32..).zip(switches) {
            let first = screen.nodes().into_iter().find(
                |node| matches!(&node.kind, NodeKind::Case(case) if case.switch.same_words(switch)),
            );
            let at = self.at(first.and_then(|node| node.loc));
            self.blank();
            self.line(2, &format!("/// Prop `{}`{at}.", switch.source()));
            self.line(
                2,
                &format!(
                    "pub const {}: ui::SwitchId = ui::SwitchId::new({number});",
                    switch.screaming_snake_case()
                ),
            );
            self.line(2, &format!("/// The cases of `{}`.", switch.source()));
            self.line(2, &format!("pub mod {} {{", switch.snake_case()));
            for (when, case) in (0_u32..).zip(cases) {
                self.line(3, &format!("/// `\"{}\"`.", case.source()));
                self.line(
                    3,
                    &format!("pub const {}: u8 = {when};", case.screaming_snake_case()),
                );
            }
            self.line(2, "}");
        }
        self.line(1, "}");
    }

    /// A module of numbered constants named after props: a screen's groups
    /// or slots. Each constant's documentation names the prop and where it
    /// is first used.
    fn numbered(&mut self, screen: &Screen, names: &[&Name], module: &str, ty: &str, doc: &str) {
        self.line(1, &format!("/// {doc}"));
        self.line(1, &format!("pub mod {module} {{"));
        self.line(2, "use super::ui;");
        self.blank();
        for (number, name) in (1_u32..).zip(names) {
            let first = screen.nodes().into_iter().find(|node| match &node.kind {
                NodeKind::Group(group) => group.name.same_words(name),
                NodeKind::Slot { slot, .. } | NodeKind::Progress { slot } => slot.same_words(name),
                _ => false,
            });
            let at = self.at(first.and_then(|node| node.loc));
            self.line(2, &format!("/// Prop `{}`{at}.", name.source()));
            self.line(
                2,
                &format!(
                    "pub const {}: ui::{ty} = ui::{ty}::new({number});",
                    name.screaming_snake_case()
                ),
            );
        }
        self.line(1, "}");
    }

    /// Emits `node` and its subtree children first; returns its variable.
    /// Variables are numbered in document order (the root is `n0`).
    fn node(&self, node: &Node, body: &mut Body) -> String {
        let var = format!("n{}", body.next);
        body.next = body.next.saturating_add(1);
        if let Some(id) = &node.id {
            body.handles.push((
                id.snake_case(),
                var.clone(),
                id.source().to_owned(),
                node.loc,
            ));
        }
        let call = match &node.kind {
            NodeKind::Column(container) | NodeKind::Row(container) => {
                let method = if matches!(node.kind, NodeKind::Column(_)) {
                    "column"
                } else {
                    "row"
                };
                let mut layout = "ui::Layout::new()".to_owned();
                // A theme's spacing token stands in place of a number
                // (contract v7).
                match &container.gap {
                    Some(Length::Space(space)) => {
                        layout.push_str(&format!(".gap_space(ui::Space::{})", space.rust()));
                    }
                    Some(gap) => layout.push_str(&format!(".gap({})", length(gap))),
                    None => {}
                }
                match &container.padding {
                    Some(Length::Space(space)) => {
                        layout.push_str(&format!(".padding_space(ui::Space::{})", space.rust()));
                    }
                    Some(padding) => layout.push_str(&format!(".padding({})", length(padding))),
                    None => {}
                }
                // What the container's style declares (contract v8); a
                // container that scrolls gives its size to its scroll view
                // instead (contract v17).
                let mut sized = "ui::Layout::new()".to_owned();
                for (method, size) in [
                    ("width", &container.width),
                    ("height", &container.height),
                    ("min_width", &container.min_width),
                    ("max_width", &container.max_width),
                ] {
                    if let Some(size) = size {
                        let call = format!(".{method}({})", length(size));
                        if container.overflow.is_some() {
                            sized.push_str(&call);
                        } else {
                            layout.push_str(&call);
                        }
                    }
                }
                if let Some(align) = container.align {
                    layout.push_str(&format!(".align(ui::Align::{})", align.rust()));
                }
                if let Some(justify) = container.justify {
                    layout.push_str(&format!(".justify(ui::Justify::{})", justify.rust()));
                }
                if !container.look.is_empty() {
                    layout.push_str(&format!(".look({})", look(container.look)));
                }
                let children: Vec<String> = container
                    .children
                    .iter()
                    .map(|child| self.node(child, body))
                    .collect();
                let call = match container.surface {
                    // On a surface (contract v3).
                    Some(surface) => format!(
                        "b.{method}_on({}, {layout}, &[{}])",
                        surface.rust(),
                        children.join(", ")
                    ),
                    None => format!("b.{method}({layout}, &[{}])", children.join(", ")),
                };
                match container.overflow {
                    // A scroll view of its size holding it (contract v17).
                    Some(axes) => format!(
                        "{{ let inner = {call}?; b.scroll_view_axes({sized}, ui::ScrollAxes::{}, inner) }}",
                        axes.rust()
                    ),
                    None => call,
                }
            }
            NodeKind::Text { text, style } if *style == TextStyle::BODY => {
                format!("b.text({})", rust::string_literal(text))
            }
            NodeKind::Text { text, style } => format!(
                "b.text_styled({}, {})",
                rust::string_literal(text),
                text_style(*style)
            ),
            // A rich text (contract v21): its runs a constant, so they
            // live as long as the program.
            NodeKind::Rich { runs, style } => {
                let runs: Vec<String> = runs
                    .iter()
                    .map(|run| {
                        let mut out = format!("ui::Run::new({})", rust::string_literal(&run.text));
                        for (set, method) in [
                            (run.bold, ".bold()"),
                            (run.italic, ".italic()"),
                            (run.code, ".code()"),
                        ] {
                            if set {
                                out.push_str(method);
                            }
                        }
                        if let Some(colour) = run.colour {
                            out.push_str(&format!(".colour(ui::Colour::{})", colour.rust()));
                        }
                        if let Some(link) = &run.link {
                            out.push_str(&format!(
                                ".link(actions::{})",
                                link.screaming_snake_case()
                            ));
                        }
                        out
                    })
                    .collect();
                format!(
                    "b.rich_text({{ const RUNS: &[ui::Run] = &[{}]; RUNS }}, {})",
                    runs.join(", "),
                    text_style(*style)
                )
            }
            // A button with a look of its own (contract v8).
            NodeKind::Button {
                label,
                action,
                variant,
                look: style,
            } if !style.is_empty() => format!(
                "b.button_look({}, actions::{}, ui::ButtonVariant::{}, {})",
                rust::string_literal(label),
                action.screaming_snake_case(),
                variant.rust(),
                look(*style)
            ),
            NodeKind::Button {
                label,
                action,
                variant: ButtonVariant::Primary,
                ..
            } => format!(
                "b.button({}, actions::{})",
                rust::string_literal(label),
                action.screaming_snake_case()
            ),
            // A secondary or quiet button (contract v3).
            NodeKind::Button {
                label,
                action,
                variant,
                ..
            } => format!(
                "b.button_with({}, actions::{}, ui::ButtonVariant::{})",
                rust::string_literal(label),
                action.screaming_snake_case(),
                variant.rust()
            ),
            NodeKind::TextInput {
                placeholder,
                max_len,
                mode,
            } => {
                let mut input = "ui::Input::new()".to_owned();
                if let Some(p) = placeholder {
                    input.push_str(&format!(".placeholder({})", rust::string_literal(p)));
                }
                if let Some(max_len) = max_len {
                    input.push_str(&format!(".max_len({})", length(max_len)));
                }
                match mode {
                    InputMode::Text => {}
                    InputMode::Digits => input.push_str(".mode(ui::InputMode::Digits)"),
                    InputMode::Email => input.push_str(".mode(ui::InputMode::Email)"),
                }
                format!("b.input({}, {input})", field(node))
            }
            // A text area (contract v19).
            NodeKind::TextArea {
                placeholder,
                rows,
                max_len,
            } => format!(
                "b.text_area({}, {}, {}, {})",
                field(node),
                rows.unwrap_or(2),
                length(max_len),
                rust::string_literal(placeholder.as_deref().unwrap_or(""))
            ),
            NodeKind::Checkbox { label } => format!(
                "b.checkbox({}, {})",
                field(node),
                rust::string_literal(label)
            ),
            NodeKind::RadioGroup { options } => {
                let options: Vec<String> =
                    options.iter().map(|o| rust::string_literal(o)).collect();
                format!("b.radio_group({}, &[{}])", field(node), options.join(", "))
            }
            // A list picker (contract v10).
            NodeKind::Select {
                options,
                placeholder,
                rows,
            } => {
                let options: Vec<String> =
                    options.iter().map(|o| rust::string_literal(o)).collect();
                let mut picker = "ui::Picker::new()".to_owned();
                if let Some(placeholder) = placeholder {
                    picker.push_str(&format!(
                        ".placeholder({})",
                        rust::string_literal(placeholder)
                    ));
                }
                if let Some(rows) = rows {
                    picker.push_str(&format!(".rows({rows})"));
                }
                format!(
                    "b.list_picker({}, &[{}], {picker})",
                    field(node),
                    options.join(", ")
                )
            }
            // The theme's spacing token (contract v7).
            NodeKind::Spacer {
                size: Some(Length::Space(space)),
            } => format!("b.spacer_space(ui::Space::{})", space.rust()),
            NodeKind::Spacer { size } => format!(
                "b.spacer({})",
                size.as_ref()
                    .map_or_else(|| "None".to_owned(), |s| format!("Some({})", length(s)))
            ),
            // Between literal text compiled in (contract v14).
            NodeKind::Slot {
                slot,
                style,
                template: Some(template),
                capacity: None,
            } if *style == TextStyle::BODY => format!(
                "b.slot_template(slots::{}, {})",
                slot.screaming_snake_case(),
                template_literal(template)
            ),
            NodeKind::Slot {
                slot,
                style,
                template: Some(template),
                capacity: None,
            } => format!(
                "b.slot_template_styled(slots::{}, {}, {})",
                slot.screaming_snake_case(),
                template_literal(template),
                text_style(*style)
            ),
            // Paragraphs of any length, in the tree's text store (contract
            // v16).
            NodeKind::Slot {
                slot,
                style,
                capacity: Some(capacity),
                ..
            } => format!(
                "b.long_text(slots::{}, {}, {})",
                slot.screaming_snake_case(),
                length(capacity),
                text_style(*style)
            ),
            NodeKind::Slot { slot, style, .. } if *style == TextStyle::BODY => {
                format!("b.slot(slots::{})", slot.screaming_snake_case())
            }
            NodeKind::Slot { slot, style, .. } => format!(
                "b.slot_styled(slots::{}, {})",
                slot.screaming_snake_case(),
                text_style(*style)
            ),
            // An icon, by its static, so only the icons named are linked
            // (contract v6).
            NodeKind::Icon { name } => {
                format!(
                    "b.icon(&ui::icons::{})",
                    name.to_uppercase().replace('-', "_")
                )
            }
            // A picture, by the product's static in its generated
            // `pictures` module at the crate's root, so only the pictures
            // named are linked (contract v15).
            NodeKind::Image {
                picture,
                alt,
                fit,
                width,
                height,
            } => {
                let mut layout = "ui::Layout::new()".to_owned();
                for (method, size) in [("width", width), ("height", height)] {
                    if let Some(size) = size {
                        layout.push_str(&format!(".{method}({})", length(size)));
                    }
                }
                format!(
                    "b.image(&crate::pictures::{}, ui::Fit::{}, {}, {layout})",
                    picture.to_uppercase(),
                    fit.rust(),
                    rust::string_literal(alt)
                )
            }
            // A virtual list: its pool of rows (contract v18).
            NodeKind::VirtualList {
                list,
                row_height,
                rows,
                width,
                height,
            } => {
                let mut layout = "ui::Layout::new()".to_owned();
                for (method, size) in [("width", width), ("height", height)] {
                    if let Some(size) = size {
                        layout.push_str(&format!(".{method}({})", length(size)));
                    }
                }
                let rows: Vec<String> = rows.iter().map(|row| self.node(row, body)).collect();
                format!(
                    "b.virtual_list({layout}, lists::{}, {}, &[{}])",
                    list.screaming_snake_case(),
                    length(row_height),
                    rows.join(", ")
                )
            }
            // The on-screen keyboard (contract v11).
            NodeKind::Keyboard => "b.keyboard()".to_owned(),
            // A field's error line (contract v12).
            NodeKind::FieldError { field } => {
                format!("b.field_error(fields::{})", field.screaming_snake_case())
            }
            // A layer over the screen (contract v20), beside the control
            // its `for` names, which is written before it.
            NodeKind::Layer {
                kind,
                open,
                anchor,
                children,
            } => {
                let anchor = anchor.as_ref().map(|anchor| handle(body, anchor));
                let children: Vec<String> = children
                    .iter()
                    .map(|child| self.node(child, body))
                    .collect();
                let layer = format!("layers::{}", open.screaming_snake_case());
                match anchor {
                    Some(anchor) => format!(
                        "b.{}({layer}, {anchor}, {})",
                        kind.call(),
                        children.join(", ")
                    ),
                    None => format!("b.{}({layer}, {})", kind.call(), children.join(", ")),
                }
            }
            NodeKind::Tooltip { anchor, text } => format!(
                "b.tooltip({}, {})",
                handle(body, anchor),
                rust::string_literal(text)
            ),
            // A scroll view of its one child (contract v9).
            NodeKind::ScrollView { children } => {
                let children: Vec<String> = children
                    .iter()
                    .map(|child| self.node(child, body))
                    .collect();
                format!("b.scroll_view(ui::Layout::new(), {})", children.join(", "))
            }
            // A progress bar (contract v5).
            NodeKind::Progress { slot } => {
                format!("b.progress(slots::{})", slot.screaming_snake_case())
            }
            NodeKind::Group(group) => {
                let children: Vec<String> = group
                    .children
                    .iter()
                    .map(|child| self.node(child, body))
                    .collect();
                let name = group.name.screaming_snake_case();
                if children.len() > 1 {
                    format!("b.either(groups::{name}, {})", children.join(", "))
                } else if group.unless {
                    format!("b.unless(groups::{name}, {})", children.join(", "))
                } else {
                    format!("b.group(groups::{name}, {})", children.join(", "))
                }
            }
            // A switch's case (contract v13).
            NodeKind::Case(case) => {
                let children: Vec<String> = case
                    .children
                    .iter()
                    .map(|child| self.node(child, body))
                    .collect();
                let switch = case.switch.screaming_snake_case();
                let cases = case.switch.snake_case();
                let when = case.when.screaming_snake_case();
                let method = if children.len() > 1 {
                    "case_or"
                } else {
                    "case"
                };
                format!(
                    "b.{method}(switches::{switch}, switches::{cases}::{when}, {})",
                    children.join(", ")
                )
            }
        };
        let at = node
            .loc
            .map(|l| format!("{}:{l} ", self.file(l)))
            .unwrap_or_default();
        let what = match &node.kind {
            NodeKind::Group(group) if group.children.len() > 1 => {
                format!("{{{} ? ... : ...}}", group.name.source())
            }
            NodeKind::Group(group) if group.unless => {
                format!("{{!{} && ...}}", group.name.source())
            }
            NodeKind::Group(group) => format!("{{{} && ...}}", group.name.source()),
            NodeKind::Case(case) if case.children.len() > 1 => format!(
                "{{{} === \"{}\" ? ... : ...}}",
                case.switch.source(),
                case.when.source()
            ),
            NodeKind::Case(case) => format!(
                "{{{} === \"{}\" && ...}}",
                case.switch.source(),
                case.when.source()
            ),
            _ => format!("<{}>", node.component()),
        };
        body.lines
            .push(format!("let {var} = {call}?; // {at}{what}"));
        var
    }
}

/// `fields::NAME` for a value-holding control, named after its id (which
/// the IR guarantees it has).
/// The variable of the element with id `id`, emitted before: what a layer
/// shows beside. Validation has made sure there is one.
fn handle(body: &Body, id: &screen_compiler_ir::Name) -> String {
    body.handles
        .iter()
        .find(|(_, _, written, _)| written == id.source())
        .map_or_else(|| "n0".to_owned(), |(_, var, _, _)| var.clone())
}

fn field(node: &Node) -> String {
    let name = node
        .id
        .as_ref()
        .map(Name::screaming_snake_case)
        .unwrap_or_default();
    format!("fields::{name}")
}

/// A slot's literal text in the target API (contract v14), a reference to
/// a constant the compiler promotes to a static:
/// `&ui::Template { before: "…", after: "…" }`.
fn template_literal(template: &Template) -> String {
    format!(
        "&ui::Template {{ before: {}, after: {} }}",
        rust::string_literal(&template.before),
        rust::string_literal(&template.after)
    )
}

/// A text style in the target API (contract v2): its role, in its
/// own weight, and `.bold()` for body, label or caption text in Bold; then
/// `.lines(N)` for text that wraps onto up to N lines (contract v4);
/// then `.align(…)` for text not set against the start (contract v7);
/// then `.colour(…)` for text in a colour of its own (contract v8).
fn text_style(style: TextStyle) -> String {
    let mut out = format!("ui::TextStyle::new(ui::TextRole::{})", style.role.rust());
    if style.weight != TextStyle::new(style.role).weight {
        out.push_str(".bold()");
    }
    if style.lines != 1 {
        out.push_str(&format!(".lines({})", style.lines));
    }
    if style.align != TextAlign::Start {
        out.push_str(&format!(".align(ui::TextAlign::{})", style.align.rust()));
    }
    if let Some(colour) = style.colour {
        out.push_str(&format!(".colour(ui::Colour::{})", colour.rust()));
    }
    out
}

/// A look in the target API (contract v8): `ui::Look::NONE` with
/// each part the style names.
fn look(look: Look) -> String {
    let mut out = "ui::Look::NONE".to_owned();
    if let Some(fill) = look.fill {
        out.push_str(&format!(".fill(ui::Colour::{})", fill.rust()));
    }
    if let Some(border) = look.border {
        out.push_str(&format!(".border(ui::Colour::{})", border.rust()));
    }
    if let Some(width) = look.border_width {
        out.push_str(&format!(".border_width({width})"));
    }
    if let Some(radius) = look.radius {
        out.push_str(&format!(".radius({radius})"));
    }
    if let Some(text) = look.text {
        out.push_str(&format!(".text(ui::Colour::{})", text.rust()));
    }
    if let Some(shadow) = look.shadow {
        out.push_str(&format!(".shadow(ui::Elevation::{})", shadow.rust()));
    }
    if let Some(gradient) = look.gradient {
        let (from, to) = (gradient.from.rust(), gradient.to.rust());
        out.push_str(&match gradient.angle {
            Some(angle) => format!(
                ".gradient(ui::Gradient::linear({angle}, ui::Colour::{from}, ui::Colour::{to}))"
            ),
            None => {
                format!(".gradient(ui::Gradient::radial(ui::Colour::{from}, ui::Colour::{to}))")
            }
        });
    }
    if let Some(opacity) = look.opacity {
        out.push_str(&format!(".opacity({opacity})"));
    }
    if let Some(t) = look.transition {
        // A constant, by reference: a node stays small.
        out.push_str(&format!(
            ".transition({{ const T: ui::Transition = ui::Transition::new({}, {}).delay({}).easing({}); &T }})",
            t.property.rust(),
            t.duration,
            t.delay,
            t.easing.rust()
        ));
    }
    if let Some(a) = look.animation {
        let base = match a.name {
            AnimationName::Spin => format!(
                "ui::Animation::new(ui::Property::Turn, 0, 3600, {})",
                a.duration
            ),
            // Growing for half the run and back for the other half.
            AnimationName::Pulse => format!(
                "ui::Animation::new(ui::Property::Scale, 1000, 1060, {}).alternate()",
                a.duration / 2
            ),
            AnimationName::Fade => format!(
                "ui::Animation::new(ui::Property::Opacity, 0, 255, {})",
                a.duration
            ),
            AnimationName::Indeterminate => {
                format!(
                    "ui::Animation::new(ui::Property::Sweep, 0, 1000, {})",
                    a.duration
                )
            }
            AnimationName::Shimmer => {
                format!(
                    "ui::Animation::new(ui::Property::Shimmer, 0, 1000, {})",
                    a.duration
                )
            }
        };
        // A pulse's run is two of the runtime's, there and back.
        let runs = |times: u16| {
            if a.name == AnimationName::Pulse {
                times.saturating_mul(2)
            } else {
                times
            }
        };
        let repeat = match a.repeat {
            Some(times) => format!("ui::Repeat::Times({})", runs(times)),
            None => "ui::Repeat::Forever".to_owned(),
        };
        let mut animation = format!(
            "{base}.delay({}).easing({}).repeat({repeat})",
            a.delay,
            a.easing.rust()
        );
        if a.alternate && a.name != AnimationName::Pulse {
            animation.push_str(".alternate()");
        }
        if !a.keep {
            animation.push_str(".release()");
        }
        out.push_str(&format!(
            ".animation({{ const A: ui::Animation = {animation}; &A }})"
        ));
    }
    out
}

/// A number of logical pixels, or a screen's own design token. The IR
/// allows a theme's spacing token only where the target API takes one
/// (`validate`), which the callers emit themselves; were one to reach here
/// it is written as the token, which no numeric argument accepts, so the
/// generated code fails to build rather than take a number in its place.
fn length(length: &Length) -> String {
    match length {
        Length::Px(px) => px.to_string(),
        Length::Token(name) => format!("super::tokens::{}", name.screaming_snake_case()),
        Length::Space(space) => format!("ui::Space::{}", space.rust()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        reason = "test code: a failed check panics, and tests index and compute on known data"
    )]

    use super::*;
    use screen_compiler_ir::{Container, Surface, TextRole, Token, Tone};

    fn text(text: &str) -> NodeKind {
        NodeKind::Text {
            text: text.into(),
            style: TextStyle::BODY,
        }
    }

    fn name(s: &str) -> Name {
        Name::parse(s).unwrap()
    }

    fn node(kind: NodeKind, id: Option<&str>) -> Node {
        Node {
            kind,
            id: id.map(name),
            loc: None,
        }
    }

    fn module(root: Node) -> Module {
        Module {
            source_name: "demo.tsx".into(),
            imported: Vec::new(),
            tokens: vec![Token {
                name: name("GAP"),
                value: 4,
                loc: None,
            }],
            screens: vec![Screen {
                name: name("Demo"),
                root,
                loc: None,
            }],
        }
    }

    #[test]
    fn emits_children_first_with_document_order_numbering() {
        let root = node(
            NodeKind::Column(Container {
                gap: Some(Length::Token(name("GAP"))),
                padding: None,
                surface: Some(Surface::Notice(Tone::Caution)),
                children: vec![
                    node(text("Hi"), Some("n1")),
                    node(
                        NodeKind::Text {
                            text: "A047".into(),
                            style: TextStyle::new(TextRole::Display),
                        },
                        None,
                    ),
                    node(
                        NodeKind::Text {
                            text: "Small print".into(),
                            style: TextStyle::new(TextRole::Caption).bold(),
                        },
                        None,
                    ),
                    node(
                        NodeKind::Button {
                            label: "Go".into(),
                            action: name("onGo").without_leading("on"),
                            variant: ButtonVariant::Primary,
                            look: Look::default(),
                        },
                        Some("go"),
                    ),
                    node(
                        NodeKind::Row(Container {
                            surface: Some(Surface::Header),
                            children: vec![node(
                                NodeKind::Button {
                                    label: "Back".into(),
                                    action: name("onBack").without_leading("on"),
                                    variant: ButtonVariant::Quiet,
                                    look: Look::default(),
                                },
                                None,
                            )],
                            ..Container::default()
                        }),
                        None,
                    ),
                ],
                ..Container::default()
            }),
            None,
        );
        let expected = "\
// @generated by screen-compiler from demo.tsx. Do not edit by hand.
// Target: nowayos-ui builder contract v23 (provisional). Regenerate with `screen-compiler convert`.

use nowayos_ui::builder as ui;

/// Design tokens declared in `demo.tsx`, in logical pixels.
pub mod tokens {
    /// `GAP`.
    pub const GAP: u16 = 4;
}

/// Screen `Demo`.
pub mod demo {
    use super::ui;

    /// Actions the screen emits, numbered from 1 in document order.
    pub mod actions {
        use super::ui;

        /// Callback prop `onGo`.
        pub const GO: ui::ActionId = ui::ActionId::new(1);
        /// Callback prop `onBack`.
        pub const BACK: ui::ActionId = ui::ActionId::new(2);
    }

    /// Handles to the screen's root node and to each element with an `id`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Nodes<N> {
        /// The root node: install it as the screen's root.
        pub root: N,
        /// `id=\"n1\"`.
        pub n1: N,
        /// `id=\"go\"`.
        pub go: N,
    }

    /// Builds the screen with `b`, children before their parents.
    ///
    /// # Errors
    ///
    /// Returns the builder's error, for example when its node arena is full.
    pub fn build<B: ui::UiBuilder>(b: &mut B) -> Result<Nodes<B::Node>, B::Error> {
        let n1 = b.text(\"Hi\")?; // <Text>
        let n2 = b.text_styled(\"A047\", ui::TextStyle::new(ui::TextRole::Display))?; // <Text>
        let n3 = b.text_styled(\"Small print\", ui::TextStyle::new(ui::TextRole::Caption).bold())?; // <Text>
        let n4 = b.button(\"Go\", actions::GO)?; // <Button>
        let n6 = b.button_with(\"Back\", actions::BACK, ui::ButtonVariant::Quiet)?; // <Button>
        let n5 = b.row_on(ui::Surface::Header, ui::Layout::new(), &[n6])?; // <HeaderBar>
        let n0 = b.column_on(ui::Surface::Notice(ui::Tone::Caution), ui::Layout::new().gap(super::tokens::GAP), &[n1, n2, n3, n4, n5])?; // <Notice>
        Ok(Nodes { root: n0, n1, go: n4 })
    }
}
";
        assert_eq!(generate(&module(root)).unwrap(), expected);
    }

    #[test]
    fn keywords_and_reserved_names_are_reported() {
        let root = node(
            NodeKind::Column(Container {
                children: vec![node(text("a"), Some("type")), node(text("b"), Some("root"))],
                ..Container::default()
            }),
            None,
        );
        let mut m = module(root);
        m.screens[0].name = name("Tokens");
        let diagnostics = generate(&m).unwrap_err();
        assert_eq!(diagnostics.len(), 3, "{diagnostics:#?}");
        assert!(diagnostics.iter().all(|d| d.code == Code::ReservedName));
    }

    #[test]
    fn a_screen_cannot_shadow_the_api_crate() {
        // `pub mod nowayos_ui` would shadow the crate in
        // `use nowayos_ui::builder as ui;`, and the file would not compile.
        for screen in ["NowayosUi", "nowayos_ui", "Nowayos_UI"] {
            let mut m = module(node(text("a"), None));
            m.screens[0].name = name(screen);
            let diagnostics = generate(&m).unwrap_err();
            assert_eq!(diagnostics.len(), 1, "{screen}: {diagnostics:#?}");
            assert_eq!(diagnostics[0].code, Code::ReservedName, "{screen}");
        }
    }

    #[test]
    fn file_names_are_reduced_to_plain_labels() {
        // Generated docs go through rustdoc, which passes HTML through and
        // turns [text](url) into links: the file name must not carry either.
        // Only the last path component is shown.
        let hostile = "dir/sub\\<img src=x onerror=alert(1)>[a](javascript:b)`&.tsx";
        let mut m = module(node(text("a"), None));
        m.source_name = hostile.into();
        m.screens[0].loc = Some(Loc::new(1, 1));
        let rust = generate(&m).unwrap();
        let header = rust.lines().next().unwrap();
        assert_eq!(
            header,
            "// @generated by screen-compiler from \\u{3c}img\\u{20}src\\u{3d}x\\u{20}onerror\\u{3d}alert\\u{28}1\\u{29}\\u{3e}\\u{5b}a\\u{5d}\\u{28}javascript\\u{3a}b\\u{29}\\u{60}\\u{26}.tsx. Do not edit by hand."
        );
        for hostile in [
            "<img",
            "onerror=",
            "[a]",
            "](",
            "javascript:",
            "`&",
            "dir/",
            "sub\\",
        ] {
            assert!(
                !rust.contains(hostile),
                "{hostile:?} reached the output:\n{rust}"
            );
        }
        let label = header
            .strip_prefix("// @generated by screen-compiler from ")
            .and_then(|rest| rest.strip_suffix(". Do not edit by hand."))
            .unwrap();
        let docs_naming_the_file = rust
            .lines()
            .filter(|l| l.trim_start().starts_with("///") && l.contains(".tsx"))
            .collect::<Vec<_>>();
        assert!(!docs_naming_the_file.is_empty());
        assert!(
            docs_naming_the_file.iter().all(|l| l.contains(label)),
            "{rust}"
        );
        // The same file under different directories generates the same text.
        let mut plain = m.clone();
        plain.source_name = "screen.tsx".into();
        let mut nested = m.clone();
        nested.source_name = "C:\\work\\ui/screen.tsx".into();
        assert_eq!(generate(&plain).unwrap(), generate(&nested).unwrap());
    }

    #[test]
    fn every_generated_identifier_must_be_valid_rust() {
        assert!(rust::is_identifier("welcome_screen") && rust::is_identifier("ON_2FA"));
        assert!(rust::is_identifier("_x") && rust::is_identifier("r2d2"));
        for bad in ["2FA", "", "_", "a-b", "type", "a b", "caf\u{e9}"] {
            assert!(!rust::is_identifier(bad), "{bad:?}");
        }
    }

    #[test]
    fn invalid_ir_is_refused() {
        let root = node(
            NodeKind::Spacer {
                size: Some(Length::Token(name("MISSING"))),
            },
            None,
        );
        let diagnostics = generate(&module(root)).unwrap_err();
        assert_eq!(diagnostics[0].code, Code::UnknownName);
    }

    #[test]
    fn output_is_deterministic_and_ascii() {
        let root = node(text("Caf\u{e9} \u{ff}"), None);
        let m = module(root);
        let first = generate(&m).unwrap();
        assert_eq!(first, generate(&m).unwrap());
        assert!(first.is_ascii());
        assert!(first.contains("b.text(\"Caf\\u{e9} \\u{ff}\")?"));
    }
}
