//! Every component in the gallery, one file each.
//!
//! - **Core** components come in two looks, Clean and Classic: Inter type,
//!   their own color tokens and hairline-shadow edges.
//! - **Pink** components are drawn in Pink's design language: Geist type,
//!   Pink's neutral surfaces and accent, its radius and spacing ladders.
//!
//! Each module exports `NAME`, `DESCRIPTION` and `story`, which builds its demo.

use gpui::{AnyView, App, Window};

pub mod agent_cursors;
pub mod agent_inbox;
pub mod agent_lanes;
pub mod agent_plan;
pub mod agent_steering;
pub mod agent_tree;
pub mod ai_answer;
pub mod approval_card;
pub mod autonomy_dial;
pub mod billing_schedule;
pub mod breakdown_chart;
pub mod button;
pub mod change_summary;
pub mod chat_composer;
pub mod cited_answer;
pub mod combobox;
pub mod command_menu;
pub mod comment_thread;
pub mod comments_panel;
pub mod compact_breakdown;
pub mod context_window;
pub mod data_table;
pub mod date_range_picker;
pub mod decision_inbox;
pub mod delegate_board;
pub mod dropzone;
pub mod email_draft;
pub mod experiment_card;
pub mod filter_menu;
pub mod inline_diff;
pub mod integration_picker;
pub mod integration_settings;
pub mod kpi_chart;
pub mod memory_panel;
pub mod message_actions;
pub mod model_picker;
pub mod notification_stack;
pub mod otp_input;
pub mod permission_request;
pub mod profile_menu;
pub mod progress_tracker;
pub mod project_card;
pub mod prompt_library;
pub mod reaction_bar;
pub mod segmented_control;
pub mod setup_guide;
pub mod smart_recommendation;
pub mod stepper_form;
pub mod thinking_loader;
pub mod thinking_state;
pub mod tips_panel;
pub mod toaster;
pub mod tool_call_card;
pub mod trace_waterfall;
pub mod usage_meter;
pub mod writing_assistant;

/// Category order used in the gallery sidebar.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Comes in both looks, Clean and Classic.
    Core,
    /// Drawn in Pink's design language, in one look.
    Pink,
}

pub struct ComponentEntry {
    pub slug: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub kind: Kind,
    pub story: fn(&mut Window, &mut App) -> AnyView,
}

/// All components, in gallery order.
pub static COMPONENTS: &[ComponentEntry] = &[
    ComponentEntry {
        slug: "ai-answer",
        name: ai_answer::NAME,
        category: "Chat",
        description: ai_answer::DESCRIPTION,
        kind: Kind::Pink,
        story: ai_answer::story,
    },
    ComponentEntry {
        slug: "chat-composer",
        name: chat_composer::NAME,
        category: "Chat",
        description: chat_composer::DESCRIPTION,
        kind: Kind::Pink,
        story: chat_composer::story,
    },
    ComponentEntry {
        slug: "thinking-state",
        name: thinking_state::NAME,
        category: "Chat",
        description: thinking_state::DESCRIPTION,
        kind: Kind::Pink,
        story: thinking_state::story,
    },
    ComponentEntry {
        slug: "writing-assistant",
        name: writing_assistant::NAME,
        category: "Chat",
        description: writing_assistant::DESCRIPTION,
        kind: Kind::Pink,
        story: writing_assistant::story,
    },
    ComponentEntry {
        slug: "thinking-loader",
        name: thinking_loader::NAME,
        category: "Chat",
        description: thinking_loader::DESCRIPTION,
        kind: Kind::Core,
        story: thinking_loader::story,
    },
    ComponentEntry {
        slug: "cited-answer",
        name: cited_answer::NAME,
        category: "Chat",
        description: cited_answer::DESCRIPTION,
        kind: Kind::Pink,
        story: cited_answer::story,
    },
    ComponentEntry {
        slug: "comment-thread",
        name: comment_thread::NAME,
        category: "Chat",
        description: comment_thread::DESCRIPTION,
        kind: Kind::Core,
        story: comment_thread::story,
    },
    ComponentEntry {
        slug: "comments-panel",
        name: comments_panel::NAME,
        category: "Chat",
        description: comments_panel::DESCRIPTION,
        kind: Kind::Pink,
        story: comments_panel::story,
    },
    ComponentEntry {
        slug: "message-actions",
        name: message_actions::NAME,
        category: "Chat",
        description: message_actions::DESCRIPTION,
        kind: Kind::Core,
        story: message_actions::story,
    },
    ComponentEntry {
        slug: "model-picker",
        name: model_picker::NAME,
        category: "Chat",
        description: model_picker::DESCRIPTION,
        kind: Kind::Pink,
        story: model_picker::story,
    },
    ComponentEntry {
        slug: "agent-cursors",
        name: agent_cursors::NAME,
        category: "Agents",
        description: agent_cursors::DESCRIPTION,
        kind: Kind::Pink,
        story: agent_cursors::story,
    },
    ComponentEntry {
        slug: "agent-inbox",
        name: agent_inbox::NAME,
        category: "Agents",
        description: agent_inbox::DESCRIPTION,
        kind: Kind::Pink,
        story: agent_inbox::story,
    },
    ComponentEntry {
        slug: "agent-lanes",
        name: agent_lanes::NAME,
        category: "Agents",
        description: agent_lanes::DESCRIPTION,
        kind: Kind::Pink,
        story: agent_lanes::story,
    },
    ComponentEntry {
        slug: "agent-plan",
        name: agent_plan::NAME,
        category: "Agents",
        description: agent_plan::DESCRIPTION,
        kind: Kind::Pink,
        story: agent_plan::story,
    },
    ComponentEntry {
        slug: "agent-steering",
        name: agent_steering::NAME,
        category: "Agents",
        description: agent_steering::DESCRIPTION,
        kind: Kind::Pink,
        story: agent_steering::story,
    },
    ComponentEntry {
        slug: "agent-tree",
        name: agent_tree::NAME,
        category: "Agents",
        description: agent_tree::DESCRIPTION,
        kind: Kind::Pink,
        story: agent_tree::story,
    },
    ComponentEntry {
        slug: "approval-card",
        name: approval_card::NAME,
        category: "Agents",
        description: approval_card::DESCRIPTION,
        kind: Kind::Pink,
        story: approval_card::story,
    },
    ComponentEntry {
        slug: "autonomy-dial",
        name: autonomy_dial::NAME,
        category: "Agents",
        description: autonomy_dial::DESCRIPTION,
        kind: Kind::Pink,
        story: autonomy_dial::story,
    },
    ComponentEntry {
        slug: "context-window",
        name: context_window::NAME,
        category: "Agents",
        description: context_window::DESCRIPTION,
        kind: Kind::Pink,
        story: context_window::story,
    },
    ComponentEntry {
        slug: "delegate-board",
        name: delegate_board::NAME,
        category: "Agents",
        description: delegate_board::DESCRIPTION,
        kind: Kind::Pink,
        story: delegate_board::story,
    },
    ComponentEntry {
        slug: "inline-diff",
        name: inline_diff::NAME,
        category: "Agents",
        description: inline_diff::DESCRIPTION,
        kind: Kind::Pink,
        story: inline_diff::story,
    },
    ComponentEntry {
        slug: "memory-panel",
        name: memory_panel::NAME,
        category: "Agents",
        description: memory_panel::DESCRIPTION,
        kind: Kind::Pink,
        story: memory_panel::story,
    },
    ComponentEntry {
        slug: "permission-request",
        name: permission_request::NAME,
        category: "Agents",
        description: permission_request::DESCRIPTION,
        kind: Kind::Pink,
        story: permission_request::story,
    },
    ComponentEntry {
        slug: "tool-call-card",
        name: tool_call_card::NAME,
        category: "Agents",
        description: tool_call_card::DESCRIPTION,
        kind: Kind::Core,
        story: tool_call_card::story,
    },
    ComponentEntry {
        slug: "trace-waterfall",
        name: trace_waterfall::NAME,
        category: "Agents",
        description: trace_waterfall::DESCRIPTION,
        kind: Kind::Pink,
        story: trace_waterfall::story,
    },
    ComponentEntry {
        slug: "email-draft",
        name: email_draft::NAME,
        category: "Cards",
        description: email_draft::DESCRIPTION,
        kind: Kind::Core,
        story: email_draft::story,
    },
    ComponentEntry {
        slug: "project-card",
        name: project_card::NAME,
        category: "Cards",
        description: project_card::DESCRIPTION,
        kind: Kind::Pink,
        story: project_card::story,
    },
    ComponentEntry {
        slug: "smart-recommendation",
        name: smart_recommendation::NAME,
        category: "Cards",
        description: smart_recommendation::DESCRIPTION,
        kind: Kind::Core,
        story: smart_recommendation::story,
    },
    ComponentEntry {
        slug: "tips-panel",
        name: tips_panel::NAME,
        category: "Cards",
        description: tips_panel::DESCRIPTION,
        kind: Kind::Core,
        story: tips_panel::story,
    },
    ComponentEntry {
        slug: "breakdown-chart",
        name: breakdown_chart::NAME,
        category: "Data display",
        description: breakdown_chart::DESCRIPTION,
        kind: Kind::Pink,
        story: breakdown_chart::story,
    },
    ComponentEntry {
        slug: "change-summary",
        name: change_summary::NAME,
        category: "Data display",
        description: change_summary::DESCRIPTION,
        kind: Kind::Core,
        story: change_summary::story,
    },
    ComponentEntry {
        slug: "compact-breakdown",
        name: compact_breakdown::NAME,
        category: "Data display",
        description: compact_breakdown::DESCRIPTION,
        kind: Kind::Pink,
        story: compact_breakdown::story,
    },
    ComponentEntry {
        slug: "data-table",
        name: data_table::NAME,
        category: "Data display",
        description: data_table::DESCRIPTION,
        kind: Kind::Pink,
        story: data_table::story,
    },
    ComponentEntry {
        slug: "experiment-card",
        name: experiment_card::NAME,
        category: "Data display",
        description: experiment_card::DESCRIPTION,
        kind: Kind::Pink,
        story: experiment_card::story,
    },
    ComponentEntry {
        slug: "kpi-chart",
        name: kpi_chart::NAME,
        category: "Data display",
        description: kpi_chart::DESCRIPTION,
        kind: Kind::Pink,
        story: kpi_chart::story,
    },
    ComponentEntry {
        slug: "usage-meter",
        name: usage_meter::NAME,
        category: "Data display",
        description: usage_meter::DESCRIPTION,
        kind: Kind::Pink,
        story: usage_meter::story,
    },
    ComponentEntry {
        slug: "command-menu",
        name: command_menu::NAME,
        category: "Navigation",
        description: command_menu::DESCRIPTION,
        kind: Kind::Pink,
        story: command_menu::story,
    },
    ComponentEntry {
        slug: "integration-picker",
        name: integration_picker::NAME,
        category: "Navigation",
        description: integration_picker::DESCRIPTION,
        kind: Kind::Pink,
        story: integration_picker::story,
    },
    ComponentEntry {
        slug: "profile-menu",
        name: profile_menu::NAME,
        category: "Navigation",
        description: profile_menu::DESCRIPTION,
        kind: Kind::Core,
        story: profile_menu::story,
    },
    ComponentEntry {
        slug: "segmented-control",
        name: segmented_control::NAME,
        category: "Navigation",
        description: segmented_control::DESCRIPTION,
        kind: Kind::Core,
        story: segmented_control::story,
    },
    ComponentEntry {
        slug: "button",
        name: button::NAME,
        category: "Buttons",
        description: button::DESCRIPTION,
        kind: Kind::Core,
        story: button::story,
    },
    ComponentEntry {
        slug: "reaction-bar",
        name: reaction_bar::NAME,
        category: "Buttons",
        description: reaction_bar::DESCRIPTION,
        kind: Kind::Core,
        story: reaction_bar::story,
    },
    ComponentEntry {
        slug: "billing-schedule",
        name: billing_schedule::NAME,
        category: "Inputs",
        description: billing_schedule::DESCRIPTION,
        kind: Kind::Pink,
        story: billing_schedule::story,
    },
    ComponentEntry {
        slug: "combobox",
        name: combobox::NAME,
        category: "Inputs",
        description: combobox::DESCRIPTION,
        kind: Kind::Pink,
        story: combobox::story,
    },
    ComponentEntry {
        slug: "date-range-picker",
        name: date_range_picker::NAME,
        category: "Inputs",
        description: date_range_picker::DESCRIPTION,
        kind: Kind::Pink,
        story: date_range_picker::story,
    },
    ComponentEntry {
        slug: "dropzone",
        name: dropzone::NAME,
        category: "Inputs",
        description: dropzone::DESCRIPTION,
        kind: Kind::Pink,
        story: dropzone::story,
    },
    ComponentEntry {
        slug: "filter-menu",
        name: filter_menu::NAME,
        category: "Inputs",
        description: filter_menu::DESCRIPTION,
        kind: Kind::Core,
        story: filter_menu::story,
    },
    ComponentEntry {
        slug: "integration-settings",
        name: integration_settings::NAME,
        category: "Inputs",
        description: integration_settings::DESCRIPTION,
        kind: Kind::Pink,
        story: integration_settings::story,
    },
    ComponentEntry {
        slug: "otp-input",
        name: otp_input::NAME,
        category: "Inputs",
        description: otp_input::DESCRIPTION,
        kind: Kind::Core,
        story: otp_input::story,
    },
    ComponentEntry {
        slug: "prompt-library",
        name: prompt_library::NAME,
        category: "Inputs",
        description: prompt_library::DESCRIPTION,
        kind: Kind::Pink,
        story: prompt_library::story,
    },
    ComponentEntry {
        slug: "stepper-form",
        name: stepper_form::NAME,
        category: "Inputs",
        description: stepper_form::DESCRIPTION,
        kind: Kind::Pink,
        story: stepper_form::story,
    },
    ComponentEntry {
        slug: "decision-inbox",
        name: decision_inbox::NAME,
        category: "Feedback",
        description: decision_inbox::DESCRIPTION,
        kind: Kind::Core,
        story: decision_inbox::story,
    },
    ComponentEntry {
        slug: "notification-stack",
        name: notification_stack::NAME,
        category: "Feedback",
        description: notification_stack::DESCRIPTION,
        kind: Kind::Core,
        story: notification_stack::story,
    },
    ComponentEntry {
        slug: "progress-tracker",
        name: progress_tracker::NAME,
        category: "Feedback",
        description: progress_tracker::DESCRIPTION,
        kind: Kind::Core,
        story: progress_tracker::story,
    },
    ComponentEntry {
        slug: "setup-guide",
        name: setup_guide::NAME,
        category: "Feedback",
        description: setup_guide::DESCRIPTION,
        kind: Kind::Pink,
        story: setup_guide::story,
    },
    ComponentEntry {
        slug: "toaster",
        name: toaster::NAME,
        category: "Feedback",
        description: toaster::DESCRIPTION,
        kind: Kind::Pink,
        story: toaster::story,
    },
];

pub fn find(slug: &str) -> Option<&'static ComponentEntry> {
    COMPONENTS.iter().find(|c| c.slug == slug)
}
