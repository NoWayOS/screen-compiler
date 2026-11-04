# Fixtures

Synthetic React/TSX screens written for Screen Compiler's tests. They are generic
examples only: no screen, text or data here comes from any product or
customer, and none may be added from one.

* `*.tsx` — screens inside the subset. Each must convert; the generated Rust
  is kept in `crates/screen-compiler-codegen-rust/tests/snapshots/`, and compiled
  and run by `crates/screen-compiler-codegen-rust/tests/generated_code.rs`.
  `parcel-status.tsx` uses subset v1's slots and conditional rendering, and
  `visit-booking.tsx` its form controls: text inputs with a length limit and
  an input mode, a radio group and a checkbox. `details-form.tsx` gives each
  field a line for its errors, which `make contract-check` sets in
  the shown frame. `queue-ticket.tsx` uses the
  type scale's roles and Bold, with a slot in the display role, and
  `check-in.tsx` the button variants and surfaces: a header bar, a
  card and a notice in each tone; `check-in-parts.tsx` is the same screen
  built of its own components and a fragment, expanded at build time, and
  must convert and draw exactly as `check-in.tsx` does (its approved image
  is a copy of check-in's), and `check-in-files.tsx` too, built of parts
  imported from `parts/visitor.tsx` and its design tokens from
  `parts/theme.tsx` - files of parts, not screens, which the fixture
  loops do not check alone; `order-summary.tsx` has lists: steps from a
  `const` array, expanded at build time, and up to four order lines from a
  prop, rows the application shows and fills (`order_summary.shown.png`);
  `bike-hire.tsx` has contract v13's conditions: a note shown while a prop
  is not set, and a step prop compared with three literals, one switch whose
  cases `make contract-check` draws one by one (`bike_hire.case-step-pay.png`
  and the others); `table-ready.tsx` has contract v14's text mixing
  literal words with one prop, in JSX and as a template literal, in the
  heading, body and display roles; `form-controls.tsx`, once refused, has
  converted since text could mix literals and a prop. `notice-board.tsx` wraps text onto
  several lines: static text in a notice and a slot in a card;
  `fuelling.tsx` live figures in slots and a progress bar; `tiles.tsx`
  icons on a header bar and on cards; `message.tsx` layers (contract v20) - a menu, a tooltip, a
  popover and a dialog - which `make contract-check` draws open one at a time; `feedback.tsx` a text area (contract v19), which
  `make contract-check` fills with two paragraphs in its shown frame; `inbox.tsx` a virtual list (contract v18) through a pool of twelve rows,
  which `make contract-check` gives a thousand items; `panels.tsx` containers that scroll (contract v17): a side panel down, a row of
  tiles across and a grid both ways; `article.tsx` a long text (contract v16), which `make contract-check`
  fills with `article.txt`, a synthetic 10 KB article; `photo-card.tsx` pictures (contract
  v15) - NoWayOS's samples, which `pictures.names` lists and every
  conversion here is given - covering a box, at their own size and
  contained; `order-ready.tsx` text aligned
  to the centre and the end, and the theme's spacing tokens, so
  `make contract-check` draws it in the large-text theme too
  (`order_ready.large.png`); `parking-paid.tsx` every property a
  `style={{…}}` may name, so it is drawn in the large-text theme
  too and its colours checked for contrast in each theme;
  `parking-paid-css.tsx` is the same screen written as CSS writes it,
  lengths in px and colours as `var(--name)`, and must draw exactly as it
  does (its approved images are copies of parking-paid's). The three
  `lost-property-*.tsx` are the steps of one invented form, which together
  use every control the UI core lists for form-heavy kiosks: the
  screens subset v1 is designed against. Each moved up from `unsupported/`
  once it converted: the review step with the scroll view, the item
  step with the list picker and the contact step with the on-screen
  keyboard, which `make contract-check` draws in each of its
  layouts (`contact_step.keys-digits.png` and the others).
* `unsupported/*.tsx` — screens using constructs outside the subset. Each must
  be refused; the expected diagnostics are in
  `crates/screen-compiler-analyse/tests/snapshots/`.

After adding or changing a fixture, run `make bless`, review the snapshot
diff, and (for a new convertible fixture) add its `include!` module to
`generated_code.rs`. Then run `make bless-images` and look at its approved
images in `contract-check/golden/` (with groups or slots, also
`SCREEN.shown.png`) before committing them. `make fixtures` runs
the command-line tool on all of them.
