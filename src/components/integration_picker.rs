//! Integration Picker – the integrations page, where apps are connected.
//!
//! A Pink component, built as a Pink settings page (Pink: `settings/
//! extensions.rs` `ExtensionsPage`): a page title with its count and a
//! search field, then a Connected and an Available section card of service
//! rows. Each row is Pink's package row – identity tile, name, description
//! and a quiet meta line, a status badge and small push buttons. Connecting
//! opens Pink's enable review, a dialog with one switch per permission where
//! required ones stay locked on; confirming shows a pending badge while the
//! connection is made, then the row moves into Connected.
//!
//! Keys: type to filter, Tab to reach the row actions, Escape to clear the
//! search (or go back when it is empty). In the review, Escape cancels and
//! Enter confirms.
//!
//! ```ignore
//! IntegrationPicker::new("integrations", "Orbit", default_integrations())
//!     .on_connect(|service, connected, _, _| println!("{} → {connected}", service.name))
//! ```

use std::{collections::HashMap, rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AnyElement, AnyView, App, Context, ElementId, Entity, FocusHandle, Focusable, FontWeight,
    IntoElement, KeyDownEvent, ScrollHandle, SharedString, Subscription, Task, Window, div, point,
    prelude::*, px, svg,
};

use crate::{
    motion::{self, TOOL_ROW_REVEAL},
    pink::{
        self, ButtonKind, ControlSize, FillTier, Pink, TextStyle, Tone, TypeStyleExt, layout,
        loaders::label_id, radius,
    },
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Integration Picker";
pub const DESCRIPTION: &str = "A settings page of apps to connect – Connected and Available cards of rows with a status badge and small actions, a search in the title bar, and a review dialog that gives each permission its own switch, required ones locked on";

const ICON_SEARCH: &str = "icons/integration-picker/magnifer.svg";

/// The settings plane's corner (Pink `workbench/render.rs` `CANVAS_RADIUS`).
const CANVAS_RADIUS: f32 = 14.0;
/// Inset between the plane's edge and its page (Pink `VIEWPORT_INSET - 1`).
const PLANE_INSET: f32 = 5.0;
const PLANE_W: f32 = 760.0;
const PLANE_H: f32 = 760.0;
/// The search field in the title bar.
const SEARCH_W: f32 = 220.0;
/// The review dialog's width (Pink `extensions.rs` `render_review`).
const REVIEW_W: f32 = 460.0;
/// How long the demo connection takes.
const HANDSHAKE: Duration = Duration::from_millis(1400);

type ConnectHandler = Rc<dyn Fn(&Integration, bool, &mut Window, &mut App)>;
type BackHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Something the integration may do once connected.
#[derive(Clone, Debug)]
pub struct Permission {
    pub label: SharedString,
    pub detail: SharedString,
    /// On when the review opens.
    pub granted: bool,
    /// Locked on: the integration can't connect without it.
    pub required: bool,
}

impl Permission {
    /// On by default; the user can turn it off in the review.
    pub fn new(label: impl Into<SharedString>, detail: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            granted: true,
            required: false,
        }
    }

    /// Off until the user turns it on.
    pub fn optional(mut self) -> Self {
        self.granted = false;
        self.required = false;
        self
    }

    /// Always granted; its switch is locked on.
    pub fn required(mut self) -> Self {
        self.granted = true;
        self.required = true;
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
    /// Shown in a connected service's meta line, e.g. "Synced 4 min ago".
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
        pink::monogram(&self.name)
    }

    /// The permissions' default switches.
    fn default_grants(&self) -> Vec<bool> {
        self.permissions
            .iter()
            .map(|p| p.required || p.granted)
            .collect()
    }

    /// This service with `grants` written into its permissions, for callbacks.
    fn with_grants(&self, grants: &[bool]) -> Self {
        let mut service = self.clone();
        for (permission, granted) in service.permissions.iter_mut().zip(grants) {
            permission.granted = permission.required || *granted;
        }
        service
    }
}

/// Twelve services, four of them connected.
pub fn default_integrations() -> Vec<Integration> {
    vec![
        Integration::new("github", "GitHub", "Code")
            .domain("github.com")
            .about("Link pull requests to tasks, and post deploy notes back to the thread that asked for them.")
            .permission(Permission::new("Read repositories", "Code, branches, releases").required())
            .permission(Permission::new("Comment on pull requests", "As the Orbit app"))
            .permission(Permission::new("Manage webhooks", "Push and deploy events").optional())
            .connected("Synced 4 min ago"),
        Integration::new("slack", "Slack", "Chat")
            .domain("slack.com")
            .about("Turn a message into a task with one emoji, and get a digest of what moved each morning.")
            .permission(Permission::new("Read channels you pick", "Public channels only").required())
            .permission(Permission::new("Post messages", "Digests and replies"))
            .permission(Permission::new("Read direct messages", "Never stored").optional())
            .connected("Synced just now"),
        Integration::new("sentry", "Sentry", "Errors")
            .domain("sentry.io")
            .about("Open a task for every new issue and close it when the fix ships.")
            .permission(Permission::new("Read issues", "Titles, counts, stack traces").required())
            .permission(Permission::new("Resolve issues", "When a linked task closes"))
            .permission(Permission::new("Read releases", "Versions and commits").optional())
            .connected("Synced 1 h ago"),
        Integration::new("dropbox", "Dropbox", "Files")
            .domain("dropbox.com")
            .about("Attach files from any folder and keep the newest version on every task.")
            .permission(Permission::new("Read files you choose", "Picked per task").required())
            .permission(Permission::new("Create shared links", "View-only by default"))
            .permission(Permission::new("Read all files", "Full Dropbox").optional())
            .connected("Synced yesterday"),
        Integration::new("jira", "Jira", "Issues")
            .domain("atlassian.com")
            .about("Mirror Jira issues as tasks and keep status, assignee and comments in step.")
            .permission(Permission::new("Read issues", "Projects you pick").required())
            .permission(Permission::new("Update status", "Both directions"))
            .permission(Permission::new("Create issues", "From any task").optional()),
        Integration::new("zendesk", "Zendesk", "Support")
            .domain("zendesk.com")
            .about("Escalate a ticket into a task with the whole conversation attached.")
            .permission(Permission::new("Read tickets", "Subject, requester, thread").required())
            .permission(Permission::new("Add internal notes", "When a task changes"))
            .permission(Permission::new("Read customer profiles", "Name and plan").optional()),
        Integration::new("salesforce", "Salesforce", "CRM")
            .domain("salesforce.com")
            .about("See the account behind every request, with plan, owner and open deals.")
            .permission(Permission::new("Read accounts", "Name, owner, plan").required())
            .permission(Permission::new("Read opportunities", "Open deals only"))
            .permission(Permission::new("Log activity", "Task updates").optional()),
        Integration::new("pagerduty", "PagerDuty", "On-call")
            .domain("pagerduty.com")
            .about("Page the right person from a task, and track incidents next to the work they block.")
            .permission(Permission::new("Read schedules", "Who is on call").required())
            .permission(Permission::new("Trigger incidents", "From a task"))
            .permission(Permission::new("Acknowledge incidents", "From a task").optional()),
        Integration::new("intercom", "Intercom", "Messaging")
            .domain("intercom.com")
            .about("Bring customer conversations into planning without copying and pasting.")
            .permission(Permission::new("Read conversations", "Tagged ones only").required())
            .permission(Permission::new("Reply as a teammate", "With your name"))
            .permission(Permission::new("Read contacts", "Name and company").optional()),
        Integration::new("asana", "Asana", "Projects")
            .domain("asana.com")
            .about("Import projects with their sections, owners and due dates intact.")
            .permission(Permission::new("Read projects", "Ones you pick").required())
            .permission(Permission::new("Keep tasks in sync", "Both directions"))
            .permission(Permission::new("Read comments", "On synced tasks").optional()),
        Integration::new("zoom", "Zoom", "Meetings")
            .domain("zoom.us")
            .about("Start a call from any task and attach the recording and summary afterwards.")
            .permission(Permission::new("Create meetings", "As you").required())
            .permission(Permission::new("Read recordings", "Of meetings started here"))
            .permission(Permission::new("Read transcripts", "For summaries").optional()),
        Integration::new("datadog", "Datadog", "Monitoring")
            .domain("datadoghq.com")
            .about("Pin live graphs to tasks and open one automatically when a monitor fires.")
            .permission(Permission::new("Read monitors", "Status and history").required())
            .permission(Permission::new("Read dashboards", "For embeds"))
            .permission(Permission::new("Mute monitors", "From a task").optional()),
    ]
}

/// The open review: one service and the switches left on (Pink
/// `extensions.rs` `Review`).
struct Review {
    service: SharedString,
    granted: Vec<bool>,
    /// Changing a connected service's permissions rather than connecting it.
    manage: bool,
    /// Keys the switches' motion, so a reopened review starts still.
    generation: u32,
}

struct PickerState {
    input: Entity<TextInput>,
    _subscription: Subscription,
    query: SharedString,
    connected: Vec<SharedString>,
    /// The service being connected; every row action waits for it (Pink's
    /// `busy`).
    connecting: Option<SharedString>,
    /// The switches each connected service was given.
    grants: HashMap<SharedString, Vec<bool>>,
    review: Option<Review>,
    /// The row that last changed section, and the generation its arrival
    /// reveal is keyed on.
    arrived: Option<(SharedString, u32)>,
    generation: u32,
    dialog_focus: FocusHandle,
    scroll: ScrollHandle,
    _handshake: Option<Task<()>>,
    integrations: Rc<Vec<Integration>>,
    on_connect: Option<ConnectHandler>,
    on_back: Option<BackHandler>,
}

/// The visible services as `(section, rows)`, Connected first.
fn arranged<'a>(
    all: &'a [Integration],
    connected: &[SharedString],
    query: &str,
) -> Vec<(Section, Vec<&'a Integration>)> {
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
    [(Section::Connected, on), (Section::Available, off)]
        .into_iter()
        .filter(|(_, rows)| !rows.is_empty())
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Connected,
    Available,
}

impl Section {
    fn title(self) -> &'static str {
        match self {
            Section::Connected => "Connected",
            Section::Available => "Available",
        }
    }

    fn description(self, app: &str) -> String {
        match self {
            Section::Connected => format!(
                "These apps act in {app} with the access you gave them. Disconnecting revokes it at once."
            ),
            Section::Available => {
                "Connecting shows what an app asks for first. Nothing is shared until you confirm."
                    .into()
            }
        }
    }
}

impl PickerState {
    fn service(&self, id: &SharedString) -> Option<&Integration> {
        self.integrations.iter().find(|i| i.id == *id)
    }

    fn on_input(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Change(text) => {
                self.query = text.clone();
                self.scroll.set_offset(point(px(0.), px(0.)));
            }
            InputEvent::Tab => window.focus_next(cx),
            InputEvent::Escape => {
                if !self.query.is_empty() {
                    self.input.update(cx, |input, cx| input.clear(cx));
                    self.query = SharedString::default();
                } else if let Some(f) = self.on_back.clone() {
                    window.defer(cx, move |window, cx| f(window, cx));
                }
            }
            _ => return,
        }
        cx.notify();
    }

    /// Opens the review for `id` (Pink `open_review`): required and default
    /// permissions on, optional ones off; a connected service starts from
    /// what it was given.
    fn open_review(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        if self.connecting.is_some() {
            return;
        }
        let Some(service) = self.service(&id) else {
            return;
        };
        let manage = self.connected.contains(&id);
        let granted = self
            .grants
            .get(&id)
            .filter(|_| manage)
            .cloned()
            .unwrap_or_else(|| service.default_grants());
        self.generation += 1;
        self.review = Some(Review {
            service: id,
            granted,
            manage,
            generation: self.generation,
        });
        window.focus(&self.dialog_focus, cx);
        cx.notify();
    }

    fn toggle_grant(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(review) = self.review.as_mut() else {
            return;
        };
        let required = self
            .integrations
            .iter()
            .find(|i| i.id == review.service)
            .and_then(|i| i.permissions.get(ix))
            .is_some_and(|p| p.required);
        if required {
            return;
        }
        if let Some(on) = review.granted.get_mut(ix) {
            *on = !*on;
        }
        cx.notify();
    }

    fn cancel_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.review = None;
        TextInput::focus(&self.input, window, cx);
        cx.notify();
    }

    /// Confirms the review (Pink `confirm_review`): a save applies at once;
    /// a connection runs while every row action waits, then the row moves
    /// into Connected.
    fn confirm_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(review) = self.review.take() else {
            return;
        };
        TextInput::focus(&self.input, window, cx);
        let Some(service) = self.service(&review.service).cloned() else {
            return;
        };
        let id = review.service.clone();
        self.grants.insert(id.clone(), review.granted.clone());
        let on_connect = self.on_connect.clone();
        let granted = service.with_grants(&review.granted);
        if review.manage {
            if let Some(f) = on_connect {
                window.defer(cx, move |window, cx| f(&granted, true, window, cx));
            }
            cx.notify();
            return;
        }
        self.connecting = Some(id.clone());
        self._handshake = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(HANDSHAKE).await;
            this.update_in(cx, |s, window, cx| {
                s.connecting = None;
                s.connected.push(id.clone());
                s.generation += 1;
                s.arrived = Some((id.clone(), s.generation));
                cx.notify();
                if let Some(f) = on_connect {
                    window.defer(cx, move |window, cx| f(&granted, true, window, cx));
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// Disconnects `id` at once (Pink `disable`).
    fn disconnect(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        if self.connecting.is_some() {
            return;
        }
        let Some(service) = self.service(&id).cloned() else {
            return;
        };
        self.connected.retain(|c| *c != id);
        self.grants.remove(&id);
        self.generation += 1;
        self.arrived = Some((id, self.generation));
        if let Some(f) = self.on_connect.clone() {
            window.defer(cx, move |window, cx| f(&service, false, window, cx));
        }
        cx.notify();
    }
}

#[derive(IntoElement)]
pub struct IntegrationPicker {
    id: ElementId,
    app: SharedString,
    integrations: Vec<Integration>,
    reviewing: Option<SharedString>,
    autofocus: bool,
    on_connect: Option<ConnectHandler>,
    on_back: Option<BackHandler>,
}

impl IntegrationPicker {
    /// `app` names your workspace in the page's copy.
    pub fn new(
        id: impl Into<ElementId>,
        app: impl Into<SharedString>,
        integrations: Vec<Integration>,
    ) -> Self {
        Self {
            id: id.into(),
            app: app.into(),
            integrations,
            reviewing: None,
            autofocus: true,
            on_connect: None,
            on_back: None,
        }
    }

    /// Opens the review for this service (by id) on the first render, e.g.
    /// when a command routes straight to "Connect Jira".
    pub fn reviewing(mut self, id: impl Into<SharedString>) -> Self {
        self.reviewing = Some(id.into());
        self
    }

    /// Focuses the search field when the page first renders (on by default).
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    /// Called once a service finishes connecting or its permissions are saved
    /// (`true`, with the chosen grants in `permissions[..].granted`), and when
    /// it disconnects (`false`).
    pub fn on_connect(
        mut self,
        handler: impl Fn(&Integration, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_connect = Some(Rc::new(handler));
        self
    }

    /// Escape on an empty search.
    pub fn on_back(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_back = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for IntegrationPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let total = self.integrations.len();
        let integrations = Rc::new(self.integrations);
        let initial_connected: Vec<SharedString> = integrations
            .iter()
            .filter(|i| i.connected)
            .map(|i| i.id.clone())
            .collect();
        let reviewing = self.reviewing.clone();
        let autofocus = self.autofocus;
        let seed = integrations.clone();

        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            move |window, cx| {
                let input = cx.new(|cx| TextInput::new(window, cx).placeholder("Search apps"));
                let subscription = cx.subscribe_in(
                    &input,
                    window,
                    |s: &mut PickerState, _, event, window, cx| s.on_input(event, window, cx),
                );
                let mut state = PickerState {
                    input,
                    _subscription: subscription,
                    query: SharedString::default(),
                    connected: initial_connected,
                    connecting: None,
                    grants: HashMap::new(),
                    review: None,
                    arrived: None,
                    generation: 0,
                    dialog_focus: cx.focus_handle(),
                    scroll: ScrollHandle::new(),
                    _handshake: None,
                    integrations: seed,
                    on_connect: None,
                    on_back: None,
                };
                if let Some(service) = reviewing.and_then(|id| state.service(&id).cloned()) {
                    let manage = state.connected.contains(&service.id);
                    state.generation += 1;
                    state.review = Some(Review {
                        granted: service.default_grants(),
                        service: service.id,
                        manage,
                        generation: state.generation,
                    });
                    let handle = state.dialog_focus.clone();
                    window.defer(cx, move |window, cx| window.focus(&handle, cx));
                } else if autofocus {
                    let handle = state.input.read(cx).focus_handle(cx);
                    window.defer(cx, move |window, cx| window.focus(&handle, cx));
                }
                state
            },
        );

        let input = state.update(cx, |s, _| {
            s.integrations = integrations.clone();
            s.on_connect = self.on_connect.clone();
            s.on_back = self.on_back.clone();
            s.input.clone()
        });
        input.update(cx, |input, _| {
            input.set_colors(Some(t.placeholder), Some(t.caret), Some(t.selection))
        });

        let s = state.read(cx);
        let groups = arranged(&integrations, &s.connected, &s.query);
        let busy = s.connecting.is_some();
        let rows = RowContext {
            key: &key,
            connecting: s.connecting.as_ref(),
            arrived: s.arrived.as_ref(),
            busy,
            state: &state,
        };
        let mut sections: Vec<AnyElement> = groups
            .iter()
            .map(|(section, services)| {
                pink::section_card(&t)
                    .child(pink::section_header(
                        section.title(),
                        Some(section.description(&self.app).into()),
                        &t,
                    ))
                    .children(
                        services
                            .iter()
                            .map(|service| service_row(service, *section, &rows, &t)),
                    )
                    .into_any_element()
            })
            .collect();
        if groups.is_empty() {
            sections.push(no_matches(&s.query, &t).into_any_element());
        }
        let dialog = s.review.as_ref().and_then(|review| {
            let service = integrations.iter().find(|i| i.id == review.service)?;
            Some(review_dialog(
                &key,
                &self.app,
                service,
                review,
                &s.dialog_focus,
                &state,
                &t,
            ))
        });
        let scroll = s.scroll.clone();

        // Pink: the file search field (`files/mod.rs` `render_header` on
        // `surface_chrome::input`), 28 tall like the title bar's actions.
        let search = pink::toolbar_input(&t)
            .flex_none()
            .w(px(SEARCH_W))
            .line_height(px(16.))
            .child(
                svg()
                    .path(ICON_SEARCH)
                    .size(px(pink::icon_size::XS))
                    .flex_none()
                    .text_color(t.text_faint),
            )
            .child(div().flex_1().min_w_0().child(input))
            .into_any_element();

        let page = page_column()
            .child(page_title_bar("Integrations", Some(total), vec![search], &t))
            .child(page_subtitle(
                format!(
                    "Apps that read from and act in {}. Each one asks before it connects, and gets only the access you leave on.",
                    self.app
                ),
                &t,
            ))
            .children(sections);

        motion::keep_alive(window);
        div()
            .id(self.id)
            .relative()
            .w(px(PLANE_W))
            .h(px(PLANE_H))
            .flex()
            .flex_col()
            .rounded(px(CANVAS_RADIUS))
            .border_1()
            .border_color(t.hairline(0.08))
            .bg(t.canvas())
            .overflow_hidden()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .p(px(PLANE_INSET))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("integration-page")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&scroll)
                            .child(page),
                    ),
            )
            .children(dialog)
    }
}

/// What every row reads from the page's state.
struct RowContext<'a> {
    key: &'a str,
    connecting: Option<&'a SharedString>,
    arrived: Option<&'a (SharedString, u32)>,
    busy: bool,
    state: &'a Entity<PickerState>,
}

/// One service (Pink: `extensions.rs` `package_row`): the identity tile, the
/// name over its description and a meta line, then the status badge and the
/// row's actions – Connect… while available, Permissions… and Disconnect once
/// connected. Every action waits while a connection is being made.
fn service_row(service: &Integration, section: Section, rows: &RowContext, t: &Pink) -> AnyElement {
    let key = rows.key;
    let id = service.id.clone();
    let connecting = rows.connecting == Some(&service.id);
    let enabled = !rows.busy;
    let arrived = rows
        .arrived
        .filter(|(arrived, _)| *arrived == service.id)
        .map(|(_, generation)| *generation);

    let mut meta: Vec<AnyElement> = Vec::new();
    if !service.category.is_empty() {
        meta.push(div().child(service.category.clone()).into_any_element());
    }
    if !service.domain.is_empty() {
        // Pink: the package hash fragment, in mono 11.
        meta.push(
            div()
                .font_family(pink::MONO)
                .text_size(px(11.))
                .child(service.domain.clone())
                .into_any_element(),
        );
    }
    if section == Section::Connected {
        let status = service
            .status
            .clone()
            .filter(|_| arrived.is_none())
            .unwrap_or_else(|| "Connected just now".into());
        meta.push(div().child(status).into_any_element());
    }
    let text = pink::row_text(
        service.name.clone(),
        (!service.about.is_empty()).then(|| service.about.clone()),
        t,
    )
    .child(meta_line(meta, t));

    // Pink: `badge_active` once connected; MCP's pending badge (warning copy
    // on the neutral outline) while connecting.
    let badge = match section {
        Section::Connected => Some(("Connected", pink::badge("Connected", Tone::Success, t))),
        Section::Available if connecting => Some((
            "Connecting…",
            pink::badge("Connecting…", Tone::Neutral, t)
                .text_color(t.readable_tone(t.warning, 1.0)),
        )),
        Section::Available => None,
    };
    let badge = badge.map(|(label, badge)| {
        // A status that just changed settles in (Pink `STATUS_SWAP`).
        if connecting || arrived.is_some() {
            motion::status_swap(
                label_id(&format!("{key}-badge-{id}-{}", arrived.unwrap_or(0)), label),
                div().flex_none().child(badge),
            )
            .into_any_element()
        } else {
            badge.into_any_element()
        }
    });

    let action = |name: &str, label: &str, kind: ButtonKind| {
        pink::button_enabled(
            format!("{key}-{name}-{id}"),
            kind,
            ControlSize::Sm,
            enabled,
            t,
        )
        .aria_label(SharedString::from(format!("{label} {}", service.name)))
        .child(SharedString::from(label.to_string()))
    };
    let mut controls = pink::row_controls().children(badge);
    match section {
        Section::Available => {
            let (picker, target) = (rows.state.clone(), id.clone());
            controls = controls.child(action("connect", "Connect…", ButtonKind::Secondary).when(
                enabled,
                |el| {
                    el.on_click(move |_, window, cx| {
                        picker.update(cx, |s, cx| s.open_review(target.clone(), window, cx))
                    })
                },
            ));
        }
        Section::Connected => {
            let (picker, target) = (rows.state.clone(), id.clone());
            controls = controls.child(action("access", "Permissions…", ButtonKind::Ghost).when(
                enabled,
                |el| {
                    el.on_click(move |_, window, cx| {
                        picker.update(cx, |s, cx| s.open_review(target.clone(), window, cx))
                    })
                },
            ));
            let (picker, target) = (rows.state.clone(), id.clone());
            controls = controls.child(action("disconnect", "Disconnect", ButtonKind::Ghost).when(
                enabled,
                |el| {
                    el.on_click(move |_, window, cx| {
                        picker.update(cx, |s, cx| s.disconnect(target.clone(), window, cx))
                    })
                },
            ));
        }
    }

    let row = pink::card_row(false, t)
        .child(service_tile(service, t))
        .child(text)
        .child(controls);
    match arrived {
        // The row that just changed section arrives like a new list row
        // (Pink `TOOL_ROW_REVEAL`: 360 ms expo-out, 4 px rise).
        Some(generation) => motion::rise_in(
            ElementId::NamedInteger(format!("{key}-arrive-{id}").into(), u64::from(generation)),
            TOOL_ROW_REVEAL,
            4.0,
            row,
        )
        .into_any_element(),
        None => row.into_any_element(),
    }
}

/// The row's identity tile (Pink `widgets::row_tile`: 28, radius 7, `border`
/// on a Quaternary plate). The service's initial stands in for the 16 px
/// icon, sized and weighted like Pink's project tile letter.
fn service_tile(service: &Integration, t: &Pink) -> impl IntoElement {
    div()
        .flex_none()
        .size(px(28.))
        .rounded(px(radius::TILE))
        .border_1()
        .border_color(t.border)
        .bg(t.fill(FillTier::Quaternary))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(14.))
        .line_height(px(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.text_muted)
        .child(service.monogram())
}

/// The quiet line under a row's title (Pink `widgets::meta_line`): 12 in
/// `readable_muted(0.8)`, fragments 8 apart with a "·" between them that
/// fades to 30% on dark.
fn meta_line(fragments: Vec<AnyElement>, t: &Pink) -> impl IntoElement {
    let dot = if t.dark {
        t.text_muted.opacity(0.3)
    } else {
        t.text_muted
    };
    let mut line = div()
        .min_w_0()
        .mt(px(pink::space::TEXT_STACK_GAP))
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x(px(8.))
        .gap_y(px(2.))
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(t.readable_muted(0.8));
    for (ix, fragment) in fragments.into_iter().enumerate() {
        if ix > 0 {
            line = line.child(div().text_color(dot).child("·"));
        }
        line = line.child(div().min_w_0().flex_shrink(1.).child(fragment));
    }
    line
}

/// A search that matches nothing (Pink's empty package row: a tile and a
/// row text in a section card).
fn no_matches(query: &str, t: &Pink) -> impl IntoElement {
    pink::section_card(t).child(
        pink::card_row(true, t)
            .child(pink::row_tile(ICON_SEARCH, t))
            .child(pink::row_text(
                format!("No apps match “{}”", query.trim()),
                Some("Search by name, by a category like Chat, or by domain.".into()),
                t,
            )),
    )
}

/// The review (Pink: `extensions.rs` `render_review` in `popover::modal`):
/// a 460 dialog on the scrim with the title, body and the service's domain in
/// mono, one row per permission with its switch – required ones locked on –
/// then Cancel and the one Primary action. Escape cancels, Enter confirms.
fn review_dialog(
    key: &str,
    app: &SharedString,
    service: &Integration,
    review: &Review,
    focus: &FocusHandle,
    state: &Entity<PickerState>,
    t: &Pink,
) -> impl IntoElement {
    let name = &service.name;
    let (title, body, confirm) = if review.manage {
        (
            format!("{name} permissions"),
            format!(
                "Changes apply when you save. Required access stays on while {name} is connected."
            ),
            "Save",
        )
    } else {
        (
            format!("Connect {name}?"),
            format!(
                "{name} asks for the access below. Connecting applies to {app} only, and you can disconnect at any time."
            ),
            "Connect",
        )
    };

    // Pink: the empty lifecycle block's `mt 8` plus the capabilities' `mt 12`.
    let mut permissions = div().mt(px(20.)).flex().flex_col().gap(px(10.));
    if service.permissions.is_empty() {
        permissions = permissions.child(pink::dialog_body("This app asks for no permissions.", t));
    }
    for (ix, permission) in service.permissions.iter().enumerate() {
        let on = review
            .granted
            .get(ix)
            .copied()
            .unwrap_or(permission.granted);
        let switch_id = format!("{key}-grant-{}-{ix}", review.generation);
        let switch = if permission.required {
            pink::switch_control_disabled(
                switch_id,
                permission.label.clone(),
                true,
                format!("Required to connect {name}"),
                t,
            )
        } else {
            let flip = state.clone();
            pink::switch_control(switch_id, permission.label.clone(), on, t)
                .on_click(move |_, _, cx| flip.update(cx, |s, cx| s.toggle_grant(ix, cx)))
        };
        let description = if permission.required {
            format!("{} · required", permission.detail)
        } else {
            permission.detail.to_string()
        };
        permissions = permissions.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .child(pink::row_text(
                    permission.label.clone(),
                    Some(description.into()),
                    t,
                ))
                .child(switch),
        );
    }

    let (cancel, confirm_state) = (state.clone(), state.clone());
    let (escape, enter) = (state.clone(), state.clone());
    let card = pink::dialog_card(t)
        .w(px(REVIEW_W))
        .track_focus(focus)
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            match event.keystroke.key.as_str() {
                "escape" => escape.update(cx, |s, cx| s.cancel_review(window, cx)),
                "enter" => enter.update(cx, |s, cx| s.confirm_review(window, cx)),
                _ => return,
            }
            cx.stop_propagation();
        })
        .child(pink::dialog_title(title, t))
        .child(div().mt(px(8.)).child(pink::dialog_body(body, t)))
        .when(!service.domain.is_empty(), |card| {
            card.child(
                div()
                    .mt(px(6.))
                    .font_family(pink::MONO)
                    .text_size(px(11.))
                    .text_color(t.readable_muted(0.8))
                    .child(service.domain.clone()),
            )
        })
        .child(permissions)
        .child(
            div()
                .mt(px(16.))
                .flex()
                .flex_row()
                .justify_end()
                .gap(px(8.))
                .child(
                    pink::button(
                        format!("{key}-review-cancel"),
                        ButtonKind::Ghost,
                        ControlSize::Lg,
                        t,
                    )
                    .aria_label("Cancel")
                    .child("Cancel")
                    .on_click(move |_, window, cx| {
                        cancel.update(cx, |s, cx| s.cancel_review(window, cx))
                    }),
                )
                .child(
                    pink::button(
                        format!("{key}-review-confirm"),
                        ButtonKind::Primary,
                        ControlSize::Lg,
                        t,
                    )
                    .aria_label(SharedString::from(format!("{confirm} {name}")))
                    .child(confirm)
                    .on_click(move |_, window, cx| {
                        confirm_state.update(cx, |s, cx| s.confirm_review(window, cx))
                    }),
                ),
        );

    // Pink `popover::modal`: the scrim swallows the pointer and the card
    // arrives with `dialog_in`. Here it covers the page's plane.
    pink::modal_scrim(t)
        .id(SharedString::from(format!("{key}-review-scrim")))
        .absolute()
        .top_0()
        .left_0()
        .rounded(px(CANVAS_RADIUS))
        .occlude()
        .child(motion::dialog_in(
            ElementId::NamedInteger(format!("{key}-review").into(), u64::from(review.generation)),
            div().child(card),
        ))
}

/// Pink `widgets::page_column`: the centered page, at most 720 wide,
/// `px 32 / pt 28 / pb 72`.
fn page_column() -> gpui::Div {
    div()
        .w_full()
        .max_w(px(layout::PAGE_MAX_WIDTH))
        .mx_auto()
        .px(px(32.))
        .pt(px(28.))
        .pb(px(72.))
        .flex()
        .flex_col()
}

/// Pink `widgets::page_title_bar` + `page_header`: the Title1 title with its
/// count (13, `readable_muted(0.7)`) on one baseline, and the page's actions
/// on the title's center line at the right. It wraps.
fn page_title_bar(
    title: &'static str,
    count: Option<usize>,
    actions: Vec<AnyElement>,
    t: &Pink,
) -> gpui::Div {
    let header = div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(px(10.))
        .child(
            div()
                .type_style(TextStyle::Title1)
                .text_color(t.text)
                .child(title),
        )
        .when_some(count, |el, count| {
            el.child(
                div()
                    .text_size(px(13.))
                    .text_color(t.readable_muted(0.7))
                    .child(count.to_string()),
            )
        });
    div()
        .w_full()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_x(px(16.))
        .gap_y(px(8.))
        .child(header)
        .when(!actions.is_empty(), |bar| {
            bar.child(
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .children(actions),
            )
        })
}

/// Pink `widgets::page_subtitle`: `mt 6`, 13/19 `readable_muted(0.9)`, held
/// to 600.
fn page_subtitle(copy: String, t: &Pink) -> gpui::Div {
    div()
        .mt(px(6.))
        .max_w(px(600.))
        .text_size(px(13.))
        .line_height(px(19.))
        .text_color(t.readable_muted(0.9))
        .child(copy)
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        IntegrationPicker::new("integration-picker", "Orbit", default_integrations())
    })
}
