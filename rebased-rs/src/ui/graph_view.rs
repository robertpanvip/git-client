use gpui::{
    canvas, div, fill, hsla, point, px, size, Bounds, Hsla, IntoElement, ParentElement, PathBuilder,
    Pixels, Styled, Window,
};
use rebased_rs::git::{ChangeStatus, GraphRow, MAX_COLORS};

pub const ROW_HEIGHT: f32 = 40.0;
const LANE_WIDTH: f32 = 16.0;
const DOT_RADIUS: f32 = 3.5;
const LINE_WIDTH: f32 = 2.0;

const HUES: [f32; MAX_COLORS] = [0.58, 0.0, 0.33, 0.83, 0.13, 0.45, 0.65, 0.95];

pub fn lane_color(index: usize) -> Hsla {
    hsla(HUES[index % MAX_COLORS], 0.65, 0.55, 1.0)
}

pub fn status_color(status: &ChangeStatus) -> Hsla {
    match status {
        ChangeStatus::Added => lane_color(2),
        ChangeStatus::Modified => lane_color(0),
        ChangeStatus::Deleted => lane_color(1),
        ChangeStatus::Renamed => lane_color(4),
        ChangeStatus::Copied => lane_color(5),
        ChangeStatus::TypeChanged => lane_color(7),
        ChangeStatus::Conflicted => lane_color(3),
        ChangeStatus::Untracked => lane_color(6),
    }
}

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
