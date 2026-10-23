# Third-party notices

## Components derived from adfects

Seventeen components in `src/components/` started as ports of MIT-licensed web
components by Florian Höller: Button, Change Summary, Comment Thread, Decision
Inbox, Email Draft, Filter Menu, Message Actions, Notification Stack, OTP Input,
Profile Menu, Progress Tracker, Reaction Bar, Segmented Control, Smart
Recommendation, Thinking Loader, Tips Panel and Tool Call Card. They are now
drawn in Pink's design language, but their copy, demo content and parts of
their behavior still come from that work, as do the avatar pictures
`assets/icons/comment-thread/lena.svg` and `sofia.svg`. It is used under the
following license:

```
MIT License

Copyright (c) 2026 Florian Höller (adfects)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

The demos show other companies' names and logos as sample content (e.g.
GitHub, Slack, Stripe). Those marks belong to their owners.

## GPUI

`src/primitives/text_input.rs` is adapted from GPUI's `examples/input.rs`
([zed-industries/zed](https://github.com/zed-industries/zed)), licensed under the
Apache License 2.0.

## Pink app UI crate

`src/pink.rs`, `src/pink/`, `src/motion.rs` and `src/interaction.rs` are
adapted from the UI and theme crates of the
[Pink app](https://github.com/novnski/pink) (MIT), by the same author: its
theme tokens and color math, controls, popovers, loaders, motion and
interaction modules. Components that reproduce a part of the app follow its
source, which each one names in its doc comments.

## Icons

Most SVG icons in `assets/icons/` are copied from the Pink app's icon set, or
drawn from the same sets where Pink does not bundle a glyph:

- Solar Icons (Linear and Bold) by 480 Design, licensed under
  [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Attribution:
  "Solar Icons by 480 Design". Some are recolored or have their stroke widths
  adjusted to sit at pink-ui's sizes.
- File-type icons (`file-*.svg` in `dropzone/` and `thinking-state/`) from
  [Symbols](https://github.com/miguelsolorio/vscode-symbols) by Miguel Solorio,
  MIT License (`assets/icons/licenses/LICENSE.symbols`).
- `git-pull-request.svg` and `webhook.svg`: [Lucide](https://lucide.dev), ISC
  License (`assets/icons/licenses/LICENSE.lucide`).
- `arrow-fork.svg`: [Tabler Icons](https://tabler.io/icons), MIT License
  (`assets/icons/licenses/LICENSE.tabler`).
- `mcp-mark.svg`: the Model Context Protocol mark from
  [Simple Icons](https://simpleicons.org), CC0 1.0. The mark itself belongs to
  its owner.

The rest (a few line glyphs in the same style, and the fictional model brand
marks) are drawn for pink-ui.

## Fonts

- `assets/fonts/Geist*.ttf` – [Geist](https://github.com/vercel/geist-font),
  SIL Open Font License 1.1 (`assets/fonts/Geist-OFL.txt`), as used by the Pink app.
- `assets/fonts/Inter-*.ttf` – [Inter](https://rsms.me/inter/) by Rasmus Andersson,
  SIL Open Font License 1.1 (`assets/fonts/Inter-OFL.txt`), used by the gallery
  and site chrome.
- `assets/fonts/JetBrainsMono-*.ttf` – [JetBrains Mono](https://www.jetbrains.com/lp/mono/),
  SIL Open Font License 1.1 (`assets/fonts/JetBrainsMono-OFL.txt`).
