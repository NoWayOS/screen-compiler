// SPDX-License-Identifier: MIT
//! The target API contract: what generated code expects from NoWayOS's UI
//! runtime.
//!
//! Generated code imports one module, [`API_MODULE`] (`nowayos_ui::builder`),
//! as `ui`, and uses these items from it: the [`builder::UiBuilder`] trait,
//! [`builder::Layout`], [`builder::Input`] and [`builder::InputMode`],
//! [`builder::TextStyle`], [`builder::TextRole`] and [`builder::TextAlign`],
//! [`builder::Space`], [`builder::Align`], [`builder::Justify`],
//! [`builder::Colour`] and [`builder::Look`],
//! [`builder::ButtonVariant`], [`builder::Surface`] and [`builder::Tone`],
//! and the numbers
//! [`builder::ActionId`], [`builder::FieldId`], [`builder::GroupId`] and
//! [`builder::SlotId`]. The [`builder`] module below
//! is the reference definition of that API. It is compiled here, and this
//! crate's tests build every snapshot of generated code against it, so the
//! contract and the generator cannot drift apart.
//!
//! **Status: provisional (contract v10).** NoWayOS's first-cut UI
//! core (`nowayos-ui`, in the separate `nowayos` repository) implements it:
//! its `builder` module has these items, with `UiBuilder` implemented for its
//! bounded node arena (`Tree`). `make contract-check` builds every generated
//! screen against it, so a change on either side must be made on the other
//! too; if the contract changes, this module and the generator change
//! together and [`VERSION`] is bumped.
//!
//! Contract v1 keeps
//! v0's six calls unchanged, so v0's generated screens still build, and adds
//! the calls form screens need. Built so far: `slot` (text from a prop),
//! `group` and `either` (conditional rendering), and the value-holding
//! controls `input` (a text input with a length limit and an input mode),
//! `checkbox` and `radio_group`, each bound to a field number. Every text
//! input is emitted as `input`, never v0's `text_input`, so one screen never
//! mixes v0's implicit field numbers with v1's.
//!
//! Contract v2 keeps every v1 call unchanged and adds text in a role
//! of the theme's type scale and a weight: `text_styled` and `slot_styled`,
//! with [`builder::TextStyle`], [`builder::TextRole`] and
//! [`builder::Weight`]. Body text in Regular is still emitted as `text` and
//! `slot`, so a screen that names no role generates the same calls as
//! before. The display role draws digits, capitals and a few signs only;
//! Screen Compiler refuses static text a role's glyph atlases do not draw, and the
//! runtime refuses such text in a slot when the application sets it.
//!
//! Contract v3 adds the look of buttons and surfaces:
//! `button_with`, a button in a [`builder::ButtonVariant`] - secondary or
//! quiet; a primary button is still emitted as `button` - and `column_on`
//! and `row_on`, a container drawn on a [`builder::Surface`]: `<Card>` and
//! `<Notice tone="…">` are columns, `<HeaderBar>` a row.
//!
//! Contract v12 adds `field_error`: the line where the errors the
//! application sets for a field show, `<FieldError for="phone" />`; its
//! `for` names a control on the screen and a field has one, which
//! Screen Compiler checks; the runtime refuses a second too.
//!
//! Contract v11 adds `keyboard`: the on-screen keyboard, its keys
//! typing into the focused text input in a layout for its mode -
//! `<Keyboard />`, at most one in a screen and only in one with a text
//! input, which Screen Compiler checks; the runtime refuses a second too.
//!
//! Contract v10 adds `list_picker`: one choice of up to 64
//! options, bound to a field, in a list a [`builder::Picker`] shapes - its
//! placeholder and how many rows show (3 to 8, 5 unless told) - that
//! scrolls inside itself: `<Select id placeholder options rows>`.
//!
//! Contract v9 adds `scroll_view`: one child - a column - shown
//! through a view that scrolls, `<ScrollView><Column>…</Column></ScrollView>`;
//! at most one in a screen and never nested, which Screen Compiler checks and the
//! runtime refuses too.
//!
//! Contract v8 adds what a `style={{…}}` declares: on a
//! [`builder::Layout`], a fixed `width` and `height`, a `min_width` and
//! `max_width`, children's [`builder::Align`] across and
//! [`builder::Justify`] along the main axis, and a [`builder::Look`] - a
//! fill, a border and its width, rounded corners - in the theme's
//! [`builder::Colour`]s; a text's colour, `.colour(…)` on a
//! [`builder::TextStyle`]; and `button_look`, a button with a fill or label
//! colour of its own. A colour is always the theme's, by its role.
//!
//! Contract v7 adds a text's alignment - `.align(ui::TextAlign::Center)`
//! on a [`builder::TextStyle`], from `<Text align="center">` - and the
//! theme's spacing tokens, [`builder::Space`]: a gap or padding named by
//! size, `ui::Layout::new().gap_space(ui::Space::Md)`, from
//! `gap={space.md}`, and `spacer_space`, from `<Spacer size={space.sm} />`.
//! The runtime measures a token against the theme when it lays the tree
//! out, so the large-text theme spaces half again as much; a number, or a
//! screen's own `const`, stays fixed.
//!
//! Contract v6 adds `icon`, one of Lucide's icons by its static in
//! [`builder::icons`] - `<Icon name="fuel" />` is `b.icon(&ui::icons::FUEL)`
//! - so a program links only the icons its screens name.
//!
//! Contract v5 adds `progress`, a progress bar the application
//! fills, bound to a [`builder::SlotId`] numbered with the slots:
//! `<ProgressBar value={props.fuelled} />`.
//!
//! Contract v4 adds text set on several lines: `.lines(N)` on a
//! [`builder::TextStyle`], from 1 to [`builder::MAX_LINES`] (8) - a
//! `<Text lines={N}>` is emitted as `text_styled` or `slot_styled` with it.
//! The runtime breaks the text after spaces at the node's width and gives
//! it the height of its lines; every other call is unchanged.
//!
//! What the contract asks of the runtime:
//!
//! * a node handle type that is `Copy`;
//! * construction **children first**: a container is created with the
//!   handles of its already-built children, so every node is complete when
//!   it is created and needs no later mutation;
//! * fallible construction (`Result`): a bounded arena reports "full" as an
//!   error, and generated code propagates it with `?` (it never panics);
//! * only `&'static str` text: generated screens hold literals, so the
//!   runtime can store text without copying or allocating;
//! * lengths in logical pixels as `u16`;
//! * [`builder::ActionId`] numbers: each screen numbers its actions from 1 in
//!   document order and exports them as named constants; likewise its groups
//!   ([`builder::GroupId`]) and slots ([`builder::SlotId`]), each named after
//!   the prop it comes from; and its fields ([`builder::FieldId`]), named
//!   after the controls' ids. The runtime refuses a field or slot number used
//!   twice in one screen; groups that share a number, one prop's, show and
//!   hide together. Likewise its switches ([`builder::SwitchId`], contract
//!   v13), each with its cases numbered from 0 in the order they are first
//!   written, as `u8` constants in a module named after the switch.
//!
//! Contract v20 adds layers over the screen ([`builder::LayerId`], in a
//! screen's `layers` module, one per prop that opens one): `dialog`
//! (`<Dialog open={props.confirming}>`), `menu` and `popover` beside the
//! control their `for` names (`<Menu for="more" open={props.menu}>`), and
//! `tooltip` (`<Tooltip for="delete">Deletes it</Tooltip>`).
//!
//! Contract v19 adds `text_area`: several lines a person types,
//! `<TextArea id="note" rows={4} maxLength={1000} placeholder="…" />`,
//! bound to a field as a text input is, its bytes in the text store of the
//! tree the screen is built into.
//!
//! Contract v18 adds `virtual_list`: `<VirtualList rowHeight={56}>`
//! around one list from a prop sliced to its pool,
//! `{props.items.slice(0, 12).map(…)}`. Its rows, built once, show as many
//! items as the application counts ([`builder::ListId`], in a screen's
//! `lists` module), rebound as the list scrolls; their slots are numbered
//! as any list's rows are.
//!
//! Contract v17 adds `scroll_view_axes`: a container whose style names
//! `overflow`, `overflowX` or `overflowY` of `auto` or `scroll` is a scroll
//! view of its size, down, across or both ways ([`builder::ScrollAxes`]),
//! holding the container with the rest of its style. A screen may have any
//! number of scroll views, one inside another.
//!
//! Contract v16 adds `long_text`: paragraphs of any length the
//! application sets at run time, bound to a slot and numbered with the
//! slots, up to the bytes the screen gives
//! (`<Text maxLength={10240}>{props.article}</Text>`), kept in the text
//! store of the tree the screen is built into.
//!
//! Contract v15 adds `image`: one of the product's pictures, converted at
//! build time, by its static in the `pictures` module at the root of the
//! crate the screens are compiled in (`crate::pictures::HARBOUR`, which the
//! product's picture tool writes), fitted by a [`builder::Fit`], with
//! words saying what it shows and the size its style gives it
//! (`<Image src="harbour" alt="…" style={{ width: 320, objectFit: "cover" }} />`).
//!
//! Contract v14 adds `slot_template` and `slot_template_styled`: a slot
//! between literal text compiled in (`<Text>Hello, {props.name}!</Text>`),
//! the literals a static `Template`, the application setting only its part.
//!
//! Contract v13 adds conditions beyond one prop's truth: `unless`, a child
//! shown while a group is hidden (`{!props.ready && <X/>}`), and `case` and
//! `case_or`, a child shown while a switch is on one case
//! (`{props.step === "review" && <X/>}`, or `? <X/> : <Y/>`).
//!
//! The mapping from subset-v0 TSX to emitted Rust is [`MAPPINGS`].

/// The contract version. Generated code names it in its header comment.
pub const VERSION: u32 = 20;

/// The path generated code imports (as `ui`).
pub const API_MODULE: &str = "nowayos_ui::builder";

/// How one TSX construct is emitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mapping {
    /// The TSX construct.
    pub tsx: &'static str,
    /// Its IR form.
    pub ir: &'static str,
    /// What the generator emits for it.
    pub rust: &'static str,
}

/// TSX to IR to emitted Rust, one row per construct of the subset.
pub const MAPPINGS: &[Mapping] = &[
    Mapping {
        tsx: "export function WelcomeScreen(props) { return <Screen>…</Screen>; }",
        ir: "Screen { name, root }",
        rust: "pub mod welcome_screen { pub mod actions; pub struct Nodes<N>; pub fn build<B: ui::UiBuilder>(b: &mut B) -> Result<Nodes<B::Node>, B::Error> }",
    },
    Mapping {
        tsx: "<Column gap={G} padding={P}>…</Column>",
        ir: "NodeKind::Column(Container { gap, padding, children })",
        rust: "b.column(ui::Layout::new().gap(G).padding(P), &[children…])?",
    },
    Mapping {
        tsx: "<Row gap={G} padding={P}>…</Row>",
        ir: "NodeKind::Row(Container { gap, padding, children })",
        rust: "b.row(ui::Layout::new().gap(G).padding(P), &[children…])?",
    },
    Mapping {
        tsx: "<Card gap={G} padding={P}>…</Card>",
        ir: "NodeKind::Column(Container { gap, padding, children, surface: Some(Surface::Card) })",
        rust: "b.column_on(ui::Surface::Card, ui::Layout::new().gap(G).padding(P), &[children…])?",
    },
    Mapping {
        tsx: "<Notice tone=\"success\">…</Notice>",
        ir: "NodeKind::Column(Container { …, surface: Some(Surface::Notice(Tone::Success)) })",
        rust: "b.column_on(ui::Surface::Notice(ui::Tone::Success), layout, &[children…])?",
    },
    Mapping {
        tsx: "<HeaderBar gap={G} padding={P}>…</HeaderBar>",
        ir: "NodeKind::Row(Container { …, surface: Some(Surface::Header) })",
        rust: "b.row_on(ui::Surface::Header, layout, &[children…])?",
    },
    Mapping {
        tsx: "<Text>literal</Text>",
        ir: "NodeKind::Text { text }",
        rust: "b.text(\"literal\")?",
    },
    Mapping {
        tsx: "<Text role=\"title\">literal</Text>, <Text weight=\"bold\">literal</Text>",
        ir: "NodeKind::Text { text, style: TextStyle { role, weight } }",
        rust: "b.text_styled(\"literal\", ui::TextStyle::new(ui::TextRole::Title))? or ui::TextStyle::new(ui::TextRole::Body).bold()",
    },
    Mapping {
        tsx: "<Icon name=\"fuel\" />",
        ir: "NodeKind::Icon { name: \"fuel\" }",
        rust: "b.icon(&ui::icons::FUEL)?",
    },
    Mapping {
        tsx: "<Image src=\"harbour\" alt=\"…\" fit=\"cover\" style={{ width: 320, height: 200 }} /> (fit, or a style's objectFit)",
        ir: "NodeKind::Image { picture: \"harbour\", alt, fit: Cover, width, height }",
        rust: "b.image(&crate::pictures::HARBOUR, ui::Fit::Cover, \"…\", ui::Layout::new().width(320).height(200))? (contract v15)",
    },
    Mapping {
        tsx: "<ProgressBar value={props.fuelled} />",
        ir: "NodeKind::Progress { slot: fuelled }",
        rust: "b.progress(slots::FUELLED)?, with pub const FUELLED: ui::SlotId = ui::SlotId::new(N), numbered with the slots",
    },
    Mapping {
        tsx: "<Text lines={3}>literal</Text>",
        ir: "NodeKind::Text { text, style: TextStyle { lines: 3, … } }",
        rust: "b.text_styled(\"literal\", ui::TextStyle::new(ui::TextRole::Body).lines(3))?",
    },
    Mapping {
        tsx: "<Button onPress={onContinue}>Label</Button>",
        ir: "NodeKind::Button { label, action: continue }",
        rust: "b.button(\"Label\", actions::CONTINUE)?, with pub const CONTINUE: ui::ActionId = ui::ActionId::new(N)",
    },
    Mapping {
        tsx: "<Button variant=\"secondary\" onPress={onBack}>Label</Button>",
        ir: "NodeKind::Button { label, action: back, variant: Secondary }",
        rust: "b.button_with(\"Label\", actions::BACK, ui::ButtonVariant::Secondary)?",
    },
    Mapping {
        tsx: "<TextInput id=\"phone\" placeholder=\"…\" maxLength={15} inputMode=\"tel\" />",
        ir: "NodeKind::TextInput { placeholder, max_len, mode }",
        rust: "b.input(fields::PHONE, ui::Input::new().placeholder(\"…\").max_len(15).mode(ui::InputMode::Digits))?, with pub const PHONE: ui::FieldId = ui::FieldId::new(N)",
    },
    Mapping {
        tsx: "<TextArea id=\"note\" rows={4} maxLength={1000} placeholder=\"…\" />",
        ir: "NodeKind::TextArea { placeholder, rows, max_len }",
        rust: "b.text_area(fields::NOTE, 4, 1000, \"…\")? (contract v19), numbered with the fields",
    },
    Mapping {
        tsx: "<Dialog open={props.confirming}><Column>…</Column></Dialog>",
        ir: "NodeKind::Layer { kind: Dialog, open: confirming, anchor: None, children }",
        rust: "b.dialog(layers::CONFIRMING, column)? (contract v20), with pub const CONFIRMING: ui::LayerId = ui::LayerId::new(N)",
    },
    Mapping {
        tsx: "<Button id=\"more\" …/><Menu for=\"more\" open={props.menuOpen}><Column>…</Column></Menu>, or <Popover …>",
        ir: "NodeKind::Layer { kind: Menu, open: menuOpen, anchor: Some(more), children }",
        rust: "b.menu(layers::MENU_OPEN, more, column)?, or b.popover(…)? (contract v20): the anchor is the variable of the element `for` names",
    },
    Mapping {
        tsx: "<Tooltip for=\"delete\">Deletes it for good</Tooltip>",
        ir: "NodeKind::Tooltip { anchor: delete, text }",
        rust: "b.tooltip(delete, \"Deletes it for good\")? (contract v20)",
    },
    Mapping {
        tsx: "<Checkbox id=\"remind\" label=\"…\" />",
        ir: "NodeKind::Checkbox { label }",
        rust: "b.checkbox(fields::REMIND, \"…\")?",
    },
    Mapping {
        tsx: "<RadioGroup id=\"time\" options={[\"…\", \"…\"]} />",
        ir: "NodeKind::RadioGroup { options }",
        rust: "b.radio_group(fields::TIME, &[\"…\", \"…\"])?",
    },
    Mapping {
        tsx: "<Text align=\"center\">literal</Text>",
        ir: "Text { text, style: TextStyle { align: Center, .. } }",
        rust: "b.text_styled(\"literal\", ui::TextStyle::new(ui::TextRole::Body).align(ui::TextAlign::Center))?",
    },
    Mapping {
        tsx: "<Column gap={space.md} padding={space.lg}>…</Column>",
        ir: "Container { gap: Length::Space(Md), padding: Length::Space(Lg), .. }",
        rust: "b.column(ui::Layout::new().gap_space(ui::Space::Md).padding_space(ui::Space::Lg), &[children…])?",
    },
    Mapping {
        tsx: "<Column style={{ width: 360, alignItems: \"center\", justifyContent: \"space-between\", backgroundColor: \"input\", borderColor: \"input-border\", borderWidth: 2, borderRadius: 12 }}>",
        ir: "Container { width, align, justify, look: Look { fill, border, border_width, radius, .. }, .. }",
        rust: "ui::Layout::new().width(360).align(ui::Align::Center).justify(ui::Justify::SpaceBetween).look(ui::Look::NONE.fill(ui::Colour::Input).border(ui::Colour::InputBorder).border_width(2).radius(12))",
    },
    Mapping {
        tsx: "<Text style={{ color: \"text-muted\", textAlign: \"right\" }}>literal</Text>",
        ir: "Text { style: TextStyle { colour: Some(TextMuted), align: End, .. } }",
        rust: "ui::TextStyle::new(ui::TextRole::Body).align(ui::TextAlign::End).colour(ui::Colour::TextMuted)",
    },
    Mapping {
        tsx: "<Button style={{ backgroundColor: \"text\", color: \"background\" }} onPress={onDone}>Done</Button>",
        ir: "Button { look: Look { fill, text, .. }, .. }",
        rust: "b.button_look(\"Done\", actions::DONE, ui::ButtonVariant::Primary, ui::Look::NONE.fill(ui::Colour::Text).text(ui::Colour::Background))?",
    },
    Mapping {
        tsx: "<Select id=\"kind\" placeholder=\"…\" options={[\"…\", …]} rows={5} />",
        ir: "NodeKind::Select { options, placeholder, rows }",
        rust: "b.list_picker(fields::KIND, &[\"…\", …], ui::Picker::new().placeholder(\"…\").rows(5))?",
    },
    Mapping {
        tsx: "<FieldError for=\"phone\" />",
        ir: "NodeKind::FieldError { field: phone }",
        rust: "b.field_error(fields::PHONE)?",
    },
    Mapping {
        tsx: "<Keyboard />",
        ir: "NodeKind::Keyboard",
        rust: "b.keyboard()?",
    },
    Mapping {
        tsx: "<Column style={{ overflowY: \"auto\", height: 200 }}>…</Column> (overflowX across; overflow, or both, both ways)",
        ir: "NodeKind::Column(Container { overflow: Some(ScrollAxes::Vertical), height, … })",
        rust: "{ let inner = b.column(layout, &[…])?; b.scroll_view_axes(ui::Layout::new().height(200), ui::ScrollAxes::Vertical, inner) }? (contract v17)",
    },
    Mapping {
        tsx: "<VirtualList rowHeight={56}>{props.items.slice(0, 12).map((item) => <Row>…</Row>)}</VirtualList>",
        ir: "NodeKind::VirtualList { list: items, row_height: 56, rows: [the 12 rows] }",
        rust: "b.virtual_list(ui::Layout::new(), lists::ITEMS, 56, &[row1, …, row12])?, with pub const ITEMS: ui::ListId = ui::ListId::new(N) (contract v18)",
    },
    Mapping {
        tsx: "<ScrollView><Column>…</Column></ScrollView>",
        ir: "NodeKind::ScrollView { children: [column] }",
        rust: "b.scroll_view(ui::Layout::new(), column)?",
    },
    Mapping {
        tsx: "<Spacer size={space.sm} />",
        ir: "NodeKind::Spacer(Some(Length::Space(Sm)))",
        rust: "b.spacer_space(ui::Space::Sm)?",
    },
    Mapping {
        tsx: "<Spacer /> or <Spacer size={N} />",
        ir: "NodeKind::Spacer { size }",
        rust: "b.spacer(None)? (flexible) or b.spacer(Some(N))?",
    },
    Mapping {
        tsx: "id=\"email\" on any element",
        ir: "Node { id }",
        rust: "a field `email: N` of the screen's Nodes<N>, holding that node's handle",
    },
    Mapping {
        tsx: "<Text>{props.name}</Text> (the prop alone)",
        ir: "NodeKind::Slot { slot: name }",
        rust: "b.slot(slots::NAME)?, with pub const NAME: ui::SlotId = ui::SlotId::new(N)",
    },
    Mapping {
        tsx: "<Text>Hello, {props.name}!</Text>, or {`Hello, ${props.name}!`} (one prop between literal text)",
        ir: "NodeKind::Slot { slot: name, template: Some(Template { before: \"Hello, \", after: \"!\" }) }",
        rust: "b.slot_template(slots::NAME, &ui::Template { before: \"Hello, \", after: \"!\" })?, or b.slot_template_styled(..., style)? (contract v14)",
    },
    Mapping {
        tsx: "<Text maxLength={10240}>{props.article}</Text> (one prop alone)",
        ir: "NodeKind::Slot { slot: article, capacity: Some(10240), template: None, style }",
        rust: "b.long_text(slots::ARTICLE, 10240, ui::TextStyle::new(ui::TextRole::Body))? (contract v16)",
    },
    Mapping {
        tsx: "<Text role=\"display\">{props.ticket}</Text>",
        ir: "NodeKind::Slot { slot: ticket, style }",
        rust: "b.slot_styled(slots::TICKET, ui::TextStyle::new(ui::TextRole::Display))?",
    },
    Mapping {
        tsx: "{props.ready && <X />}",
        ir: "NodeKind::Group(Group { name: ready, children: [X] })",
        rust: "b.group(groups::READY, x)?, with pub const READY: ui::GroupId = ui::GroupId::new(N)",
    },
    Mapping {
        tsx: "{props.ready ? <X /> : <Y />} (or `: null`, as `&&`)",
        ir: "NodeKind::Group(Group { name: ready, children: [X, Y] })",
        rust: "b.either(groups::READY, x, y)?",
    },
    Mapping {
        tsx: "{!props.ready && <X />}, or {!props.ready ? <X /> : <Y />} (as {props.ready ? <Y /> : <X />})",
        ir: "NodeKind::Group(Group { name: ready, children: [X], unless: true })",
        rust: "b.unless(groups::READY, x)? (contract v13)",
    },
    Mapping {
        tsx: "{props.step === \"review\" && <X />}",
        ir: "NodeKind::Case(Case { switch: step, when: review, children: [X] })",
        rust: "b.case(switches::STEP, switches::step::REVIEW, x)?, with pub const STEP: ui::SwitchId = ui::SwitchId::new(N) and pub mod step { pub const REVIEW: u8 = 0; } (contract v13)",
    },
    Mapping {
        tsx: "{props.step === \"review\" ? <X /> : <Y />}",
        ir: "NodeKind::Case(Case { switch: step, when: review, children: [X, Y] })",
        rust: "b.case_or(switches::STEP, switches::step::REVIEW, x, y)? (contract v13)",
    },
    Mapping {
        tsx: "const SPACE_MD = 16;",
        ir: "Token { name, value }",
        rust: "pub mod tokens { pub const SPACE_MD: u16 = 16; } (used as super::tokens::SPACE_MD)",
    },
];

pub mod builder {
    //! Reference definition of `nowayos_ui::builder`, the API generated code
    //! is written against. See the [parent module](super) for the contract.

    /// The icons (contract v6): each a static of its own, by Lucide name.
    pub mod icons {
        /// One icon.
        #[derive(Debug, PartialEq, Eq)]
        pub struct Icon(&'static str);

        impl Icon {
            /// Its Lucide name.
            #[must_use]
            pub const fn name(&self) -> &'static str {
                self.0
            }
        }

        /// `clipboard-list`.
        pub static CLIPBOARD_LIST: Icon = Icon("clipboard-list");
        /// `fuel`.
        pub static FUEL: Icon = Icon("fuel");
        /// `house`.
        pub static HOUSE: Icon = Icon("house");
        /// `message-square`.
        pub static MESSAGE_SQUARE: Icon = Icon("message-square");
        /// `shield-check`.
        pub static SHIELD_CHECK: Icon = Icon("shield-check");
        /// `user-check`.
        pub static USER_CHECK: Icon = Icon("user-check");
    }

    /// One of the product's pictures (contract v15): NoWayOS's
    /// `nowayos_pictures::Picture`, here only its name, for tests, and its
    /// size in layout units.
    #[derive(Debug, PartialEq, Eq)]
    pub struct Picture {
        name: &'static str,
        width: u32,
        height: u32,
    }

    impl Picture {
        /// The picture `name`, `width` x `height` layout units.
        #[must_use]
        pub const fn new(name: &'static str, width: u32, height: u32) -> Self {
            Self {
                name,
                width,
                height,
            }
        }

        /// Its name, as the product's `pictures.toml` gives it.
        #[must_use]
        pub const fn name(&self) -> &'static str {
            self.name
        }

        /// Its width, in layout units.
        #[must_use]
        pub const fn width(&self) -> u32 {
            self.width
        }

        /// Its height, in layout units.
        #[must_use]
        pub const fn height(&self) -> u32 {
            self.height
        }
    }

    /// Which ways a scroll view scrolls (contract v17).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub enum ScrollAxes {
        /// Down.
        #[default]
        Vertical,
        /// Across.
        Horizontal,
        /// Both ways.
        Both,
    }

    /// How a picture fills a box of another shape (contract v15): CSS's
    /// `object-fit`.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub enum Fit {
        /// Stretched to the box.
        #[default]
        Fill,
        /// As large as fits inside it, its shape kept.
        Contain,
        /// As small as covers it, its shape kept, cut on two sides.
        Cover,
    }

    /// An action a screen emits, such as a button press.
    ///
    /// Generated screens number their actions from 1 in document order and
    /// export each as a named constant in their `actions` module.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ActionId(u16);

    impl ActionId {
        /// The action with number `value`.
        #[must_use]
        pub const fn new(value: u16) -> Self {
            Self(value)
        }

        /// The action's number.
        #[must_use]
        pub const fn get(self) -> u16 {
            self.0
        }
    }

    /// Which value a control holds: a text input, checkbox or radio group
    /// (contract v1). Generated screens number their fields from 1 in
    /// document order and export each, named after the control's id, in
    /// their `fields` module.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FieldId(u16);

    impl FieldId {
        /// The field with number `value`.
        #[must_use]
        pub const fn new(value: u16) -> Self {
            Self(value)
        }

        /// The field's number.
        #[must_use]
        pub const fn get(self) -> u16 {
            self.0
        }
    }

    /// Which characters a text input takes, and so the on-screen
    /// keyboard's layout (contract v1).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub enum InputMode {
        /// Any text.
        #[default]
        Text,
        /// The digits 0 to 9 only.
        Digits,
        /// An address such as an email address.
        Email,
    }

    /// How a text input is made (contract v1): its placeholder, the most
    /// bytes it takes (1 to 64) and its mode.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Input {
        placeholder: &'static str,
        max_len: u16,
        mode: InputMode,
    }

    impl Input {
        /// No placeholder, 64 bytes, any text.
        #[must_use]
        pub const fn new() -> Self {
            Self {
                placeholder: "",
                max_len: 64,
                mode: InputMode::Text,
            }
        }

        /// With `placeholder` shown while the value is empty.
        #[must_use]
        pub const fn placeholder(self, placeholder: &'static str) -> Self {
            Self {
                placeholder,
                ..self
            }
        }

        /// Taking at most `bytes` bytes, 1 to 64.
        #[must_use]
        pub const fn max_len(self, bytes: u16) -> Self {
            Self {
                max_len: bytes,
                ..self
            }
        }

        /// In `mode`.
        #[must_use]
        pub const fn mode(self, mode: InputMode) -> Self {
            Self { mode, ..self }
        }

        /// The placeholder.
        #[must_use]
        pub const fn placeholder_text(self) -> &'static str {
            self.placeholder
        }

        /// The most bytes it takes.
        #[must_use]
        pub const fn max_len_bytes(self) -> u16 {
            self.max_len
        }

        /// Its mode.
        #[must_use]
        pub const fn input_mode(self) -> InputMode {
            self.mode
        }
    }

    impl Default for Input {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Which group of content the application shows or hides (contract
    /// v1). Generated screens number their groups from 1 in document order
    /// and export each as a named constant in their `groups` module.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct GroupId(u16);

    impl GroupId {
        /// The group with number `value`.
        #[must_use]
        pub const fn new(value: u16) -> Self {
            Self(value)
        }

        /// The group's number.
        #[must_use]
        pub const fn get(self) -> u16 {
            self.0
        }
    }

    /// Which virtual list the application counts (contract v18).
    /// Generated screens number their lists from 1 in document order and
    /// export each as a named constant in their `lists` module.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ListId(u16);

    impl ListId {
        /// The list with number `value`.
        #[must_use]
        pub const fn new(value: u16) -> Self {
            Self(value)
        }

        /// The list's number.
        #[must_use]
        pub const fn get(self) -> u16 {
            self.0
        }
    }

    /// A layer over the screen, a number the application opens and closes
    /// it by (contract v20). Generated screens number their layers from 1
    /// in document order and export each as a named constant in their
    /// `layers` module.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct LayerId(u16);

    impl LayerId {
        /// The layer with number `value`.
        #[must_use]
        pub const fn new(value: u16) -> Self {
            Self(value)
        }

        /// The layer's number.
        #[must_use]
        pub const fn get(self) -> u16 {
            self.0
        }
    }

    /// Literal text compiled in around a slot's (contract v14): the
    /// literals show from the first frame, and the application sets only
    /// its part.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Template {
        /// The text before the slot's.
        pub before: &'static str,
        /// The text after it.
        pub after: &'static str,
    }

    /// Which switch a case is on (contract v13). Generated screens number
    /// their switches from 1 in document order and export each as a named
    /// constant in their `switches` module, its cases beside it.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct SwitchId(u16);

    impl SwitchId {
        /// The switch with number `value`.
        #[must_use]
        pub const fn new(value: u16) -> Self {
            Self(value)
        }

        /// The switch's number.
        #[must_use]
        pub const fn get(self) -> u16 {
            self.0
        }
    }

    /// Which slot of application text (contract v1). Generated screens
    /// number their slots from 1 in document order and export each as a
    /// named constant in their `slots` module.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct SlotId(u16);

    impl SlotId {
        /// The slot with number `value`.
        #[must_use]
        pub const fn new(value: u16) -> Self {
            Self(value)
        }

        /// The slot's number.
        #[must_use]
        pub const fn get(self) -> u16 {
            self.0
        }
    }

    /// What a line of text is for: its size in the theme's type scale
    /// (contract v2).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub enum TextRole {
        /// A number or a short reference to read across a room, in digits,
        /// capitals and a few signs only. Bold.
        Display,
        /// A screen's title. Bold.
        Title,
        /// A heading. Bold.
        Heading,
        /// Text, fields and buttons: the default.
        #[default]
        Body,
        /// A field's or a tile's label.
        Label,
        /// Small print.
        Caption,
    }

    /// A text's weight (contract v2).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub enum Weight {
        /// Regular, 400.
        #[default]
        Regular,
        /// Bold, 700.
        Bold,
    }

    /// The most lines a text may wrap onto (contract v4).
    pub const MAX_LINES: u8 = 8;

    /// Where a text's lines sit across its node (contract v7).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub enum TextAlign {
        /// Against the start edge: the default.
        #[default]
        Start,
        /// Centred.
        Center,
        /// Against the end edge.
        End,
    }

    /// How a list picker shows (contract v10): its placeholder and how many
    /// rows of options show at once, 3 to 8.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Picker {
        placeholder: &'static str,
        rows: u8,
    }

    impl Picker {
        /// No placeholder, five rows.
        #[must_use]
        pub const fn new() -> Self {
            Self {
                placeholder: "",
                rows: 5,
            }
        }

        /// With `text` shown until an option is chosen.
        #[must_use]
        pub const fn placeholder(self, text: &'static str) -> Self {
            Self {
                placeholder: text,
                ..self
            }
        }

        /// With `rows` rows showing.
        #[must_use]
        pub const fn rows(self, rows: u8) -> Self {
            Self { rows, ..self }
        }

        /// Its placeholder.
        #[must_use]
        pub const fn placeholder_text(self) -> &'static str {
            self.placeholder
        }

        /// How many rows show.
        #[must_use]
        pub const fn row_count(self) -> u8 {
            self.rows
        }
    }

    impl Default for Picker {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Where a container's children sit across its main axis (contract
    /// v8): stretched by default.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub enum Align {
        /// At the start.
        Start,
        /// Centred.
        Center,
        /// At the end.
        End,
        /// Stretched across.
        #[default]
        Stretch,
    }

    /// Where a container's children sit along its main axis when they leave
    /// room over (contract v8).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub enum Justify {
        /// From the start.
        #[default]
        Start,
        /// Centred.
        Center,
        /// At the end.
        End,
        /// The room shared between them.
        SpaceBetween,
    }

    /// One of the theme's colours, by its role (contract v8).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum Colour {
        /// The screen's background.
        Background,
        /// Ordinary text.
        Text,
        /// Text that matters less.
        TextMuted,
        /// A text input's placeholder.
        Placeholder,
        /// A primary button's fill.
        Button,
        /// A primary button's label.
        ButtonText,
        /// A text input's fill.
        Input,
        /// A text input's border.
        InputBorder,
        /// The focus ring.
        Focus,
        /// An error.
        Error,
    }

    /// How a container or a button is drawn beyond its kind's own look
    /// (contract v8): widths in layout units.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Look {
        /// Its fill.
        pub fill: Option<Colour>,
        /// Its border's colour.
        pub border: Option<Colour>,
        /// Its border's width.
        pub border_width: Option<u32>,
        /// Its corners' radius.
        pub radius: Option<u32>,
        /// A button's label colour.
        pub text: Option<Colour>,
    }

    impl Look {
        /// Nothing named.
        pub const NONE: Self = Self {
            fill: None,
            border: None,
            border_width: None,
            radius: None,
            text: None,
        };

        /// With `colour` as its fill.
        #[must_use]
        pub const fn fill(self, colour: Colour) -> Self {
            Self {
                fill: Some(colour),
                ..self
            }
        }

        /// With a border in `colour`.
        #[must_use]
        pub const fn border(self, colour: Colour) -> Self {
            Self {
                border: Some(colour),
                ..self
            }
        }

        /// With a border `units` wide.
        #[must_use]
        pub const fn border_width(self, units: u32) -> Self {
            Self {
                border_width: Some(units),
                ..self
            }
        }

        /// With corners of radius `units`.
        #[must_use]
        pub const fn radius(self, units: u32) -> Self {
            Self {
                radius: Some(units),
                ..self
            }
        }

        /// A button's label in `colour`.
        #[must_use]
        pub const fn text(self, colour: Colour) -> Self {
            Self {
                text: Some(colour),
                ..self
            }
        }
    }

    /// A theme's spacing token (contract v7): 4, 8, 16, 24 and 32 layout
    /// units in the light theme, half again in the large-text theme.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum Space {
        /// The smallest.
        Xs,
        /// Small.
        Sm,
        /// Medium.
        Md,
        /// Large.
        Lg,
        /// The largest.
        Xl,
    }

    /// How a text is set: its role and its weight (contract v2), the most
    /// lines it may wrap onto (contract v4) and where its lines sit
    /// (contract v7).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TextStyle {
        /// Its role in the type scale.
        pub role: TextRole,
        /// Its weight.
        pub weight: Weight,
        /// The most lines it may wrap onto, from 1 to [`MAX_LINES`].
        pub lines: u8,
        /// Where its lines sit across the node.
        pub align: TextAlign,
        /// Its colour, one of the theme's (contract v8).
        pub colour: Option<Colour>,
    }

    impl TextStyle {
        /// Body text, Regular.
        pub const BODY: Self = Self::new(TextRole::Body);

        /// `role` in its own weight: Bold for display, title and heading,
        /// Regular for the rest.
        #[must_use]
        pub const fn new(role: TextRole) -> Self {
            let weight = match role {
                TextRole::Display | TextRole::Title | TextRole::Heading => Weight::Bold,
                TextRole::Body | TextRole::Label | TextRole::Caption => Weight::Regular,
            };
            Self {
                role,
                weight,
                lines: 1,
                align: TextAlign::Start,
                colour: None,
            }
        }

        /// The same, in Bold.
        #[must_use]
        pub const fn bold(self) -> Self {
            Self {
                weight: Weight::Bold,
                ..self
            }
        }

        /// The same, wrapping onto at most `lines` lines, from 1 to
        /// [`MAX_LINES`] (contract v4).
        #[must_use]
        pub const fn lines(self, lines: u8) -> Self {
            let lines = if lines < 1 {
                1
            } else if lines > MAX_LINES {
                MAX_LINES
            } else {
                lines
            };
            Self { lines, ..self }
        }

        /// The same, its lines set `align` across the node (contract v7).
        #[must_use]
        pub const fn align(self, align: TextAlign) -> Self {
            Self { align, ..self }
        }

        /// The same, in the theme's `colour` (contract v8).
        #[must_use]
        pub const fn colour(self, colour: Colour) -> Self {
            Self {
                colour: Some(colour),
                ..self
            }
        }
    }

    impl Default for TextStyle {
        fn default() -> Self {
            Self::BODY
        }
    }

    /// How a button is drawn (contract v3).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub enum ButtonVariant {
        /// The one main action: filled.
        #[default]
        Primary,
        /// Another action: outlined.
        Secondary,
        /// A minor action: its label alone.
        Quiet,
    }

    /// What a notice tells (contract v3).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum Tone {
        /// Something done or ready.
        Success,
        /// Something to note.
        Caution,
        /// Something that needs care.
        Warning,
    }

    /// The surface a container is drawn on (contract v3).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum Surface {
        /// A card.
        Card,
        /// A header bar.
        Header,
        /// A notice in a tone.
        Notice(Tone),
    }

    /// Layout of a `Column` or `Row`: in logical pixels, or a spacing
    /// token in place of a number (contract v7).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Layout {
        gap: u16,
        padding: u16,
        gap_space: Option<Space>,
        padding_space: Option<Space>,
        width: Option<u16>,
        height: Option<u16>,
        min_width: u16,
        max_width: Option<u16>,
        align: Align,
        justify: Justify,
        look: Look,
    }

    impl Layout {
        /// No gap and no padding.
        #[must_use]
        pub const fn new() -> Self {
            Self {
                gap: 0,
                padding: 0,
                gap_space: None,
                padding_space: None,
                width: None,
                height: None,
                min_width: 0,
                max_width: None,
                align: Align::Stretch,
                justify: Justify::Start,
                look: Look::NONE,
            }
        }

        /// `units` wide (contract v8).
        #[must_use]
        pub const fn width(self, units: u16) -> Self {
            Self {
                width: Some(units),
                ..self
            }
        }

        /// `units` high (contract v8).
        #[must_use]
        pub const fn height(self, units: u16) -> Self {
            Self {
                height: Some(units),
                ..self
            }
        }

        /// At least `units` wide (contract v8).
        #[must_use]
        pub const fn min_width(self, units: u16) -> Self {
            Self {
                min_width: units,
                ..self
            }
        }

        /// At most `units` wide (contract v8).
        #[must_use]
        pub const fn max_width(self, units: u16) -> Self {
            Self {
                max_width: Some(units),
                ..self
            }
        }

        /// Its children across its main axis as `align` says (contract v8).
        #[must_use]
        pub const fn align(self, align: Align) -> Self {
            Self { align, ..self }
        }

        /// Its children along its main axis as `justify` says (contract v8).
        #[must_use]
        pub const fn justify(self, justify: Justify) -> Self {
            Self { justify, ..self }
        }

        /// Drawn with `look` (contract v8).
        #[must_use]
        pub const fn look(self, look: Look) -> Self {
            Self { look, ..self }
        }

        /// Its fixed width, if any.
        #[must_use]
        pub const fn width_px(self) -> Option<u16> {
            self.width
        }

        /// Its fixed height, if any.
        #[must_use]
        pub const fn height_px(self) -> Option<u16> {
            self.height
        }

        /// Its least width: 0 for none.
        #[must_use]
        pub const fn min_width_px(self) -> u16 {
            self.min_width
        }

        /// Its greatest width, if any.
        #[must_use]
        pub const fn max_width_px(self) -> Option<u16> {
            self.max_width
        }

        /// Its children's alignment.
        #[must_use]
        pub const fn align_of(self) -> Align {
            self.align
        }

        /// Its children's justification.
        #[must_use]
        pub const fn justify_of(self) -> Justify {
            self.justify
        }

        /// Its look.
        #[must_use]
        pub const fn look_of(self) -> Look {
            self.look
        }

        /// The theme's `space` between children (contract v7).
        #[must_use]
        pub const fn gap_space(self, space: Space) -> Self {
            Self {
                gap_space: Some(space),
                ..self
            }
        }

        /// The theme's `space` around the children (contract v7).
        #[must_use]
        pub const fn padding_space(self, space: Space) -> Self {
            Self {
                padding_space: Some(space),
                ..self
            }
        }

        /// The spacing token standing for the gap, if one does.
        #[must_use]
        pub const fn gap_token(self) -> Option<Space> {
            self.gap_space
        }

        /// The spacing token standing for the padding, if one does.
        #[must_use]
        pub const fn padding_token(self) -> Option<Space> {
            self.padding_space
        }

        /// Sets the space between children.
        #[must_use]
        pub const fn gap(self, px: u16) -> Self {
            Self { gap: px, ..self }
        }

        /// Sets the space around the children, on every side.
        #[must_use]
        pub const fn padding(self, px: u16) -> Self {
            Self {
                padding: px,
                ..self
            }
        }

        /// The space between children.
        #[must_use]
        pub const fn gap_px(self) -> u16 {
            self.gap
        }

        /// The space around the children.
        #[must_use]
        pub const fn padding_px(self) -> u16 {
            self.padding
        }
    }

    /// Builds a UI tree, children before parents.
    ///
    /// Every method creates one node and returns its handle, or the
    /// builder's error (for example when a bounded arena is full). A
    /// container takes the handles of its children, which must not already
    /// have a parent.
    pub trait UiBuilder {
        /// A handle to a node in the tree being built.
        type Node: Copy;
        /// Why a node could not be created.
        type Error;

        /// A vertical stack of `children`.
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn column(
            &mut self,
            layout: Layout,
            children: &[Self::Node],
        ) -> Result<Self::Node, Self::Error>;

        /// A horizontal row of `children`.
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn row(
            &mut self,
            layout: Layout,
            children: &[Self::Node],
        ) -> Result<Self::Node, Self::Error>;

        /// Static text.
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn text(&mut self, text: &'static str) -> Result<Self::Node, Self::Error>;

        /// A button labelled `label` that emits `action` when pressed.
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn button(
            &mut self,
            label: &'static str,
            action: ActionId,
        ) -> Result<Self::Node, Self::Error>;

        /// A single-line text field showing `placeholder` while empty.
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn text_input(
            &mut self,
            placeholder: Option<&'static str>,
        ) -> Result<Self::Node, Self::Error>;

        /// Empty space of a fixed `size`, or flexible space when `None`.
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn spacer(&mut self, size: Option<u16>) -> Result<Self::Node, Self::Error>;

        /// Empty space the theme's `space` across (contract v7).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn spacer_space(&mut self, space: Space) -> Result<Self::Node, Self::Error>;

        /// A button in `variant` drawn with `look` (contract v8).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn button_look(
            &mut self,
            label: &'static str,
            action: ActionId,
            variant: ButtonVariant,
            look: Look,
        ) -> Result<Self::Node, Self::Error>;

        /// A scroll view of `child` (contract v9).
        ///
        /// # Errors
        ///
        /// The builder's error: for example when it is full.
        fn scroll_view(
            &mut self,
            layout: Layout,
            child: Self::Node,
        ) -> Result<Self::Node, Self::Error>;

        /// A scroll view of `child` along `axes`, of `layout`'s size: a
        /// container whose style names `overflow`, `overflowX` or
        /// `overflowY` (contract v17).
        ///
        /// # Errors
        ///
        /// The builder's error: for example when it is full.
        fn scroll_view_axes(
            &mut self,
            layout: Layout,
            axes: ScrollAxes,
            child: Self::Node,
        ) -> Result<Self::Node, Self::Error>;

        /// A virtual list (contract v18): list `list`, its items
        /// `row_height` high, shown through `rows`, a pool built once.
        ///
        /// # Errors
        ///
        /// The builder's error: for example when it is full.
        fn virtual_list(
            &mut self,
            layout: Layout,
            list: ListId,
            row_height: u16,
            rows: &[Self::Node],
        ) -> Result<Self::Node, Self::Error>;

        /// A modal dialog holding `child` (contract v20), opened and closed
        /// by the application as `layer`, in the middle of the screen,
        /// which it veils.
        ///
        /// # Errors
        ///
        /// The builder's error: for example when it is full or `layer` is
        /// already used.
        fn dialog(&mut self, layer: LayerId, child: Self::Node) -> Result<Self::Node, Self::Error>;

        /// A menu holding `child` beside `anchor` (contract v20), opened by
        /// the application as `layer`.
        ///
        /// # Errors
        ///
        /// As [`dialog`](Self::dialog).
        fn menu(
            &mut self,
            layer: LayerId,
            anchor: Self::Node,
            child: Self::Node,
        ) -> Result<Self::Node, Self::Error>;

        /// A popover holding `child` beside `anchor` (contract v20), opened
        /// by the application as `layer`.
        ///
        /// # Errors
        ///
        /// As [`dialog`](Self::dialog).
        fn popover(
            &mut self,
            layer: LayerId,
            anchor: Self::Node,
            child: Self::Node,
        ) -> Result<Self::Node, Self::Error>;

        /// A tooltip, `text` beside `anchor` (contract v20).
        ///
        /// # Errors
        ///
        /// The builder's error: for example when it is full.
        fn tooltip(
            &mut self,
            anchor: Self::Node,
            text: &'static str,
        ) -> Result<Self::Node, Self::Error>;

        /// A text area (contract v19): several lines a person types into,
        /// bound to `field`, `rows` lines high, up to `max_len` bytes kept
        /// in the tree's text store; empty at first, `placeholder` showing.
        ///
        /// # Errors
        ///
        /// The builder's error: for example when it is full, its text
        /// store has too few bytes left, or `field` is already bound.
        fn text_area(
            &mut self,
            field: FieldId,
            rows: u8,
            max_len: u16,
            placeholder: &'static str,
        ) -> Result<Self::Node, Self::Error>;

        /// One choice of `options` bound to `field`, in a list `picker`
        /// shapes (contract v10).
        ///
        /// # Errors
        ///
        /// The builder's error: for example when it is full, `field` is
        /// already used, or the options or rows are out of range.
        fn list_picker(
            &mut self,
            field: FieldId,
            options: &'static [&'static str],
            picker: Picker,
        ) -> Result<Self::Node, Self::Error>;

        /// The line where the errors the application sets for `field` show
        /// (contract v12).
        ///
        /// # Errors
        ///
        /// The builder's error: for example when it is full, or `field`
        /// already has an error line.
        fn field_error(&mut self, field: FieldId) -> Result<Self::Node, Self::Error>;

        /// The on-screen keyboard (contract v11).
        ///
        /// # Errors
        ///
        /// The builder's error: for example when it is full, or the screen
        /// already has a keyboard.
        fn keyboard(&mut self) -> Result<Self::Node, Self::Error>;

        /// One line of text the application sets while the screen shows,
        /// empty until it does (contract v1).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full or `slot` is
        /// already used.
        fn slot(&mut self, slot: SlotId) -> Result<Self::Node, Self::Error>;

        /// Static text set in `style`'s role and weight (contract v2).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full, or text the
        /// role's glyph atlases do not draw.
        fn text_styled(
            &mut self,
            text: &'static str,
            style: TextStyle,
        ) -> Result<Self::Node, Self::Error>;

        /// A slot between `template`'s literal text, which shows from the
        /// first frame (contract v14); the application sets only its part.
        ///
        /// # Errors
        ///
        /// As `slot`.
        fn slot_template(
            &mut self,
            slot: SlotId,
            template: &'static Template,
        ) -> Result<Self::Node, Self::Error>;

        /// A templated slot set in `style`'s role and weight (contract
        /// v14).
        ///
        /// # Errors
        ///
        /// As `slot`.
        fn slot_template_styled(
            &mut self,
            slot: SlotId,
            template: &'static Template,
            style: TextStyle,
        ) -> Result<Self::Node, Self::Error>;

        /// A slot whose text is set in `style`'s role and weight (contract
        /// v2).
        ///
        /// # Errors
        ///
        /// As `slot`.
        fn slot_styled(
            &mut self,
            slot: SlotId,
            style: TextStyle,
        ) -> Result<Self::Node, Self::Error>;

        /// Paragraphs the application sets at run time, bound to `slot`
        /// and numbered with the slots, up to `capacity` bytes, set in
        /// `style` (contract v16).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full, `slot` is
        /// already used or its text store has too few bytes left.
        fn long_text(
            &mut self,
            slot: SlotId,
            capacity: u16,
            style: TextStyle,
        ) -> Result<Self::Node, Self::Error>;

        /// One of the icons, by its static (contract v6).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn icon(&mut self, icon: &'static icons::Icon) -> Result<Self::Node, Self::Error>;

        /// One of the product's pictures, by its static, fitted by `fit`,
        /// with `alt` saying what it shows ("" for decoration), sized by
        /// `layout`'s width and height, else its own (contract v15).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn image(
            &mut self,
            picture: &'static Picture,
            fit: Fit,
            alt: &'static str,
            layout: Layout,
        ) -> Result<Self::Node, Self::Error>;

        /// A progress bar bound to `slot`, empty until the application
        /// fills it (contract v5).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full or `slot` is
        /// already used.
        fn progress(&mut self, slot: SlotId) -> Result<Self::Node, Self::Error>;

        /// A button in `variant` (contract v3).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn button_with(
            &mut self,
            label: &'static str,
            action: ActionId,
            variant: ButtonVariant,
        ) -> Result<Self::Node, Self::Error>;

        /// A vertical stack of `children` drawn on `surface` (contract v3).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn column_on(
            &mut self,
            surface: Surface,
            layout: Layout,
            children: &[Self::Node],
        ) -> Result<Self::Node, Self::Error>;

        /// A horizontal row of `children` drawn on `surface` (contract v3).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn row_on(
            &mut self,
            surface: Surface,
            layout: Layout,
            children: &[Self::Node],
        ) -> Result<Self::Node, Self::Error>;

        /// `child`, shown only while the application makes `group`
        /// visible: hidden at first, and then taking no space (contract v1).
        /// Groups that share a number show and hide together.
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn group(&mut self, group: GroupId, child: Self::Node) -> Result<Self::Node, Self::Error>;

        /// `when_visible` while the application makes `group` visible, and
        /// `otherwise` while it does not, as at first (contract v1).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn either(
            &mut self,
            group: GroupId,
            when_visible: Self::Node,
            otherwise: Self::Node,
        ) -> Result<Self::Node, Self::Error>;

        /// `child`, shown only while the application does not make `group`
        /// visible: shown at first (contract v13).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn unless(&mut self, group: GroupId, child: Self::Node) -> Result<Self::Node, Self::Error>;

        /// `child`, shown only while the application sets `switch` to the
        /// case `when`: on no case at first (contract v13). Switches that
        /// share a number move together.
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn case(
            &mut self,
            switch: SwitchId,
            when: u8,
            child: Self::Node,
        ) -> Result<Self::Node, Self::Error>;

        /// `when_matched` while the application sets `switch` to the case
        /// `when`, and `otherwise` on any other case or none, as at first
        /// (contract v13).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full.
        fn case_or(
            &mut self,
            switch: SwitchId,
            when: u8,
            when_matched: Self::Node,
            otherwise: Self::Node,
        ) -> Result<Self::Node, Self::Error>;

        /// A single-line text field bound to `field`, made as `input` says
        /// (contract v1).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full, `field` is
        /// already used, or `input`'s `max_len` is out of range.
        fn input(&mut self, field: FieldId, input: Input) -> Result<Self::Node, Self::Error>;

        /// A box the person ticks or unticks, labelled `label`, bound to
        /// `field` (contract v1).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full or `field` is
        /// already used.
        fn checkbox(
            &mut self,
            field: FieldId,
            label: &'static str,
        ) -> Result<Self::Node, Self::Error>;

        /// One choice of `options`, bound to `field` (contract v1).
        ///
        /// # Errors
        ///
        /// The builder's error, for example when it is full, `field` is
        /// already used, or there are no options or too many.
        fn radio_group(
            &mut self,
            field: FieldId,
            options: &'static [&'static str],
        ) -> Result<Self::Node, Self::Error>;
    }
}
