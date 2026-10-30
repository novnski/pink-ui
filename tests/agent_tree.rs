//! The `pink-agent-tree` package's outline geometry at Interface sizes 12, 16
//! and 20, as PINK's Agents place and the gallery's Agent Tree nest
//! `pink-agent-row` rows with it. Failure modes: the tree's copy of the row
//! height drifting from `pink-agent-row` (slots and rails then miss the
//! rows, or text clips); a child's dot that no longer sits where its
//! parent's branch ends (one `INDENT` right, `RAIL_SPAN` from the trunk) or
//! stops scaling with the Interface size; a disclosure tile that changes the
//! title line's height or leaves leaves without the tile's slot (status words
//! no longer end on one column); a settled rail that draws a trunk past a
//! last child, drops an ancestor's trunk that still has a later sibling, or
//! starts a trunk under a folded branch.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::{
        agent_row::{self, dot, meta_line, row, status_word, title_line},
        agent_tree::{
            BRANCH_GAP, INDENT, RAIL_SPAN, ROW_HEIGHT, ROW_PAD_X, Rail, TILE, TRUNK_X,
            branch_points, disclosure, indent, tile_slot, trunk_x, ui_rems,
        },
    },
    pink::Pink,
};

struct Host {
    ui: f32,
}

fn agent(name: &'static str, depth: usize, branch: bool, t: &Pink) -> gpui::Div {
    let tile = if branch {
        disclosure("icons/agent-tree/alt-arrow-right.svg", 1.0, t.text, None)
            .debug_selector(move || format!("{name}-tile"))
    } else {
        tile_slot().debug_selector(move || format!("{name}-tile"))
    };
    row()
        .pl(ui_rems(indent(depth)))
        .w(gpui::px(360.0))
        .debug_selector(move || format!("{name}-row"))
        .child(
            title_line(
                t.text,
                dot(t.accent).debug_selector(move || format!("{name}-dot")),
                name,
                status_word(t.text_muted, "Running"),
            )
            .child(tile),
        )
        .child(meta_line(t.text_muted, "Rewrite casts"))
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(agent("lead", 0, true, &t))
            .child(agent("child", 1, true, &t))
            .child(agent("grandchild", 2, false, &t))
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &str) -> Bounds<Pixels> {
    // Debug selectors are looked up by a static name.
    let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn centre_x(b: Bounds<Pixels>) -> f32 {
    f32::from(b.origin.x) + f32::from(b.size.width) / 2.0
}

#[gpui::test]
fn nested_rows_put_each_dot_where_its_parents_branch_ends(cx: &mut TestAppContext) {
    assert_eq!(
        ROW_HEIGHT,
        agent_row::ROW_HEIGHT,
        "the tree's slot height drifted from pink-agent-row's row"
    );
    assert_eq!(ROW_PAD_X, agent_row::PAD_X);
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        for (name, depth) in [("lead", 0), ("child", 1), ("grandchild", 2)] {
            let row = bounds(cx, &format!("{name}-row"));
            assert!(
                (f32::from(row.size.height) - ROW_HEIGHT * scale).abs() < 1.0,
                "{name} row is {:?} tall at {ui} with its tile, expected {}",
                row.size.height,
                ROW_HEIGHT * scale
            );
            let dot = bounds(cx, &format!("{name}-dot"));
            let dot_x = centre_x(dot) - f32::from(row.origin.x);
            // The dot snaps to whole layout units: within half a pixel.
            assert!(
                (dot_x - (TRUNK_X + depth as f32 * INDENT) * scale).abs() < 0.5,
                "{name} dot centre {dot_x} at {ui}"
            );
            if depth > 0 {
                // The child's dot starts RAIL_SPAN right of the parent's
                // trunk, BRANCH_GAP after the branch ends.
                let span = f32::from(dot.origin.x - row.origin.x) - trunk_x(depth - 1) * scale;
                assert!(
                    (span - RAIL_SPAN * scale).abs() < 0.5,
                    "{name} sits {span} from its parent's trunk at {ui}"
                );
            }
            let tile = bounds(cx, &format!("{name}-tile"));
            assert!(
                (f32::from(tile.size.width) - TILE * scale).abs() < 0.05,
                "{name} tile is {:?} wide at {ui}",
                tile.size.width
            );
        }
        // Branch and leaf tiles share one slot, so status words end on one
        // column whatever the depth.
        let lead = bounds(cx, "lead-tile");
        let leaf = bounds(cx, "grandchild-tile");
        assert!((f32::from(lead.origin.x - leaf.origin.x)).abs() < 0.05);
    }
}

#[test]
fn settled_rails_follow_siblings_and_folds() {
    // The branch ends BRANCH_GAP before the child's dot.
    let tip = *branch_points(1.0).last().unwrap();
    assert!((tip.x - (RAIL_SPAN - BRANCH_GAP)).abs() < 1e-4);
    // A half-drawn branch stops short of the tip.
    assert!(branch_points(0.5).last().unwrap().x < tip.x);

    // A root with open children starts its own trunk; folded, it does not.
    let open = Rail::settled(0, &[], true);
    assert!(open.elbow.is_none());
    assert_eq!(open.start, Some((trunk_x(0), 1.0, 1.0)));
    assert_eq!(Rail::settled(0, &[], false).start, None);

    // A last child: elbow from the parent's trunk with no continuation.
    let last = Rail::settled(1, &[false], false);
    let elbow = last.elbow.unwrap();
    assert_eq!(elbow.x, trunk_x(0));
    assert_eq!(elbow.continuation, 0.0);
    assert!(last.guides.is_empty());

    // A middle child continues its parent's trunk for the next sibling.
    assert_eq!(
        Rail::settled(1, &[true], false).elbow.unwrap().continuation,
        1.0
    );

    // A grandchild whose parent has a later sibling: the grandparent's
    // trunk passes as a guide; one whose parent is last: no guide.
    assert_eq!(
        Rail::settled(2, &[true, false], false).guides,
        vec![(trunk_x(0), 1.0)]
    );
    assert!(Rail::settled(2, &[false, false], false).guides.is_empty());
}
