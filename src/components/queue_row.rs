//! Queue Row: the `pink-queue-row` package (`components/queue-row`), PINK's
//! message queue as a single-GPUI package the app pins (UI rethink decision
//! 54). The gallery compiles the package's own source here, unchanged,
//! against its GPUI; PINK compiles the same file against its patched one.
//! The Agent Steering story's queue tray draws with it; this module adds
//! only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/queue-row/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, Role, SharedString, Window, div, prelude::*, px};

use crate::{
    motion,
    pink::{FillTier, Pink},
    story,
};

pub const NAME: &str = "Queue Row";
pub const DESCRIPTION: &str = "PINK's message queue as a package the app pins: the tray tucked behind the composer and its compact rows, each with a drag marker, the message and its delivery status, image thumbnails and a +N count, permanent Remove and Edit discs and one Send now or Steer now capsule, or Save and Cancel while the message is edited in the composer";

const ICON_DRAG: &str = "icons/agent-steering/queue-drag-handle.svg";
const ICON_EDIT: &str = "icons/agent-steering/pen.svg";
const ICON_REMOVE: &str = "icons/agent-steering/trash-bin-minimalistic.svg";
const ICON_SAVE: &str = "icons/agent-steering/queue-check.svg";
const ICON_CANCEL: &str = "icons/agent-steering/queue-close.svg";
const ICON_SEND: &str = "icons/agent-steering/queue-send.svg";
const ICON_PAPERCLIP: &str = "icons/chat-composer/paperclip.svg";

/// One demo row.
struct Demo {
    key: &'static str,
    status: Option<&'static str>,
    text: &'static str,
    summary: Option<&'static str>,
    thumbnails: usize,
    more: Option<&'static str>,
    primary: &'static str,
    editing: bool,
    blocked: bool,
}

/// A row action: the package's disc with the gallery's hover fade.
fn story_action(
    key: String,
    label: &'static str,
    glyph: &'static str,
    enabled: bool,
    t: &Pink,
) -> impl IntoElement {
    let wash = t.fill(FillTier::Tertiary);
    let group = SharedString::from(format!("{key}-grp"));
    let text = t.text;
    package::action(&ActionStyle {
        fill: if enabled {
            motion::hover_blend(&key, wash.opacity(0.0), wash)
        } else {
            wash.opacity(0.0)
        },
        text: t.content_muted,
        enabled,
    })
    .id(SharedString::from(key.clone()))
    .group(group.clone())
    .role(Role::Button)
    .aria_label(label)
    .when(enabled, |el| el.on_hover(motion::hover_listener(key)))
    .child(
        action_glyph(glyph, t.readable_muted(0.8)).group_hover(group, move |s| s.text_color(text)),
    )
}

fn demo_row(prefix: &str, demo: &Demo, compact: bool, t: &Pink) -> impl IntoElement {
    let wash = t.fill(FillTier::Tertiary);
    let hover_key = format!("{prefix}-{}-row", demo.key);
    let actions = if demo.editing {
        actions()
            .child(story_action(
                format!("{prefix}-{}-save", demo.key),
                "Save to queue",
                ICON_SAVE,
                true,
                t,
            ))
            .child(story_action(
                format!("{prefix}-{}-cancel", demo.key),
                "Cancel",
                ICON_CANCEL,
                true,
                t,
            ))
    } else {
        let primary_key = format!("{prefix}-{}-primary", demo.key);
        actions()
            .child(story_action(
                format!("{prefix}-{}-drop", demo.key),
                "Remove",
                ICON_REMOVE,
                !demo.blocked,
                t,
            ))
            .child(story_action(
                format!("{prefix}-{}-edit", demo.key),
                "Edit",
                ICON_EDIT,
                !demo.blocked,
                t,
            ))
            .child(
                primary(
                    &ActionStyle {
                        fill: if demo.blocked {
                            wash.opacity(0.0)
                        } else {
                            motion::hover_blend(&primary_key, wash.opacity(0.0), wash)
                        },
                        text: if demo.blocked {
                            t.text_muted
                        } else {
                            motion::hover_blend(&primary_key, t.text_muted, t.text)
                        },
                        enabled: !demo.blocked,
                    },
                    compact,
                )
                .id(SharedString::from(primary_key.clone()))
                .role(Role::Button)
                .aria_label(demo.primary)
                .when(!demo.blocked, |el| {
                    el.on_hover(motion::hover_listener(primary_key))
                })
                .when(compact, |el| {
                    el.child(action_glyph(ICON_SEND, t.text_muted))
                })
                .when(!compact, |el| el.child(demo.primary)),
            )
    };
    let thumbnail_style = ThumbnailStyle {
        border: t.hairline(0.1),
        background: t.ink(0.035),
    };
    row(&RowStyle {
        fill: if demo.editing {
            wash
        } else {
            motion::hover_blend(&hover_key, wash.opacity(0.0), wash)
        },
        compact,
        removing: false,
    })
    .id(SharedString::from(hover_key.clone()))
    .role(Role::ListItem)
    .when(!demo.editing, |el| {
        el.on_hover(motion::hover_listener(hover_key))
            .child(drag_marker(ICON_DRAG, t.readable_muted(0.5), demo.blocked))
            .children(
                (0..demo.thumbnails)
                    .map(|_| thumbnail_placeholder(&thumbnail_style, ICON_PAPERCLIP)),
            )
            .when_some(demo.more, |el, more| {
                el.child(package::more(t.ink(0.06), t.text_muted, more))
            })
            .child(
                content()
                    .child(
                        message_line(t.text.opacity(0.9))
                            .when_some(demo.status, |line, label| {
                                line.child(status(t.text_muted, label))
                            })
                            .child(message(demo.text)),
                    )
                    .when_some(demo.summary, |content, text| {
                        content.child(summary(t.text_muted, text))
                    }),
            )
    })
    .when(demo.editing, |el| {
        el.child(edit_spacer())
            .child(editing_note(t.text_muted, "Editing in composer"))
    })
    .child(actions)
}

fn demo_tray(key: &'static str, width: f32, compact: bool, t: &Pink) -> impl IntoElement {
    let demos = [
        Demo {
            key: "plain",
            status: None,
            text: "Also cover the retry path in the webhook tests",
            summary: None,
            thumbnails: 0,
            more: None,
            primary: "Steer now",
            editing: false,
            blocked: false,
        },
        Demo {
            key: "sending",
            status: Some("Sending…"),
            text: "Then rerun the migration on staging",
            summary: None,
            thumbnails: 0,
            more: None,
            primary: "Send now",
            editing: false,
            blocked: true,
        },
        Demo {
            key: "images",
            status: None,
            text: "Match these two screens",
            summary: Some("3 attachments: before.png, after.png, notes.png"),
            thumbnails: if compact { 1 } else { 2 },
            more: Some(if compact { "+2" } else { "+1" }),
            primary: "Send now",
            editing: false,
            blocked: false,
        },
        Demo {
            key: "edited",
            status: None,
            text: "",
            summary: None,
            thumbnails: 0,
            more: None,
            primary: "Send now",
            editing: true,
            blocked: false,
        },
    ];
    div().w(px(width)).child(
        tray(&TrayStyle {
            background: t.surface_bg(),
            border: t.border,
            shadows: Some(t.menu_shadows()),
            tucked: 18.0,
        })
        .id(key)
        .role(Role::List)
        .aria_label("Queued messages")
        .children(demos.iter().map(|demo| demo_row(key, demo, compact, t))),
    )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        div()
            .flex()
            .flex_col()
            .gap(px(24.0))
            .child(demo_tray("story-queue-wide", 560.0, false, &t))
            .child(demo_tray("story-queue-compact", 420.0, true, &t))
    })
}
