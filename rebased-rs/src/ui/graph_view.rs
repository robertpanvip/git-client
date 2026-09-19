//! Commit graph 泳道绘制。
//!
//! 几何模型（唯一基准，所有圆点与连线共用，禁止 magic number 平移）：
//! - lane center：`x = lane * LANE_WIDTH + LANE_WIDTH / 2`；
//! - row center：`y = bounds.top + bounds.height / 2`（泳道画布撑满
//!   `ROW_HEIGHT` 的行容器，即圆点恒在行的垂直中心；canvas 自身必须
//!   `.size_full()`，否则 bounds 高度为 0、row center 退化为行顶）；
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

use crate::ui::theme::{
    DOT_RADIUS, FIRST_LANE_CENTER, HEAD_RING_RADIUS, HEAD_RING_WIDTH, LANE_WIDTH, LINE_WIDTH,
};

pub const ROW_HEIGHT: f32 = crate::ui::theme::ROW_HEIGHT;
pub use crate::ui::theme::lane_color;

/// elbow 圆角半径；实际取值还会受泳道间距与行高约束。
/// IDEA 转折半径约 `ROW_HEIGHT / 5`（24px 行 ≈ 5px），比 3px 更平滑自然。
const ELBOW_RADIUS: f32 = 5.0;

/// lane center x（相对 graph 列左缘）。首泳道圆心固定在
/// `FIRST_LANE_CENTER`（原版实测 15px），后续泳道按 `LANE_WIDTH` 递进。
fn lane_x(lane: usize) -> f32 {
    FIRST_LANE_CENTER + LANE_WIDTH * lane as f32
}

/// 泳道列占宽（Log 行内布局需要与之对齐，故公开）。
///
/// 列宽 = 首泳道圆心 + 最后一条泳道的圆点直径 + 少量右余量，
/// 保证最后一个 lane 的圆点/连线完整落在列内。
pub fn graph_column_width(lane_count: usize) -> Pixels {
    px(FIRST_LANE_CENTER + DOT_RADIUS + (LANE_WIDTH * (lane_count.max(1) - 1) as f32) + 4.0)
}

struct LaneCanvas {
    lane: usize,
    color: usize,
    is_merge: bool,
    is_head: bool,
    edges: Vec<(usize, usize, usize)>,
}

impl LaneCanvas {
    fn from_row(row: &GraphRow, is_merge: bool, is_head: bool) -> Self {
        Self {
            lane: row.lane,
            color: row.color,
            is_merge,
            is_head,
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
        // HEAD 提交为环形节点（外径 12px / 环厚 2px + 中心小圆点），
        // 其余提交为实心圆；merge 提交在实心圆上再挖空中心。
        let x = left + lane_x(self.lane);
        let color = lane_color(self.color);
        if self.is_head {
            window.paint_quad(
                fill(
                    Bounds::new(
                        point(px(x - HEAD_RING_RADIUS), px(mid_y - HEAD_RING_RADIUS)),
                        size(px(HEAD_RING_RADIUS * 2.0), px(HEAD_RING_RADIUS * 2.0)),
                    ),
                    color,
                )
                .corner_radii(px(HEAD_RING_RADIUS)),
            );
            let hole = HEAD_RING_RADIUS - HEAD_RING_WIDTH;
            window.paint_quad(
                fill(
                    Bounds::new(
                        point(px(x - hole), px(mid_y - hole)),
                        size(px(hole * 2.0), px(hole * 2.0)),
                    ),
                    bg,
                )
                .corner_radii(px(hole)),
            );
            let inner = HEAD_RING_WIDTH + 0.5;
            window.paint_quad(
                fill(
                    Bounds::new(
                        point(px(x - inner), px(mid_y - inner)),
                        size(px(inner * 2.0), px(inner * 2.0)),
                    ),
                    color,
                )
                .corner_radii(px(inner)),
            );
        } else {
            window.paint_quad(
                fill(
                    Bounds::new(
                        point(px(x - DOT_RADIUS), px(mid_y - DOT_RADIUS)),
                        size(px(DOT_RADIUS * 2.0), px(DOT_RADIUS * 2.0)),
                    ),
                    color,
                )
                .corner_radii(px(DOT_RADIUS)),
            );
        }
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

/// 单个提交行的泳道画布。`is_merge` 决定节点是否挖空中心（merge 双节点）；
/// `is_head` 决定节点是实心圆还是 HEAD 环形节点；`bg` 是所在列表的背景色
/// （Log = `theme::log_list_bg()`），用于挖空部分。
pub fn lane_canvas(
    row: Option<GraphRow>,
    lane_count: usize,
    is_merge: bool,
    is_head: bool,
    bg: Hsla,
) -> impl IntoElement {
    div()
        .w(graph_column_width(lane_count))
        .h(px(ROW_HEIGHT))
        .flex_none()
        .child(
            canvas(
                move |_, _, _| (row, bg),
                move |bounds, (row, bg), window, _| {
                    if let Some(row) = row.as_ref() {
                        LaneCanvas::from_row(row, is_merge, is_head).paint(bg, bounds, window);
                    }
                },
            )
            // canvas 默认 style 尺寸为 0，bounds 高度为 0 会让 row center
            // 退化为行顶，圆点上半被行容器 `overflow_hidden` 裁掉；
            // 必须显式撑满父容器。
            .size_full(),
        )
}
