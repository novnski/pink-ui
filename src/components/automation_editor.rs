//! Automation Editor – the form that creates or edits a scheduled prompt
//! (Pink: automations.rs `render_editor`, with `input_frame` and
//! `render_select`).
//!
//! One settings section card. The header names it ("New automation" or
//! "Edit automation") with what saving does and a Close button. Then its
//! groups, each a card row with a semibold caption: the name and the message
//! each run sends; the Schedule – five kinds as choice chips, the fields that
//! kind needs (a date, a time and zone, weekday chips, an interval, a
//! recurrence rule or a pull request to watch) and the schedule in words
//! beside a clock, or the first field that can't be read yet; Where it runs –
//! the destination in one sentence, the project pop-up, New chat / Existing
//! chat on a segmented track and the chat pop-up; the Model chip; and More
//! options, folded to a summary that still names every value. The footer
//! asks the host to review the exact change, shows its answer (the schedule,
//! the next runs, where and with what it runs) and only then enables Save.
//! Any edit after a review asks for another.
//!
//! The host is builder data: `on_review` answers a review, `on_save` applies
//! the change (an `Err` is a refusal the footer shows), both after a short
//! simulated round trip. Escape closes an open menu, then the editor.
//!
//! ```ignore
//! AutomationEditor::new("automation")
//!     .projects(vec![Project::new("web", "acme-web").path("~/code/acme-web")])
//!     .models(vec![ModelChoice::new("Claude Code", "Opus 4.1").reasoning("High")])
//!     .draft(AutomationDraft::new().name("Nightly triage").calendar("09:00", WEEKDAYS))
//!     .on_review(|draft| host.preview(draft))
//!     .on_save(|draft, _, _| host.apply(draft))
//!     .on_close(|_, _| close_editor())
//! ```

use std::{rc::Rc, sync::Arc, time::Duration};
use web_time::Instant;

use gpui::{
    Anchor, AnyElement, AnyView, App, ClickEvent, Context, DispatchPhase, Div, ElementId, Entity,
    FocusHandle, FontWeight, Hsla, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, Role, SharedString, Stateful, Subscription, Task, Toggled, Window, anchored,
    canvas, deferred, div, prelude::*, px, svg,
};

use crate::{
    motion::{self, MENU_OUT, MENU_RISE_DOWN},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, Segment, layout, space},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Automation Editor";
pub const DESCRIPTION: &str = "The form behind a scheduled prompt on a Pink settings card: a name and message, a schedule picked from five kinds with weekday chips and the cadence spelled out beside a clock, where each run goes, the model, folded extra options, and a host review that has to pass before Save";

const ICON_CLOCK: &str = "icons/automation-editor/clock-circle.svg";
const ICON_FOLDER: &str = "icons/automation-editor/folder.svg";
const ICON_CHAT: &str = "icons/automation-editor/chat-round-line.svg";
const ICON_BELL: &str = "icons/automation-editor/bell.svg";
const ICON_BOT: &str = "icons/automation-editor/bot.svg";

/// Pink's settings column: `max_w 720` less its 32 px gutters.
const CARD_W: f32 = 656.;
/// A setting's label column (Pink `setting`: `w 112`).
const SETTING_LABEL_W: f32 = 112.;
/// The pop-up buttons (Pink `render_select`: `min_w 180`, `max_w 320`) and
/// their menus (`w 320`, the list `max_h 280`).
const SELECT_MIN_W: f32 = 180.;
const SELECT_MAX_W: f32 = 320.;
const MENU_W: f32 = 320.;
const MENU_LIST_MAX_H: f32 = 280.;
/// A menu row's caption (Pink: `max_w 200`, 11 in `readable_muted(0.8)`).
const MENU_CAPTION_MAX_W: f32 = 200.;
/// "Each run" segmented track (Pink: 220 wide, Sm).
const FRESH_TRACK_W: f32 = 220.;
/// The lateness field (Pink: `w 72`).
const LATENESS_W: f32 = 72.;
/// The model chip (Pink pickers.rs: 28 tall, icon 16).
const CHIP_ICON: f32 = 16.;
/// A field's text line inside its frame.
const FIELD_LINE: f32 = 18.;
/// Pink's limits (automations.rs `build_definition`).
const NAME_MAX: usize = 160;
const PROMPT_MAX: usize = 32 * 1024;
const RULE_MAX: usize = 1024;

/// Weekday chips, Monday first (Pink `WEEKDAYS`).
pub const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const RRULE_DAYS: [&str; 7] = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];
/// Monday to Friday, for [`AutomationDraft::calendar`].
pub const WORKDAYS: [bool; 7] = [true, true, true, true, true, false, false];

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

/// How an automation is triggered (Pink `TriggerKind`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ScheduleKind {
    #[default]
    Once,
    Interval,
    Calendar,
    Custom,
    PullRequest,
}

impl ScheduleKind {
    pub const ALL: [ScheduleKind; 5] = [
        ScheduleKind::Once,
        ScheduleKind::Interval,
        ScheduleKind::Calendar,
        ScheduleKind::Custom,
        ScheduleKind::PullRequest,
    ];

    /// Its chip (Pink `TriggerKind::label`).
    pub fn label(self) -> &'static str {
        match self {
            Self::Once => "Once",
            Self::Interval => "Interval",
            Self::Calendar => "Daily or weekly",
            Self::Custom => "Custom",
            Self::PullRequest => "Pull request",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::Interval => "interval",
            Self::Calendar => "calendar",
            Self::Custom => "custom",
            Self::PullRequest => "pr",
        }
    }
}

/// Who hears about runs (Pink `AutomationNotificationPolicy`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NotificationPolicy {
    AllRuns,
    #[default]
    ImportantUpdates,
    Never,
}

impl NotificationPolicy {
    pub const ALL: [NotificationPolicy; 3] = [
        NotificationPolicy::AllRuns,
        NotificationPolicy::ImportantUpdates,
        NotificationPolicy::Never,
    ];

    /// Pink `notification_label`.
    pub fn label(self) -> &'static str {
        match self {
            Self::AllRuns => "All runs",
            Self::ImportantUpdates => "Important updates",
            Self::Never => "Never",
        }
    }

    /// Pink `notification_caption`.
    pub fn caption(self) -> &'static str {
        match self {
            Self::AllRuns => "Notify when every run finishes",
            Self::ImportantUpdates => "Failures, uncertain runs and attention",
            Self::Never => "Runs stay quiet; history still records them",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::AllRuns => "all",
            Self::ImportantUpdates => "important",
            Self::Never => "never",
        }
    }
}

/// A chat a run can be sent to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chat {
    pub id: SharedString,
    pub title: SharedString,
}

impl Chat {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
        }
    }
}

/// A project on this device: an automation belongs to one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: SharedString,
    pub name: SharedString,
    /// Shown under the name in the menu (Pink: the folder path).
    pub path: Option<SharedString>,
    /// Its open chats.
    pub chats: Vec<Chat>,
}

impl Project {
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            path: None,
            chats: Vec::new(),
        }
    }

    pub fn path(mut self, path: impl Into<SharedString>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn chat(mut self, id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        self.chats.push(Chat::new(id, title));
        self
    }
}

/// An agent and model the runs can use (Pink's composer model chip).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelChoice {
    /// The agent, e.g. "Claude Code".
    pub harness: SharedString,
    pub model: SharedString,
    /// The reasoning level, shown muted after the model.
    pub reasoning: Option<SharedString>,
    /// A warning under the chip while this one is picked (Pink
    /// `model_note`, e.g. a level the model doesn't offer).
    pub note: Option<SharedString>,
}

impl ModelChoice {
    pub fn new(harness: impl Into<SharedString>, model: impl Into<SharedString>) -> Self {
        Self {
            harness: harness.into(),
            model: model.into(),
            reasoning: None,
            note: None,
        }
    }

    pub fn reasoning(mut self, level: impl Into<SharedString>) -> Self {
        self.reasoning = Some(level.into());
        self
    }

    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// "Opus 4.1 on Claude Code, High" (Pink `execution_label`).
    pub fn label(&self) -> String {
        let mut label = format!("{} on {}", self.model, self.harness);
        if let Some(level) = &self.reasoning {
            label.push_str(&format!(", {level}"));
        }
        label
    }
}

/// Everything the form holds: what Review and Save send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutomationDraft {
    pub name: SharedString,
    pub prompt: SharedString,
    pub schedule: ScheduleKind,
    /// `YYYY-MM-DD` (Once; a custom rule's optional start).
    pub date: SharedString,
    /// `HH:MM` (Once, Daily or weekly; a custom rule's optional start).
    pub time: SharedString,
    /// An IANA zone, e.g. "Europe/Lisbon".
    pub timezone: SharedString,
    /// Minutes between runs (Interval).
    pub every: SharedString,
    /// The days a Daily or weekly schedule runs, Monday first; none means
    /// every day.
    pub weekdays: [bool; 7],
    /// A recurrence rule (Custom).
    pub rule: SharedString,
    /// `owner/repository`, the pull request number, minutes between checks
    /// and the wake budget (Pull request).
    pub repository: SharedString,
    pub number: SharedString,
    pub poll: SharedString,
    pub budget: SharedString,
    /// The project each run belongs to (a [`Project::id`]).
    pub project: Option<SharedString>,
    /// A new chat each run; otherwise `chat`.
    pub fresh: bool,
    pub chat: Option<SharedString>,
    /// Index into the editor's models.
    pub model: usize,
    pub notifications: NotificationPolicy,
    /// Archive a run's new chat when it finishes without problems.
    pub archive_successful_runs: bool,
    /// Skip a run that would start more than this many minutes late.
    pub lateness: SharedString,
}

impl Default for AutomationDraft {
    fn default() -> Self {
        Self::new()
    }
}

impl AutomationDraft {
    /// Pink's starting values: once, every 60 min, a 15 min check and 10
    /// wakes for a pull request, 10 min of lateness, a new chat each run.
    pub fn new() -> Self {
        Self {
            name: SharedString::default(),
            prompt: SharedString::default(),
            schedule: ScheduleKind::Once,
            date: SharedString::default(),
            time: SharedString::default(),
            timezone: "UTC".into(),
            every: "60".into(),
            weekdays: [false; 7],
            rule: SharedString::default(),
            repository: SharedString::default(),
            number: SharedString::default(),
            poll: "15".into(),
            budget: "10".into(),
            project: None,
            fresh: true,
            chat: None,
            model: 0,
            notifications: NotificationPolicy::ImportantUpdates,
            archive_successful_runs: false,
            lateness: "10".into(),
        }
    }

    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = name.into();
        self
    }

    pub fn prompt(mut self, prompt: impl Into<SharedString>) -> Self {
        self.prompt = prompt.into();
        self
    }

    pub fn timezone(mut self, zone: impl Into<SharedString>) -> Self {
        self.timezone = zone.into();
        self
    }

    /// Once, at `date` (`YYYY-MM-DD`) and `time` (`HH:MM`).
    pub fn once(mut self, date: impl Into<SharedString>, time: impl Into<SharedString>) -> Self {
        self.schedule = ScheduleKind::Once;
        self.date = date.into();
        self.time = time.into();
        self
    }

    /// Every `minutes`.
    pub fn interval(mut self, minutes: u32) -> Self {
        self.schedule = ScheduleKind::Interval;
        self.every = minutes.to_string().into();
        self
    }

    /// Daily or weekly at `time`, on `weekdays` (Monday first; none: daily).
    pub fn calendar(mut self, time: impl Into<SharedString>, weekdays: [bool; 7]) -> Self {
        self.schedule = ScheduleKind::Calendar;
        self.time = time.into();
        self.weekdays = weekdays;
        self
    }

    /// A recurrence rule, e.g. `FREQ=WEEKLY;BYDAY=MO;BYHOUR=9;BYMINUTE=30`.
    pub fn custom(mut self, rule: impl Into<SharedString>) -> Self {
        self.schedule = ScheduleKind::Custom;
        self.rule = rule.into();
        self
    }

    /// Watch a pull request.
    pub fn pull_request(mut self, repository: impl Into<SharedString>, number: u64) -> Self {
        self.schedule = ScheduleKind::PullRequest;
        self.repository = repository.into();
        self.number = number.to_string().into();
        self
    }

    pub fn project(mut self, id: impl Into<SharedString>) -> Self {
        self.project = Some(id.into());
        self
    }

    /// Send each run to an existing chat instead of a new one.
    pub fn chat(mut self, id: impl Into<SharedString>) -> Self {
        self.fresh = false;
        self.chat = Some(id.into());
        self
    }

    pub fn model(mut self, index: usize) -> Self {
        self.model = index;
        self
    }

    pub fn notifications(mut self, policy: NotificationPolicy) -> Self {
        self.notifications = policy;
        self
    }

    pub fn archive_successful_runs(mut self, archive: bool) -> Self {
        self.archive_successful_runs = archive;
        self
    }

    pub fn lateness(mut self, minutes: u32) -> Self {
        self.lateness = minutes.to_string().into();
        self
    }
}

/// The host's answer to Review (Pink `AutomationPreview`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutomationReview {
    /// The schedule as the host reads it.
    pub description: SharedString,
    /// The next few runs, already formatted ("09:00, in 3 h").
    pub next_runs: Vec<SharedString>,
    /// The device the runs happen on.
    pub host: SharedString,
    /// The most access a run gets ("Limited access").
    pub access: SharedString,
}

impl AutomationReview {
    pub fn new(description: impl Into<SharedString>) -> Self {
        Self {
            description: description.into(),
            next_runs: Vec::new(),
            host: "this device".into(),
            access: "Limited access".into(),
        }
    }

    pub fn next_runs(mut self, runs: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.next_runs = runs.into_iter().map(Into::into).collect();
        self
    }

    pub fn host(mut self, host: impl Into<SharedString>) -> Self {
        self.host = host.into();
        self
    }

    pub fn access(mut self, access: impl Into<SharedString>) -> Self {
        self.access = access.into();
        self
    }
}

type ReviewHandler = Rc<dyn Fn(&AutomationDraft) -> Result<AutomationReview, String>>;
type SaveHandler = Rc<dyn Fn(&AutomationDraft, &mut Window, &mut App) -> Result<(), String>>;
type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

// ---------------------------------------------------------------------------
// Pure logic (Pink automations.rs)
// ---------------------------------------------------------------------------

/// A calendar date typed as `YYYY-MM-DD` (Pink `parse_date`).
pub fn parse_date(text: &str) -> Result<(i32, u32, u32), String> {
    const FORMAT: &str = "Enter the date as YYYY-MM-DD.";
    let text = text.trim();
    let mut parts = text.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(FORMAT.into());
    };
    let digits =
        |part: &str, len: usize| part.len() == len && part.bytes().all(|b| b.is_ascii_digit());
    if !digits(year, 4) || !digits(month, 2) || !digits(day, 2) {
        return Err(FORMAT.into());
    }
    let (year, month, day): (i32, u32, u32) = (
        year.parse().map_err(|_| FORMAT.to_string())?,
        month.parse().map_err(|_| FORMAT.to_string())?,
        day.parse().map_err(|_| FORMAT.to_string())?,
    );
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return Err(format!("{text} is not a calendar date."));
    }
    Ok((year, month, day))
}

/// A wall-clock time typed as `HH:MM` (Pink `parse_clock`).
pub fn parse_clock(text: &str) -> Result<(u32, u32), String> {
    const FORMAT: &str = "Enter the time as HH:MM.";
    let text = text.trim();
    let (hour, minute) = text.split_once(':').ok_or_else(|| FORMAT.to_string())?;
    let digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit());
    if hour.is_empty() || hour.len() > 2 || minute.len() != 2 || !digits(hour) || !digits(minute) {
        return Err(FORMAT.into());
    }
    let hour: u32 = hour.parse().map_err(|_| FORMAT.to_string())?;
    let minute: u32 = minute.parse().map_err(|_| FORMAT.to_string())?;
    if hour > 23 || minute > 59 {
        return Err("Enter a time between 00:00 and 23:59.".into());
    }
    Ok((hour, minute))
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => 31,
    }
}

/// A recurrence rule in words ("Every 2 weeks on Mon, Wed at 09:30"), or
/// `None` for anything the host's supported fields don't cover (Pink
/// `describe_rule`).
pub fn describe_rule(rule: &str) -> Option<String> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let body = rule.trim();
    let body = body.strip_prefix("RRULE:").unwrap_or(body);
    let mut freq = None;
    let mut interval = 1u32;
    let mut days = Vec::new();
    let mut month_days = Vec::new();
    let mut months = Vec::new();
    let mut hours = Vec::new();
    let mut minutes = Vec::new();
    let mut until = None;
    for part in body.split(';').filter(|part| !part.is_empty()) {
        let (key, value) = part.split_once('=')?;
        match key.to_ascii_uppercase().as_str() {
            "FREQ" => freq = Some(value.to_ascii_uppercase()),
            "INTERVAL" => interval = value.parse().ok().filter(|n| *n > 0)?,
            "BYDAY" => {
                for day in value.split(',') {
                    let split = day.len().checked_sub(2)?;
                    let (ordinal, code) = (day.get(..split)?, day.get(split..)?);
                    let index = RRULE_DAYS
                        .iter()
                        .position(|known| known.eq_ignore_ascii_case(code))?;
                    let name = WEEKDAYS[index];
                    days.push(match ordinal {
                        "" => name.to_string(),
                        "-1" => format!("last {name}"),
                        ordinal => {
                            let n: i32 = ordinal.trim_start_matches('+').parse().ok()?;
                            match n {
                                1 => format!("first {name}"),
                                2 => format!("second {name}"),
                                3 => format!("third {name}"),
                                4 => format!("fourth {name}"),
                                5 => format!("fifth {name}"),
                                -2 => format!("second-to-last {name}"),
                                _ => return None,
                            }
                        }
                    });
                }
            }
            "BYMONTHDAY" => {
                for day in value.split(',') {
                    let day: i32 = day.parse().ok()?;
                    month_days.push(match day {
                        1..=31 => day.to_string(),
                        -1 => "the last day".to_string(),
                        -31..=-2 => format!("{} days before the end", -day - 1),
                        _ => return None,
                    });
                }
            }
            "BYMONTH" => {
                for month in value.split(',') {
                    let month: usize = month.parse().ok().filter(|m| (1..=12).contains(m))?;
                    months.push(MONTHS[month - 1]);
                }
            }
            "BYHOUR" => {
                for hour in value.split(',') {
                    hours.push(hour.parse::<u32>().ok().filter(|h| *h < 24)?);
                }
            }
            "BYMINUTE" => {
                for minute in value.split(',') {
                    minutes.push(minute.parse::<u32>().ok().filter(|m| *m < 60)?);
                }
            }
            "WKST" => {
                RRULE_DAYS
                    .iter()
                    .position(|known| known.eq_ignore_ascii_case(value))?;
            }
            "UNTIL" => {
                let date = value.get(..8)?;
                if !date.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                until = Some(format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..8]));
            }
            _ => return None,
        }
    }
    let unit = match freq?.as_str() {
        "MINUTELY" => ("minute", "minutes"),
        "HOURLY" => ("hour", "hours"),
        "DAILY" => ("day", "days"),
        "WEEKLY" => ("week", "weeks"),
        "MONTHLY" => ("month", "months"),
        "YEARLY" => ("year", "years"),
        _ => return None,
    };
    let mut text = if interval == 1 {
        format!("Every {}", unit.0)
    } else {
        format!("Every {interval} {}", unit.1)
    };
    if !months.is_empty() {
        text.push_str(&format!(" in {}", months.join(", ")));
    }
    if !days.is_empty() {
        text.push_str(&format!(" on {}", days.join(", ")));
    }
    if !month_days.is_empty() {
        text.push_str(&format!(" on day {}", month_days.join(", ")));
    }
    match (hours.as_slice(), minutes.as_slice()) {
        ([hour], [minute]) => text.push_str(&format!(" at {hour:02}:{minute:02}")),
        ([hour], []) => text.push_str(&format!(" at {hour:02}:00")),
        ([], [minute]) => text.push_str(&format!(" at minute {minute}")),
        ([], []) => {}
        _ => {
            let hours: Vec<String> = hours.iter().map(|h| format!("{h:02}")).collect();
            let minutes: Vec<String> = minutes.iter().map(|m| format!("{m:02}")).collect();
            text.push_str(&format!(
                " at hours {} and minutes {}",
                if hours.is_empty() {
                    "any".into()
                } else {
                    hours.join(", ")
                },
                if minutes.is_empty() {
                    "00".into()
                } else {
                    minutes.join(", ")
                },
            ));
        }
    }
    if let Some(until) = until {
        text.push_str(&format!(", until {until}"));
    }
    Some(text)
}

/// The schedule in words, or the first field that can't be read yet (Pink
/// `editor_schedule` + `describe_trigger`, as the Schedule group shows it).
pub fn schedule_words(draft: &AutomationDraft) -> Result<String, String> {
    let zone = draft.timezone.trim();
    if zone.is_empty() {
        return Err("Enter an IANA time zone, e.g. Europe/Bucharest.".into());
    }
    let number = |text: &str, what: &str| {
        text.trim()
            .parse::<u64>()
            .map_err(|_| format!("Enter a whole number for {what}."))
    };
    let words = match draft.schedule {
        ScheduleKind::Once => {
            let (year, month, day) = parse_date(&draft.date)?;
            let (hour, minute) = parse_clock(&draft.time)?;
            format!("Once, {year:04}-{month:02}-{day:02} {hour:02}:{minute:02} {zone}")
        }
        ScheduleKind::Interval => {
            let every = number(&draft.every, "the interval")?;
            if every == 0 || every > u64::from(u32::MAX) {
                return Err("Enter an interval of at least one minute.".into());
            }
            let words = match every {
                1 => "Every minute".into(),
                60 => "Every hour".into(),
                1440 => "Every day".into(),
                minutes if minutes % 1440 == 0 => format!("Every {} days", minutes / 1440),
                minutes if minutes % 60 == 0 => format!("Every {} hours", minutes / 60),
                minutes => format!("Every {minutes} minutes"),
            };
            format!("{words} in {zone}")
        }
        ScheduleKind::Calendar => {
            let (hour, minute) = parse_clock(&draft.time)?;
            let days: Vec<&str> = WEEKDAYS
                .iter()
                .zip(draft.weekdays)
                .filter(|(_, on)| *on)
                .map(|(day, _)| *day)
                .collect();
            let days = if days.is_empty() {
                "Every day".to_string()
            } else {
                days.join(", ")
            };
            format!("{days} at {hour:02}:{minute:02} ({zone})")
        }
        ScheduleKind::Custom => {
            let rule = draft.rule.trim();
            let body = rule.strip_prefix("RRULE:").unwrap_or(rule);
            if body.is_empty() {
                return Err("Enter a recurrence rule, e.g. FREQ=DAILY;BYHOUR=9;BYMINUTE=0.".into());
            }
            if rule.len() > RULE_MAX {
                return Err(format!("Keep the rule under {RULE_MAX} bytes."));
            }
            if !body
                .split(';')
                .any(|part| part.to_ascii_uppercase().starts_with("FREQ="))
            {
                return Err("A rule needs a frequency, e.g. FREQ=WEEKLY.".into());
            }
            if !draft.date.trim().is_empty() || !draft.time.trim().is_empty() {
                parse_date(&draft.date)?;
                parse_clock(&draft.time)?;
            }
            let words = describe_rule(rule).unwrap_or_else(|| format!("Custom rule {rule}"));
            format!("{words} in {zone}")
        }
        ScheduleKind::PullRequest => {
            let repository = draft.repository.trim();
            let valid = repository.split_once('/').is_some_and(|(owner, repo)| {
                !owner.is_empty() && !repo.is_empty() && !repo.contains('/')
            });
            if !valid {
                return Err("Enter the repository as owner/repository.".into());
            }
            let pr = number(&draft.number, "the pull request number")?;
            let poll = number(&draft.poll, "the check interval")?;
            let budget = number(&draft.budget, "the wake budget")?;
            if !(1..=1440).contains(&poll) {
                return Err("Check every 1 to 1440 minutes.".into());
            }
            if !(1..=100).contains(&budget) {
                return Err("Allow 1 to 100 wakes.".into());
            }
            format!("Watch {repository}#{pr}, checked every {poll} min, up to {budget} wakes")
        }
    };
    Ok(words)
}

/// Why the draft can't be sent for review, if it can't (Pink
/// `build_definition`: name, message, destination, schedule, lateness).
pub fn validate(draft: &AutomationDraft, projects: &[Project]) -> Result<(), String> {
    let name = draft.name.trim();
    if name.is_empty() || name.len() > NAME_MAX {
        return Err("Enter a name (up to 160 bytes).".into());
    }
    if draft.prompt.trim().is_empty() || draft.prompt.len() > PROMPT_MAX {
        return Err("Enter what each run sends (up to 32 KiB).".into());
    }
    let project_id = draft
        .project
        .as_ref()
        .ok_or_else(|| "Choose the project each run belongs to.".to_string())?;
    let Some(project) = projects.iter().find(|p| &p.id == project_id) else {
        return Err("That project is no longer on this device. Choose another project.".into());
    };
    if !draft.fresh {
        let chat = draft
            .chat
            .as_ref()
            .ok_or_else(|| "Choose the chat each run is sent to.".to_string())?;
        if !project.chats.iter().any(|c| &c.id == chat) {
            return Err(format!("Choose a chat in {}.", project.name));
        }
    }
    schedule_words(draft)?;
    let lateness = draft
        .lateness
        .trim()
        .parse::<u64>()
        .map_err(|_| "Enter a whole number for the missed-run tolerance.".to_string())?;
    if lateness > 24 * 60 {
        return Err("The missed-run tolerance is at most 24 hours.".into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The component
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct AutomationEditor {
    id: ElementId,
    existing: bool,
    paused: bool,
    created_from: Option<SharedString>,
    projects: Vec<Project>,
    models: Vec<ModelChoice>,
    draft: AutomationDraft,
    latency: Duration,
    on_review: Option<ReviewHandler>,
    on_save: Option<SaveHandler>,
    on_close: Option<CloseHandler>,
}

impl AutomationEditor {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            existing: false,
            paused: false,
            created_from: None,
            projects: Vec::new(),
            models: Vec::new(),
            draft: AutomationDraft::new(),
            latency: Duration::from_millis(900),
            on_review: None,
            on_save: None,
            on_close: None,
        }
    }

    /// Edit a saved automation ("Edit automation") instead of a new one.
    pub fn existing(mut self, existing: bool) -> Self {
        self.existing = existing;
        self
    }

    /// The saved automation is paused: Save keeps it paused.
    pub fn paused(mut self, paused: bool) -> Self {
        self.paused = paused;
        self
    }

    /// A new automation started from a chat ("Created from …").
    pub fn created_from(mut self, chat: impl Into<SharedString>) -> Self {
        self.created_from = Some(chat.into());
        self
    }

    /// The projects on this device and their chats.
    pub fn projects(mut self, projects: impl IntoIterator<Item = Project>) -> Self {
        self.projects = projects.into_iter().collect();
        self
    }

    /// The agents and models the model chip offers.
    pub fn models(mut self, models: impl IntoIterator<Item = ModelChoice>) -> Self {
        self.models = models.into_iter().collect();
        self
    }

    /// The values the form opens with.
    pub fn draft(mut self, draft: AutomationDraft) -> Self {
        self.draft = draft;
        self
    }

    /// The simulated round trip to the host for Review and Save.
    pub fn latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    /// The host's review of the exact draft (Pink `preview`); an `Err` is
    /// its refusal.
    pub fn on_review(
        mut self,
        handler: impl Fn(&AutomationDraft) -> Result<AutomationReview, String> + 'static,
    ) -> Self {
        self.on_review = Some(Rc::new(handler));
        self
    }

    /// Applies the reviewed draft (Pink `mutate`). `Ok` means it is saved:
    /// Pink closes the editor then. An `Err` is a refusal the footer shows.
    pub fn on_save(
        mut self,
        handler: impl Fn(&AutomationDraft, &mut Window, &mut App) -> Result<(), String> + 'static,
    ) -> Self {
        self.on_save = Some(Rc::new(handler));
        self
    }

    /// Close (the button, or Escape with no menu open).
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

/// The form's text fields, in tab order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Name,
    Prompt,
    Date,
    Time,
    Timezone,
    Every,
    Rule,
    Repository,
    Number,
    Poll,
    Budget,
    Lateness,
}

impl Field {
    const ALL: [Field; 12] = [
        Field::Name,
        Field::Prompt,
        Field::Date,
        Field::Time,
        Field::Timezone,
        Field::Every,
        Field::Rule,
        Field::Repository,
        Field::Number,
        Field::Poll,
        Field::Budget,
        Field::Lateness,
    ];

    /// Pink's placeholders (automations.rs `open_editor`).
    fn placeholder(self) -> &'static str {
        match self {
            Field::Name => "Name",
            Field::Prompt => "What each run sends",
            Field::Date => "YYYY-MM-DD",
            Field::Time => "HH:MM",
            Field::Timezone => "IANA time zone, e.g. Europe/Bucharest",
            Field::Every | Field::Poll | Field::Lateness => "Minutes",
            Field::Rule => "FREQ=WEEKLY;BYDAY=MO,WE;BYHOUR=9;BYMINUTE=30",
            Field::Repository => "owner/repository",
            Field::Number => "PR number",
            Field::Budget => "Wakes",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Field::Name => "name",
            Field::Prompt => "prompt",
            Field::Date => "date",
            Field::Time => "time",
            Field::Timezone => "timezone",
            Field::Every => "every",
            Field::Rule => "rule",
            Field::Repository => "repository",
            Field::Number => "number",
            Field::Poll => "poll",
            Field::Budget => "budget",
            Field::Lateness => "lateness",
        }
    }

    fn text(self, draft: &AutomationDraft) -> SharedString {
        match self {
            Field::Name => draft.name.clone(),
            Field::Prompt => draft.prompt.clone(),
            Field::Date => draft.date.clone(),
            Field::Time => draft.time.clone(),
            Field::Timezone => draft.timezone.clone(),
            Field::Every => draft.every.clone(),
            Field::Rule => draft.rule.clone(),
            Field::Repository => draft.repository.clone(),
            Field::Number => draft.number.clone(),
            Field::Poll => draft.poll.clone(),
            Field::Budget => draft.budget.clone(),
            Field::Lateness => draft.lateness.clone(),
        }
    }
}

/// A text field: the input and a tab stop in its frame that hands focus to
/// it (the input's own handle isn't a tab stop).
struct FieldInput {
    input: Entity<TextInput>,
    stop: FocusHandle,
}

/// Where the editor is in its round trip with the host (Pink `Phase`).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Phase {
    Editing,
    Previewing,
    Reviewed,
    Applying,
    Error(String),
}

/// The editor's pop-up menus, one open at a time (Pink `Menu`, plus the
/// model chip's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Menu {
    Project,
    Chat,
    Notifications,
    Model,
}

impl Menu {
    fn heading(self) -> &'static str {
        match self {
            Menu::Project => "Projects on this device",
            Menu::Chat => "Chats in this project",
            Menu::Notifications => "Notifications",
            Menu::Model => "Model",
        }
    }
}

/// The builder's host data, refreshed every render for the listeners.
#[derive(Default)]
struct Host {
    projects: Vec<Project>,
    models: Vec<ModelChoice>,
    latency: Duration,
    on_review: Option<ReviewHandler>,
    on_save: Option<SaveHandler>,
    on_close: Option<CloseHandler>,
}

struct EditorState {
    seeded: bool,
    colors_dark: Option<bool>,
    fields: Vec<FieldInput>,
    _subscriptions: Vec<Subscription>,
    schedule: ScheduleKind,
    weekdays: [bool; 7],
    project: Option<SharedString>,
    fresh: bool,
    chat: Option<SharedString>,
    model: usize,
    notifications: NotificationPolicy,
    archive: bool,
    more: bool,
    phase: Phase,
    /// The draft the review answered, and the answer.
    review: Option<(AutomationDraft, AutomationReview)>,
    /// The draft a refusal was about: once the form differs, it reads as
    /// editable again (Pink `error_basis`).
    error_basis: Option<AutomationDraft>,
    menu: Option<Menu>,
    menu_open: bool,
    menu_closing: Option<Instant>,
    /// The trigger press found its menu mounted, so the click leaves it
    /// closed (Pink `note_trigger_press`).
    menu_pressed_open: bool,
    menu_cursor: Option<usize>,
    menu_reap: Option<Task<()>>,
    flow: Option<Task<()>>,
    host: Rc<Host>,
}

impl EditorState {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut fields = Vec::with_capacity(Field::ALL.len());
        let mut subscriptions = Vec::new();
        for field in Field::ALL {
            let input = cx.new(|cx| {
                let input = TextInput::new(window, cx).placeholder(field.placeholder());
                if field == Field::Prompt {
                    input.multiline()
                } else {
                    input
                }
            });
            let stop = cx.focus_handle().tab_stop(true);
            // Tab lands on the frame; the frame hands focus to its input.
            subscriptions.push(cx.on_focus(&stop, window, {
                let input = input.clone();
                move |_, window, cx| TextInput::focus(&input, window, cx)
            }));
            subscriptions.push(cx.subscribe_in(&input, window, {
                let stop = stop.clone();
                move |s: &mut Self, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change(_) => cx.notify(),
                    InputEvent::Tab => {
                        // From the frame's own stop, so Tab continues past it.
                        window.focus(&stop, cx);
                        window.focus_next(cx);
                    }
                    InputEvent::Escape => s.escape(window, cx),
                    _ => {}
                }
            }));
            fields.push(FieldInput { input, stop });
        }
        Self {
            seeded: false,
            colors_dark: None,
            fields,
            _subscriptions: subscriptions,
            schedule: ScheduleKind::Once,
            weekdays: [false; 7],
            project: None,
            fresh: true,
            chat: None,
            model: 0,
            notifications: NotificationPolicy::ImportantUpdates,
            archive: false,
            more: false,
            phase: Phase::Editing,
            review: None,
            error_basis: None,
            menu: None,
            menu_open: false,
            menu_closing: None,
            menu_pressed_open: false,
            menu_cursor: None,
            menu_reap: None,
            flow: None,
            host: Rc::default(),
        }
    }

    fn field(&self, field: Field) -> &FieldInput {
        &self.fields[field as usize]
    }

    fn seed(&mut self, draft: &AutomationDraft, cx: &mut Context<Self>) {
        self.seeded = true;
        for field in Field::ALL {
            let text = field.text(draft);
            self.field(field)
                .input
                .update(cx, |input, cx| input.set_text(text, cx));
        }
        self.schedule = draft.schedule;
        self.weekdays = draft.weekdays;
        self.project = draft.project.clone();
        self.fresh = draft.fresh;
        self.chat = draft.chat.clone();
        self.model = draft.model;
        self.notifications = draft.notifications;
        self.archive = draft.archive_successful_runs;
    }

    /// What the form would send now.
    fn draft(&self, cx: &App) -> AutomationDraft {
        let text = |field: Field| self.field(field).input.read(cx).text();
        AutomationDraft {
            name: text(Field::Name),
            prompt: text(Field::Prompt),
            schedule: self.schedule,
            date: text(Field::Date),
            time: text(Field::Time),
            timezone: text(Field::Timezone),
            every: text(Field::Every),
            weekdays: self.weekdays,
            rule: text(Field::Rule),
            repository: text(Field::Repository),
            number: text(Field::Number),
            poll: text(Field::Poll),
            budget: text(Field::Budget),
            project: self.project.clone(),
            fresh: self.fresh,
            chat: self.chat.clone(),
            model: self.model,
            notifications: self.notifications,
            archive_successful_runs: self.fresh && self.archive,
            lateness: text(Field::Lateness),
        }
    }

    /// The phase as the footer shows it: a refusal about a form that has
    /// since changed reads as Editing (Pink `editor_phase`).
    fn shown_phase(&self, draft: &AutomationDraft) -> Phase {
        match &self.phase {
            Phase::Error(_) if self.error_basis.as_ref().is_some_and(|b| b != draft) => {
                Phase::Editing
            }
            phase => phase.clone(),
        }
    }

    /// The review still describes exactly what Save would send.
    fn review_current(&self, draft: &AutomationDraft) -> bool {
        self.review.as_ref().is_some_and(|(d, _)| d == draft)
    }

    fn busy(&self) -> bool {
        matches!(self.phase, Phase::Previewing | Phase::Applying)
    }

    /// Ask the host to review the exact draft (Pink `review_editor`).
    fn request_review(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let draft = self.draft(cx);
        if let Err(error) = validate(&draft, &self.host.projects) {
            self.error_basis = Some(draft);
            self.phase = Phase::Error(error);
            cx.notify();
            return;
        }
        self.close_menu(cx);
        self.phase = Phase::Previewing;
        self.review = None;
        let latency = self.host.latency;
        self.flow = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(latency).await;
            this.update(cx, |s, cx| {
                let answer = match &s.host.on_review {
                    Some(review) => review(&draft),
                    None => schedule_words(&draft).map(AutomationReview::new),
                };
                match answer {
                    Ok(review) => {
                        s.review = Some((draft, review));
                        s.phase = Phase::Reviewed;
                    }
                    Err(error) => {
                        s.error_basis = Some(draft);
                        s.phase = Phase::Error(error);
                    }
                }
                s.flow = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_open {
            self.close_menu(cx);
        } else if let Some(close) = self.host.on_close.clone() {
            close(window, cx);
        }
        cx.notify();
    }

    fn menu_shown(&self, menu: Menu) -> bool {
        self.menu == Some(menu) && (self.menu_open || self.menu_closing.is_some())
    }

    fn menu_is_open(&self, menu: Menu) -> bool {
        self.menu_open && self.menu == Some(menu)
    }

    /// The options a menu lists: value, title, caption (Pink
    /// `menu_options`).
    fn menu_options(&self, menu: Menu) -> Vec<(SharedString, SharedString, Option<SharedString>)> {
        match menu {
            Menu::Project => self
                .host
                .projects
                .iter()
                .map(|p| (p.id.clone(), p.name.clone(), p.path.clone()))
                .collect(),
            Menu::Chat => self
                .project_ref()
                .map(|p| {
                    p.chats
                        .iter()
                        .map(|c| (c.id.clone(), c.title.clone(), None))
                        .collect()
                })
                .unwrap_or_default(),
            Menu::Notifications => NotificationPolicy::ALL
                .iter()
                .map(|p| (p.key().into(), p.label().into(), Some(p.caption().into())))
                .collect(),
            Menu::Model => self
                .host
                .models
                .iter()
                .enumerate()
                .map(|(ix, m)| {
                    (
                        ix.to_string().into(),
                        match &m.reasoning {
                            Some(level) => format!("{}, {level}", m.model).into(),
                            None => m.model.clone(),
                        },
                        Some(m.harness.clone()),
                    )
                })
                .collect(),
        }
    }

    /// The menu's current value (Pink `menu_value`).
    fn menu_value(&self, menu: Menu) -> Option<SharedString> {
        match menu {
            Menu::Project => self.project.clone(),
            Menu::Chat => self.chat.clone(),
            Menu::Notifications => Some(self.notifications.key().into()),
            Menu::Model => Some(self.model.to_string().into()),
        }
    }

    fn project_ref(&self) -> Option<&Project> {
        let id = self.project.as_ref()?;
        self.host.projects.iter().find(|p| &p.id == id)
    }

    fn open_menu(&mut self, menu: Menu, cx: &mut Context<Self>) {
        let options = self.menu_options(menu);
        if options.is_empty() {
            return;
        }
        let value = self.menu_value(menu);
        self.menu_cursor = options
            .iter()
            .position(|(v, _, _)| Some(v) == value.as_ref());
        self.menu = Some(menu);
        self.menu_open = true;
        self.menu_closing = None;
        self.menu_reap = None;
        cx.notify();
    }

    /// Starts the menu's exit fade and unmounts it once the fade has run
    /// (Pink `Popup::close` + `reap_popup`).
    fn close_menu(&mut self, cx: &mut Context<Self>) {
        if !self.menu_open {
            return;
        }
        self.menu_open = false;
        let since = Instant::now();
        self.menu_closing = Some(since);
        let exit = MENU_OUT.total().mul_f32(motion::speed_scale());
        self.menu_reap = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(exit).await;
            this.update(cx, |s, cx| {
                if s.menu_closing == Some(since) {
                    s.menu_closing = None;
                    s.menu = None;
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// Apply a menu choice (Pink `pick`).
    fn pick(&mut self, menu: Menu, value: SharedString, cx: &mut Context<Self>) {
        self.close_menu(cx);
        match menu {
            Menu::Project => {
                if self.project.as_ref() != Some(&value) {
                    // A chat from the previous project is never sent to
                    // silently (Pink `set_project`).
                    let keeps_chat = self.host.projects.iter().any(|p| {
                        p.id == value
                            && self
                                .chat
                                .as_ref()
                                .is_some_and(|chat| p.chats.iter().any(|c| &c.id == chat))
                    });
                    if !keeps_chat {
                        self.chat = None;
                    }
                    self.project = Some(value);
                }
            }
            Menu::Chat => self.chat = Some(value),
            Menu::Notifications => {
                if let Some(policy) = NotificationPolicy::ALL
                    .iter()
                    .find(|p| p.key() == value.as_ref())
                {
                    self.notifications = *policy;
                }
            }
            Menu::Model => {
                if let Ok(ix) = value.parse() {
                    self.model = ix;
                }
            }
        }
        cx.notify();
    }
}

impl RenderOnce for AutomationEditor {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let element = self.build(window, cx);
        // Followers and hover fades started while building need frames.
        motion::keep_alive(window);
        element
    }
}

impl AutomationEditor {
    fn build(self, window: &mut Window, cx: &mut App) -> AnyElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            EditorState::new,
        );
        let host = Rc::new(Host {
            projects: self.projects.clone(),
            models: self.models.clone(),
            latency: self.latency,
            on_review: self.on_review.clone(),
            on_save: self.on_save.clone(),
            on_close: self.on_close.clone(),
        });
        let seed = (!state.read(cx).seeded).then(|| self.draft.clone());
        let recolor = (state.read(cx).colors_dark != Some(t.dark)).then_some((
            t.placeholder,
            t.caret,
            t.selection,
        ));
        state.update(cx, |s, cx| {
            s.host = host;
            if let Some(draft) = seed {
                s.seed(&draft, cx);
            }
            if let Some((placeholder, caret, selection)) = recolor {
                s.colors_dark = Some(t.dark);
                for field in &s.fields {
                    field.input.update(cx, |i, _| {
                        i.set_colors(Some(placeholder), Some(caret), Some(selection))
                    });
                }
            }
        });

        let draft = state.read(cx).draft(cx);
        let close = {
            let on_close = self.on_close.clone();
            pink::button(
                "automation-editor-close",
                ButtonKind::Ghost,
                ControlSize::Sm,
                &t,
            )
            .aria_label("Close the automation editor")
            .child("Close")
            .on_click(move |_, window, cx| {
                if let Some(f) = &on_close {
                    f(window, cx);
                }
            })
        };
        let subtitle: SharedString = match (self.existing, self.paused) {
            (true, true) => "Paused. Saving keeps it paused; resume it from its row.".into(),
            (true, false) => "Saving applies to its next runs.".into(),
            (false, _) => match &self.created_from {
                Some(chat) => format!("Created from {chat}.").into(),
                None => "Created from the Automations page.".into(),
            },
        };

        let ctx = Ctx {
            key: key.clone(),
            state: state.clone(),
            t: t.clone(),
        };
        pink::section_card(&t)
            .id(self.id.clone())
            .mt_0()
            .w(px(CARD_W))
            .max_w_full()
            .font_family(pink::SANS)
            .text_color(t.text)
            // Escape closes an open menu first, then the editor; Tab walks
            // its controls in order (Pink `render_editor`).
            .on_key_down({
                let state = state.clone();
                move |event: &KeyDownEvent, window, cx| match event.keystroke.key.as_str() {
                    "escape" => {
                        state.update(cx, |s, cx| s.escape(window, cx));
                        cx.stop_propagation();
                    }
                    "tab" => {
                        if event.keystroke.modifiers.shift {
                            window.focus_prev(cx);
                        } else {
                            window.focus_next(cx);
                        }
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            })
            .child(
                pink::section_header(
                    if self.existing {
                        "Edit automation"
                    } else {
                        "New automation"
                    },
                    Some(subtitle),
                    &t,
                )
                .child(pink::row_controls().child(close)),
            )
            .child(essentials(&ctx, cx))
            .child(schedule_group(&ctx, &draft, cx))
            .child(destination_group(&ctx, &draft, window, cx))
            .child(model_group(&ctx, window, cx))
            .child(options_group(&ctx, &draft, window, cx))
            .child(review_footer(&ctx, &draft, self.paused, cx))
            .into_any_element()
    }
}

/// What every group builder needs.
struct Ctx {
    key: String,
    state: Entity<EditorState>,
    t: Pink,
}

/// A group: its caption over its controls, one hairline above (Pink
/// `group`: a card row, column, gap 10; the caption 12/16 semibold in
/// `readable_muted(0.9)`).
fn group(id: String, title: Option<&'static str>, t: &Pink) -> Stateful<Div> {
    pink::card_row(false, t)
        .id(SharedString::from(id))
        .flex_col()
        .items_start()
        .gap(px(10.))
        .when_some(title, |group, title| group.child(group_caption(title, t)))
}

fn group_caption(title: &'static str, t: &Pink) -> Div {
    div()
        .text_size(px(12.))
        .line_height(px(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.readable_muted(0.9))
        .child(title)
}

/// A field's caption (Pink `caption`): 11/14 in `readable_muted(0.85)`.
fn caption(label: impl Into<SharedString>, t: &Pink) -> Div {
    div()
        .text_size(px(11.))
        .line_height(px(14.))
        .text_color(t.readable_muted(0.85))
        .child(label.into())
}

/// A line of copy under the controls (Pink `note`).
fn note(copy: impl Into<SharedString>, color: Hsla, t: &Pink) -> Div {
    pink::row_description(copy, t).w_full().text_color(color)
}

/// The editor's text frame (Pink `input_frame`: the dialog field at
/// `px 10 / py 6`, radius 8, a `separator(true)` edge, the Quaternary fill,
/// 13). It holds the tab stop that hands focus to the input; Shift-Tab out
/// of the input leaves from it.
fn input_frame(ctx: &Ctx, field: Field, cx: &App) -> Stateful<Div> {
    let s = ctx.state.read(cx);
    let FieldInput { input, stop } = s.field(field);
    let (input, stop) = (input.clone(), stop.clone());
    let focus = input.clone();
    pink::input_frame(&ctx.t)
        .id(SharedString::from(format!("{}-{}", ctx.key, field.key())))
        .track_focus(&stop)
        .line_height(px(FIELD_LINE))
        .cursor_text()
        .overflow_hidden()
        .on_click(move |_, window, cx| TextInput::focus(&focus, window, cx))
        .on_key_down({
            let stop = stop.clone();
            move |event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "tab" && event.keystroke.modifiers.shift {
                    window.focus(&stop, cx);
                    window.focus_prev(cx);
                    cx.stop_propagation();
                }
            }
        })
        .child(input)
}

/// A labelled input (Pink `field`): the caption over the frame, gap 4;
/// fields share a line and wrap below `min`.
fn field(ctx: &Ctx, label: &'static str, which: Field, min: f32, cx: &App) -> Div {
    div()
        .flex_1()
        .min_w(px(min))
        .flex()
        .flex_col()
        .gap(px(space::S4))
        .child(caption(label, &ctx.t))
        .child(input_frame(ctx, which, cx))
}

/// A setting inside a group (Pink `setting`): its 12/16 label on a 112
/// column, the control beside it; both wrap in a narrow pane.
fn setting(label: &'static str, control: impl IntoElement, t: &Pink) -> Div {
    div()
        .w_full()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x(px(space::S12))
        .gap_y(px(space::S6))
        .child(
            div()
                .flex_none()
                .w(px(SETTING_LABEL_W))
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(t.text)
                .child(label),
        )
        .child(div().min_w_0().child(control))
}

/// A choice chip (Pink `choice`: a Sm chip with its role).
fn choice(id: String, label: &'static str, on: bool, role: Role, t: &Pink) -> Stateful<Div> {
    pink::chip(id, on, ControlSize::Sm, t)
        .role(role)
        .aria_label(label)
        .aria_toggled(if on { Toggled::True } else { Toggled::False })
        .child(label)
}

/// Name and message.
fn essentials(ctx: &Ctx, cx: &App) -> Stateful<Div> {
    group(format!("{}-essentials", ctx.key), None, &ctx.t)
        .child(
            div()
                .w_full()
                .flex()
                .child(field(ctx, "Name", Field::Name, 160., cx)),
        )
        .child(div().w_full().flex().child(field(
            ctx,
            "Message each run sends",
            Field::Prompt,
            160.,
            cx,
        )))
}

/// The Schedule group: the kind, its fields, and what they mean in words
/// beside a clock (or the first field that can't be read yet).
fn schedule_group(ctx: &Ctx, draft: &AutomationDraft, cx: &App) -> Stateful<Div> {
    let t = &ctx.t;
    let key = &ctx.key;
    let (kind, weekdays) = {
        let s = ctx.state.read(cx);
        (s.schedule, s.weekdays)
    };
    let mut kinds = pink::row_block().gap(px(space::S6));
    for k in ScheduleKind::ALL {
        let state = ctx.state.clone();
        kinds = kinds.child(
            choice(
                format!("{key}-kind-{}", k.key()),
                k.label(),
                kind == k,
                Role::RadioButton,
                t,
            )
            .on_click(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.schedule = k;
                    cx.notify();
                })
            }),
        );
    }
    let row = || pink::row_block().items_start();
    let mut schedule = group(format!("{key}-schedule"), Some("Schedule"), t).child(kinds);
    schedule = match kind {
        ScheduleKind::Once => schedule.child(
            row()
                .child(field(ctx, "Date (YYYY-MM-DD)", Field::Date, 120., cx))
                .child(field(ctx, "Time (HH:MM)", Field::Time, 96., cx))
                .child(field(ctx, "Time zone", Field::Timezone, 160., cx)),
        ),
        ScheduleKind::Interval => schedule.child(
            row()
                .child(field(ctx, "Every (minutes)", Field::Every, 96., cx))
                .child(field(ctx, "Time zone", Field::Timezone, 160., cx)),
        ),
        ScheduleKind::Calendar => {
            let mut days = pink::row_block().gap(px(space::S6));
            for (ix, day) in WEEKDAYS.into_iter().enumerate() {
                let state = ctx.state.clone();
                days = days.child(
                    choice(
                        format!("{key}-day-{ix}"),
                        day,
                        weekdays[ix],
                        Role::CheckBox,
                        t,
                    )
                    .on_click(move |_, _, cx| {
                        state.update(cx, |s, cx| {
                            s.weekdays[ix] = !s.weekdays[ix];
                            cx.notify();
                        })
                    }),
                );
            }
            schedule
                .child(
                    row()
                        .child(field(ctx, "Time (HH:MM)", Field::Time, 96., cx))
                        .child(field(ctx, "Time zone", Field::Timezone, 160., cx)),
                )
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap(px(space::S6))
                        .child(caption("Days (none chosen: every day)", t))
                        .child(days),
                )
        }
        ScheduleKind::Custom => schedule
            .child(
                div()
                    .w_full()
                    .flex()
                    .child(field(ctx, "Rule (RRULE)", Field::Rule, 160., cx)),
            )
            .when(draft.rule.trim().is_empty(), |schedule| {
                schedule.child(pink::row_description(
                    "Fields separated by semicolons: FREQ (DAILY, WEEKLY, MONTHLY or YEARLY; \
                     MINUTELY and HOURLY as fixed intervals), INTERVAL, BYDAY (1MO, -1FR), \
                     BYMONTHDAY, BYMONTH, BYHOUR, BYMINUTE, WKST and UNTIL.",
                    t,
                ))
            })
            .child(
                row()
                    .child(field(ctx, "Starts on (optional)", Field::Date, 120., cx))
                    .child(field(ctx, "At (optional)", Field::Time, 96., cx))
                    .child(field(ctx, "Time zone", Field::Timezone, 160., cx)),
            ),
        ScheduleKind::PullRequest => schedule
            .child(
                row()
                    .child(field(
                        ctx,
                        "Repository (owner/repo)",
                        Field::Repository,
                        160.,
                        cx,
                    ))
                    .child(field(ctx, "Pull request", Field::Number, 96., cx)),
            )
            .child(
                row()
                    .child(field(
                        ctx,
                        "Check every (1 to 1440 min)",
                        Field::Poll,
                        120.,
                        cx,
                    ))
                    .child(field(
                        ctx,
                        "Wake budget (1 to 100)",
                        Field::Budget,
                        120.,
                        cx,
                    )),
            )
            .child(pink::row_description(
                "Wakes only on meaningful changes. PINK never posts GitHub comments.",
                t,
            )),
    };
    // The schedule in words, settling in as it changes (Pink: a 14 clock in
    // the accent and the words 13/18 medium).
    let summary = match schedule_words(draft) {
        Ok(words) => div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(space::S6))
            .child(
                svg()
                    .path(ICON_CLOCK)
                    .size(px(14.))
                    .flex_none()
                    .text_color(t.accent),
            )
            .child(
                div()
                    .min_w_0()
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .child(words),
            ),
        Err(reason) => div()
            .w_full()
            .child(note(reason, t.readable_muted(0.85), t)),
    };
    schedule.child(
        div()
            .id(SharedString::from(format!("{key}-schedule-summary")))
            .w_full()
            .child(summary),
    )
}

/// Where each run goes: the project, then a new chat or an existing one;
/// the sentence above the controls is the only restatement.
fn destination_group(
    ctx: &Ctx,
    draft: &AutomationDraft,
    window: &mut Window,
    cx: &App,
) -> Stateful<Div> {
    let t = &ctx.t;
    let s = ctx.state.read(cx);
    let project = s.project_ref();
    let project_label: SharedString = project
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "Choose a project".into());
    let chat_title = project.and_then(|p| {
        draft
            .chat
            .as_ref()
            .and_then(|id| p.chats.iter().find(|c| &c.id == id))
            .map(|c| c.title.clone())
    });
    let no_chats = project.is_none_or(|p| p.chats.is_empty());
    let destination = match (draft.fresh, &chat_title) {
        (true, _) => format!("Each run starts a new chat in {project_label}."),
        (false, Some(chat)) => format!("Each run is sent to {chat} in {project_label}."),
        (false, None) => "Choose the chat each run is sent to.".into(),
    };
    let fresh = draft.fresh;
    let chat_label: SharedString = chat_title.unwrap_or_else(|| "Choose a chat".into());
    let state = ctx.state.clone();
    let mut target = group(format!("{}-destination", ctx.key), Some("Where it runs"), t)
        .child(note(destination, t.text, t))
        .child(setting(
            "Project",
            select(
                ctx,
                Menu::Project,
                project_label.clone(),
                ICON_FOLDER,
                window,
                cx,
            ),
            t,
        ))
        .child(setting(
            "Each run",
            pink::segmented(
                format!("{}-fresh", ctx.key),
                vec![
                    Segment::new("new", "New chat"),
                    Segment::new("existing", "Existing chat"),
                ],
                usize::from(!fresh),
                FRESH_TRACK_W,
                ControlSize::Sm,
                t,
                move |ix: &usize, _, cx| {
                    state.update(cx, |s, cx| {
                        s.fresh = *ix == 0;
                        cx.notify();
                    })
                },
            ),
            t,
        ));
    if !fresh {
        target = target.child(setting(
            "Chat",
            select(ctx, Menu::Chat, chat_label, ICON_CHAT, window, cx),
            t,
        ));
        if no_chats {
            target = target.child(note(
                "This project has no open chats on this device. Start each run in a new chat \
                 instead.",
                t.readable_tone(t.warning, 1.0),
                t,
            ));
        }
    }
    target
}

/// The model: the composer's own chip (Pink pickers.rs `trigger_chip`, the
/// detached HarnessModel chip) – the agent's mark, the model at 12/500 and
/// the reasoning level muted after it – and its warning, if any.
fn model_group(ctx: &Ctx, window: &mut Window, cx: &App) -> Stateful<Div> {
    let t = &ctx.t;
    let s = ctx.state.read(cx);
    let picked = s.host.models.get(s.model).cloned();
    let open = s.menu_is_open(Menu::Model);
    let id = format!("{}-model", ctx.key);
    let (label, suffix, title) = match &picked {
        Some(m) => (m.model.clone(), m.reasoning.clone(), m.label()),
        None => ("Model".into(), None, "Model".into()),
    };
    let chip = pink::trigger_chip(id.clone(), picked.is_some(), open, t)
        .relative()
        .flex_none()
        .aria_label(SharedString::from(title))
        .child(
            svg()
                .path(ICON_BOT)
                .size(px(CHIP_ICON))
                .flex_none()
                .text_color(t.text_muted),
        )
        .child(div().min_w_0().truncate().child(label))
        .when_some(suffix, |chip, suffix| {
            chip.child(
                div()
                    .flex_none()
                    .text_color(t.readable_muted(0.7))
                    .child(suffix),
            )
        });
    let chip = menu_trigger(ctx, Menu::Model, chip, Anchor::TopLeft, window, cx);
    let warning = picked.and_then(|m| m.note);
    group(format!("{}-model-group", ctx.key), Some("Model"), t)
        .child(
            div()
                .id(SharedString::from(format!("{}-model-pickers", ctx.key)))
                .w_full()
                .min_w_0()
                .flex()
                .child(chip),
        )
        .when_some(warning, |group, copy| {
            group.child(note(copy, t.readable_tone(t.warning, 1.0), t))
        })
}

/// More options: folded to a summary that names every value, so nothing
/// saved is hidden.
fn options_group(
    ctx: &Ctx,
    draft: &AutomationDraft,
    window: &mut Window,
    cx: &App,
) -> Stateful<Div> {
    let t = &ctx.t;
    let key = &ctx.key;
    let s = ctx.state.read(cx);
    // A refusal about the lateness opens the group (Pink `late_error`).
    let late_error =
        matches!(&s.shown_phase(draft), Phase::Error(e) if e.contains("missed-run tolerance"));
    let more = s.more || late_error;
    let fresh = draft.fresh;
    let ring = t.focus_ring();
    let disclosure = {
        let state = ctx.state.clone();
        div()
            .id(SharedString::from(format!("{key}-more-toggle")))
            .role(Role::Button)
            .aria_label("More options")
            .aria_expanded(more)
            .tab_index(0)
            .rounded(px(pink::radius::XS))
            .focus_visible(move |s| pink::focus_ring(s, ring))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(space::S8))
            .cursor_pointer()
            .on_key_down(pink::swallow_activation_key)
            .child(group_caption("More options", t))
            .child(pink::line_glyph(
                if more {
                    pink::glyphs::ALT_ARROW_UP
                } else {
                    pink::glyphs::ALT_ARROW_DOWN
                },
                12.,
                t.readable_muted(0.85),
            ))
            .on_click(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.more = !s.more;
                    cx.notify();
                })
            })
    };
    let mut options = pink::card_row(false, t)
        .id(SharedString::from(format!("{key}-more")))
        .flex_col()
        .items_start()
        .gap(px(10.))
        .child(disclosure);
    if more {
        let policy = draft.notifications;
        options = options
            .child(setting(
                "Notifications",
                select(
                    ctx,
                    Menu::Notifications,
                    policy.label().into(),
                    ICON_BELL,
                    window,
                    cx,
                ),
                t,
            ))
            .child(note(policy.caption(), t.readable_muted(0.85), t));
        if fresh {
            let state = ctx.state.clone();
            options = options.child(setting(
                "Archive run chats",
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(space::S8))
                    .child(
                        pink::switch_control(
                            format!("{key}-archive"),
                            "Archive successful runs",
                            s.archive,
                            t,
                        )
                        .on_click(move |_, _, cx| {
                            state.update(cx, |s, cx| {
                                s.archive = !s.archive;
                                cx.notify();
                            })
                        }),
                    )
                    .child(
                        pink::row_description("When a run finishes without problems", t)
                            .flex_shrink(1.),
                    ),
                t,
            ));
        }
        options = options.child(setting(
            "Skip if late by",
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(space::S8))
                .child(
                    div()
                        .w(px(LATENESS_W))
                        .child(input_frame(ctx, Field::Lateness, cx)),
                )
                .child(pink::row_description("minutes (0 to 1440)", t)),
            t,
        ));
    } else {
        let mut summary = vec![
            format!("Notifications: {}", draft.notifications.label()),
            format!("skips runs over {} min late", draft.lateness.trim()),
        ];
        if fresh {
            summary.push(if draft.archive_successful_runs {
                "archives successful runs".into()
            } else {
                "keeps run chats".into()
            });
        }
        options = options.child(note(summary.join(", "), t.readable_muted(0.85), t));
    }
    options
}

/// The footer (Pink: a card row on the Quaternary fill): the host's answer
/// for the exact request on the left, Review and Save on the right. Save
/// only applies a review of the form as it stands.
fn review_footer(ctx: &Ctx, draft: &AutomationDraft, paused: bool, cx: &App) -> Div {
    let t = &ctx.t;
    let key = &ctx.key;
    let s = ctx.state.read(cx);
    let warning = t.readable_tone(t.warning, 1.0);
    let danger = t.readable_tone(t.danger_muted, 1.0);
    let phase = s.shown_phase(draft);
    let current = s.review_current(draft);
    let mut lines: Vec<Div> = Vec::new();
    match &phase {
        Phase::Previewing => lines.push(pink::row_description("Asking the host to review…", t)),
        Phase::Reviewed if current => {
            if let Some((_, review)) = &s.review {
                lines.push(pink::row_description(review.description.clone(), t).text_color(t.text));
                lines.push(pink::row_description(
                    if review.next_runs.is_empty() {
                        if paused {
                            "Paused: no runs until you resume it.".to_string()
                        } else {
                            "No runs are scheduled.".to_string()
                        }
                    } else {
                        format!(
                            "{}: {}",
                            if paused { "After Resume" } else { "Next runs" },
                            review
                                .next_runs
                                .iter()
                                .map(|r| r.as_ref())
                                .collect::<Vec<&str>>()
                                .join(", ")
                        )
                    },
                    t,
                ));
                let runs_with = s
                    .host
                    .models
                    .get(draft.model)
                    .map(ModelChoice::label)
                    .unwrap_or_else(|| "the provider and model it captured".into());
                lines.push(pink::row_description(
                    format!(
                        "Runs with {runs_with} on {}, access up to {}.",
                        review.host, review.access
                    ),
                    t,
                ));
            }
        }
        Phase::Reviewed => lines.push(
            pink::row_description("You changed the form after review. Review it again.", t)
                .text_color(warning),
        ),
        Phase::Applying => lines.push(pink::row_description("Saving…", t)),
        Phase::Error(error) => {
            lines.push(pink::row_description(error.clone(), t).text_color(danger))
        }
        Phase::Editing => lines.push(pink::row_description(
            "Review asks the host to check the change and show the next runs before you save.",
            t,
        )),
    }
    let review_lines = div()
        .flex_1()
        .min_w(px(layout::ROW_TEXT_MIN))
        .flex()
        .flex_col()
        .gap(px(space::S2))
        .children(lines);

    let busy = s.busy();
    let mut buttons = pink::row_controls();
    if matches!(phase, Phase::Error(_)) {
        let state = ctx.state.clone();
        buttons = buttons.child(
            pink::button(
                format!("{key}-reload"),
                ButtonKind::Ghost,
                ControlSize::Md,
                t,
            )
            .aria_label("Reload")
            .child("Reload")
            .on_click(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.phase = Phase::Editing;
                    s.error_basis = None;
                    cx.notify();
                })
            }),
        );
    }
    let review = {
        let state = ctx.state.clone();
        pink::button_enabled(
            format!("{key}-review"),
            ButtonKind::Secondary,
            ControlSize::Md,
            !busy,
            t,
        )
        .aria_label("Review")
        .child("Review")
        .when(!busy, |b| {
            b.on_click(move |_, _, cx| state.update(cx, |s, cx| s.request_review(cx)))
        })
    };
    let can_save = matches!(s.phase, Phase::Reviewed) && current;
    let save = {
        let state = ctx.state.clone();
        pink::button_enabled(
            format!("{key}-save"),
            ButtonKind::Primary,
            ControlSize::Md,
            can_save,
            t,
        )
        .aria_label("Save")
        .child("Save")
        .when(can_save, |b| {
            b.on_click(move |_, window, cx| apply(&state, window, cx))
        })
    };
    pink::card_row(false, t)
        .bg(t.fill(FillTier::Quaternary))
        .child(review_lines)
        .child(buttons.child(review).child(save))
}

/// Apply exactly the reviewed draft (Pink `apply_editor`), then hand the
/// host's answer to `on_save`: saved, or a refusal the footer shows.
fn apply(state: &Entity<EditorState>, window: &mut Window, cx: &mut App) {
    let (draft, latency, on_save) = {
        let s = state.read(cx);
        let Some((draft, _)) = s.review.clone() else {
            return;
        };
        if !s.review_current(&s.draft(cx)) || s.busy() {
            return;
        }
        (draft, s.host.latency, s.host.on_save.clone())
    };
    let task = window.spawn(cx, {
        let state = state.clone();
        async move |cx| {
            cx.background_executor().timer(latency).await;
            cx.update(|window, cx| {
                let answer = match &on_save {
                    Some(save) => save(&draft, window, cx),
                    None => Ok(()),
                };
                state.update(cx, |s, cx| {
                    match answer {
                        // Saved: what was reviewed is what the form holds.
                        Ok(()) => {
                            s.phase = Phase::Editing;
                            s.review = None;
                        }
                        Err(reason) => {
                            s.error_basis = Some(draft);
                            s.phase = Phase::Error(reason);
                        }
                    }
                    s.flow = None;
                    cx.notify();
                });
            })
            .ok();
        }
    });
    state.update(cx, |s, cx| {
        s.close_menu(cx);
        s.phase = Phase::Applying;
        s.flow = Some(task);
        cx.notify();
    });
}

/// A pop-up button (Pink `render_select`): a Md Secondary frame 180–320
/// wide with the menu's 14 glyph in `readable_muted(0.85)`, the current
/// choice and the chevron. Disabled while it has nothing to offer.
fn select(
    ctx: &Ctx,
    menu: Menu,
    label: SharedString,
    icon: &'static str,
    window: &mut Window,
    cx: &App,
) -> AnyElement {
    let t = &ctx.t;
    let s = ctx.state.read(cx);
    let enabled = !s.menu_options(menu).is_empty();
    let open = s.menu_is_open(menu);
    let id = format!("{}-{}", ctx.key, menu_key(menu));
    let trigger = pink::select_frame(id, ControlSize::Md, open, enabled, t)
        .relative()
        .min_w(px(SELECT_MIN_W))
        .max_w(px(SELECT_MAX_W))
        .aria_label(SharedString::from(format!(
            "{}: {label}",
            match menu {
                Menu::Project => "Project",
                Menu::Chat => "Chat",
                Menu::Notifications => "Notifications",
                Menu::Model => "Model",
            }
        )))
        .child(pink::glyph(icon, ControlSize::Md).text_color(t.readable_muted(0.85)))
        .child(div().flex_1().min_w_0().truncate().child(label))
        .child(pink::select_chevron(t));
    if !enabled {
        return trigger.into_any_element();
    }
    menu_trigger(ctx, menu, trigger, Anchor::TopRight, window, cx).into_any_element()
}

fn menu_key(menu: Menu) -> &'static str {
    match menu {
        Menu::Project => "project",
        Menu::Chat => "chat",
        Menu::Notifications => "notifications",
        Menu::Model => "model",
    }
}

/// Wires a trigger to its menu (Pink `render_select`): a press on an open
/// menu closes it; a keyboard click picks the cursor row or opens; Up and
/// Down open it or move the cursor; Escape closes it. While shown, the
/// menu hangs from the trigger, `anchor` naming the corner.
fn menu_trigger(
    ctx: &Ctx,
    menu: Menu,
    trigger: Stateful<Div>,
    anchor: Anchor,
    _window: &mut Window,
    cx: &App,
) -> Stateful<Div> {
    let s = ctx.state.read(cx);
    let shown = s.menu_shown(menu);
    let state = ctx.state.clone();
    let mut trigger = trigger
        .on_mouse_down(MouseButton::Left, {
            let state = state.clone();
            move |_, _, cx| {
                state.update(cx, |s, _| s.menu_pressed_open = s.menu_shown(menu));
            }
        })
        .on_click({
            let state = state.clone();
            move |event: &ClickEvent, _, cx| {
                state.update(cx, |s, cx| {
                    let open = s.menu_is_open(menu);
                    let pressed_open = std::mem::take(&mut s.menu_pressed_open);
                    if event.is_keyboard() {
                        if open {
                            let options = s.menu_options(menu);
                            match s.menu_cursor.and_then(|ix| options.get(ix)) {
                                Some((value, _, _)) => {
                                    let value = value.clone();
                                    s.pick(menu, value, cx)
                                }
                                None => s.close_menu(cx),
                            }
                        } else {
                            s.open_menu(menu, cx);
                        }
                    } else if pressed_open || open {
                        // This press's outside-click already closed it.
                        s.close_menu(cx);
                    } else {
                        s.open_menu(menu, cx);
                    }
                    cx.notify();
                })
            }
        })
        .on_key_down({
            let state = state.clone();
            move |event: &KeyDownEvent, _, cx| {
                let key = event.keystroke.key.as_str();
                let ctrl = event.keystroke.modifiers.control;
                let delta: isize = match key {
                    "up" => -1,
                    "down" => 1,
                    "p" if ctrl => -1,
                    "n" if ctrl => 1,
                    "escape" => {
                        let open = state.read(cx).menu_is_open(menu);
                        if open {
                            state.update(cx, |s, cx| s.close_menu(cx));
                            cx.stop_propagation();
                        }
                        return;
                    }
                    "enter" | "space" => {
                        // The keyboard click handles it.
                        cx.stop_propagation();
                        return;
                    }
                    _ => return,
                };
                state.update(cx, |s, cx| {
                    if s.menu_is_open(menu) {
                        let count = s.menu_options(menu).len();
                        s.menu_cursor = menu_step(s.menu_cursor, count, delta);
                    } else {
                        s.open_menu(menu, cx);
                    }
                    cx.notify();
                });
                cx.stop_propagation();
            }
        });
    if shown {
        let card = menu_card(ctx, menu, cx);
        let exit = s.menu_closing.filter(|_| !s.menu_open).map(|since| {
            if motion::reduced_motion(cx) {
                1.
            } else {
                motion::menu_exit_progress(since)
            }
        });
        trigger = trigger.child(anchored_menu(
            SharedString::from(format!("{}-{}-menu", ctx.key, menu_key(menu))),
            anchor,
            exit,
            card,
        ));
    }
    trigger
}

/// Moves a menu cursor by `delta`, wrapping (Pink `popover::menu_step`).
fn menu_step(active: Option<usize>, count: usize, delta: isize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let count = count as isize;
    let next = match active {
        None if delta >= 0 => 0,
        None => count - 1,
        Some(at) => (at as isize + delta).rem_euclid(count),
    };
    Some(next as usize)
}

/// The menu (Pink `render_select`): a 320 popover card with its heading and
/// a list that scrolls past 280; rows share one highlight with the pointer,
/// the current choice checked, a caption trailing at 11.
fn menu_card(ctx: &Ctx, menu: Menu, cx: &App) -> Div {
    let t = &ctx.t;
    let s = ctx.state.read(cx);
    let value = s.menu_value(menu);
    let cursor = s.menu_cursor;
    let rows: Vec<AnyElement> = s
        .menu_options(menu)
        .into_iter()
        .enumerate()
        .map(|(ix, (option, title, caption))| {
            let selected = value.as_ref() == Some(&option);
            let move_state = ctx.state.clone();
            let click_state = ctx.state.clone();
            pink::menu_row_nav(selected, cursor == Some(ix), t)
                .id(SharedString::from(format!(
                    "{}-{}-option-{ix}",
                    ctx.key,
                    menu_key(menu)
                )))
                .role(Role::MenuItemRadio)
                .aria_label(title.clone())
                .on_mouse_move(move |_: &MouseMoveEvent, _, cx| {
                    move_state.update(cx, |s, cx| {
                        if s.menu_cursor != Some(ix) {
                            s.menu_cursor = Some(ix);
                            cx.notify();
                        }
                    })
                })
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    click_state.update(cx, |s, cx| s.pick(menu, option.clone(), cx));
                })
                .child(div().flex_1().min_w_0().truncate().child(title))
                .children(caption.map(|caption| {
                    div()
                        .flex_none()
                        .max_w(px(MENU_CAPTION_MAX_W))
                        .truncate()
                        .text_size(px(11.))
                        .text_color(t.readable_muted(0.8))
                        .child(caption)
                }))
                .into_any_element()
        })
        .collect();
    let state = ctx.state.clone();
    pink::popover_card(t)
        .w(px(MENU_W))
        .on_mouse_down_out(move |_, _, cx| state.update(cx, |s, cx| s.close_menu(cx)))
        .flex()
        .flex_col()
        .child(pink::menu_heading(menu.heading(), t))
        .child(
            div()
                .id(SharedString::from(format!(
                    "{}-{}-list",
                    ctx.key,
                    menu_key(menu)
                )))
                .max_h(px(MENU_LIST_MAX_H))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(px(layout::MENU_GAP))
                .children(rows),
        )
}

/// A menu hung from the trigger's bottom corner, 6 pt below it and kept 8
/// pt inside the window (Pink popover.rs `anchored_menu_below_end`; the
/// model chip's from its start). It drops 6 pt into place on `MENU_IN`;
/// while `exit` runs it fades on `MENU_OUT` under an occluder, so the dying
/// rows take no clicks.
fn anchored_menu(
    id: SharedString,
    anchor: Anchor,
    exit: Option<f32>,
    card: impl IntoElement,
) -> impl IntoElement {
    let inner = div()
        .occlude()
        .pt(px(layout::POPOVER_GAP))
        .child(div().relative().child(press_guard()).child(card));
    let layer = match exit {
        Some(t) => motion::menu_out(
            SharedString::from(format!("{id}-out")),
            t,
            inner.relative().child(div().absolute().inset_0().occlude()),
        )
        .into_any_element(),
        None => motion::menu_in_from(id, MENU_RISE_DOWN, inner).into_any_element(),
    };
    let host = div().absolute().bottom_0().size_0();
    let host = if anchor == Anchor::TopLeft {
        host.left_0()
    } else {
        host.right_0()
    };
    host.child(
        deferred(
            anchored()
                .anchor(anchor)
                .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                .child(layer),
        )
        .priority(1),
    )
}

/// Swallows the press that dismisses the menu: the card closes on it, and
/// nothing behind the menu (the trigger included) acts on it too (Pink
/// `frosted_menu_rounded`'s guard).
fn press_guard() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |bounds, _, window, _| {
            window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble && !bounds.contains(&event.position) {
                    cx.stop_propagation();
                }
            });
        },
    )
    .absolute()
    .inset_0()
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

/// The demo's clock: Saturday 2026-10-24, 10:12 in the device's zone.
const NOW: (i32, u32, u32, u32) = (2026, 10, 24, 10 * 60 + 12);
const DEVICE_ZONE: &str = "Europe/Lisbon";

/// Monday = 0 (Sakamoto).
fn weekday(year: i32, month: u32, day: u32) -> usize {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let sunday_first = (y + y / 4 - y / 100 + y / 400 + T[month as usize - 1] + day as i32) % 7;
    ((sunday_first + 6) % 7) as usize
}

fn add_days(mut date: (i32, u32, u32), days: u32) -> (i32, u32, u32) {
    for _ in 0..days {
        let (y, m, d) = date;
        date = if d < days_in_month(y, m) {
            (y, m, d + 1)
        } else if m < 12 {
            (y, m + 1, 1)
        } else {
            (y + 1, 1, 1)
        };
    }
    date
}

/// A run time as Pink's `when_brief` writes it: the clock alone today,
/// else the date too; the zone when it isn't the device's; ", in 3 h"
/// within a day.
fn when_brief(day_offset: u32, minute_of_day: u32, zone: &str) -> String {
    let (y, m, d) = add_days((NOW.0, NOW.1, NOW.2), day_offset);
    let clock = format!("{:02}:{:02}", minute_of_day / 60, minute_of_day % 60);
    let mut text = if day_offset == 0 {
        clock
    } else {
        format!("{y:04}-{m:02}-{d:02} {clock}")
    };
    if zone != DEVICE_ZONE {
        text.push_str(&format!(" {zone}"));
    }
    let ahead = (day_offset * 1440 + minute_of_day) as i64 - NOW.3 as i64;
    if (0..1440).contains(&ahead) {
        text.push_str(&match ahead {
            0 => ", in under a minute".to_string(),
            1..=59 => format!(", in {ahead} min"),
            _ => format!(", in {} h", ahead / 60),
        });
    }
    text
}

/// The demo host's review: the schedule in its words and the next three
/// runs from the demo's clock.
fn demo_review(draft: &AutomationDraft) -> Result<AutomationReview, String> {
    let words = schedule_words(draft)?;
    let zone = draft.timezone.trim();
    let mut runs = Vec::new();
    match draft.schedule {
        ScheduleKind::Calendar => {
            let (hour, minute) = parse_clock(&draft.time)?;
            let at = hour * 60 + minute;
            let any = !draft.weekdays.iter().any(|on| *on);
            for offset in 0..21 {
                let (y, m, d) = add_days((NOW.0, NOW.1, NOW.2), offset);
                if (any || draft.weekdays[weekday(y, m, d)]) && (offset > 0 || at > NOW.3) {
                    runs.push(when_brief(offset, at, zone));
                }
                if runs.len() == 3 {
                    break;
                }
            }
        }
        ScheduleKind::Interval => {
            let every: u32 = draft.every.trim().parse().unwrap_or(60);
            for k in 1..=3 {
                let total = NOW.3 + every * k;
                runs.push(when_brief(total / 1440, total % 1440, zone));
            }
        }
        ScheduleKind::Once => {
            let (y, m, d) = parse_date(&draft.date)?;
            let (hour, minute) = parse_clock(&draft.time)?;
            let at = hour * 60 + minute;
            let offset = (0..400).find(|o| add_days((NOW.0, NOW.1, NOW.2), *o) == (y, m, d));
            match offset {
                Some(o) if o > 0 || at > NOW.3 => runs.push(when_brief(o, at, zone)),
                _ => return Err("That time has already passed. Choose a later one.".into()),
            }
        }
        ScheduleKind::Custom | ScheduleKind::PullRequest => {}
    }
    Ok(AutomationReview::new(words)
        .next_runs(runs)
        .host("Lena's MacBook Pro")
        .access("Limited access"))
}

fn demo_projects() -> Vec<Project> {
    vec![
        Project::new("acme-web", "acme-web")
            .path("~/code/acme-web")
            .chat("c-triage", "Triage flaky checkout tests")
            .chat("c-deps", "Weekly dependency bumps")
            .chat("c-a11y", "Audit form labels"),
        Project::new("billing", "billing-service")
            .path("~/code/billing-service")
            .chat("c-invoices", "Invoice PDF regressions"),
        Project::new("docs", "docs-site").path("~/code/docs-site"),
    ]
}

fn demo_models() -> Vec<ModelChoice> {
    vec![
        ModelChoice::new("Claude Code", "Opus 4.1").reasoning("High"),
        ModelChoice::new("Claude Code", "Sonnet 4.5").reasoning("Medium"),
        ModelChoice::new("Codex", "GPT-5 Codex")
            .reasoning("Max")
            .note("Max isn't offered by this model; runs use High."),
    ]
}

fn demo_draft() -> AutomationDraft {
    AutomationDraft::new()
        .name("Morning CI triage")
        .prompt(
            "Read last night's failed CI runs on main, group them by cause, and open a short \
             report with the flakiest tests first.",
        )
        .calendar("09:00", WORKDAYS)
        .timezone(DEVICE_ZONE)
        .project("acme-web")
}

struct Demo {
    /// Bumped to open a fresh editor.
    run: usize,
    open: bool,
    saved: Option<AutomationDraft>,
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let t = Pink::get(cx);
        let demo = window.use_keyed_state("automation-editor-demo", cx, |_, _| Demo {
            run: 0,
            open: true,
            saved: None,
        });
        let (run, open, saved) = {
            let d = demo.read(cx);
            (d.run, d.open, d.saved.clone())
        };
        let column = div().w(px(CARD_W)).flex().flex_col().gap(px(space::S12));
        if !open {
            // The page's own action reopens it (Pink `header_actions`: New).
            let demo = demo.clone();
            return column.child(
                div().flex().justify_end().child(
                    pink::button("automation-new", ButtonKind::Secondary, ControlSize::Md, &t)
                        .aria_label("New automation")
                        .child(pink::line_glyph(pink::glyphs::PLUS, 14., t.text))
                        .child("New")
                        .on_click(move |_, _, cx| {
                            demo.update(cx, |d, cx| {
                                d.run += 1;
                                d.open = true;
                                d.saved = None;
                                cx.notify();
                            })
                        }),
                ),
            );
        }
        let editor = AutomationEditor::new(ElementId::NamedInteger(
            "automation-editor".into(),
            run as u64,
        ))
        .projects(demo_projects())
        .models(demo_models())
        .on_review(demo_review)
        .on_save({
            let demo = demo.clone();
            move |draft, _, cx| {
                // Pink closes the editor on success; the demo reopens the
                // saved automation for editing.
                let draft = draft.clone();
                demo.update(cx, |d, cx| {
                    d.run += 1;
                    d.saved = Some(draft);
                    cx.notify();
                });
                Ok(())
            }
        })
        .on_close({
            let demo = demo.clone();
            move |_, cx| {
                demo.update(cx, |d, cx| {
                    d.open = false;
                    cx.notify();
                })
            }
        });
        let editor = match saved {
            Some(draft) => editor.existing(true).draft(draft),
            None => editor.draft(demo_draft()),
        };
        column.child(editor)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_clocks_parse_like_pink() {
        assert_eq!(parse_date("2026-02-28"), Ok((2026, 2, 28)));
        assert_eq!(
            parse_date("2026-02-29"),
            Err("2026-02-29 is not a calendar date.".into())
        );
        assert_eq!(parse_date("2028-02-29"), Ok((2028, 2, 29)));
        assert_eq!(
            parse_date("26-2-1"),
            Err("Enter the date as YYYY-MM-DD.".into())
        );
        assert_eq!(parse_clock("9:05"), Ok((9, 5)));
        assert_eq!(parse_clock("09:5"), Err("Enter the time as HH:MM.".into()));
        assert_eq!(
            parse_clock("24:00"),
            Err("Enter a time between 00:00 and 23:59.".into())
        );
    }

    #[test]
    fn schedules_read_as_pink_words() {
        let base = AutomationDraft::new().timezone("Europe/Lisbon");
        assert_eq!(
            schedule_words(&base.clone().calendar("09:00", WORKDAYS)).unwrap(),
            "Mon, Tue, Wed, Thu, Fri at 09:00 (Europe/Lisbon)"
        );
        assert_eq!(
            schedule_words(&base.clone().calendar("7:30", [false; 7])).unwrap(),
            "Every day at 07:30 (Europe/Lisbon)"
        );
        assert_eq!(
            schedule_words(&base.clone().interval(120)).unwrap(),
            "Every 2 hours in Europe/Lisbon"
        );
        assert_eq!(
            schedule_words(
                &base
                    .clone()
                    .custom("FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE;BYHOUR=9;BYMINUTE=30")
            )
            .unwrap(),
            "Every 2 weeks on Mon, Wed at 09:30 in Europe/Lisbon"
        );
        assert_eq!(
            schedule_words(&base.clone().custom("BYHOUR=9")),
            Err("A rule needs a frequency, e.g. FREQ=WEEKLY.".into())
        );
        assert_eq!(
            schedule_words(&base.clone().pull_request("acme/web", 412)).unwrap(),
            "Watch acme/web#412, checked every 15 min, up to 10 wakes"
        );
        assert_eq!(
            schedule_words(&base.clone().pull_request("acme", 412)),
            Err("Enter the repository as owner/repository.".into())
        );
        assert_eq!(
            schedule_words(&base.timezone(" ").interval(5)),
            Err("Enter an IANA time zone, e.g. Europe/Bucharest.".into())
        );
    }

    #[test]
    fn validation_follows_pinks_order() {
        let projects = demo_projects();
        let ok = demo_draft();
        assert_eq!(validate(&ok, &projects), Ok(()));
        assert_eq!(
            validate(&ok.clone().name(" "), &projects),
            Err("Enter a name (up to 160 bytes).".into())
        );
        let mut no_project = ok.clone();
        no_project.project = None;
        assert_eq!(
            validate(&no_project, &projects),
            Err("Choose the project each run belongs to.".into())
        );
        let mut no_chat = ok.clone();
        no_chat.fresh = false;
        assert_eq!(
            validate(&no_chat, &projects),
            Err("Choose the chat each run is sent to.".into())
        );
        assert_eq!(
            validate(&ok.clone().chat("c-invoices"), &projects),
            Err("Choose a chat in acme-web.".into())
        );
        assert_eq!(
            validate(&ok.lateness(2000), &projects),
            Err("The missed-run tolerance is at most 24 hours.".into())
        );
    }

    #[test]
    fn demo_clock_and_menu_cursor() {
        assert_eq!(weekday(2026, 10, 24), 5, "a Saturday");
        assert_eq!(add_days((2026, 12, 31), 1), (2027, 1, 1));
        assert_eq!(when_brief(0, 13 * 60 + 12, DEVICE_ZONE), "13:12, in 3 h");
        assert_eq!(when_brief(2, 9 * 60, DEVICE_ZONE), "2026-10-26 09:00");
        assert_eq!(menu_step(None, 3, 1), Some(0));
        assert_eq!(menu_step(None, 3, -1), Some(2));
        assert_eq!(menu_step(Some(2), 3, 1), Some(0));
        assert_eq!(menu_step(Some(0), 0, 1), None);
        let review = demo_review(&demo_draft()).unwrap();
        assert_eq!(
            review.next_runs,
            vec![
                SharedString::from("2026-10-26 09:00"),
                "2026-10-27 09:00".into(),
                "2026-10-28 09:00".into()
            ]
        );
    }
}
