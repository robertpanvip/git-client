//! Commit graph 泳道绘制。
//!
//! 几何对齐 IntelliJ `GraphCommitCell` / `GitRebaseCommitsTableView`：
//! - 节点直径 `GRAPH_NODE_WIDTH = 8px`（`DOT_RADIUS = 4`）；
//! - 普通提交为实心圆，**merge 提交为双节点（环形，中心挖空）**；
//! - 跨泳道连线是**直角折线 + 圆角 elbow**，不是贝塞尔 S 曲线；
//! - 单泳道内容宽 ≈22px（`LANE_WIDTH * n + DOT_RADIUS`）。

use gpui::{
    Bounds, IntoElement, ParentElement, PathBuilder, Pixels, Styled, Window, canvas, div, fill,
    point, px, size,
};
use rebased_rs::git::GraphRow;

use crate::ui::theme::{DOT_RADIUS, LANE_WIDTH, LINE_WIDTH};

pub const ROW_HEIGHT: f32 = crate::ui::theme::ROW_HEIGHT;
pub use crate::ui::theme::lane_color;

/// elbow 圆角半径；实际取值还会受泳道间距与行高约束。
const ELBOW_RADIUS: f32 = 3.0;

fn lane_x(lane: usize) -> f32 {
    LANE_WIDTH * lane as f32 + LANE_WIDTH / 2.0
}

/// 泳道列占宽（Log 表头需要与之对齐，故公开）。
pub fn graph_column_width(lane_count: usize) -> Pixels {
    // 单泳道 = LANE_WIDTH + DOT_RADIUS = 22px，与原版单列内容宽一致。
    px(LANE_WIDTH * lane_count.max(1) as f32 + DOT_RADIUS)
}

struct LaneCanvas {
    lane: usize,
    color: usize,
    is_merge: bool,
    edges: Vec<(usize, usize, usize)>,
}

impl LaneCanvas {
    fn from_row(row: &GraphRow, is_merge: bool) -> Self {
        Self {
            lane: row.lane,
            color: row.color,
            is_merge,
            edges: row
                .edges
                .iter()
                .map(|edge| (edge.x_above, edge.x_below, edge.color))
                .collect(),
        }
    }

    fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let left = bounds.origin.x.as_f32();
        let top = bounds.origin.y.as_f32();
        let height = bounds.size.height.as_f32();
        let bottom = top + height;
        let mid_y = top + height / 2.0;

        for (x_above, x_below, color) in &self.edges {
            let xa = left + lane_x(*x_above);
            let xb = left + lane_x(*x_below);
            let mut builder = PathBuilder::stroke(px(LINE_WIDTH));
            if (xa - xb).abs() < f32::EPSILON {
                builder.move_to(point(px(xa), px(top)));
                builder.line_to(point(px(xa), px(bottom)));
            } else {
                // 直角折线 + 圆角：竖直 → 圆角 → 水平 → 圆角 → 竖直。
                let dir = if xb > xa { 1.0 } else { -1.0 };
                let r = ELBOW_RADIUS
                    .min((xb - xa).abs() / 2.0)
                    .min(height / 4.0)
                    .max(0.0);
                builder.move_to(point(px(xa), px(top)));
                builder.line_to(point(px(xa), px(mid_y - r)));
                builder.curve_to(point(px(xa + dir * r), px(mid_y)), point(px(xa), px(mid_y)));
                builder.line_to(point(px(xb - dir * r), px(mid_y)));
                builder.curve_to(point(px(xb), px(mid_y + r)), point(px(xb), px(mid_y)));
                builder.line_to(point(px(xb), px(bottom)));
            }
            if let Ok(path) = builder.build() {
                window.paint_path(path, lane_color(*color));
            }
        }

        let x = left + lane_x(self.lane);
        window.paint_quad(
            fill(
                Bounds::new(
                    point(px(x - DOT_RADIUS), px(mid_y - DOT_RADIUS)),
                    size(px(DOT_RADIUS * 2.0), px(DOT_RADIUS * 2.0)),
                ),
                lane_color(self.color),
            )
            .corner_radii(px(DOT_RADIUS)),
        );
        if self.is_merge {
            // 双节点：中心挖空，与原版 merge 提交呈现一致。
            let inner = DOT_RADIUS * 0.5;
            window.paint_quad(
                fill(
                    Bounds::new(
                        point(px(x - inner), px(mid_y - inner)),
                        size(px(inner * 2.0), px(inner * 2.0)),
                    ),
                    crate::ui::theme::bg_main(),
                )
                .corner_radii(px(inner)),
            );
        }
    }
}

/// 单个提交行的泳道画布。`is_merge` 决定节点是实心还是双节点。
pub fn lane_canvas(row: Option<GraphRow>, lane_count: usize, is_merge: bool) -> impl IntoElement {
    div()
        .w(graph_column_width(lane_count))
        .h_full()
        .flex_none()
        .child(canvas(
            move |_, _, _| row,
            move |bounds, row, window, _| {
                if let Some(row) = row.as_ref() {
                    LaneCanvas::from_row(row, is_merge).paint(bounds, window);
                }
            },
        ))
}
