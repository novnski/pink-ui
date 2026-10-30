# Decision Inbox

`pink-decision-inbox` draws the pieces of PINK's Decision Inbox (issue #266):
the questions and approvals waiting in other chats, answered from Activity.
Like the other packages under `components/` it depends on crates.io
`gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles this same
`src/lib.rs` in place (`src/components/decision_inbox.rs`, the Decision Inbox
story). PINK pins the package and patches `gpui` to its vendored runtime, so
there is one GPUI type graph and no copied source.

The decision being answered opens in the question panel, drawn by
[`pink-question-card`](../question-card/README.md) inside the
[`pink-composer-pill`](../composer-pill/README.md) chrome.

## What it draws

- `card(style, icon, title, status)`: a waiting decision as a `CARD_H` row
  (the transcript's `async_question_card`): the glyph tile, the question cut
  with an ellipsis at `TITLE_MAX_W` or when the row runs out of room, and its
  status taking the rest. `CardStyle` carries the resolved colours: an accent
  edge and status while it waits, a hairline and a muted status once settled.
  Lists keep `CARD_GAP_Y` above and below each card.
- `facts()` and `fact(color, text)`: the wrapped lines under a question, for
  its expiry, a detail the provider did not share, or the engine's refusal.

Every size goes through `ui_rems`, so the pieces grow with the host's
Interface size.

## What the host adds

Ids, roles and accessible names (`Role::Button` on a card, named for the
question and its status), focus rings, keyboard activation, hover fills, the
click that opens a decision, and every word. The package never invents text:
a host words what the engine said, including why a field is unavailable.
