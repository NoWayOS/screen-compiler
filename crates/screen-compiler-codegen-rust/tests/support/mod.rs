// SPDX-License-Identifier: MIT
//! A recording implementation of the builder contract, for tests.
//!
//! It checks the tree properties the contract promises (children exist and
//! have no other parent, capacity is bounded) and renders the tree in the
//! same outline format as `screen_compiler_ir::outline`, so a test can compare the
//! tree generated code builds with the IR it was generated from.

#![allow(
    dead_code,
    reason = "shared by several test binaries, each of which uses only part of it"
)]

use screen_compiler_codegen_rust::contract::builder::{
    ActionId, Align, ButtonVariant, Colour, FieldId, Fit, GroupId, Input, InputMode, Justify,
    Layout, ListId, Look, Picker, Picture, ScrollAxes, SlotId, Space, Surface, SwitchId, Template,
    TextAlign, TextStyle, Tone, UiBuilder, Weight, icons,
};

/// A theme colour's TSX name, as `screen_compiler_ir::outline` writes it.
fn colour(colour: Colour) -> &'static str {
    match colour {
        Colour::Background => "background",
        Colour::Text => "text",
        Colour::TextMuted => "text-muted",
        Colour::Placeholder => "placeholder",
        Colour::Button => "button",
        Colour::ButtonText => "button-text",
        Colour::Input => "input",
        Colour::InputBorder => "input-border",
        Colour::Focus => "focus",
        Colour::Error => "error",
    }
}

/// A look as `screen_compiler_ir::outline` writes it.
fn looked(look: Look) -> String {
    let mut out = String::new();
    if let Some(fill) = look.fill {
        out.push_str(&format!(" fill={}", colour(fill)));
    }
    if let Some(border) = look.border {
        out.push_str(&format!(" border={}", colour(border)));
    }
    if let Some(width) = look.border_width {
        out.push_str(&format!(" border-width={width}"));
    }
    if let Some(radius) = look.radius {
        out.push_str(&format!(" radius={radius}"));
    }
    if let Some(text) = look.text {
        out.push_str(&format!(" label={}", colour(text)));
    }
    out
}

/// A container's style as `screen_compiler_ir::outline` writes it.
fn container_style(layout: Layout) -> String {
    let mut out = String::new();
    for (name, units) in [
        ("width", layout.width_px()),
        ("height", layout.height_px()),
        ("min-width", Some(layout.min_width_px()).filter(|w| *w > 0)),
        ("max-width", layout.max_width_px()),
    ] {
        if let Some(units) = units {
            out.push_str(&format!(" {name}={units}"));
        }
    }
    match layout.align_of() {
        Align::Stretch => {}
        Align::Start => out.push_str(" align=start"),
        Align::Center => out.push_str(" align=center"),
        Align::End => out.push_str(" align=end"),
    }
    match layout.justify_of() {
        Justify::Start => {}
        Justify::Center => out.push_str(" justify=center"),
        Justify::End => out.push_str(" justify=end"),
        Justify::SpaceBetween => out.push_str(" justify=space-between"),
    }
    out.push_str(&looked(layout.look_of()));
    out
}

/// A theme's spacing token as `screen_compiler_ir::outline` writes it:
/// `space.md`.
fn space(space: Space) -> String {
    format!("space.{}", format!("{space:?}").to_lowercase())
}

/// A layout's gap and padding as `screen_compiler_ir::outline` writes them: a
/// number, or a theme's spacing token by name.
fn spaced(layout: Layout) -> String {
    let gap = layout
        .gap_token()
        .map_or_else(|| layout.gap_px().to_string(), space);
    let padding = layout
        .padding_token()
        .map_or_else(|| layout.padding_px().to_string(), space);
    format!("gap={gap} padding={padding}")
}

/// A text style as `screen_compiler_ir::outline` writes it: nothing for body
/// text, Regular; otherwise ` role=ROLE weight=WEIGHT`.
fn styled(style: TextStyle) -> String {
    let mut lines = if style.lines == 1 {
        String::new()
    } else {
        format!(" lines={}", style.lines)
    };
    if style.align != TextAlign::Start {
        let align = format!("{:?}", style.align).to_lowercase();
        lines.push_str(&format!(" align={align}"));
    }
    if let Some(text) = style.colour {
        lines.push_str(&format!(" colour={}", colour(text)));
    }
    let plain = TextStyle {
        lines: 1,
        align: TextAlign::Start,
        colour: None,
        ..style
    };
    if plain == TextStyle::BODY {
        return lines;
    }
    let role = format!("{:?}", style.role).to_lowercase();
    let weight = match style.weight {
        Weight::Regular => "regular",
        Weight::Bold => "bold",
    };
    format!(" role={role} weight={weight}{lines}")
}

/// A container's surface as `screen_compiler_ir::outline` writes it: nothing for
/// none; otherwise ` surface=SURFACE`.
fn surfaced(surface: Surface) -> String {
    let name = match surface {
        Surface::Card => "card",
        Surface::Header => "header",
        Surface::Notice(Tone::Success) => "notice-success",
        Surface::Notice(Tone::Caution) => "notice-caution",
        Surface::Notice(Tone::Warning) => "notice-warning",
    };
    format!(" surface={name}")
}

/// A handle to a recorded node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeRef(usize);

/// Why the recorder refused a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordError {
    /// The capacity is used up.
    Full,
    /// A child handle this recorder did not issue.
    UnknownNode,
    /// A child that already has a parent.
    AlreadyHasParent,
    /// A field or slot number used twice.
    Duplicate,
}

struct Recorded {
    line: String,
    children: Vec<usize>,
    has_parent: bool,
}

/// Records every node a generated `build` function creates.
pub struct Recorder {
    nodes: Vec<Recorded>,
    capacity: usize,
    slots: Vec<u16>,
    fields: Vec<u16>,
}

impl Recorder {
    /// A recorder that accepts at most `capacity` nodes.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: Vec::new(),
            capacity,
            slots: Vec::new(),
            fields: Vec::new(),
        }
    }

    /// The node's outline line, e.g. `text "Welcome"`.
    pub fn describe(&self, node: NodeRef) -> &str {
        &self.nodes[node.0].line
    }

    /// How many nodes have no parent (a complete tree has exactly one).
    pub fn roots(&self) -> usize {
        self.nodes.iter().filter(|n| !n.has_parent).count()
    }

    /// The subtree at `root`, in `screen_compiler_ir::outline` format.
    pub fn outline(&self, root: NodeRef) -> String {
        let mut out = String::new();
        let mut stack = vec![(root.0, 0_usize)];
        while let Some((index, depth)) = stack.pop() {
            let node = &self.nodes[index];
            out.push_str(&"  ".repeat(depth));
            out.push_str(&node.line);
            out.push('\n');
            stack.extend(node.children.iter().rev().map(|&c| (c, depth + 1)));
        }
        out
    }

    /// Records a value control bound to `field`, refusing a number used
    /// twice.
    fn add_field(&mut self, field: FieldId, line: String) -> Result<NodeRef, RecordError> {
        if self.fields.contains(&field.get()) {
            return Err(RecordError::Duplicate);
        }
        let node = self.add(line, &[])?;
        self.fields.push(field.get());
        Ok(node)
    }

    fn add(&mut self, line: String, children: &[NodeRef]) -> Result<NodeRef, RecordError> {
        if self.nodes.len() >= self.capacity {
            return Err(RecordError::Full);
        }
        for child in children {
            let node = self.nodes.get(child.0).ok_or(RecordError::UnknownNode)?;
            if node.has_parent {
                return Err(RecordError::AlreadyHasParent);
            }
        }
        for child in children {
            self.nodes[child.0].has_parent = true;
        }
        self.nodes.push(Recorded {
            line,
            children: children.iter().map(|c| c.0).collect(),
            has_parent: false,
        });
        Ok(NodeRef(self.nodes.len() - 1))
    }
}

impl UiBuilder for Recorder {
    type Node = NodeRef;
    type Error = RecordError;

    fn column(&mut self, layout: Layout, children: &[NodeRef]) -> Result<NodeRef, RecordError> {
        let line = format!("column {}{}", spaced(layout), container_style(layout));
        self.add(line, children)
    }

    fn row(&mut self, layout: Layout, children: &[NodeRef]) -> Result<NodeRef, RecordError> {
        let line = format!("row {}{}", spaced(layout), container_style(layout));
        self.add(line, children)
    }

    fn text(&mut self, text: &'static str) -> Result<NodeRef, RecordError> {
        self.add(format!("text {text:?}"), &[])
    }

    fn button(&mut self, label: &'static str, action: ActionId) -> Result<NodeRef, RecordError> {
        self.add(format!("button {label:?} action={}", action.get()), &[])
    }

    fn text_input(&mut self, placeholder: Option<&'static str>) -> Result<NodeRef, RecordError> {
        let line = match placeholder {
            None => "text_input".to_owned(),
            Some(p) => format!("text_input placeholder={p:?}"),
        };
        self.add(line, &[])
    }

    fn spacer(&mut self, size: Option<u16>) -> Result<NodeRef, RecordError> {
        let line = match size {
            None => "spacer".to_owned(),
            Some(px) => format!("spacer size={px}"),
        };
        self.add(line, &[])
    }

    fn button_look(
        &mut self,
        label: &'static str,
        action: ActionId,
        variant: ButtonVariant,
        look: Look,
    ) -> Result<NodeRef, RecordError> {
        let variant = match variant {
            ButtonVariant::Primary => "",
            ButtonVariant::Secondary => " variant=secondary",
            ButtonVariant::Quiet => " variant=quiet",
        };
        self.add(
            format!(
                "button {label:?} action={}{variant}{}",
                action.get(),
                looked(look)
            ),
            &[],
        )
    }

    fn scroll_view(&mut self, _layout: Layout, child: NodeRef) -> Result<NodeRef, RecordError> {
        self.add("scroll".to_owned(), &[child])
    }

    fn virtual_list(
        &mut self,
        layout: Layout,
        list: ListId,
        row_height: u16,
        rows: &[NodeRef],
    ) -> Result<NodeRef, RecordError> {
        let mut line = format!("virtual-list list {} row-height={row_height}", list.get());
        if let Some(width) = layout.width_px() {
            line.push_str(&format!(" width={width}"));
        }
        if let Some(height) = layout.height_px() {
            line.push_str(&format!(" height={height}"));
        }
        self.add(line, rows)
    }

    fn scroll_view_axes(
        &mut self,
        layout: Layout,
        axes: ScrollAxes,
        child: NodeRef,
    ) -> Result<NodeRef, RecordError> {
        let axes = match axes {
            ScrollAxes::Vertical => "vertical",
            ScrollAxes::Horizontal => "horizontal",
            ScrollAxes::Both => "both",
        };
        self.add(
            format!("scroll axes={axes}{}", container_style(layout)),
            &[child],
        )
    }

    fn keyboard(&mut self) -> Result<NodeRef, RecordError> {
        self.add("keyboard".to_owned(), &[])
    }

    fn field_error(&mut self, field: FieldId) -> Result<NodeRef, RecordError> {
        self.add(format!("field_error field={}", field.get()), &[])
    }

    fn spacer_space(&mut self, space_token: Space) -> Result<NodeRef, RecordError> {
        self.add(format!("spacer size={}", space(space_token)), &[])
    }

    fn slot(&mut self, slot: SlotId) -> Result<NodeRef, RecordError> {
        if self.slots.contains(&slot.get()) {
            return Err(RecordError::Duplicate);
        }
        let node = self.add(format!("slot {}", slot.get()), &[])?;
        self.slots.push(slot.get());
        Ok(node)
    }

    fn text_styled(
        &mut self,
        text: &'static str,
        style: TextStyle,
    ) -> Result<NodeRef, RecordError> {
        self.add(format!("text {text:?}{}", styled(style)), &[])
    }

    fn icon(&mut self, icon: &'static icons::Icon) -> Result<NodeRef, RecordError> {
        self.add(format!("icon {}", icon.name()), &[])
    }

    fn image(
        &mut self,
        picture: &'static Picture,
        fit: Fit,
        alt: &'static str,
        layout: Layout,
    ) -> Result<NodeRef, RecordError> {
        let fit = match fit {
            Fit::Fill => "fill",
            Fit::Contain => "contain",
            Fit::Cover => "cover",
        };
        let mut line = format!("image {} alt={alt:?} fit={fit}", picture.name());
        if let Some(width) = layout.width_px() {
            line.push_str(&format!(" width={width}"));
        }
        if let Some(height) = layout.height_px() {
            line.push_str(&format!(" height={height}"));
        }
        self.add(line, &[])
    }

    fn progress(&mut self, slot: SlotId) -> Result<NodeRef, RecordError> {
        if self.slots.contains(&slot.get()) {
            return Err(RecordError::Duplicate);
        }
        let node = self.add(format!("progress slot {}", slot.get()), &[])?;
        self.slots.push(slot.get());
        Ok(node)
    }

    fn slot_template(
        &mut self,
        slot: SlotId,
        template: &'static Template,
    ) -> Result<NodeRef, RecordError> {
        if self.slots.contains(&slot.get()) {
            return Err(RecordError::Duplicate);
        }
        let node = self.add(
            format!(
                "slot {} between {:?} and {:?}",
                slot.get(),
                template.before,
                template.after
            ),
            &[],
        )?;
        self.slots.push(slot.get());
        Ok(node)
    }

    fn slot_template_styled(
        &mut self,
        slot: SlotId,
        template: &'static Template,
        style: TextStyle,
    ) -> Result<NodeRef, RecordError> {
        if self.slots.contains(&slot.get()) {
            return Err(RecordError::Duplicate);
        }
        let node = self.add(
            format!(
                "slot {}{} between {:?} and {:?}",
                slot.get(),
                styled(style),
                template.before,
                template.after
            ),
            &[],
        )?;
        self.slots.push(slot.get());
        Ok(node)
    }

    fn long_text(
        &mut self,
        slot: SlotId,
        capacity: u16,
        style: TextStyle,
    ) -> Result<NodeRef, RecordError> {
        if self.slots.contains(&slot.get()) {
            return Err(RecordError::Duplicate);
        }
        let line = format!(
            "long-text slot {} capacity={capacity}{}",
            slot.get(),
            styled(style)
        );
        let node = self.add(line, &[])?;
        self.slots.push(slot.get());
        Ok(node)
    }

    fn slot_styled(&mut self, slot: SlotId, style: TextStyle) -> Result<NodeRef, RecordError> {
        if self.slots.contains(&slot.get()) {
            return Err(RecordError::Duplicate);
        }
        let node = self.add(format!("slot {}{}", slot.get(), styled(style)), &[])?;
        self.slots.push(slot.get());
        Ok(node)
    }

    fn button_with(
        &mut self,
        label: &'static str,
        action: ActionId,
        variant: ButtonVariant,
    ) -> Result<NodeRef, RecordError> {
        let variant = match variant {
            ButtonVariant::Primary => "",
            ButtonVariant::Secondary => " variant=secondary",
            ButtonVariant::Quiet => " variant=quiet",
        };
        self.add(
            format!("button {label:?} action={}{variant}", action.get()),
            &[],
        )
    }

    fn column_on(
        &mut self,
        surface: Surface,
        layout: Layout,
        children: &[NodeRef],
    ) -> Result<NodeRef, RecordError> {
        let line = format!(
            "column {}{}{}",
            spaced(layout),
            surfaced(surface),
            container_style(layout)
        );
        self.add(line, children)
    }

    fn row_on(
        &mut self,
        surface: Surface,
        layout: Layout,
        children: &[NodeRef],
    ) -> Result<NodeRef, RecordError> {
        let line = format!(
            "row {}{}{}",
            spaced(layout),
            surfaced(surface),
            container_style(layout)
        );
        self.add(line, children)
    }

    fn group(&mut self, group: GroupId, child: NodeRef) -> Result<NodeRef, RecordError> {
        self.add(format!("group {}", group.get()), &[child])
    }

    fn either(
        &mut self,
        group: GroupId,
        when_visible: NodeRef,
        otherwise: NodeRef,
    ) -> Result<NodeRef, RecordError> {
        self.add(format!("group {}", group.get()), &[when_visible, otherwise])
    }

    fn unless(&mut self, group: GroupId, child: NodeRef) -> Result<NodeRef, RecordError> {
        self.add(format!("unless group {}", group.get()), &[child])
    }

    fn case(&mut self, switch: SwitchId, when: u8, child: NodeRef) -> Result<NodeRef, RecordError> {
        self.add(format!("switch {} case {when}", switch.get()), &[child])
    }

    fn case_or(
        &mut self,
        switch: SwitchId,
        when: u8,
        when_matched: NodeRef,
        otherwise: NodeRef,
    ) -> Result<NodeRef, RecordError> {
        self.add(
            format!("switch {} case {when}", switch.get()),
            &[when_matched, otherwise],
        )
    }

    fn input(&mut self, field: FieldId, input: Input) -> Result<NodeRef, RecordError> {
        let mode = match input.input_mode() {
            InputMode::Text => "text",
            InputMode::Digits => "digits",
            InputMode::Email => "email",
        };
        let mut line = format!(
            "input field={} max_len={} mode={mode}",
            field.get(),
            input.max_len_bytes()
        );
        if !input.placeholder_text().is_empty() {
            line.push_str(&format!(" placeholder={:?}", input.placeholder_text()));
        }
        self.add_field(field, line)
    }

    fn text_area(
        &mut self,
        field: FieldId,
        rows: u8,
        max_len: u16,
        placeholder: &'static str,
    ) -> Result<NodeRef, RecordError> {
        let mut line = format!(
            "text-area field={} rows={rows} max_len={max_len}",
            field.get()
        );
        if !placeholder.is_empty() {
            line.push_str(&format!(" placeholder={placeholder:?}"));
        }
        self.add_field(field, line)
    }

    fn checkbox(&mut self, field: FieldId, label: &'static str) -> Result<NodeRef, RecordError> {
        self.add_field(field, format!("checkbox field={} {label:?}", field.get()))
    }

    fn radio_group(
        &mut self,
        field: FieldId,
        options: &'static [&'static str],
    ) -> Result<NodeRef, RecordError> {
        self.add_field(
            field,
            format!("radio_group field={} {options:?}", field.get()),
        )
    }

    fn list_picker(
        &mut self,
        field: FieldId,
        options: &'static [&'static str],
        picker: Picker,
    ) -> Result<NodeRef, RecordError> {
        self.add_field(
            field,
            format!(
                "list_picker field={} rows={} placeholder={:?} {options:?}",
                field.get(),
                picker.row_count(),
                picker.placeholder_text()
            ),
        )
    }
}

/// Runs the whole pipeline on a fixture under `fixtures/`: the IR module and
/// the generated Rust. Panics with the rendered diagnostics if it fails.
pub fn convert_fixture(name: &str) -> (screen_compiler_ir::Module, String) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name);
    let source = std::fs::read_to_string(&path).unwrap();
    // The files it imports are read from beside it, as the tool reads them.
    let mut folder = screen_compiler_analyse::files::Folder::of(&path).unwrap();
    convert_in(name, &source, &mut folder)
}

/// Runs the whole pipeline on `source`, recorded as file `name`; the files
/// it imports are read from `fixtures/`.
pub fn convert_source(name: &str, source: &str) -> (screen_compiler_ir::Module, String) {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut folder = screen_compiler_analyse::files::Folder::new(&fixtures).unwrap();
    convert_in(name, source, &mut folder)
}

/// The names `fixtures/pictures.names` lists, one a line.
pub fn picture_names() -> Vec<String> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pictures.names");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// Runs the whole pipeline on `source`, recorded as file `name`, reading the
/// files it imports through `load`.
fn convert_in(
    name: &str,
    source: &str,
    load: &mut dyn screen_compiler_analyse::files::Load,
) -> (screen_compiler_ir::Module, String) {
    let render = |diagnostics: &[screen_compiler_ir::Diagnostic]| -> String {
        diagnostics
            .iter()
            .map(|d| d.render(name))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let parsed = match screen_compiler_tsx::parse(source) {
        Ok(parsed) => parsed,
        Err(diagnostics) => panic!("{name} does not parse:\n{}", render(&diagnostics)),
    };
    // The pictures fixtures may show: NoWayOS's samples, as
    // `fixtures/pictures.names` lists them.
    let mut options = screen_compiler_analyse::Options::new(name);
    options.pictures = Some(picture_names());
    let analysis = screen_compiler_analyse::analyse_with(&parsed, &options, load);
    let Some(module) = analysis.module else {
        panic!(
            "{name} is not supported:\n{}",
            render(&analysis.diagnostics)
        );
    };
    match screen_compiler_codegen_rust::generate(&module) {
        Ok(rust) => (module, rust),
        Err(diagnostics) => panic!("{name} does not generate:\n{}", render(&diagnostics)),
    }
}
