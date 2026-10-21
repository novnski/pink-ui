//! Integration Picker – the integrations page of a command menu.
//!
//! An original in luna's design language. A searchable sidebar splits the
//! services into Connected and Available; the picked one fills the detail
//! pane, where your workspace and the service sit on two tiles joined by a
//! dotted link. Connecting runs a short handshake – the link pulses, the
//! badge rolls over and the service glides into the Connected group – and
//! each permission can be granted or withheld on its own.
//!
//! Keys: type to filter, ↑/↓ to browse, Enter to connect or disconnect,
//! Escape to clear the search (or go back when it is empty).
//!
//! ```ignore
//! IntegrationPicker::new("integrations", "Orbit", default_integrations())
//!     .on_connect(|service, connected, _, _| println!("{} → {connected}", service.name))
//! ```

use std::{collections::HashMap, rc::Rc, time::Duration};

use gpui::{
    Animation, AnimationExt, AnyView, App, Context, ElementId, Entity, Focusable, FontWeight, Hsla,
    IntoElement, ScrollHandle, SharedString, Subscription, Task, Transformation, Window, div,
    percentage, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, Tone, radius, text},
    motion::{self, EASE, EASE_OUT_QUINT, MotionSpec, ROW_IN, VIEW_IN},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Integration Picker";
pub const DESCRIPTION: &str = "The integrations page of a command menu – search the services, open one to see your workspace and it joined on two tiles, then connect it and choose what it may do";

const ICON_BACK: &str = "icons/integration-picker/back.svg";
const ICON_SEARCH: &str = "icons/integration-picker/search.svg";
const ICON_CHECK: &str = "icons/integration-picker/check.svg";
const ICON_LINK: &str = "icons/integration-picker/link.svg";
const ICON_ORBIT: &str = "icons/integration-picker/orbit.svg";
const ICON_SPINNER: &str = "icons/integration-picker/spinner.svg";
const ICON_EXTERNAL: &str = "icons/integration-picker/external.svg";
const ICON_SHIELD: &str = "icons/integration-picker/shield.svg";
const ICON_CLOCK: &str = "icons/integration-picker/clock.svg";

const WIDTH: f32 = 728.;
const SIDEBAR_W: f32 = 264.;
const ROW_H: f32 = 36.;
const HEADER_H: f32 = 28.;
const LIST_PAD: f32 = 6.;
const BODY_H: f32 = 376.;
const DOTS: usize = 8;
/// How long the demo handshake takes.
const HANDSHAKE: Duration = Duration::from_millis(1400);

type ConnectHandler = Rc<dyn Fn(&Integration, bool, &mut Window, &mut App)>;

/// Something the integration may do once connected.
#[derive(Clone, Debug)]
pub struct Permission {
    pub label: SharedString,
    pub detail: SharedString,
    /// Granted by default.
    pub granted: bool,
}

impl Permission {
    pub fn new(label: impl Into<SharedString>, detail: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            granted: true,
        }
    }

    /// Withheld until the user grants it.
    pub fn optional(mut self) -> Self {
        self.granted = false;
        self
    }
}

#[derive(Clone, Debug)]
pub struct Integration {
    pub id: SharedString,
    pub name: SharedString,
    pub category: SharedString,
    pub domain: SharedString,
    pub about: SharedString,
    pub permissions: Vec<Permission>,
    pub connected: bool,
    /// Shown under a connected service, e.g. "Synced 4 min ago".
    pub status: Option<SharedString>,
}

impl Integration {
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        category: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            category: category.into(),
            domain: SharedString::default(),
            about: SharedString::default(),
            permissions: Vec::new(),
            connected: false,
            status: None,
        }
    }

    pub fn domain(mut self, domain: impl Into<SharedString>) -> Self {
        self.domain = domain.into();
        self
    }

    pub fn about(mut self, about: impl Into<SharedString>) -> Self {
        self.about = about.into();
        self
    }

    pub fn permission(mut self, permission: Permission) -> Self {
        self.permissions.push(permission);
        self
    }

    pub fn connected(mut self, status: impl Into<SharedString>) -> Self {
        self.connected = true;
        self.status = Some(status.into());
        self
    }

    fn monogram(&self) -> SharedString {
        self.name.chars().next().unwrap_or('?').to_string().into()
    }
}

/// Twelve services, four of them connected.
pub fn default_integrations() -> Vec<Integration> {
    vec![
        Integration::new("github", "GitHub", "Code")
            .domain("github.com")
            .about("Link pull requests to tasks, and post deploy notes back to the thread that asked for them.")
            .permission(Permission::new("Read repositories", "Code, branches, releases"))
            .permission(Permission::new("Comment on pull requests", "As the Orbit app"))
            .permission(Permission::new("Manage webhooks", "Push and deploy events").optional())
            .connected("Synced 4 min ago · 3 repositories"),
        Integration::new("slack", "Slack", "Chat")
            .domain("slack.com")
            .about("Turn a message into a task with one emoji, and get a digest of what moved each morning.")
            .permission(Permission::new("Read channels you pick", "Public channels only"))
            .permission(Permission::new("Post messages", "Digests and replies"))
            .permission(Permission::new("Read direct messages", "Never stored").optional())
            .connected("Synced just now · 6 channels"),
        Integration::new("sentry", "Sentry", "Errors")
            .domain("sentry.io")
            .about("Open a task for every new issue and close it when the fix ships.")
            .permission(Permission::new("Read issues", "Titles, counts, stack traces"))
            .permission(Permission::new("Resolve issues", "When a linked task closes"))
            .permission(Permission::new("Read releases", "Versions and commits").optional())
            .connected("Synced 1 h ago · 2 projects"),
        Integration::new("dropbox", "Dropbox", "Files")
            .domain("dropbox.com")
            .about("Attach files from any folder and keep the newest version on every task.")
            .permission(Permission::new("Read files you choose", "Picked per task"))
            .permission(Permission::new("Create shared links", "View-only by default"))
            .permission(Permission::new("Read all files", "Full Dropbox").optional())
            .connected("Synced yesterday · 18 files"),
        Integration::new("jira", "Jira", "Issues")
            .domain("atlassian.com")
            .about("Mirror Jira issues as tasks and keep status, assignee and comments in step.")
            .permission(Permission::new("Read issues", "Projects you pick"))
            .permission(Permission::new("Update status", "Both directions"))
            .permission(Permission::new("Create issues", "From any task").optional()),
        Integration::new("zendesk", "Zendesk", "Support")
            .domain("zendesk.com")
            .about("Escalate a ticket into a task with the whole conversation attached.")
            .permission(Permission::new("Read tickets", "Subject, requester, thread"))
            .permission(Permission::new("Add internal notes", "When a task changes"))
            .permission(Permission::new("Read customer profiles", "Name and plan").optional()),
        Integration::new("salesforce", "Salesforce", "CRM")
            .domain("salesforce.com")
            .about("See the account behind every request, with plan, owner and open deals.")
            .permission(Permission::new("Read accounts", "Name, owner, plan"))
            .permission(Permission::new("Read opportunities", "Open deals only"))
            .permission(Permission::new("Log activity", "Task updates").optional()),
        Integration::new("pagerduty", "PagerDuty", "On-call")
            .domain("pagerduty.com")
            .about("Page the right person from a task, and track incidents next to the work they block.")
            .permission(Permission::new("Read schedules", "Who is on call"))
            .permission(Permission::new("Trigger incidents", "From a task"))
            .permission(Permission::new("Acknowledge incidents", "From a task").optional()),
        Integration::new("intercom", "Intercom", "Messaging")
            .domain("intercom.com")
            .about("Bring customer conversations into planning without copying and pasting.")
            .permission(Permission::new("Read conversations", "Tagged ones only"))
            .permission(Permission::new("Reply as a teammate", "With your name"))
            .permission(Permission::new("Read contacts", "Name and company").optional()),
        Integration::new("asana", "Asana", "Projects")
            .domain("asana.com")
            .about("Import projects with their sections, owners and due dates intact.")
            .permission(Permission::new("Read projects", "Ones you pick"))
            .permission(Permission::new("Keep tasks in sync", "Both directions"))
            .permission(Permission::new("Read comments", "On synced tasks").optional()),
        Integration::new("zoom", "Zoom", "Meetings")
            .domain("zoom.us")
            .about("Start a call from any task and attach the recording and summary afterwards.")
            .permission(Permission::new("Create meetings", "As you"))
            .permission(Permission::new("Read recordings", "Of meetings started here"))
            .permission(Permission::new("Read transcripts", "For summaries").optional()),
        Integration::new("datadog", "Datadog", "Monitoring")
            .domain("datadoghq.com")
            .about("Pin live graphs to tasks and open one automatically when a monitor fires.")
            .permission(Permission::new("Read monitors", "Status and history"))
            .permission(Permission::new("Read dashboards", "For embeds"))
            .permission(Permission::new("Mute monitors", "From a task").optional()),
    ]
}

struct PickerState {
    input: Entity<TextInput>,
    _subscription: Subscription,
    query: SharedString,
    selected: Option<SharedString>,
    connected: Vec<SharedString>,
    connecting: Option<SharedString>,
    /// Bumped when a handshake finishes, so the link celebrates once.
    joined: u32,
    grants: HashMap<(SharedString, usize), bool>,
    scroll: ScrollHandle,
    _handshake: Option<Task<()>>,
    integrations: Rc<Vec<Integration>>,
    on_connect: Option<ConnectHandler>,
    on_back: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

/// The visible services, Connected first, as `(group, rows)`.
fn arranged<'a>(
    all: &'a [Integration],
    connected: &[SharedString],
    query: &str,
) -> Vec<(&'static str, Vec<&'a Integration>)> {
    let q = query.trim().to_lowercase();
    let matches = |i: &&Integration| {
        q.is_empty()
            || i.name.to_lowercase().contains(&q)
            || i.category.to_lowercase().contains(&q)
            || i.domain.to_lowercase().contains(&q)
    };
    let (on, off): (Vec<&Integration>, Vec<&Integration>) = all
        .iter()
        .filter(matches)
        .partition(|i| connected.contains(&i.id));
    [("Connected", on), ("Available", off)]
        .into_iter()
        .filter(|(_, rows)| !rows.is_empty())
        .collect()
}

impl PickerState {
    fn visible(&self) -> Vec<Integration> {
        arranged(&self.integrations, &self.connected, &self.query)
            .into_iter()
            .flat_map(|(_, rows)| rows.into_iter().cloned())
            .collect()
    }

    fn on_input(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Change(text) => {
                self.query = text.clone();
                // Keep the selection if it still shows, else take the first match.
                let visible = self.visible();
                if !visible
                    .iter()
                    .any(|i| Some(&i.id) == self.selected.as_ref())
                {
                    self.selected = visible.first().map(|i| i.id.clone());
                }
                self.reveal();
            }
            InputEvent::MoveDown | InputEvent::MoveUp => {
                let visible = self.visible();
                let len = visible.len();
                if len > 0 {
                    let at = visible
                        .iter()
                        .position(|i| Some(&i.id) == self.selected.as_ref());
                    let next = match (at, *event == InputEvent::MoveDown) {
                        (None, _) => 0,
                        (Some(at), true) => (at + 1) % len,
                        (Some(at), false) => (at + len - 1) % len,
                    };
                    self.selected = Some(visible[next].id.clone());
                    self.reveal();
                }
            }
            InputEvent::Submit(_) => {
                if let Some(id) = self.selected.clone() {
                    self.toggle(id, window, cx);
                }
            }
            InputEvent::Escape => {
                if !self.query.is_empty() {
                    self.input.update(cx, |input, cx| input.clear(cx));
                    self.query = SharedString::default();
                    self.reveal();
                } else if let Some(f) = self.on_back.clone() {
                    window.defer(cx, move |window, cx| f(window, cx));
                }
            }
            _ => return,
        }
        cx.notify();
    }

    /// Scrolls the selected row into view. Child 0 is the plate.
    fn reveal(&self) {
        let mut child = 1;
        for (group, (_, rows)) in arranged(&self.integrations, &self.connected, &self.query)
            .into_iter()
            .enumerate()
        {
            child += 1;
            for (ix, row) in rows.into_iter().enumerate() {
                if Some(&row.id) == self.selected.as_ref() {
                    if group == 0 && ix == 0 {
                        // Show the first group's header too.
                        self.scroll.set_offset(gpui::point(px(0.), px(0.)));
                        return;
                    }
                    self.scroll.scroll_to_item(child);
                    return;
                }
                child += 1;
            }
        }
    }

    /// Connects (after a short handshake) or disconnects `id`.
    fn toggle(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        if self.connecting.is_some() {
            return;
        }
        let Some(service) = self.integrations.iter().find(|i| i.id == id).cloned() else {
            return;
        };
        let on_connect = self.on_connect.clone();
        if self.connected.contains(&id) {
            self.connected.retain(|c| *c != id);
            self.reveal();
            if let Some(f) = on_connect {
                window.defer(cx, move |window, cx| f(&service, false, window, cx));
            }
        } else {
            self.connecting = Some(id.clone());
            self._handshake = Some(cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(HANDSHAKE).await;
                this.update_in(cx, |s, window, cx| {
                    s.connecting = None;
                    s.connected.push(id.clone());
                    s.joined += 1;
                    s.reveal();
                    cx.notify();
                    if let Some(f) = on_connect {
                        window.defer(cx, move |window, cx| f(&service, true, window, cx));
                    }
                })
                .ok();
            }));
        }
        cx.notify();
    }
}

#[derive(IntoElement)]
pub struct IntegrationPicker {
    id: ElementId,
    app: SharedString,
    integrations: Vec<Integration>,
    selected: Option<SharedString>,
    autofocus: bool,
    on_connect: Option<ConnectHandler>,
    on_back: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

impl IntegrationPicker {
    /// `app` names your workspace on the left-hand tile.
    pub fn new(
        id: impl Into<ElementId>,
        app: impl Into<SharedString>,
        integrations: Vec<Integration>,
    ) -> Self {
        Self {
            id: id.into(),
            app: app.into(),
            integrations,
            selected: None,
            autofocus: true,
            on_connect: None,
            on_back: None,
        }
    }

    /// The service shown first (by id); defaults to the first one.
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    /// Focuses the search field when the page first renders (on by default).
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    /// Called once a service finishes connecting (`true`) or disconnects (`false`).
    pub fn on_connect(
        mut self,
        handler: impl Fn(&Integration, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_connect = Some(Rc::new(handler));
        self
    }

    /// The back arrow, and Escape on an empty search.
    pub fn on_back(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_back = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for IntegrationPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let total = self.integrations.len();
        let initial_connected: Vec<SharedString> = self
            .integrations
            .iter()
            .filter(|i| i.connected)
            .map(|i| i.id.clone())
            .collect();
        let initial_selected = self
            .selected
            .clone()
            .or_else(|| self.integrations.first().map(|i| i.id.clone()));
        let autofocus = self.autofocus;
        let placeholder = format!("Search {total} apps…");

        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "state".into()),
            cx,
            move |window, cx| {
                let input = cx.new(|cx| TextInput::new(window, cx).placeholder(placeholder));
                let subscription = cx.subscribe_in(
                    &input,
                    window,
                    |s: &mut PickerState, _, event, window, cx| s.on_input(event, window, cx),
                );
                if autofocus {
                    let handle = input.read(cx).focus_handle(cx);
                    window.defer(cx, move |window, _| window.focus(&handle));
                }
                PickerState {
                    input,
                    _subscription: subscription,
                    query: SharedString::default(),
                    selected: initial_selected,
                    connected: initial_connected,
                    connecting: None,
                    joined: 0,
                    grants: HashMap::new(),
                    scroll: ScrollHandle::new(),
                    _handshake: None,
                    integrations: Rc::new(Vec::new()),
                    on_connect: None,
                    on_back: None,
                }
            },
        );

        let integrations = Rc::new(self.integrations);
        let input = state.update(cx, |s, _| {
            s.integrations = integrations.clone();
            s.on_connect = self.on_connect.clone();
            s.on_back = self.on_back.clone();
            s.input.clone()
        });
        input.update(cx, |input, _| {
            input.set_colors(Some(t.text_faint), Some(t.accent), Some(t.selection))
        });

        let s = state.read(cx);
        let groups = arranged(&integrations, &s.connected, &s.query);
        let selected = s
            .selected
            .as_ref()
            .and_then(|id| integrations.iter().find(|i| i.id == *id))
            .cloned();
        let connected_count = s.connected.len();
        let connecting = s.connecting.clone();
        let joined = s.joined;
        let connected_ids = s.connected.clone();
        let grants = s.grants.clone();
        let scroll = s.scroll.clone();
        let reduced = motion::reduced_motion(cx);

        let selected_id = selected.as_ref().map(|i| i.id.clone());
        let sidebar = sidebar(
            &key,
            &groups,
            selected_id.as_ref(),
            connecting.as_ref(),
            &input,
            &state,
            &scroll,
            self.on_back.clone(),
            &t,
        );

        let detail = match selected {
            Some(service) => {
                let status = if connecting.as_ref() == Some(&service.id) {
                    Link::Connecting
                } else if connected_ids.contains(&service.id) {
                    Link::Connected
                } else {
                    Link::Available
                };
                detail(
                    &key, &self.app, &service, status, joined, &grants, &state, reduced, &t,
                )
                .into_any_element()
            }
            None => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(text::BODY))
                .text_color(t.text_faint)
                .child("No service matches your search")
                .into_any_element(),
        };

        motion::keep_alive(window);
        div()
            .id(self.id)
            .w(px(WIDTH))
            .font_family(luna::SANS)
            .text_color(t.text)
            .bg(t.surface_overlay)
            .border_1()
            .border_color(t.border)
            .rounded(px(radius::XL))
            .shadow(t.menu_shadow())
            .overflow_hidden()
            .child(
                div()
                    .h(px(BODY_H))
                    .flex()
                    .child(sidebar)
                    .child(div().w(px(1.)).h_full().bg(t.border))
                    .child(detail),
            )
            .child(footer(&key, connected_count, total, &t))
            .with_animation("integration-picker-in", ROW_IN.animation(), |el, k| {
                el.opacity(k).mt(px(8. * (1. - k)))
            })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Link {
    Available,
    Connecting,
    Connected,
}

#[allow(clippy::too_many_arguments)]
fn sidebar(
    key: &str,
    groups: &[(&'static str, Vec<&Integration>)],
    selected: Option<&SharedString>,
    connecting: Option<&SharedString>,
    input: &Entity<TextInput>,
    state: &Entity<PickerState>,
    scroll: &ScrollHandle,
    on_back: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    t: &Luna,
) -> impl IntoElement {
    // Plate position from the fixed row and header heights.
    let mut y = LIST_PAD;
    let mut plate: Option<f32> = None;
    for (_, rows) in groups {
        y += HEADER_H;
        for row in rows {
            if Some(&row.id) == selected {
                plate = Some(y);
            }
            y += ROW_H;
        }
    }
    let plate_top = motion::follow(
        &format!("{key}-plate"),
        plate.unwrap_or(LIST_PAD + HEADER_H),
        MotionSpec::new(260, EASE_OUT_QUINT),
    );

    let mut list = div()
        .id("services")
        .relative()
        .flex_1()
        .min_h_0()
        .px(px(LIST_PAD))
        .py(px(LIST_PAD))
        .overflow_y_scroll()
        .track_scroll(scroll)
        .child(
            div()
                .absolute()
                .left(px(LIST_PAD))
                .right(px(LIST_PAD))
                .top(px(plate_top))
                .h(px(ROW_H))
                .rounded(px(radius::MD))
                .bg(t.fill(FillTier::Tertiary))
                .when(plate.is_none(), |el| el.opacity(0.)),
        );
    for (group, rows) in groups {
        list = list.child(
            div()
                .h(px(HEADER_H))
                .px(px(8.))
                .pt(px(6.))
                .flex()
                .items_center()
                .text_size(px(text::CAPTION))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text_faint)
                .child(*group),
        );
        for service in rows {
            let picker = state.clone();
            let id = service.id.clone();
            let is_selected = Some(&service.id) == selected;
            let lit = motion::follow(
                &format!("{key}-lit-{}", service.id),
                if is_selected { 1. } else { 0. },
                MotionSpec::new(160, EASE),
            );
            let busy = connecting == Some(&service.id);
            let is_connected = *group == "Connected";
            list = list.child(
                div()
                    .id(SharedString::from(format!("service-{}", service.id)))
                    .relative()
                    .h(px(ROW_H))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .rounded(px(radius::MD))
                    .cursor_pointer()
                    .hover_fill(
                        format!("{key}-row-{}", service.id),
                        t.fill(FillTier::Quaternary).opacity(0.),
                        if is_selected {
                            t.fill(FillTier::Quaternary).opacity(0.)
                        } else {
                            t.fill(FillTier::Quaternary)
                        },
                    )
                    .on_click(move |_, window, cx| {
                        picker.update(cx, |s, cx| {
                            s.selected = Some(id.clone());
                            s.input.read(cx).focus(window);
                            cx.notify();
                        })
                    })
                    .child(monogram(&service.monogram(), 24., lit, t))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_baseline()
                            .gap(px(6.))
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(text::BODY))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(service.name.clone()),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(text::FOOTNOTE))
                                    .text_color(t.text_faint)
                                    .child(service.category.clone()),
                            ),
                    )
                    .when(busy, |el| el.child(spinner("row-spinner", 14., t.accent)))
                    .when(is_connected && !busy, |el| {
                        el.child(div().size(px(6.)).rounded_full().bg(t.success))
                    })
                    .with_animation(
                        SharedString::from(format!("service-in-{}", service.id)),
                        MotionSpec::new(260, EASE_OUT_QUINT).animation(),
                        |el, k| el.opacity(k).top(px(3. * (1. - k))),
                    ),
            );
        }
    }
    if groups.is_empty() {
        list = list.child(
            div()
                .pt(px(24.))
                .flex()
                .justify_center()
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_faint)
                .child("No apps match"),
        );
    }

    let back = luna::icon_button("ip-back", ICON_BACK, ControlSize::Md, t)
        .when_some(on_back, |el, f| {
            el.on_click(move |_, window, cx| f(window, cx))
        });

    div()
        .w(px(SIDEBAR_W))
        .flex_none()
        .h_full()
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(48.))
                .pl(px(10.))
                .pr(px(14.))
                .flex()
                .items_center()
                .gap(px(6.))
                .child(back)
                .child(
                    div()
                        .flex_1()
                        .text_size(px(text::CALLOUT))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Integrations"),
                )
                .child(luna::kbd("Esc", t)),
        )
        .child(
            div().px(px(12.)).pb(px(4.)).child(
                div()
                    .h(px(32.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(radius::MD))
                    .bg(t.input_bg)
                    .border_1()
                    .border_color(t.border)
                    .child(
                        svg()
                            .path(ICON_SEARCH)
                            .size(px(14.))
                            .flex_none()
                            .text_color(t.text_faint),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(text::BODY))
                            .line_height(px(18.))
                            .child(input.clone()),
                    ),
            ),
        )
        .child(list)
}

/// A neutral lettered tile standing in for a service's logo.
fn monogram(letter: &SharedString, size: f32, lit: f32, t: &Luna) -> impl IntoElement {
    let rest = if t.dark {
        t.surface_raised
    } else {
        t.surface_overlay
    };
    div()
        .size(px(size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(if size > 32. { radius::LG } else { radius::SM }))
        .bg(rest)
        .border_1()
        .border_color(motion::mix(t.border_strong, t.text_faint, lit * 0.6))
        .text_size(px(size * 0.46))
        .font_weight(FontWeight::BOLD)
        .text_color(t.text)
        .child(letter.clone())
}

fn spinner(id: &'static str, size: f32, color: Hsla) -> impl IntoElement {
    svg()
        .path(ICON_SPINNER)
        .size(px(size))
        .flex_none()
        .text_color(color)
        .with_animation(
            id,
            Animation::new(Duration::from_millis(800)).repeat(),
            |el, k| el.with_transformation(Transformation::rotate(percentage(k))),
        )
}

#[allow(clippy::too_many_arguments)]
fn detail(
    key: &str,
    app: &SharedString,
    service: &Integration,
    link: Link,
    joined: u32,
    grants: &HashMap<(SharedString, usize), bool>,
    state: &Entity<PickerState>,
    reduced: bool,
    t: &Luna,
) -> impl IntoElement {
    let id = service.id.clone();
    let toggler = state.clone();
    let action = match link {
        Link::Available => luna::button("ip-connect", ButtonKind::Accent, ControlSize::Md, t)
            .child("Connect")
            .on_click(move |_, window, cx| {
                toggler.update(cx, |s, cx| s.toggle(id.clone(), window, cx))
            }),
        Link::Connecting => luna::button("ip-connect", ButtonKind::Accent, ControlSize::Md, t)
            .opacity(0.8)
            .cursor_default()
            .child(spinner("button-spinner", 14., t.on_accent))
            .child("Connecting…"),
        Link::Connected => luna::button("ip-connect", ButtonKind::Secondary, ControlSize::Md, t)
            .child("Disconnect")
            .on_click(move |_, window, cx| {
                toggler.update(cx, |s, cx| s.toggle(id.clone(), window, cx))
            }),
    };
    let (badge_label, tone) = match link {
        Link::Available => ("Not connected", Tone::Neutral),
        Link::Connecting => ("Connecting", Tone::Accent),
        Link::Connected => ("Connected", Tone::Success),
    };
    let badge = div()
        .child(luna::badge(badge_label, tone, t))
        .with_animation(
            SharedString::from(format!("{key}-badge-{}-{badge_label}", service.id)),
            MotionSpec::new(300, EASE_OUT_QUINT).animation(),
            |el, k| el.relative().opacity(k).top(px(4. * (1. - k))),
        );

    let permissions = service.permissions.iter().enumerate().map(|(ix, p)| {
        let granted = grants
            .get(&(service.id.clone(), ix))
            .copied()
            .unwrap_or(p.granted);
        let flip = state.clone();
        let sid = service.id.clone();
        let k = motion::follow(
            &format!("{key}-perm-{}-{ix}", service.id),
            if granted { 1. } else { 0. },
            MotionSpec::new(180, EASE_OUT_QUINT),
        );
        div()
            .id(("permission", ix))
            .h(px(28.))
            .px(px(8.))
            .mx(px(-8.))
            .flex()
            .items_center()
            .gap(px(10.))
            .rounded(px(radius::MD))
            .cursor_pointer()
            .hover_fill(
                format!("{key}-perm-hover-{}-{ix}", service.id),
                t.fill(FillTier::Quaternary).opacity(0.),
                t.fill(FillTier::Quaternary),
            )
            .on_click(move |_, _, cx| {
                flip.update(cx, |s, cx| {
                    s.grants.insert((sid.clone(), ix), !granted);
                    cx.notify();
                })
            })
            .child(
                div()
                    .size(px(16.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::XS))
                    .border_1()
                    .border_color(motion::mix(t.border_strong, t.accent_strong, k))
                    .bg(motion::mix(t.input_bg, t.accent_strong, k))
                    .child(
                        svg()
                            .path(ICON_CHECK)
                            .size(px(12.))
                            .text_color(t.on_accent.opacity(k)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .text_size(px(text::BODY))
                    .text_color(t.text)
                    .child(p.label.clone()),
            )
            .child(
                div()
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_faint)
                    .child(p.detail.clone()),
            )
    });

    let meta: (&'static str, SharedString) = match link {
        Link::Connected => (
            ICON_CLOCK,
            service
                .status
                .clone()
                .unwrap_or_else(|| "Synced just now".into()),
        ),
        Link::Connecting => (
            ICON_SHIELD,
            "Waiting for the authorization to finish…".into(),
        ),
        Link::Available => (
            ICON_SHIELD,
            "You approve access on the next screen · revoke it any time".into(),
        ),
    };

    let body = div()
        .flex()
        .flex_col()
        .child(hero(key, app, service, link, joined, reduced, t))
        .child(
            div()
                .mt(px(14.))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(text::TITLE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(service.name.clone()),
                )
                .child(badge)
                .child(div().flex_1())
                .child(action),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_faint)
                .child(format!("{} · {}", service.category, service.domain))
                .child(
                    svg()
                        .path(ICON_EXTERNAL)
                        .size(px(12.))
                        .text_color(t.text_faint),
                ),
        )
        .child(
            div()
                .mt(px(8.))
                .text_size(px(text::BODY))
                .line_height(px(19.))
                .text_color(t.text_muted)
                .child(service.about.clone()),
        )
        .child(
            div()
                .mt(px(12.))
                .mb(px(2.))
                .text_size(px(text::CAPTION))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text_faint)
                .child(format!("{} may", service.name)),
        )
        .children(permissions)
        .child(
            div()
                .mt(px(8.))
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_faint)
                .child(svg().path(meta.0).size(px(13.)).text_color(t.text_faint))
                .child(meta.1),
        )
        .with_animation(
            SharedString::from(format!("{key}-detail-{}", service.id)),
            VIEW_IN.animation(),
            |el, k| el.relative().opacity(k).top(px(6. * (1. - k))),
        );

    div().flex_1().min_w_0().p(px(16.)).child(body)
}

/// Your workspace and the service on two tiles, joined by a dotted link.
fn hero(
    key: &str,
    app: &SharedString,
    service: &Integration,
    link: Link,
    joined: u32,
    reduced: bool,
    t: &Luna,
) -> impl IntoElement {
    let live = link == Link::Connected;
    let busy = link == Link::Connecting;
    let rest = if live {
        t.accent.opacity(0.45)
    } else {
        t.text_faint.opacity(0.4)
    };
    let bright = if live || busy { t.accent } else { t.text_muted };

    let dots = (0..DOTS).map(|i| {
        let dot = div().size(px(4.)).rounded_full().bg(rest);
        if reduced || !(live || busy) {
            return dot.into_any_element();
        }
        // A bright pulse travels from your tile to theirs.
        let period = if busy { 900 } else { 2400 };
        let offset = i as f32 / DOTS as f32;
        dot.with_animation(
            SharedString::from(format!("{key}-dot-{i}-{busy}")),
            Animation::new(Duration::from_millis(period)).repeat(),
            move |el, k| {
                let phase = (k - offset).rem_euclid(1.);
                let glow = (1. - (phase * 4.).min(1.)).powi(2);
                el.bg(motion::mix(rest, bright, glow))
            },
        )
        .into_any_element()
    });

    let center = div()
        .size(px(22.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_1()
        .border_color(t.border)
        .bg(t.surface_overlay)
        .child(match link {
            Link::Connected => svg()
                .path(ICON_CHECK)
                .size(px(13.))
                .text_color(t.success)
                .into_any_element(),
            Link::Connecting => spinner("hero-spinner", 13., t.accent).into_any_element(),
            Link::Available => svg()
                .path(ICON_LINK)
                .size(px(13.))
                .text_color(t.text_faint)
                .into_any_element(),
        })
        .with_animation(
            ElementId::NamedInteger(format!("{key}-joined").into(), u64::from(joined)),
            MotionSpec::new(420, motion::EASE_OUT_BACK).animation(),
            |el, k| el.opacity(0.4 + 0.6 * k),
        );

    let (left_dots, right_dots): (Vec<_>, Vec<_>) =
        dots.enumerate().partition(|(i, _)| *i < DOTS / 2);
    let strip = |dots: Vec<(usize, gpui::AnyElement)>| {
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .children(dots.into_iter().map(|(_, d)| d))
    };

    let our_tile = div()
        .size(px(48.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::LG))
        .bg(t.accent_strong)
        .shadow(t.card_shadow())
        .child(svg().path(ICON_ORBIT).size(px(24.)).text_color(t.on_accent));
    let their_tile = div()
        .size(px(48.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::LG))
        .bg(if t.dark {
            t.surface_raised
        } else {
            t.surface_overlay
        })
        .border_1()
        .border_color(t.border_strong)
        .shadow(t.card_shadow())
        .text_size(px(20.))
        .font_weight(FontWeight::BOLD)
        .child(service.monogram());
    let caption = |label: SharedString| {
        div()
            .text_size(px(text::CAPTION))
            .text_color(t.text_faint)
            .child(label)
    };

    div()
        .h(px(96.))
        .rounded(px(radius::LG))
        .border_1()
        .border_color(t.border)
        .bg(t.fill(FillTier::Quaternary))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.))
                .child(our_tile)
                .child(caption(app.clone())),
        )
        .child(
            div()
                .mb(px(20.))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(strip(left_dots))
                .child(center)
                .child(strip(right_dots)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.))
                .child(their_tile)
                .child(caption(service.name.clone())),
        )
}

fn footer(key: &str, connected: usize, total: usize, t: &Luna) -> impl IntoElement {
    let ticks = (0..total).map(|i| {
        let on = motion::follow(
            &format!("{key}-tick-{i}"),
            if i < connected { 1. } else { 0. },
            MotionSpec::new(260, EASE_OUT_QUINT),
        );
        div().w(px(4.)).h(px(10.)).rounded(px(1.)).bg(motion::mix(
            t.fill(FillTier::Primary),
            t.success,
            on,
        ))
    });
    let hint = |keys: &[&'static str], label: &'static str| {
        div()
            .flex()
            .items_center()
            .gap(px(5.))
            .child(
                div()
                    .flex()
                    .gap(px(2.))
                    .children(keys.iter().map(|k| luna::kbd(*k, t))),
            )
            .child(label)
    };
    div()
        .h(px(38.))
        .px(px(14.))
        .flex()
        .items_center()
        .justify_between()
        .border_t_1()
        .border_color(t.border)
        .bg(t.fill(FillTier::Quaternary))
        .text_size(px(text::FOOTNOTE))
        .text_color(t.text_faint)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(div().flex().gap(px(2.)).children(ticks))
                .child(format!("{connected} of {total} connected")),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(14.))
                .child(hint(&["↑", "↓"], "browse"))
                .child(hint(&["↵"], "connect")),
        )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        IntegrationPicker::new("integration-picker", "Orbit", default_integrations())
            .selected("github")
    })
}
