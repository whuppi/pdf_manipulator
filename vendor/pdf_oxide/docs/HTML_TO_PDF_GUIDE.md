# HTML + CSS → PDF — User Guide

**Available in:** v0.3.35+ (issue [#248](https://github.com/yfedoseev/pdf_oxide/issues/248)).

pdf_oxide v0.3.35 ships a pure-Rust HTML+CSS→PDF pipeline. Pass an
HTML string + optional CSS + a font file, get a paginated PDF back.
No external browser binary, no headless Chromium, no MPL
dependencies — all MIT/Apache.

## Quick start

```rust
use pdf_oxide::api::Pdf;

let font = std::fs::read("DejaVuSans.ttf")?;
let mut pdf = Pdf::from_html_css(
    r#"<h1>Hello</h1><p>This is a styled PDF.</p>"#,
    "h1 { color: blue; font-size: 24pt }",
    font,
)?;
pdf.save("out.pdf")?;
```

```python
import pdf_oxide
font_bytes = open("DejaVuSans.ttf", "rb").read()
pdf = pdf_oxide.from_html_css(
    "<h1>Hello</h1><p>World</p>",
    css="h1 { color: blue }",
    font=font_bytes,
)
pdf.save("out.pdf")
```

## Supported CSS surface

The v0.3.35 plan committed to "Blink minus animations, filters,
masks, 3D transforms" with explicit cuts in the R1 risk register.
What landed:

### Selectors (CSS Selectors L3 + L4 subset)

- Type, universal `*`, class `.x`, id `#x`
- Attribute selectors: `[a]`, `[a=v]`, `[a~=v]`, `[a|=v]`, `[a^=v]`,
  `[a$=v]`, `[a*=v]`, with `i`/`s` case flags
- Combinators: descendant, child `>`, next-sibling `+`,
  subsequent-sibling `~`
- Structural pseudo-classes: `:root`, `:first-child`, `:last-child`,
  `:only-child`, `:first-of-type`, `:last-of-type`, `:only-of-type`,
  `:nth-child(An+B)`, `:nth-last-child`, `:nth-of-type`,
  `:nth-last-of-type`, `:empty`
- Logical: `:is(...)`, `:where(...)`, `:not(...)`, `:has(...)`
- Pseudo-elements: `::before`, `::after`, `::first-line`, `::first-letter`
- Specificity per CSS Selectors L3 §16

### Box model & layout

- `display`: `block`, `inline`, `inline-block`, `flex`, `grid`,
  `table`, `list-item`, `none`, `contents`, all internal table parts
- `position`: `static`, `relative`, `absolute`, `fixed`, `sticky`
- `width`, `height`, `min-*`, `max-*` in every unit (px/pt/pc/in/cm/mm/em/rem/ex/ch/vw/vh/vmin/vmax/%)
- Margins (incl. `auto`), padding, border, border-radius, box-sizing
- Block + flex + grid via Taffy (block, flex, grid features)
- Inline formatting with UAX #14 line breaks via `unicode-linebreak`
- `text-align`: `left`/`right`/`center`/`justify`/`start`/`end`
- `white-space`: `normal`/`nowrap`/`pre`/`pre-wrap`/`pre-line`
- Float / clear scaffolding (line-shortening data path lands; full
  float-aware wrapping in v0.3.36)
- Margin collapsing per CSS 2.1 §8.3.1
- Multi-column (`column-count`/`column-width`/`column-gap`)
- Table layout (auto + fixed algorithms)

### Typography & colour

- `font-family` (multi-family fallback), `font-size`, `font-weight`
  (numeric + keywords), `font-style`
- `line-height`, `letter-spacing`, `word-spacing`, `text-indent`
- `text-decoration`, `text-transform`
- ~150 named colours, `#rgb`/`#rrggbb`/`#rgba`/`#rrggbbaa` hex,
  `rgb()`/`rgba()`/`hsl()`/`hsla()`, `transparent`, `currentColor`

### `calc()` and custom properties

- `calc()` / `min()` / `max()` / `clamp()` with full arithmetic and
  mixed-unit support
- `var(--name, fallback)` with cycle detection

### At-rules

- `@media print` (always true) and `@media (min/max-width|height: …)`
- `@page { size; margin; … }` plus `:first` / `:left` / `:right` /
  `:blank` selectors and margin boxes
- `@font-face` with `local()` and `url(...) format(...)` sources
- `@import` (URLs forwarded; local files resolved)
- `@supports (property: value)` evaluated against our supported set
- `@keyframes` parsed and ignored (no animations in paged output)

### Content + counters

- `::before` / `::after` with `content:`
- `counter()`, `counters()` with full styles (decimal, decimal-leading-
  zero, lower/upper roman, lower/upper alpha, lower-greek, disc/circle/
  square)
- `counter-reset` / `counter-increment` / `counter-set`
- `attr()` reference

## Pagination

Default page is A4 portrait at 96dpi reference pixels with 20mm
margins. Letter (8.5×11) + custom sizes via `@page { size: ... }`
in CSS or programmatically:

```rust
// Currently exposed via the lower-level layout::paginate::PageConfig;
// a fluent override on Pdf::from_html_css lands in v0.3.36.
```

## Fonts

v0.3.35 requires the caller to supply font bytes via the third
argument to `from_html_css`. The font is registered as the body font
and used for every text box. Multiple fonts (one per family) is a
v0.3.36 follow-up that wires into FONT-4's `SystemFontDb`.

Every glyph the document uses goes through Identity-H encoded `Tj`
operators with a ToUnicode CMap, so `extract_text` round-trips
byte-equal. Latin, Cyrillic, Greek, Hebrew, Arabic, and CJK all
work end-to-end (CJK requires a CJK-coverage font).

## Out of scope (cut list per the v0.3.35 plan)

These are deferred — the pipeline accepts the syntax but ignores or
approximates the behaviour:

- CSS filters (`blur()`, `drop-shadow()`, …) — `opacity()` aliases
  to `opacity` and works
- 3D transforms (`translate3d`, `rotateX/Y/Z`, `perspective`)
- Animations + transitions
- SVG-in-HTML (every viable Rust SVG crate is MPL — out of our deny
  list)
- MathML
- `hyphens: auto`
- `shape-outside`
- Subpixel positioning + hinting
- CSS regions, exclusions, scroll-snap, container queries,
  `view-transitions`, `@property`, `@counter-style`, `@layer`
- JavaScript execution (paged output; for JS-driven content,
  `pdf_oxide` is integrating with the sibling
  [`browser_oxide`](https://github.com/nicepkg/browser_oxide) project
  in a future release)

## Escape hatches

For documents needing rendering outside our supported surface:

- **WeasyPrint** ([weasyprint.org](https://weasyprint.org)) — the
  gold standard for HTML→PDF with comprehensive CSS Paged Media
  support (running headers, named pages, footnotes via GCPM).
- **Chromium print-to-PDF** via Playwright / Puppeteer / chromiumoxide
  — perfect web fidelity at the cost of a ~150MB browser binary.

## Roadmap

v0.3.36 will add:
- Float-aware inline wrapping (`shape-outside` still out)
- BiDi via `unicode-bidi` for proper Arabic/Hebrew visual order
- `@font-face` URL fetching (gated behind a `net` feature)
- Background gradients via PDF ShadingType 2/3
- `box-shadow` via PDF soft masks
- Multi-font cascade (one font per family, picked from `font-family`
  list against the caller's registered set)
- CJK shipping fonts (Noto Sans CJK opt-in feature crate)

Tracking: [issue #248](https://github.com/yfedoseev/pdf_oxide/issues/248).
