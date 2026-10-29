# Tool activity

This independently consumable package draws PINK's tool activity (UI rethink
D63): one verb-first sentence per call or group (`Ran pwd`, `Edited 2 files`),
its disclosure header on a 28 pt pitch at 14/20, and the failure line under a
failed call. The gallery compiles the package's own source unchanged against
its GPUI; native PINK pins it and patches crates.io `gpui` to its existing
runtime. There is one GPUI type graph and no source copying.

The host owns every fact: tool classification, the sentence's wording and
tense, persisted event order, lifecycle, disclosure state, accessibility
semantics and actions. The package owns layout and styling only:

- `sentence` lays a sentence's runs on one line box. `Text` runs (the verb)
  never shrink; `Code` runs draw as inline code in the host's mono font and
  fill; `Object` runs use the primary tone. Code and objects truncate first.
- `activity_header` draws glyph, sentence, failure count, an attention state
  word and chevron. `ActivityState::label` is `None` for running, thinking and
  settled records: the sentence's verb carries those, and a settled record
  never claims the user's goal succeeded. `ActivitySummary::description`
  gives the accessible name in drawn order.
- `failure_line` shows what the tool reported with the host's recovery
  actions (`recovery_action`) after it; the message truncates, the actions
  never do.

Failure inventory, before implementation: a running, failed or stopped call
reads as done; a failure shows only a red count; a long command pushes the
failure count or state off the row; inline code is indistinguishable from
prose; a state word repeats what the sentence says; the header or failure
line changes height when a run settles; library-owned state leaks
disclosure choices between chats. The component has no event mutation,
timers, stored state or auto-expand policy.

`tests/tool_activity.rs` pins the geometry at Interface sizes 12, 16 and 20.
The native PINK integration owns behavioural verification: real tool events
across running, failed, stopped and settled calls, light and dark themes,
keyboard and accessibility readback.
