//! Pure layout description tree mirroring Swift EditorView.swift's nested
//! NSSplitView structure, so pane arrangement is unit-testable without gpui.
//! editor_view::render_pane_layout turns this tree into elements.

use crate::pane::{LayoutPreset, PaneId, PaneLayout};
use crate::pane_resize::ResizeTarget;

/// Sizing of a node along its parent container's axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaneSize {
    /// Fixed size in logical px.
    Fixed(f32),
    /// Fills remaining space (flex_1).
    Flex,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PaneNodeKind {
    /// Horizontal container, children left→right.
    Row(Vec<PaneNode>),
    /// Vertical container, children top→bottom.
    Column(Vec<PaneNode>),
    /// A single pane's card.
    Pane(PaneId),
    /// The Toolbar + Timeline composite (Swift timelineHC).
    TimelineRegion,
    /// Zero-sized drag handle riding the seam between two siblings;
    /// `horizontal` = the divider resizes a width (col-resize cursor).
    Divider {
        target: ResizeTarget,
        horizontal: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaneNode {
    pub kind: PaneNodeKind,
    pub size: PaneSize,
}

impl PaneNode {
    fn pane(id: PaneId, size: PaneSize) -> Self {
        Self {
            kind: PaneNodeKind::Pane(id),
            size,
        }
    }

    fn divider(target: ResizeTarget, horizontal: bool) -> Self {
        Self {
            kind: PaneNodeKind::Divider { target, horizontal },
            size: PaneSize::Fixed(0.0),
        }
    }

    fn is_divider(&self) -> bool {
        matches!(self.kind, PaneNodeKind::Divider { .. })
    }
}

/// Concrete pane dimensions used to build the tree (resolved from stored
/// state + viewport by the caller).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedSizes {
    pub agent_width: f32,
    /// Media pane width (Default upper row, Media preset left column,
    /// Vertical upper-left split).
    pub media_width: f32,
    pub inspector_width: f32,
    /// Height of the Toolbar + Timeline region.
    pub timeline_height: f32,
    /// Vertical preset: width of the left (media/inspector + timeline) column.
    pub vertical_left_width: f32,
}

/// Build the pane tree for the active preset, honoring pane visibility.
/// The Agent pane is always the outermost left column (Swift: the agent
/// split item is a sibling of the preset root, not part of it).
pub fn build_pane_tree(layout: &PaneLayout, sizes: &ResolvedSizes) -> PaneNode {
    let visible = |id: PaneId| layout.is_visible(id);

    // Push `node` onto `list`, first laying a divider on the seam when the
    // list already ends with a pane/container (never two dividers in a row).
    fn push_seamed(list: &mut Vec<PaneNode>, node: PaneNode, seam: Option<(ResizeTarget, bool)>) {
        if let Some((target, horizontal)) = seam {
            if list.last().is_some_and(|last| !last.is_divider()) {
                list.push(PaneNode::divider(target, horizontal));
            }
        }
        list.push(node);
    }

    // Upper row + Toolbar/Timeline region stacked vertically (each Swift
    // preset root). An empty upper row lets the timeline region flex-fill,
    // matching NSSplitView collapse behavior.
    let preset_column = |upper: Vec<PaneNode>, size: PaneSize| {
        let mut col = Vec::new();
        let has_upper = !upper.is_empty();
        if has_upper {
            col.push(PaneNode {
                kind: PaneNodeKind::Row(upper),
                size: PaneSize::Flex,
            });
        }
        if visible(PaneId::Timeline) {
            let region = PaneNode {
                kind: PaneNodeKind::TimelineRegion,
                size: if has_upper {
                    PaneSize::Fixed(sizes.timeline_height)
                } else {
                    PaneSize::Flex
                },
            };
            let seam = has_upper.then_some((ResizeTarget::TimelineHeight, false));
            push_seamed(&mut col, region, seam);
        }
        PaneNode {
            kind: PaneNodeKind::Column(col),
            size,
        }
    };

    let mut root: Vec<PaneNode> = Vec::new();
    if visible(PaneId::Agent) {
        root.push(PaneNode::pane(
            PaneId::Agent,
            PaneSize::Fixed(sizes.agent_width),
        ));
    }

    match layout.preset {
        LayoutPreset::Default => {
            // [Agent] | [Media | Preview | Inspector (70%) / Toolbar+Timeline (30%)]
            let mut upper = Vec::new();
            if visible(PaneId::Media) {
                upper.push(PaneNode::pane(
                    PaneId::Media,
                    PaneSize::Fixed(sizes.media_width),
                ));
            }
            if visible(PaneId::Preview) {
                push_seamed(
                    &mut upper,
                    PaneNode::pane(PaneId::Preview, PaneSize::Flex),
                    Some((ResizeTarget::MediaWidth, true)),
                );
            }
            if visible(PaneId::Inspector) {
                push_seamed(
                    &mut upper,
                    PaneNode::pane(PaneId::Inspector, PaneSize::Fixed(sizes.inspector_width)),
                    Some((ResizeTarget::InspectorWidth, true)),
                );
            }
            fill_if_no_flex(&mut upper);
            push_seamed(
                &mut root,
                preset_column(upper, PaneSize::Flex),
                Some((ResizeTarget::AgentWidth, true)),
            );
        }
        LayoutPreset::Media => {
            // [Agent] | [Media] | [Preview | Inspector (55%) / Toolbar+Timeline]
            if visible(PaneId::Media) {
                push_seamed(
                    &mut root,
                    PaneNode::pane(PaneId::Media, PaneSize::Fixed(sizes.media_width)),
                    Some((ResizeTarget::AgentWidth, true)),
                );
            }
            let mut upper = Vec::new();
            if visible(PaneId::Preview) {
                upper.push(PaneNode::pane(PaneId::Preview, PaneSize::Flex));
            }
            if visible(PaneId::Inspector) {
                push_seamed(
                    &mut upper,
                    PaneNode::pane(PaneId::Inspector, PaneSize::Fixed(sizes.inspector_width)),
                    Some((ResizeTarget::InspectorWidth, true)),
                );
            }
            fill_if_no_flex(&mut upper);
            // The column's left seam drags the media column when present,
            // else the agent column.
            let seam_target = if visible(PaneId::Media) {
                ResizeTarget::MediaWidth
            } else {
                ResizeTarget::AgentWidth
            };
            push_seamed(
                &mut root,
                preset_column(upper, PaneSize::Flex),
                Some((seam_target, true)),
            );
        }
        LayoutPreset::Vertical => {
            // [Agent] | [Media | Inspector (55%) / Toolbar+Timeline] | [Preview]
            let mut upper = Vec::new();
            if visible(PaneId::Media) {
                upper.push(PaneNode::pane(
                    PaneId::Media,
                    PaneSize::Fixed(sizes.media_width),
                ));
            }
            if visible(PaneId::Inspector) {
                push_seamed(
                    &mut upper,
                    PaneNode::pane(PaneId::Inspector, PaneSize::Flex),
                    Some((ResizeTarget::MediaWidth, true)),
                );
            }
            fill_if_no_flex(&mut upper);
            let left = preset_column(upper, PaneSize::Fixed(sizes.vertical_left_width));
            let left_is_empty = matches!(&left.kind, PaneNodeKind::Column(c) if c.is_empty());
            if !left_is_empty {
                push_seamed(&mut root, left, Some((ResizeTarget::AgentWidth, true)));
            }
            if visible(PaneId::Preview) {
                let seam_target = if left_is_empty {
                    ResizeTarget::AgentWidth
                } else {
                    ResizeTarget::VerticalLeftWidth
                };
                push_seamed(
                    &mut root,
                    PaneNode::pane(PaneId::Preview, PaneSize::Flex),
                    Some((seam_target, true)),
                );
            }
        }
    }

    fill_if_no_flex(&mut root);
    PaneNode {
        kind: PaneNodeKind::Row(root),
        size: PaneSize::Flex,
    }
}

/// NSSplitView parity: when every sibling is fixed (its flex partners are
/// collapsed), the last surviving pane/container stretches to fill.
fn fill_if_no_flex(children: &mut [PaneNode]) {
    if !children.is_empty() && !children.iter().any(|c| c.size == PaneSize::Flex) {
        if let Some(last) = children.iter_mut().rev().find(|c| !c.is_divider()) {
            last.size = PaneSize::Flex;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sizes() -> ResolvedSizes {
        ResolvedSizes {
            agent_width: 240.0,
            media_width: 500.0,
            inspector_width: 260.0,
            timeline_height: 300.0,
            vertical_left_width: 600.0,
        }
    }

    fn layout_with(preset: LayoutPreset) -> PaneLayout {
        let mut l = PaneLayout::new();
        l.apply_preset(preset);
        l
    }

    /// Collect the PaneIds of a Row's Pane children (ignoring containers).
    fn row_pane_ids(node: &PaneNode) -> Vec<PaneId> {
        match &node.kind {
            PaneNodeKind::Row(children) => children
                .iter()
                .filter_map(|c| match c.kind {
                    PaneNodeKind::Pane(id) => Some(id),
                    _ => None,
                })
                .collect(),
            _ => panic!("expected Row, got {node:?}"),
        }
    }

    fn children(node: &PaneNode) -> &Vec<PaneNode> {
        match &node.kind {
            PaneNodeKind::Row(c) | PaneNodeKind::Column(c) => c,
            _ => panic!("expected container, got {node:?}"),
        }
    }

    /// Container children with dividers stripped.
    fn solids(node: &PaneNode) -> Vec<&PaneNode> {
        children(node).iter().filter(|c| !c.is_divider()).collect()
    }

    /// Divider targets among a container's direct children, in order.
    fn seam_targets(node: &PaneNode) -> Vec<ResizeTarget> {
        children(node)
            .iter()
            .filter_map(|c| match c.kind {
                PaneNodeKind::Divider { target, .. } => Some(target),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn default_tree_puts_timeline_full_width() {
        let tree = build_pane_tree(&layout_with(LayoutPreset::Default), &sizes());
        // Root: Row [Agent ⋮ Column[upper Row ⋮ TimelineRegion]]
        let root = solids(&tree);
        assert_eq!(root.len(), 2);
        assert_eq!(root[0].kind, PaneNodeKind::Pane(PaneId::Agent));
        assert_eq!(seam_targets(&tree), vec![ResizeTarget::AgentWidth]);
        let preset_col = root[1];
        assert_eq!(preset_col.size, PaneSize::Flex);
        let col = solids(preset_col);
        assert_eq!(col.len(), 2, "upper region + timeline region");
        assert_eq!(
            seam_targets(preset_col),
            vec![ResizeTarget::TimelineHeight]
        );
        // Upper: Media | Preview | Inspector, flex height
        assert_eq!(col[0].size, PaneSize::Flex);
        assert_eq!(
            row_pane_ids(col[0]),
            vec![PaneId::Media, PaneId::Preview, PaneId::Inspector]
        );
        assert_eq!(
            seam_targets(col[0]),
            vec![ResizeTarget::MediaWidth, ResizeTarget::InspectorWidth]
        );
        // Lower: TimelineRegion is a DIRECT child of the preset column →
        // spans the full preset width (below media AND inspector).
        assert_eq!(col[1].kind, PaneNodeKind::TimelineRegion);
        assert_eq!(col[1].size, PaneSize::Fixed(300.0));
        // Preview is the flex column of the upper row; sides are fixed.
        let upper = solids(col[0]);
        assert_eq!(upper[0].size, PaneSize::Fixed(500.0));
        assert_eq!(upper[1].size, PaneSize::Flex);
        assert_eq!(upper[2].size, PaneSize::Fixed(260.0));
    }

    #[test]
    fn default_tree_timeline_stays_full_width_without_side_panes() {
        let mut layout = layout_with(LayoutPreset::Default);
        layout.toggle_pane(PaneId::Media);
        layout.toggle_pane(PaneId::Inspector);
        let tree = build_pane_tree(&layout, &sizes());
        let root = solids(&tree);
        let col = solids(root[1]);
        assert_eq!(row_pane_ids(col[0]), vec![PaneId::Preview]);
        assert_eq!(seam_targets(col[0]), vec![], "no dividers around lone preview");
        assert_eq!(col[1].kind, PaneNodeKind::TimelineRegion);
    }

    #[test]
    fn media_tree_matches_swift() {
        let tree = build_pane_tree(&layout_with(LayoutPreset::Media), &sizes());
        // Root: Row [Agent ⋮ Media ⋮ Column[Row[Preview, Inspector] ⋮ TimelineRegion]]
        let root = solids(&tree);
        assert_eq!(root.len(), 3);
        assert_eq!(root[0].kind, PaneNodeKind::Pane(PaneId::Agent));
        assert_eq!(root[1].kind, PaneNodeKind::Pane(PaneId::Media));
        assert_eq!(root[1].size, PaneSize::Fixed(500.0));
        assert_eq!(
            seam_targets(&tree),
            vec![ResizeTarget::AgentWidth, ResizeTarget::MediaWidth]
        );
        let right = solids(root[2]);
        assert_eq!(right.len(), 2);
        assert_eq!(
            row_pane_ids(right[0]),
            vec![PaneId::Preview, PaneId::Inspector]
        );
        assert_eq!(seam_targets(right[0]), vec![ResizeTarget::InspectorWidth]);
        assert_eq!(right[1].kind, PaneNodeKind::TimelineRegion);
    }

    #[test]
    fn vertical_tree_matches_swift() {
        let tree = build_pane_tree(&layout_with(LayoutPreset::Vertical), &sizes());
        // Root: Row [Agent ⋮ Column[Row[Media, Inspector] ⋮ TimelineRegion] ⋮ Preview]
        let root = solids(&tree);
        assert_eq!(root.len(), 3);
        assert_eq!(root[0].kind, PaneNodeKind::Pane(PaneId::Agent));
        assert_eq!(
            seam_targets(&tree),
            vec![ResizeTarget::AgentWidth, ResizeTarget::VerticalLeftWidth]
        );
        let left = root[1];
        assert_eq!(left.size, PaneSize::Fixed(600.0));
        let left_col = solids(left);
        assert_eq!(left_col.len(), 2);
        assert_eq!(
            row_pane_ids(left_col[0]),
            vec![PaneId::Media, PaneId::Inspector]
        );
        assert_eq!(seam_targets(left_col[0]), vec![ResizeTarget::MediaWidth]);
        // Vertical upper-left: media fixed 500, inspector takes the rest.
        let upper = solids(left_col[0]);
        assert_eq!(upper[0].size, PaneSize::Fixed(500.0));
        assert_eq!(upper[1].size, PaneSize::Flex);
        assert_eq!(left_col[1].kind, PaneNodeKind::TimelineRegion);
        assert_eq!(root[2].kind, PaneNodeKind::Pane(PaneId::Preview));
        assert_eq!(root[2].size, PaneSize::Flex);
    }

    #[test]
    fn agent_is_outer_column_in_all_presets() {
        for preset in [
            LayoutPreset::Default,
            LayoutPreset::Media,
            LayoutPreset::Vertical,
        ] {
            let tree = build_pane_tree(&layout_with(preset), &sizes());
            let root = children(&tree);
            assert_eq!(
                root[0].kind,
                PaneNodeKind::Pane(PaneId::Agent),
                "agent must lead in {preset:?}"
            );
            assert_eq!(root[0].size, PaneSize::Fixed(240.0));
            // Agent must not appear anywhere deeper in the tree.
            fn count_agents(n: &PaneNode) -> usize {
                match &n.kind {
                    PaneNodeKind::Pane(PaneId::Agent) => 1,
                    PaneNodeKind::Row(c) | PaneNodeKind::Column(c) => {
                        c.iter().map(count_agents).sum()
                    }
                    _ => 0,
                }
            }
            assert_eq!(count_agents(&tree), 1);
        }
    }

    #[test]
    fn hidden_panes_are_pruned_and_lower_fills_empty_upper() {
        let mut layout = layout_with(LayoutPreset::Default);
        layout.toggle_pane(PaneId::Agent);
        let tree = build_pane_tree(&layout, &sizes());
        let root = children(&tree);
        assert_eq!(root.len(), 1, "agent hidden → only preset column");
        // Maximize timeline: everything else hidden → TimelineRegion fills.
        let mut max_layout = layout_with(LayoutPreset::Default);
        max_layout.maximize(PaneId::Timeline);
        let tree = build_pane_tree(&max_layout, &sizes());
        let root = children(&tree);
        let col = children(&root[0]);
        assert_eq!(col.len(), 1);
        assert_eq!(col[0].kind, PaneNodeKind::TimelineRegion);
        assert_eq!(col[0].size, PaneSize::Flex, "lower fills when upper empty");
    }
}
