//! The free (MIT) components from affectui.com/components, one module each.
//!
//! Every module exposes `story`, which builds the same demo the website shows.

use gpui::{AnyView, App, Window};

pub mod button;
pub mod change_summary;
pub mod comment_thread;
pub mod decision_inbox;
pub mod email_draft;
pub mod filter_menu;
pub mod message_actions;
pub mod notification_stack;
pub mod otp_input;
pub mod profile_menu;
pub mod progress_tracker;
pub mod reaction_bar;
pub mod segmented_control;
pub mod smart_recommendation;
pub mod thinking_loader;
pub mod tips_panel;
pub mod tool_call_card;

/// Category order used on the website sidebar.
pub const CATEGORIES: &[&str] = &[
    "Chat",
    "Agents",
    "Cards",
    "Data display",
    "Navigation",
    "Buttons",
    "Inputs",
    "Feedback",
];

pub struct ComponentEntry {
    pub slug: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub story: fn(&mut Window, &mut App) -> AnyView,
}

/// All components, in the order the website lists them.
pub static COMPONENTS: &[ComponentEntry] = &[
    ComponentEntry {
        slug: "thinking-loader",
        name: "Thinking Loader",
        category: "Chat",
        description: "A small working pill for AI replies – an eight-ray spinner and a shimmering label that rolls through phases and settles on a check",
        story: thinking_loader::story,
    },
    ComponentEntry {
        slug: "comment-thread",
        name: "Comment Thread",
        category: "Chat",
        description: "One comment with replies – an assistant reports what it did, folds open details and answers @mentions",
        story: comment_thread::story,
    },
    ComponentEntry {
        slug: "message-actions",
        name: "Message Actions",
        category: "Chat",
        description: "The quiet bar under an AI message – copy turns into a check, regenerate spins and unfolds a version switcher, thumbs pop and ask why, read aloud becomes a tiny equalizer",
        story: message_actions::story,
    },
    ComponentEntry {
        slug: "tool-call-card",
        name: "Tool Call Card",
        category: "Agents",
        description: "An agent's tool call in one line – live timer, status and summary, folds open to highlighted arguments and result with copy, retry on errors and a group that sums them up",
        story: tool_call_card::story,
    },
    ComponentEntry {
        slug: "email-draft",
        name: "Email Draft",
        category: "Cards",
        description: "Agent-written email for review – switch recipient and tone, edit inline, remove the attachment, send or discard with undo",
        story: email_draft::story,
    },
    ComponentEntry {
        slug: "smart-recommendation",
        name: "Smart Recommendation",
        category: "Cards",
        description: "Suggestion card with a confidence signal, alternatives to switch to and accept with undo",
        story: smart_recommendation::story,
    },
    ComponentEntry {
        slug: "tips-panel",
        name: "Tips Panel",
        category: "Cards",
        description: "Grid of short tips with one action each – taken tips turn into a done state",
        story: tips_panel::story,
    },
    ComponentEntry {
        slug: "change-summary",
        name: "Change Summary",
        category: "Data display",
        description: "What moved since last time – one metric per row with a colored trend",
        story: change_summary::story,
    },
    ComponentEntry {
        slug: "profile-menu",
        name: "Profile Menu",
        category: "Navigation",
        description: "Account card with the signed-in user, menu actions and profile setup progress",
        story: profile_menu::story,
    },
    ComponentEntry {
        slug: "segmented-control",
        name: "Segmented Control",
        category: "Navigation",
        description: "Switch between two to six options – a raised pill or underline that glides, stretches a little and can be dragged across, with icons, counts and badges",
        story: segmented_control::story,
    },
    ComponentEntry {
        slug: "button",
        name: "Button",
        category: "Buttons",
        description: "Primary, secondary, ghost and destructive buttons with icons, sizes and a shimmering loading state",
        story: button::story,
    },
    ComponentEntry {
        slug: "reaction-bar",
        name: "Reaction Bar",
        category: "Buttons",
        description: "Like, comment, repost and share in one pill – icons pop and fill, counts roll in digit by digit",
        story: reaction_bar::story,
    },
    ComponentEntry {
        slug: "filter-menu",
        name: "Filter Menu",
        category: "Inputs",
        description: "Grouped filters with removable chips, collapsible checkbox groups, Clear all and full keyboard control",
        story: filter_menu::story,
    },
    ComponentEntry {
        slug: "otp-input",
        name: "OTP Input",
        category: "Inputs",
        description: "The verification code field done right – a gliding focus ring, paste and SMS autofill, a calm shake on errors and a morph into Verified",
        story: otp_input::story,
    },
    ComponentEntry {
        slug: "decision-inbox",
        name: "Decision Inbox",
        category: "Feedback",
        description: "Open decisions with two quick answers each – answered ones confirm and fold away",
        story: decision_inbox::story,
    },
    ComponentEntry {
        slug: "notification-stack",
        name: "Notification Stack",
        category: "Feedback",
        description: "Notice cards stacked behind each other – dismiss the front one to reveal the next",
        story: notification_stack::story,
    },
    ComponentEntry {
        slug: "progress-tracker",
        name: "Progress Tracker",
        category: "Feedback",
        description: "Onboarding checklist with an animated progress bar, ticks that draw themselves and auto play",
        story: progress_tracker::story,
    },
];

pub fn find(slug: &str) -> Option<&'static ComponentEntry> {
    COMPONENTS.iter().find(|c| c.slug == slug)
}
