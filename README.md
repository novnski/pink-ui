# affectUI for GPUI

The free components from [affectUI](https://www.affectui.com/components),
rebuilt in Rust for [GPUI](https://gpui.rs), Zed's GPU-accelerated UI
framework. Every component comes in both affectUI looks (**Clean** and
**Classic**) in light and dark, with the same tokens, sizes, copy and motion as
the website.

Like affectUI's "one `.tsx` file per component", each component here is one
Rust file in [`src/components/`](src/components), and its API follows the
builder style of luna's GPUI controls: build it, drop it into `.child(…)`.

```rust
use affectui::components::profile_menu::{ProfileMenu, ProfileSetup};

div().child(
    ProfileMenu::new("account", "Lena Hartmann", "lena@fastmail.com")
        .setup(ProfileSetup::new(3, 5).next("Add a profile photo")),
)
```

## Components

| Category     | Component            | What it is |
| ------------ | -------------------- | ---------- |
| Chat         | Thinking Loader      | A small working pill for AI replies – an eight-ray spinner and a shimmering label that rolls through phases and settles on a check |
| Chat         | Comment Thread       | One comment with replies – an assistant reports what it did, folds open details and answers @mentions |
| Chat         | Message Actions      | The quiet bar under an AI message – copy, regenerate with a version switcher, feedback thumbs and read aloud |
| Agents       | Tool Call Card       | An agent's tool call in one line – live timer, status and summary, folds open to arguments and result |
| Cards        | Email Draft          | Agent-written email for review – switch recipient and tone, edit inline, send or discard with undo |
| Cards        | Smart Recommendation | Suggestion card with a confidence signal, alternatives to switch to and accept with undo |
| Cards        | Tips Panel           | Grid of short tips with one action each – taken tips turn into a done state |
| Data display | Change Summary       | What moved since last time – one metric per row with a colored trend |
| Navigation   | Profile Menu         | Account card with the signed-in user, menu actions and profile setup progress |
| Navigation   | Segmented Control    | Two to six options – a raised pill or underline that glides and can be dragged across |
| Buttons      | Button               | Primary, secondary, ghost and destructive buttons with icons, sizes and a shimmering loading state |
| Buttons      | Reaction Bar         | Like, comment, repost and share in one pill – icons pop and fill, counts roll in digit by digit |
| Inputs       | Filter Menu          | Grouped filters with removable chips, collapsible checkbox groups, Clear all and keyboard control |
| Inputs       | OTP Input            | Verification code field – a gliding focus ring, paste, a calm shake on errors and a morph into Verified |
| Feedback     | Decision Inbox       | Open decisions with two quick answers each – answered ones confirm and fold away |
| Feedback     | Notification Stack   | Notice cards stacked behind each other – dismiss the front one to reveal the next |
| Feedback     | Progress Tracker     | Onboarding checklist with an animated progress bar, ticks that draw themselves and auto play |

affectUI's [Pro components](https://www.affectui.com/pricing) are not included:
their license forbids redistributing them in a component library.

## Running the gallery

```sh
cargo run                                    # every component, like the website
cargo run -- reaction-bar --theme dark       # open one component
cargo run -- reaction-bar --look classic     # the Classic style
cargo run -- --list
```

The header switches light/dark and Clean/Classic; Ctrl+↑/↓ steps through the
components and Ctrl+R replays the current demo.

## Using it in an app

```rust
use gpui::{App, Application, Context, Window, WindowOptions, div, prelude::*};

struct Root;

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        affectui::motion::frame(window, cx); // drives the eased hover fades
        div().size_full().child(/* components */)
    }
}

fn main() {
    Application::new().with_assets(affectui::Assets).run(|cx: &mut App| {
        affectui::init(cx); // theme, bundled Inter font, text input keys
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

MIT. The components port affectUI's free items (MIT, © Florian Höller); see
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for them, GPUI, luna-ui and the
bundled fonts.
