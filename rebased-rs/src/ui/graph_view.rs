//! Commit graph 泳道绘制。
//!
//! 几何模型（唯一基准，所有圆点与连线共用，禁止 magic number 平移）：
//! - lane center：`x = lane * LANE_WIDTH + LANE_WIDTH / 2`；
//! - row center：`y = bounds.top + bounds.height / 2`（泳道画布 `h_full`
//!   等于行高 `ROW_HEIGHT`，即圆点恒在行的垂直中心）；
//! - dot：圆心 = `(lane_x(lane), row_center)`，直径 `2 * DOT_RADIUS`；
//! - line：路径中心线必须经过 `lane_x(lane)`，描边以中心线对称展开；
//! - 泳道切换：`竖直 → 圆角 elbow → 水平 → 圆角 elbow → 竖直`，
//!   两个 elbow 均关于 row center 对称（水平段恰在 row center 上）。
//!
//! `LANE_WIDTH = 18`、`DOT_RADIUS = 4`：lane center 落在整数像素上，
//! 圆点两侧各留 5px，graph 列宽恰好等于 `LANE_WIDTH * lane_count`，
//! 最后一个 lane 的圆点/连线完整落在列内；Graph 与 Subject 的间距由
//! 提交行的 `gap` 独立控制，与泳道数量无关（对齐 IntelliJ
//! `GraphCommitCell`：节点直径 8px，跨泳道连线为直角折线 + 圆角
//! elbow，不是贝塞尔 S 曲线）。
//!
//! merge 提交为双节点（环形，中心挖空）：挖空色使用调用方传入的
//! 列表背景色（Log = `log_list_bg()`），不硬编码 `bg_main()`，避免
//! 与真实容器背景产生色差。

use gpui::{
    Bounds, Hsla, IntoElement, ParentElement, PathBuilder, Pixels, Styled, Window, canvas, div,
    fill, point, px, size,
};
use rebased_rs::git::GraphRow;

use crate::ui::theme::{DOT_RADIUS, LANE_WIDTH, LINE_WIDTH};

pub const ROW_HEIGHT: f32 = crate::ui::theme::ROW_HEIGHT;
pub use crate::ui::theme::lane_color;

/// elbow 圆角半径；实际取值还会受泳道间距与行高约束。
/// IDEA 转折半径约 `ROW_HEIGHT / 5`（24px 行 ≈ 5px），比 3px 更平滑自然。
const ELBOW_RADIUS: f32 = 5.0;

/// lane center x（相对 graph 列左缘）。`LANE_WIDTH = 18` 保证所有
/// lane center 落在整数像素上，圆点与连线不会产生半像素错位。
fn lane_x(lane: usize) -> f32 {
    LANE_WIDTH * lane as f32 + LANE_WIDTH / 2.0
}

/// 泳道列占宽（Log 表头需要与之对齐，故公开）。
///
/// 列宽 = `LANE_WIDTH * lane_count`：每条泳道内的圆点/线条天然居中
/// （左右各 5px 余量），不额外加 `DOT_RADIUS`，避免列右侧多出与
/// 左侧不对称的空隙。
pub fn graph_column_width(lane_count: usize) -> Pixels {
    px(LANE_WIDTH * lane_count.max(1) as f32)
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

    fn paint(&self, bg: Hsla, bounds: Bounds<Pixels>, window: &mut Window) {
        let left = bounds.origin.x.as_f32();
        let top = bounds.origin.y.as_f32();
        let height = bounds.size.height.as_f32();
        let bottom = top + height;
        // row center：所有转折的水平段与圆点圆心都在这条线上。
        let mid_y = top + height / 2.0;

        for (x_above, x_below, color) in &self.edges {
            let xa = left + lane_x(*x_above);
            let xb = left + lane_x(*x_below);
            let mut builder = PathBuilder::stroke(px(LINE_WIDTH));
            if (xa - xb).abs() < f32::EPSILON {
                builder.move_to(point(px(xa), px(top)));
                builder.line_to(point(px(xa), px(bottom)));
            } else {
                // 竖直 → 圆角 elbow → 水平 → 圆角 elbow → 竖直。
                // GPUI `curve_to(to, ctrl)` 是二次贝塞尔：终点在前、控制点
                // 在后（内部转 `quadratic_bezier_to(ctrl, to)`）；控制点取
                // 转角顶点，起止切线分别为竖直/水平，两段 elbow 关于
                // row center 严格对称。
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

        // 圆点：圆心 = (lane_x(lane), row center)，不叠加任何平移。
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
            // 双节点：中心挖空成环形。挖空色 = 所在列表的真实背景，
            // 由调用方传入，避免依赖 `bg_main()` 造成色差。
            let inner = DOT_RADIUS * 0.5;
            window.paint_quad(
                fill(
                    Bounds::new(
                        point(px(x - inner), px(mid_y - inner)),
                        size(px(inner * 2.0), px(inner * 2.0)),
                    ),
                    bg,
                )
                .corner_radii(px(inner)),
            );
        }
    }
}

/// 单个提交行的泳道画布。`is_merge` 决定节点是实心还是双节点；
/// `bg` 是所在列表的背景色（Log = `theme::log_list_bg()`），用于
/// merge 环形节点的中心挖空。
pub fn lane_canvas(
    row: Option<GraphRow>,
    lane_count: usize,
    is_merge: bool,
    bg: Hsla,
) -> impl IntoElement {
    div()
        .w(graph_column_width(lane_count))
        .h_full()
        .flex_none()
        .child(canvas(
            move |_, _, _| (row, bg),
            move |bounds, (row, bg), window, _| {
                if let Some(row) = row.as_ref() {
                    LaneCanvas::from_row(row, is_merge).paint(bg, bounds, window);
                }
            },
        ))
}
