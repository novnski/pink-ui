//! Every component in the gallery, one file each, all drawn in Pink's design
//! language with the [`crate::pink`] kit.
//!
//! Each module exports `NAME`, `DESCRIPTION` and `story`, which builds its demo.

use gpui::{AnyView, App, Window};

pub mod agent_cursors;
pub mod agent_inbox;
pub mod agent_lanes;
pub mod agent_plan;
pub mod agent_row;
pub mod agent_steering;
pub mod agent_tree;
pub mod ai_answer;
pub mod appearance_picker;
pub mod approval_card;
pub mod attachment_thumb;
pub mod automation_editor;
pub mod autonomy_dial;
pub mod billing_schedule;
pub mod breakdown_chart;
pub mod button;
pub mod change_summary;
pub mod chat_composer;
pub mod cited_answer;
pub mod code_block;
pub mod combobox;
pub mod command_menu;
pub mod command_row;
pub mod comment_card;
pub mod comment_thread;
pub mod comments_panel;
pub mod commit_history;
pub mod commit_row;
pub mod compact_breakdown;
pub mod composer_disc;
pub mod composer_pill;
pub mod context_ring;
pub mod context_window;
pub mod data_table;
pub mod date_range_picker;
pub mod decision_inbox;
pub mod delegate_board;
pub mod diff_header;
pub mod diff_rows;
pub mod diff_viewer;
pub mod dropzone;
pub mod email_draft;
pub mod empty_state;
pub mod experiment_card;
pub mod file_badge;
pub mod file_row;
pub mod file_tree;
pub mod filter_menu;
pub mod find_bar;
pub mod git_actions;
pub mod git_card;
pub mod icon_button;
pub mod image_lightbox;
pub mod inline_diff;
pub mod integration_picker;
pub mod integration_settings;
pub mod kpi_chart;
pub mod list_row;
pub mod loaders;
pub mod markdown_list;
pub mod markdown_message;
pub mod memory_panel;
pub mod menu_card;
pub mod message_actions;
pub mod message_badge;
pub mod model_picker;
pub mod notice;
pub mod notification_stack;
pub mod otp_input;
pub mod overlay_card;
pub mod overview_card;
pub mod permission_request;
pub mod place_tab;
pub mod pr_badge;
pub mod profile_menu;
pub mod progress_tracker;
pub mod project_card;
pub mod project_row;
pub mod prompt_library;
pub mod question_card;
pub mod queue_row;
pub mod reaction_bar;
pub mod recovery_card;
pub mod row_card;
pub mod scroll_rail;
pub mod section_header;
pub mod segmented_control;
pub mod settings_page;
pub mod settings_parts;
pub mod settings_section;
pub mod setup_guide;
pub mod sidebar;
pub mod skeleton;
pub mod smart_recommendation;
pub mod stepper_form;
pub mod subagent_cards;
pub mod surface_toolbar;
pub mod switch;
pub mod tab_bar;
pub mod tab_chip;
pub mod terminal;
pub mod terminal_tabs;
pub mod thinking_loader;
pub mod thinking_state;
pub mod tips_panel;
pub mod toaster;
pub mod tool_activity;
pub mod tool_call_card;
pub mod trace_waterfall;
pub mod trigger_chip;
pub mod usage_meter;
pub mod user_bubble;
pub mod welcome_screen;
pub mod working_trailer;
pub mod writing_assistant;

/// Category order used in the gallery sidebar.
pub const CATEGORIES: &[&str] = &[
    "Chat",
    "Agents",
    "Code",
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

/// All components, in gallery order.
pub static COMPONENTS: &[ComponentEntry] = &[
    ComponentEntry {
        slug: "ai-answer",
        name: ai_answer::NAME,
        category: "Chat",
        description: ai_answer::DESCRIPTION,
        story: ai_answer::story,
    },
    ComponentEntry {
        slug: "chat-composer",
        name: chat_composer::NAME,
        category: "Chat",
        description: chat_composer::DESCRIPTION,
        story: chat_composer::story,
    },
    ComponentEntry {
        slug: "trigger-chip",
        name: trigger_chip::NAME,
        category: "Chat",
        description: trigger_chip::DESCRIPTION,
        story: trigger_chip::story,
    },
    ComponentEntry {
        slug: "message-badge",
        name: message_badge::NAME,
        category: "Chat",
        description: message_badge::DESCRIPTION,
        story: message_badge::story,
    },
    ComponentEntry {
        slug: "queue-row",
        name: queue_row::NAME,
        category: "Chat",
        description: queue_row::DESCRIPTION,
        story: queue_row::story,
    },
    ComponentEntry {
        slug: "composer-disc",
        name: composer_disc::NAME,
        category: "Chat",
        description: composer_disc::DESCRIPTION,
        story: composer_disc::story,
    },
    ComponentEntry {
        slug: "composer-pill",
        name: composer_pill::NAME,
        category: "Chat",
        description: composer_pill::DESCRIPTION,
        story: composer_pill::story,
    },
    ComponentEntry {
        slug: "thinking-state",
        name: thinking_state::NAME,
        category: "Chat",
        description: thinking_state::DESCRIPTION,
        story: thinking_state::story,
    },
    ComponentEntry {
        slug: "writing-assistant",
        name: writing_assistant::NAME,
        category: "Chat",
        description: writing_assistant::DESCRIPTION,
        story: writing_assistant::story,
    },
    ComponentEntry {
        slug: "thinking-loader",
        name: thinking_loader::NAME,
        category: "Chat",
        description: thinking_loader::DESCRIPTION,
        story: thinking_loader::story,
    },
    ComponentEntry {
        slug: "working-trailer",
        name: working_trailer::NAME,
        category: "Chat",
        description: working_trailer::DESCRIPTION,
        story: working_trailer::story,
    },
    ComponentEntry {
        slug: "cited-answer",
        name: cited_answer::NAME,
        category: "Chat",
        description: cited_answer::DESCRIPTION,
        story: cited_answer::story,
    },
    ComponentEntry {
        slug: "comment-card",
        name: comment_card::NAME,
        category: "Chat",
        description: comment_card::DESCRIPTION,
        story: comment_card::story,
    },
    ComponentEntry {
        slug: "comment-thread",
        name: comment_thread::NAME,
        category: "Chat",
        description: comment_thread::DESCRIPTION,
        story: comment_thread::story,
    },
    ComponentEntry {
        slug: "comments-panel",
        name: comments_panel::NAME,
        category: "Chat",
        description: comments_panel::DESCRIPTION,
        story: comments_panel::story,
    },
    ComponentEntry {
        slug: "message-actions",
        name: message_actions::NAME,
        category: "Chat",
        description: message_actions::DESCRIPTION,
        story: message_actions::story,
    },
    ComponentEntry {
        slug: "user-bubble",
        name: user_bubble::NAME,
        category: "Chat",
        description: user_bubble::DESCRIPTION,
        story: user_bubble::story,
    },
    ComponentEntry {
        slug: "model-picker",
        name: model_picker::NAME,
        category: "Chat",
        description: model_picker::DESCRIPTION,
        story: model_picker::story,
    },
    ComponentEntry {
        slug: "markdown-message",
        name: markdown_message::NAME,
        category: "Chat",
        description: markdown_message::DESCRIPTION,
        story: markdown_message::story,
    },
    ComponentEntry {
        slug: "markdown-list",
        name: markdown_list::NAME,
        category: "Chat",
        description: markdown_list::DESCRIPTION,
        story: markdown_list::story,
    },
    ComponentEntry {
        slug: "agent-cursors",
        name: agent_cursors::NAME,
        category: "Agents",
        description: agent_cursors::DESCRIPTION,
        story: agent_cursors::story,
    },
    ComponentEntry {
        slug: "agent-inbox",
        name: agent_inbox::NAME,
        category: "Agents",
        description: agent_inbox::DESCRIPTION,
        story: agent_inbox::story,
    },
    ComponentEntry {
        slug: "agent-lanes",
        name: agent_lanes::NAME,
        category: "Agents",
        description: agent_lanes::DESCRIPTION,
        story: agent_lanes::story,
    },
    ComponentEntry {
        slug: "agent-plan",
        name: agent_plan::NAME,
        category: "Agents",
        description: agent_plan::DESCRIPTION,
        story: agent_plan::story,
    },
    ComponentEntry {
        slug: "agent-steering",
        name: agent_steering::NAME,
        category: "Agents",
        description: agent_steering::DESCRIPTION,
        story: agent_steering::story,
    },
    ComponentEntry {
        slug: "agent-tree",
        name: agent_tree::NAME,
        category: "Agents",
        description: agent_tree::DESCRIPTION,
        story: agent_tree::story,
    },
    ComponentEntry {
        slug: "approval-card",
        name: approval_card::NAME,
        category: "Agents",
        description: approval_card::DESCRIPTION,
        story: approval_card::story,
    },
    ComponentEntry {
        slug: "question-card",
        name: question_card::NAME,
        category: "Agents",
        description: question_card::DESCRIPTION,
        story: question_card::story,
    },
    ComponentEntry {
        slug: "autonomy-dial",
        name: autonomy_dial::NAME,
        category: "Agents",
        description: autonomy_dial::DESCRIPTION,
        story: autonomy_dial::story,
    },
    ComponentEntry {
        slug: "context-window",
        name: context_window::NAME,
        category: "Agents",
        description: context_window::DESCRIPTION,
        story: context_window::story,
    },
    ComponentEntry {
        slug: "context-ring",
        name: context_ring::NAME,
        category: "Agents",
        description: context_ring::DESCRIPTION,
        story: context_ring::story,
    },
    ComponentEntry {
        slug: "delegate-board",
        name: delegate_board::NAME,
        category: "Agents",
        description: delegate_board::DESCRIPTION,
        story: delegate_board::story,
    },
    ComponentEntry {
        slug: "inline-diff",
        name: inline_diff::NAME,
        category: "Agents",
        description: inline_diff::DESCRIPTION,
        story: inline_diff::story,
    },
    ComponentEntry {
        slug: "memory-panel",
        name: memory_panel::NAME,
        category: "Agents",
        description: memory_panel::DESCRIPTION,
        story: memory_panel::story,
    },
    ComponentEntry {
        slug: "permission-request",
        name: permission_request::NAME,
        category: "Agents",
        description: permission_request::DESCRIPTION,
        story: permission_request::story,
    },
    ComponentEntry {
        slug: "tool-activity",
        name: tool_activity::NAME,
        category: "Agents",
        description: tool_activity::DESCRIPTION,
        story: tool_activity::story,
    },
    ComponentEntry {
        slug: "tool-call-card",
        name: tool_call_card::NAME,
        category: "Agents",
        description: tool_call_card::DESCRIPTION,
        story: tool_call_card::story,
    },
    ComponentEntry {
        slug: "file-badge",
        name: file_badge::NAME,
        category: "Agents",
        description: file_badge::DESCRIPTION,
        story: file_badge::story,
    },
    ComponentEntry {
        slug: "recovery-card",
        name: recovery_card::NAME,
        category: "Agents",
        description: recovery_card::DESCRIPTION,
        story: recovery_card::story,
    },
    ComponentEntry {
        slug: "trace-waterfall",
        name: trace_waterfall::NAME,
        category: "Agents",
        description: trace_waterfall::DESCRIPTION,
        story: trace_waterfall::story,
    },
    ComponentEntry {
        slug: "automation-editor",
        name: automation_editor::NAME,
        category: "Agents",
        description: automation_editor::DESCRIPTION,
        story: automation_editor::story,
    },
    ComponentEntry {
        slug: "subagent-cards",
        name: subagent_cards::NAME,
        category: "Agents",
        description: subagent_cards::DESCRIPTION,
        story: subagent_cards::story,
    },
    ComponentEntry {
        slug: "agent-row",
        name: agent_row::NAME,
        category: "Agents",
        description: agent_row::DESCRIPTION,
        story: agent_row::story,
    },
    ComponentEntry {
        slug: "row-card",
        name: row_card::NAME,
        category: "Agents",
        description: row_card::DESCRIPTION,
        story: row_card::story,
    },
    ComponentEntry {
        slug: "email-draft",
        name: email_draft::NAME,
        category: "Cards",
        description: email_draft::DESCRIPTION,
        story: email_draft::story,
    },
    ComponentEntry {
        slug: "project-card",
        name: project_card::NAME,
        category: "Cards",
        description: project_card::DESCRIPTION,
        story: project_card::story,
    },
    ComponentEntry {
        slug: "smart-recommendation",
        name: smart_recommendation::NAME,
        category: "Cards",
        description: smart_recommendation::DESCRIPTION,
        story: smart_recommendation::story,
    },
    ComponentEntry {
        slug: "tips-panel",
        name: tips_panel::NAME,
        category: "Cards",
        description: tips_panel::DESCRIPTION,
        story: tips_panel::story,
    },
    ComponentEntry {
        slug: "breakdown-chart",
        name: breakdown_chart::NAME,
        category: "Data display",
        description: breakdown_chart::DESCRIPTION,
        story: breakdown_chart::story,
    },
    ComponentEntry {
        slug: "change-summary",
        name: change_summary::NAME,
        category: "Data display",
        description: change_summary::DESCRIPTION,
        story: change_summary::story,
    },
    ComponentEntry {
        slug: "compact-breakdown",
        name: compact_breakdown::NAME,
        category: "Data display",
        description: compact_breakdown::DESCRIPTION,
        story: compact_breakdown::story,
    },
    ComponentEntry {
        slug: "data-table",
        name: data_table::NAME,
        category: "Data display",
        description: data_table::DESCRIPTION,
        story: data_table::story,
    },
    ComponentEntry {
        slug: "experiment-card",
        name: experiment_card::NAME,
        category: "Data display",
        description: experiment_card::DESCRIPTION,
        story: experiment_card::story,
    },
    ComponentEntry {
        slug: "kpi-chart",
        name: kpi_chart::NAME,
        category: "Data display",
        description: kpi_chart::DESCRIPTION,
        story: kpi_chart::story,
    },
    ComponentEntry {
        slug: "usage-meter",
        name: usage_meter::NAME,
        category: "Data display",
        description: usage_meter::DESCRIPTION,
        story: usage_meter::story,
    },
    ComponentEntry {
        slug: "attachment-thumb",
        name: attachment_thumb::NAME,
        category: "Data display",
        description: attachment_thumb::DESCRIPTION,
        story: attachment_thumb::story,
    },
    ComponentEntry {
        slug: "image-lightbox",
        name: image_lightbox::NAME,
        category: "Data display",
        description: image_lightbox::DESCRIPTION,
        story: image_lightbox::story,
    },
    ComponentEntry {
        slug: "menu-card",
        name: menu_card::NAME,
        category: "Navigation",
        description: menu_card::DESCRIPTION,
        story: menu_card::story,
    },
    ComponentEntry {
        slug: "overview-card",
        name: overview_card::NAME,
        category: "Navigation",
        description: overview_card::DESCRIPTION,
        story: overview_card::story,
    },
    ComponentEntry {
        slug: "scroll-rail",
        name: scroll_rail::NAME,
        category: "Navigation",
        description: scroll_rail::DESCRIPTION,
        story: scroll_rail::story,
    },
    ComponentEntry {
        slug: "command-menu",
        name: command_menu::NAME,
        category: "Navigation",
        description: command_menu::DESCRIPTION,
        story: command_menu::story,
    },
    ComponentEntry {
        slug: "command-row",
        name: command_row::NAME,
        category: "Navigation",
        description: command_row::DESCRIPTION,
        story: command_row::story,
    },
    ComponentEntry {
        slug: "project-row",
        name: project_row::NAME,
        category: "Navigation",
        description: project_row::DESCRIPTION,
        story: project_row::story,
    },
    ComponentEntry {
        slug: "integration-picker",
        name: integration_picker::NAME,
        category: "Navigation",
        description: integration_picker::DESCRIPTION,
        story: integration_picker::story,
    },
    ComponentEntry {
        slug: "profile-menu",
        name: profile_menu::NAME,
        category: "Navigation",
        description: profile_menu::DESCRIPTION,
        story: profile_menu::story,
    },
    ComponentEntry {
        slug: "segmented-control",
        name: segmented_control::NAME,
        category: "Navigation",
        description: segmented_control::DESCRIPTION,
        story: segmented_control::story,
    },
    ComponentEntry {
        slug: "sidebar",
        name: sidebar::NAME,
        category: "Navigation",
        description: sidebar::DESCRIPTION,
        story: sidebar::story,
    },
    ComponentEntry {
        slug: "section-header",
        name: section_header::NAME,
        category: "Navigation",
        description: section_header::DESCRIPTION,
        story: section_header::story,
    },
    ComponentEntry {
        slug: "icon-button",
        name: icon_button::NAME,
        category: "Navigation",
        description: icon_button::DESCRIPTION,
        story: icon_button::story,
    },
    ComponentEntry {
        slug: "list-row",
        name: list_row::NAME,
        category: "Navigation",
        description: list_row::DESCRIPTION,
        story: list_row::story,
    },
    ComponentEntry {
        slug: "tab-bar",
        name: tab_bar::NAME,
        category: "Navigation",
        description: tab_bar::DESCRIPTION,
        story: tab_bar::story,
    },
    ComponentEntry {
        slug: "tab-chip",
        name: tab_chip::NAME,
        category: "Navigation",
        description: tab_chip::DESCRIPTION,
        story: tab_chip::story,
    },
    ComponentEntry {
        slug: "place-tab",
        name: place_tab::NAME,
        category: "Navigation",
        description: place_tab::DESCRIPTION,
        story: place_tab::story,
    },
    ComponentEntry {
        slug: "terminal-tabs",
        name: terminal_tabs::NAME,
        category: "Navigation",
        description: terminal_tabs::DESCRIPTION,
        story: terminal_tabs::story,
    },
    ComponentEntry {
        slug: "file-tree",
        name: file_tree::NAME,
        category: "Navigation",
        description: file_tree::DESCRIPTION,
        story: file_tree::story,
    },
    ComponentEntry {
        slug: "file-row",
        name: file_row::NAME,
        category: "Navigation",
        description: file_row::DESCRIPTION,
        story: file_row::story,
    },
    ComponentEntry {
        slug: "surface-toolbar",
        name: surface_toolbar::NAME,
        category: "Navigation",
        description: surface_toolbar::DESCRIPTION,
        story: surface_toolbar::story,
    },
    ComponentEntry {
        slug: "button",
        name: button::NAME,
        category: "Buttons",
        description: button::DESCRIPTION,
        story: button::story,
    },
    ComponentEntry {
        slug: "reaction-bar",
        name: reaction_bar::NAME,
        category: "Buttons",
        description: reaction_bar::DESCRIPTION,
        story: reaction_bar::story,
    },
    ComponentEntry {
        slug: "billing-schedule",
        name: billing_schedule::NAME,
        category: "Inputs",
        description: billing_schedule::DESCRIPTION,
        story: billing_schedule::story,
    },
    ComponentEntry {
        slug: "combobox",
        name: combobox::NAME,
        category: "Inputs",
        description: combobox::DESCRIPTION,
        story: combobox::story,
    },
    ComponentEntry {
        slug: "date-range-picker",
        name: date_range_picker::NAME,
        category: "Inputs",
        description: date_range_picker::DESCRIPTION,
        story: date_range_picker::story,
    },
    ComponentEntry {
        slug: "dropzone",
        name: dropzone::NAME,
        category: "Inputs",
        description: dropzone::DESCRIPTION,
        story: dropzone::story,
    },
    ComponentEntry {
        slug: "filter-menu",
        name: filter_menu::NAME,
        category: "Inputs",
        description: filter_menu::DESCRIPTION,
        story: filter_menu::story,
    },
    ComponentEntry {
        slug: "integration-settings",
        name: integration_settings::NAME,
        category: "Inputs",
        description: integration_settings::DESCRIPTION,
        story: integration_settings::story,
    },
    ComponentEntry {
        slug: "otp-input",
        name: otp_input::NAME,
        category: "Inputs",
        description: otp_input::DESCRIPTION,
        story: otp_input::story,
    },
    ComponentEntry {
        slug: "prompt-library",
        name: prompt_library::NAME,
        category: "Inputs",
        description: prompt_library::DESCRIPTION,
        story: prompt_library::story,
    },
    ComponentEntry {
        slug: "stepper-form",
        name: stepper_form::NAME,
        category: "Inputs",
        description: stepper_form::DESCRIPTION,
        story: stepper_form::story,
    },
    ComponentEntry {
        slug: "find-bar",
        name: find_bar::NAME,
        category: "Inputs",
        description: find_bar::DESCRIPTION,
        story: find_bar::story,
    },
    ComponentEntry {
        slug: "settings-page",
        name: settings_page::NAME,
        category: "Inputs",
        description: settings_page::DESCRIPTION,
        story: settings_page::story,
    },
    ComponentEntry {
        slug: "settings-section",
        name: settings_section::NAME,
        category: "Inputs",
        description: settings_section::DESCRIPTION,
        story: settings_section::story,
    },
    ComponentEntry {
        slug: "settings-parts",
        name: settings_parts::NAME,
        category: "Inputs",
        description: settings_parts::DESCRIPTION,
        story: settings_parts::story,
    },
    ComponentEntry {
        slug: "switch",
        name: switch::NAME,
        category: "Inputs",
        description: switch::DESCRIPTION,
        story: switch::story,
    },
    ComponentEntry {
        slug: "appearance-picker",
        name: appearance_picker::NAME,
        category: "Inputs",
        description: appearance_picker::DESCRIPTION,
        story: appearance_picker::story,
    },
    ComponentEntry {
        slug: "overlay-card",
        name: overlay_card::NAME,
        category: "Feedback",
        description: overlay_card::DESCRIPTION,
        story: overlay_card::story,
    },
    ComponentEntry {
        slug: "decision-inbox",
        name: decision_inbox::NAME,
        category: "Feedback",
        description: decision_inbox::DESCRIPTION,
        story: decision_inbox::story,
    },
    ComponentEntry {
        slug: "empty-state",
        name: empty_state::NAME,
        category: "Feedback",
        description: empty_state::DESCRIPTION,
        story: empty_state::story,
    },
    ComponentEntry {
        slug: "notice",
        name: notice::NAME,
        category: "Feedback",
        description: notice::DESCRIPTION,
        story: notice::story,
    },
    ComponentEntry {
        slug: "notification-stack",
        name: notification_stack::NAME,
        category: "Feedback",
        description: notification_stack::DESCRIPTION,
        story: notification_stack::story,
    },
    ComponentEntry {
        slug: "progress-tracker",
        name: progress_tracker::NAME,
        category: "Feedback",
        description: progress_tracker::DESCRIPTION,
        story: progress_tracker::story,
    },
    ComponentEntry {
        slug: "setup-guide",
        name: setup_guide::NAME,
        category: "Feedback",
        description: setup_guide::DESCRIPTION,
        story: setup_guide::story,
    },
    ComponentEntry {
        slug: "toaster",
        name: toaster::NAME,
        category: "Feedback",
        description: toaster::DESCRIPTION,
        story: toaster::story,
    },
    ComponentEntry {
        slug: "welcome-screen",
        name: welcome_screen::NAME,
        category: "Feedback",
        description: welcome_screen::DESCRIPTION,
        story: welcome_screen::story,
    },
    ComponentEntry {
        slug: "loaders",
        name: loaders::NAME,
        category: "Feedback",
        description: loaders::DESCRIPTION,
        story: loaders::story,
    },
    ComponentEntry {
        slug: "skeleton",
        name: skeleton::NAME,
        category: "Feedback",
        description: skeleton::DESCRIPTION,
        story: skeleton::story,
    },
    ComponentEntry {
        slug: "code-block",
        name: code_block::NAME,
        category: "Code",
        description: code_block::DESCRIPTION,
        story: code_block::story,
    },
    ComponentEntry {
        slug: "diff-rows",
        name: diff_rows::NAME,
        category: "Code",
        description: diff_rows::DESCRIPTION,
        story: diff_rows::story,
    },
    ComponentEntry {
        slug: "diff-viewer",
        name: diff_viewer::NAME,
        category: "Code",
        description: diff_viewer::DESCRIPTION,
        story: diff_viewer::story,
    },
    ComponentEntry {
        slug: "diff-header",
        name: diff_header::NAME,
        category: "Code",
        description: diff_header::DESCRIPTION,
        story: diff_header::story,
    },
    ComponentEntry {
        slug: "commit-history",
        name: commit_history::NAME,
        category: "Code",
        description: commit_history::DESCRIPTION,
        story: commit_history::story,
    },
    ComponentEntry {
        slug: "commit-row",
        name: commit_row::NAME,
        category: "Code",
        description: commit_row::DESCRIPTION,
        story: commit_row::story,
    },
    ComponentEntry {
        slug: "git-actions",
        name: git_actions::NAME,
        category: "Code",
        description: git_actions::DESCRIPTION,
        story: git_actions::story,
    },
    ComponentEntry {
        slug: "git-card",
        name: git_card::NAME,
        category: "Code",
        description: git_card::DESCRIPTION,
        story: git_card::story,
    },
    ComponentEntry {
        slug: "pr-badge",
        name: pr_badge::NAME,
        category: "Code",
        description: pr_badge::DESCRIPTION,
        story: pr_badge::story,
    },
    ComponentEntry {
        slug: "terminal",
        name: terminal::NAME,
        category: "Code",
        description: terminal::DESCRIPTION,
        story: terminal::story,
    },
];

pub fn find(slug: &str) -> Option<&'static ComponentEntry> {
    COMPONENTS.iter().find(|c| c.slug == slug)
}
