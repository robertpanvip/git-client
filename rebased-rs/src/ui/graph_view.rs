use gpui::{
    canvas, div, fill, point, px, size, Bounds, IntoElement, ParentElement, PathBuilder, Pixels,
    Styled, Window,
};
use rebased_rs::git::GraphRow;

use crate::ui::theme::{DOT_RADIUS, LANE_WIDTH, LINE_WIDTH};

pub const ROW_HEIGHT: f32 = crate::ui::theme::ROW_HEIGHT;
pub use crate::ui::theme::{lane_color, status_color};

fn lane_x(lane: usize) -> f32 {
    LANE_WIDTH * lane as f32 + LANE_WIDTH / 2.0
}

fn graph_width(lane_count: usize) -> Pixels {
    px(LANE_WIDTH * lane_count.max(1) as f32 + DOT_RADIUS * 2.0)
}

struct LaneCanvas {
    lane: usize,
    color: usize,
    edges: Vec<(usize, usize, usize)>,
}

impl LaneCanvas {
    fn from_row(row: &GraphRow) -> Self {
        Self {
            lane: row.lane,
            color: row.color,
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
        let bottom = top + bounds.size.height.as_f32();
        let mid_y = top + bounds.size.height.as_f32() / 2.0;
        let x = left + lane_x(self.lane);

        for (x_above, x_below, color) in &self.edges {
            let x_above = left + lane_x(*x_above);
            let x_below = left + lane_x(*x_below);
            let mut builder = PathBuilder::stroke(px(LINE_WIDTH));
            if x_above == x_below {
                builder.move_to(point(px(x_above), px(top)));
                builder.line_to(point(px(x_below), px(bottom)));
            } else {
                let mid = (top + bottom) / 2.0;
                builder.move_to(point(px(x_above), px(top)));
                builder.curve_to(
                    point(px((x_above + x) / 2.0), px(mid)),
                    point(px(x_above), px(mid)),
                );
                builder.curve_to(
                    point(px(x_below), px(bottom)),
                    point(px((x + x_below) / 2.0), px(mid)),
                );
            }
            if let Ok(path) = builder.build() {
                window.paint_path(path, lane_color(*color));
            }
        }

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
    }
}

pub fn lane_canvas(row: Option<GraphRow>, lane_count: usize) -> impl IntoElement {
    div()
        .w(graph_width(lane_count))
        .h_full()
        .flex_none()
        .child(canvas(
            move |_, _, _| row,
            |bounds, row, window, _| {
                if let Some(row) = row.as_ref() {
                    LaneCanvas::from_row(row).paint(bounds, window);
                }
            },
        ))
}
