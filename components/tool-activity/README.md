# Tool activity header

This independently consumable package is the bounded bridge for PINK #123.
The gallery remains on gpui-pre; native PINK remains on its patched GPUI.
Hosts pin `pink-tool-activity` from this repository and patch crates.io `gpui`
to their existing runtime. There is one GPUI type graph, no source copying,
and no global theme/input migration. This bridge does not complete PINK #119.

`ActivitySummary` accepts factual counts and an explicit lifecycle. Recorded
means records settled, never that the user's goal succeeded. Reasoning is
excluded from executable totals. Failures remain visible during running work.
The host owns tool classification, persisted event order, disclosure state,
accessibility semantics and actions; `activity_header` owns layout and styling.
Use `description()` in the control's accessible name. Supply icons and title
as elements so the host retains its assets and reduced-motion shimmer.

Failure inventory, before implementation: truncating titles hide failures;
many concurrent calls imply an invented sequence; reasoning counts as commands;
settled records claim successful accomplishments; running updates change height;
library-owned state leaks disclosure choices between chats. The component
has no event mutation, timers, stored state or auto-expand policy.

The native PINK integration owns behavioral verification: real live tool
events, concurrent calls, failure, settlement, expansion and history reload,
light/dark and narrow layouts, keyboard/focus, and retained screenshots/readback.
Compile and lint through that host's guarded locked Cargo plan, which proves
the patched runtime integration. Run `rustfmt --edition 2024 --check src/lib.rs`
here for standalone formatting. No test-only production seam is introduced.
