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
  card and a notice in each tone. `notice-board.tsx` wraps text onto
  several lines: static text in a notice and a slot in a card;
  `fuelling.tsx` live figures in slots and a progress bar; `tiles.tsx`
  icons on a header bar and on cards; `order-ready.tsx` text aligned
  to the centre and the end, and the theme's spacing tokens, so
  `make contract-check` draws it in the large-text theme too
  (`order_ready.large.png`); `parking-paid.tsx` every property a
  `style={{…}}` may name, so it is drawn in the large-text theme
  too and its colours checked for contrast in each theme. The three
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
