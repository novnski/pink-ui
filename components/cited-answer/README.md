# Cited Answer

`pink-cited-answer` draws the pieces of PINK's cited answers (issue #371): a
web-research reply beside the sources it rests on. Like the other packages
under `components/` it depends on crates.io `gpui = "=0.2.2"` in its own
workspace. The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/cited_answer.rs`, the Cited Answer story). PINK pins the
package and patches `gpui` to its vendored runtime, so there is one GPUI type
graph and no copied source.

PINK's transcript already draws the web search and fetched pages behind an
answer as its tool group ("Searched the web, fetched 4 pages"), so the gallery
AI Answer's tool group needs no package of its own.

## What it draws

- `marker_font(family)` and `marker_text(numbers)`: the superscript citation
  numbers that end a cited span (OpenType `sups` at medium weight, several
  numbers thin-spaced by `CITE_GAP`). The host shapes them into its own text
  runs.
- `heading(style, title, count)`: the Sources column's `HEADING_H` heading
  row with its count trailing.
- `card(style)` and `header(style, tile, title, meta, chevron)`: a source's
  r10 chip card and its two-line `HEADER_H` header. `Tile::Number` holds the
  number its citations carry; `Tile::Icon` marks a source the provider listed
  without saying which part of the reply it supports. The optional chevron
  turns with the fold.
- `quote(style, passage)`: the folded body, the quoted passage in the
  transcript's blockquote at the 12/18 tool-detail size. The host marks the
  backing words inside its own text element.
- `preview(style, mono, number, site, title, snippet)`: the `PREVIEW_W`
  quoted-passage hover card a pointed-at citation shows.

Every size goes through `ui_rems`, so the pieces grow with the host's
Interface size.

## What the host adds

Ids, roles and accessible names (`Role::Heading` on the heading,
`Role::Button` with the expanded state on a header), focus rings, keyboard
activation, hover state and every blended colour, the fold animation, the
span wash under a pointed-at citation, tooltips, the copy and open actions,
and every word. The package never invents text: a host shows only the titles,
sites and passages its provider actually returned.
