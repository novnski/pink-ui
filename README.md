# affectUI for GPUI

Every component from [affectUI's component page](https://www.affectui.com/components),
rebuilt in Rust for [GPUI](https://gpui.rs), Zed's GPU-accelerated UI
framework – 56 components, each one file:

- **17 ports** of affectUI's free components, in both affectUI looks
  (**Clean** and **Classic**), light and dark, with the same tokens, sizes,
  copy and motion as the website.
- **39 originals** in luna's design language, built around the ideas behind
  affectUI's Pro components (which can't be redistributed).

![The gallery](docs/images/gallery-light.png)

Like affectUI's "one `.tsx` file per component", each component here is one
Rust file in [`src/components/`](src/components), and its API follows the
builder style of luna's GPUI controls: build it, drop it into `.child(…)`.
State lives in the component (keyed by its id), and callbacks report what
the user did.

```rust
use affectui::components::profile_menu::{ProfileMenu, ProfileSetup};

div().child(
    ProfileMenu::new("account", "Lena Hartmann", "lena@fastmail.com")
        .setup(ProfileSetup::new(3, 5).next("Add a profile photo")),
)
```

## Components

![Every component in light mode](docs/images/components-light.png)

### Ports of affectUI's free components

Faithful ports of the 17 free (MIT) components, in both affectUI looks (Clean
and Classic) and both themes, matched against the website screenshot by
screenshot.

| Category | Component | What it is |
| -------- | --------- | ---------- |
| Chat | Thinking Loader | A small working pill for AI replies – an eight-ray spinner and a shimmering label that rolls through phases and settles on a check |
| Chat | Comment Thread | One comment with replies – an assistant reports what it did, folds open details and answers @mentions |
| Chat | Message Actions | The quiet bar under an AI message – copy turns into a check, regenerate spins and unfolds a version switcher, thumbs pop and ask why, read aloud becomes a tiny equalizer |
| Agents | Tool Call Card | An agent's tool call in one line – live timer, status and summary, folds open to highlighted arguments and result with copy, retry on errors and a group that sums them up |
| Cards | Email Draft | Agent-written email for review – switch recipient and tone, edit inline, remove the attachment, send or discard with undo |
| Cards | Smart Recommendation | Suggestion card with a confidence signal, alternatives to switch to and accept with undo |
| Cards | Tips Panel | Grid of short tips with one action each – taken tips turn into a done state |
| Data display | Change Summary | What moved since last time – one metric per row with a colored trend |
| Navigation | Profile Menu | Account card with the signed-in user, menu actions and profile setup progress |
| Navigation | Segmented Control | Switch between two to six options – a raised pill or underline that glides, stretches a little and can be dragged across, with icons, counts and badges |
| Buttons | Button | Primary, secondary, ghost and destructive buttons with icons, sizes and a shimmering loading state |
| Buttons | Reaction Bar | Like, comment, repost and share in one pill – icons pop and fill, counts roll in digit by digit |
| Inputs | Filter Menu | Grouped filters with removable chips, collapsible checkbox groups, Clear all and full keyboard control |
| Inputs | OTP Input | The verification code field done right – a gliding focus ring, paste and SMS autofill, a calm shake on errors and a morph into Verified |
| Feedback | Decision Inbox | Open decisions with two quick answers each – answered ones confirm and fold away |
| Feedback | Notification Stack | Notice cards stacked behind each other – dismiss the front one to reveal the next |
| Feedback | Progress Tracker | Onboarding checklist with an animated progress bar, ticks that draw themselves and auto play |

### Originals in luna style

affectUI's 39 Pro components are commercially licensed, and their terms forbid
redistributing them in a component library. For each one we took only the
idea – what it is for and how its motion feels – and designed our own
component in luna's design language (Geist, luna's neutral surfaces and
indigo accent, its radius/spacing ladders and control kit), with our own
layout, copy, demo data and icons. Originals follow light/dark; the
Clean/Classic look applies to ports only.

| Category | Component | What it is |
| -------- | --------- | ---------- |
| Chat | AI Answer | A streamed answer card: sources stack up while it searches, the reply writes itself word by word with numbered citations that preview their source, then copy, retry, rating and follow-up questions settle in underneath |
| Chat | Chat Composer | A prompt box with file cards, rotating example prompts, web search and ask-first toggles, a keyboard-driven model menu, voice input with a live level meter and a project-scoped footer |
| Chat | Thinking State | A live working panel: a ticking timer and a filling hairline over steps that rise in one by one, the files each step read popping in as chips, folding to a one-line summary when the work is done |
| Chat | Writing Assistant | A rewrite companion for selected text: pick a direction, type or dictate an instruction, and watch the new version stream in over a drifting glow, then step through versions and replace the selection |
| Chat | Cited Answer | An answer wired to its evidence: point at a citation to light its sentence and its source, click it to unfold the quoted passage beside the answer with the supporting words marked |
| Chat | Comments Panel | A document's comment sidebar: anchored notes, slash-command comments that run as edits with a before/after preview and undo, resolving that folds comments away, and a composer that completes commands |
| Chat | Model Picker | A composer's model menu with a live spec sheet – point at a model to glide its meters and roll its latency and price, then unfold a reasoning budget |
| Agents | Agent Cursors | Agents edit a shared document like teammates – named pointers glide to their spot, type suggestions letter by letter, resize a figure, fix a config and leave margin notes to accept or dismiss |
| Agents | Agent Inbox | One calm list for every background agent task – urgent work floats up, counts roll as states change, and each row unfolds into its last steps and the right next action |
| Agents | Agent Lanes | A live column of time for each agent – work grows down into now, hand-offs link one agent's output to the next, and approvals wait right on the block |
| Agents | Agent Plan | Shape an agent's plan before it starts – rename, reorder, drop or add steps – then follow the run step by step and pick it up again from a failed step |
| Agents | Agent Steering | Steer a running agent: queue a note for its next step and watch the plan carry it into the work, or interrupt to apply it at once |
| Agents | Agent Tree | A multi-agent run as a live outline – status, spend and runtime on every agent, branch costs rolled up, folding branches and an inspector for the selected agent |
| Agents | Approval Card | An agent pauses for sign-off before a risky action – a risk meter, a preview of exactly what will run, shortcuts, a countdown that holds while you read, and a quiet resolved record with undo |
| Agents | Autonomy Dial | A rotary dial for how freely an agent may act – turn it and the agent's actions glide between an unattended lane and a needs-approval lane, with locks for actions that must always wait for you |
| Agents | Context Window | A model's context as a field of cells, one per slice of tokens – hover a part to light up its cells, detach files, and summarize the chat history while its cells ripple and clear |
| Agents | Delegate Board | Assign work by dropping tasks on agent columns – each column previews its new free-at time before you let go, and Auto hands new work to whoever frees up first |
| Agents | Inline Diff | An agent's edits marked word by word inside the code – step through them and keep or drop each one with a floating rail, Y / N and undo |
| Agents | Memory Panel | Everything the assistant remembers about you, by category – pin, edit in place or forget each memory with Undo in the tray, add your own, or pause memory with one switch |
| Agents | Permission Request | An agent asks for one exact right and spells out the commands it will run – grant it once, for an hour or for good, and it folds into a live grant line with a countdown and Revoke |
| Agents | Trace Waterfall | An agent run laid out on a live timeline – every sub-agent, model call and tool call as a bar with its duration and cost, the slowest step flagged, branches that fold and a readout for the step under the pointer |
| Cards | Project Card | A two-line project summary that unfolds on hover into a status picker, a people filter and its subtasks – all linked to a progress ring with one segment per subtask |
| Data display | Breakdown Chart | Stacked columns you set up in a sentence – measure, slice and split – with a floating breakdown on hover and a linked, sortable table |
| Data display | Compact Breakdown | A total split into ranked parts around a ring; switching the period glides the segments, rolls the numbers and reorders the rows |
| Data display | Data Table | A compact spreadsheet: range selection with a gliding cursor, typed in-place editing, tab-separated copy and paste, undo and redo, and sorting that slides the rows |
| Data display | Experiment Card | An A/B test folded into two lines that unfolds on hover into lift trends and confidence intervals per variant, with a metric switch and a two-step ship |
| Data display | KPI Chart | A headline metric over a multi-series trend: a crosshair walks the days, the number rolls with it and a readout splits it by series |
| Data display | Usage Meter | A billing cycle at a glance: credits by category with a projection to cycle end, a burn-up chart, plan limits and a call to action that speaks up before the allowance runs out |
| Navigation | Command Menu | One field to search and run everything – app actions on numbered tiles, everyday commands in groups, scopes on Tab and matched letters lit as you type |
| Navigation | Integration Picker | The integrations page of a command menu – search the services, open one to see your workspace and it joined on two tiles, then connect it and choose what it may do |
| Inputs | Billing Schedule | Compose a billing plan from line items with their own cadence, a discount and a monthly minimum, and watch the invoice schedule and totals update live |
| Inputs | Combobox | Pick one or many from a searchable, grouped list – avatar chips in the field, matched letters lit, Backspace to unpick and new entries created as you type |
| Inputs | Date Range Picker | Two months side by side with preset chips, a live preview band while you pick, an optional comparison period and full keyboard control |
| Inputs | Dropzone | A drop target for files from the desktop with per-file type tiles, upload speed and time left, progress rings and retry on failure |
| Inputs | Integration Settings | Settings card for a connected service: a switch that pauses it, a token you can reveal and rotate, a validated address, sync cadence, time zone and a save bar that rises when something changes |
| Inputs | Prompt Library | Saved prompt templates you fill in place – type into the blanks of the sentence, watch the prompt assemble below and use it once nothing is missing |
| Inputs | Stepper Form | A multi-step setup flow with a vertical rail that checks steps off, steps that slide in, a card that resizes to each step, inline validation and a success screen |
| Feedback | Setup Guide | Onboarding checklist with a progress ring, accordion task groups and tasks that strike through as they get done |
| Feedback | Toaster | A stack of quiet notifications that spreads open on hover, pauses its timers, morphs promise toasts into their result and flicks away with a drag |

## Running the gallery

```sh
cargo run                                    # every component, like the website
cargo run -- reaction-bar --theme dark       # open one component
cargo run -- reaction-bar --look classic     # the Classic style
cargo run -- --list
```

The header switches light/dark (and Clean/Classic for ports); Ctrl+↑/↓ steps
through the components and Ctrl+R replays the current demo. Dark mode:

![The gallery in dark mode](docs/images/gallery-dark.png)

## Using it in an app

```rust
use gpui::{App, Application, Context, Window, WindowOptions, div, prelude::*};

struct Root;

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        affectui::motion::frame(window, cx); // advances hover fades and glides
        div()
            .size_full()
            .child(/* components */)
            .child(affectui::motion::driver()) // keeps frames coming while they move
    }
}

fn main() {
    Application::new().with_assets(affectui::Assets).run(|cx: &mut App| {
        affectui::init(cx); // theme, bundled fonts, text input keys
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Root)).unwrap();
    });
}
```

- **Theme**: `Theme::set_mode(ThemeMode::Dark, cx)` and
  `Theme::set_look(Look::Classic, cx)` switch every component; `.look(..)` on a
  component overrides it for that instance.
- **Motion** ([`src/motion.rs`](src/motion.rs)): CSS `cubic-bezier` curves,
  transition specs, entrance helpers and eased hover fades. Hovers ease through
  the [`HoverFill`](src/interaction.rs) trait (`.hover_fill(..)`,
  `.hover_fill_fg(..)`), like the CSS transitions on the web.
- **Text input** ([`src/primitives/text_input.rs`](src/primitives/text_input.rs)):
  single- and multi-line fields with IME, selection, clipboard and word movement.
- **luna kit** ([`src/luna.rs`](src/luna.rs)): luna's tokens (`Luna::get(cx)`),
  radius/spacing/type ladders, Geist, and controls (`button`, `icon_button`,
  `chip`, `badge`, `kbd`, `card`, `overlay`) – what the originals are built from.
- **Fonts**: Inter (ports), Geist and Geist Mono (originals) and JetBrains Mono
  are bundled and registered by `init`.

## Development

GPUI on Linux needs the X11/Wayland and Vulkan development packages:

```sh
sudo apt install libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev \
  libx11-xcb-dev libxcb1-dev libfontconfig-dev libfreetype-dev
```

`Cargo.lock` pins `libc` to 0.2.189: newer releases dropped `ENOATTR`, which
breaks the `xattr` crate that GPUI's HTTP client pulls in. Keep the pin when
updating dependencies.

`scripts/screenshot.sh <slug> [light|dark] [out.png] [WxH] [clean|classic]`
renders a component headlessly (Xvfb, openbox, xdotool, ImageMagick) for
side-by-side checks against the website. See [CONTRIBUTING.md](CONTRIBUTING.md)
for how a component is ported.

## License

MIT. The ports reproduce affectUI's free items (MIT, © Florian Höller); the
originals are our own designs and contain no affectUI Pro code or styling. See
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for affectUI, GPUI, luna-ui
and the bundled fonts.
