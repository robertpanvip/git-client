use super::types::{Commit, CommitId};

pub const MAX_COLORS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowEdge {
    pub x_above: usize,
    pub x_below: usize,
    pub color: usize,
}

#[derive(Debug, Clone)]
pub struct GraphRow {
    pub commit: CommitId,
    pub lane: usize,
    pub color: usize,
    pub edges: Vec<RowEdge>,
}

#[derive(Debug, Clone, Default)]
pub struct Graph {
    pub rows: Vec<GraphRow>,
    pub lane_count: usize,
}

impl Graph {
    pub fn row(&self, index: usize) -> Option<&GraphRow> {
        self.rows.get(index)
    }
}

struct Lane {
    waiting_for: CommitId,
    color: usize,
}

struct LaneSet {
    lanes: Vec<Option<Lane>>,
}

impl LaneSet {
    fn new() -> Self {
        Self { lanes: Vec::new() }
    }

    fn position_of(&self, id: &CommitId) -> Option<usize> {
        self.lanes
            .iter()
            .position(|l| l.as_ref().is_some_and(|l| &l.waiting_for == id))
    }

    fn take(&mut self, index: usize) -> Option<Lane> {
        self.lanes[index].take()
    }

    fn first_free(&mut self) -> usize {
        match self.lanes.iter().position(|l| l.is_none()) {
            Some(idx) => idx,
            None => {
                self.lanes.push(None);
                self.lanes.len() - 1
            }
        }
    }

    fn place(&mut self, index: usize, waiting_for: CommitId, color: usize) {
        self.lanes[index] = Some(Lane {
            waiting_for,
            color,
        });
    }
}

fn push_edge(edges: &mut Vec<RowEdge>, edge: RowEdge) {
    if !edges.contains(&edge) {
        edges.push(edge);
    }
}

pub fn build_graph(commits: &[Commit]) -> Graph {
    let mut set = LaneSet::new();
    let mut rows = Vec::new();
    let mut next_color = 0usize;
    let mut lane_count = 0usize;

    for commit in commits {
        let (lane, color, has_incoming) = match set.position_of(&commit.id) {
            Some(idx) => {
                let taken = set.take(idx).expect("lane checked");
                (idx, taken.color, true)
            }
            None => {
                let idx = set.first_free();
                let c = next_color;
                next_color = (next_color + 1) % MAX_COLORS;
                (idx, c, false)
            }
        };

        let mut edges: Vec<RowEdge> = Vec::with_capacity(set.lanes.len() + commit.parents.len());
        let mut incoming: Vec<(usize, usize)> = Vec::new();
        if has_incoming {
            incoming.push((lane, color));
        }

        for (idx, slot) in set.lanes.iter().enumerate() {
            if let Some(l) = slot {
                if l.waiting_for == commit.id {
                    incoming.push((idx, l.color));
                } else {
                    edges.push(RowEdge {
                        x_above: idx,
                        x_below: idx,
                        color: l.color,
                    });
                }
            }
        }

        for (idx, c) in &incoming {
            push_edge(
                &mut edges,
                RowEdge {
                    x_above: *idx,
                    x_below: lane,
                    color: *c,
                },
            );
        }
        for (idx, _) in &incoming {
            if *idx != lane {
                set.lanes[*idx] = None;
            }
        }

        for (pi, parent) in commit.parents.iter().enumerate() {
            if let Some(existing) = set.position_of(parent) {
                edges.push(RowEdge {
                    x_above: lane,
                    x_below: existing,
                    color,
                });
                continue;
            }
            let target = if pi == 0 && set.lanes.get(lane).is_none_or(|s| s.is_none()) {
                lane
            } else {
                set.first_free()
            };
            let c = if pi == 0 {
                color
            } else {
                let c = next_color;
                next_color = (next_color + 1) % MAX_COLORS;
                c
            };
            set.place(target, parent.clone(), c);
            push_edge(
                &mut edges,
                RowEdge {
                    x_above: lane,
                    x_below: target,
                    color: c,
                },
            );
        }

        lane_count = lane_count.max(set.lanes.len());

        rows.push(GraphRow {
            commit: commit.id.clone(),
            lane,
            color,
            edges,
        });
    }

    Graph { rows, lane_count }
}

#[cfg(test)]
mod tests {
    use super::super::types::Author;
    use super::*;

    fn commit(id: &str, parents: &[&str]) -> Commit {
        Commit {
            id: CommitId(id.to_string()),
            parents: parents.iter().map(|p| CommitId(p.to_string())).collect(),
            author: Author {
                name: "T".to_string(),
                email: "t@t".to_string(),
            },
            time: 0,
            subject: id.to_string(),
            body: String::new(),
            refs: Vec::new(),
        }
    }

    #[test]
    fn linear_history_single_lane() {
        let commits = vec![commit("C", &["B"]), commit("B", &["A"]), commit("A", &[])];
        let graph = build_graph(&commits);
        assert_eq!(graph.lane_count, 1);
        for row in &graph.rows {
            assert_eq!(row.lane, 0);
            assert_eq!(row.edges.len(), 1);
            assert_eq!(row.edges[0].x_above, 0);
            assert_eq!(row.edges[0].x_below, 0);
        }
    }

    #[test]
    fn branch_fork_creates_merge_edge() {
        let e = commit("E", &["C"]);
        let d = commit("D", &["C"]);
        let c = commit("C", &["B"]);
        let b = commit("B", &["A"]);
        let a = commit("A", &[]);
        let graph = build_graph(&[e, d, c, b, a]);

        assert_eq!(graph.lane_count, 2);

        let row_e = graph.row(0).expect("row E");
        assert_eq!(row_e.lane, 0);
        assert_eq!(row_e.edges.len(), 1);
        assert_eq!(row_e.edges[0].x_above, 0);
        assert_eq!(row_e.edges[0].x_below, 0);

        let row_d = graph.row(1).expect("row D");
        assert_eq!(row_d.lane, 1);
        assert_eq!(row_d.edges.len(), 2);
        let pass_through = row_d
            .edges
            .iter()
            .find(|e| e.x_above == 0 && e.x_below == 0)
            .expect("C lane passes through row D");
        assert_eq!(pass_through.color, row_e.edges[0].color);
        let fork = row_d
            .edges
            .iter()
            .find(|e| e.x_above == 1 && e.x_below == 0)
            .expect("D forks toward C lane");
        assert_eq!(fork.color, row_d.color);

        let row_c = graph.row(2).expect("row C");
        assert_eq!(row_c.lane, 0);
        assert_eq!(row_c.edges[0].x_above, 0);
        assert_eq!(row_c.edges[0].x_below, 0);
    }

    #[test]
    fn merge_commit_converges_lanes() {
        let m = commit("M", &["B", "C"]);
        let b = commit("B", &["A"]);
        let c = commit("C", &["A"]);
        let a = commit("A", &[]);
        let graph = build_graph(&[m, b, c, a]);

        assert_eq!(graph.lane_count, 2);

        let row_m = graph.row(0).expect("row M");
        assert_eq!(row_m.lane, 0);
        assert_eq!(row_m.edges.len(), 2);
        assert_eq!(row_m.edges[0].x_above, 0);
        assert_eq!(row_m.edges[0].x_below, 0);
        assert_eq!(row_m.edges[1].x_above, 0);
        assert_eq!(row_m.edges[1].x_below, 1);

        let row_b = graph.row(1).expect("row B");
        let pass_through = row_b
            .edges
            .iter()
            .find(|e| e.x_above == 1 && e.x_below == 1)
            .expect("C lane passes through row B");
        assert_eq!(pass_through.x_above, 1);

        let row_c = graph.row(2).expect("row C");
        assert_eq!(row_c.lane, 1);
        let merge_edge = row_c
            .edges
            .iter()
            .find(|e| e.x_above == 1 && e.x_below == 0)
            .expect("merge edge into lane 0");
        assert_eq!(merge_edge.x_below, 0);

        let row_a = graph.row(3).expect("row A");
        assert_eq!(row_a.lane, 0);
        assert_eq!(row_a.edges.len(), 1);
        assert_eq!(row_a.edges[0].x_above, 0);
        assert_eq!(row_a.edges[0].x_below, 0);
    }

    #[test]
    fn parallel_branches_keep_lanes_stable() {
        let f = commit("F", &["D"]);
        let g = commit("G", &["E"]);
        let d = commit("D", &["B"]);
        let e = commit("E", &["B"]);
        let b = commit("B", &[]);
        let graph = build_graph(&[f, g, d, e, b]);

        assert_eq!(graph.lane_count, 2);

        let row_f = graph.row(0).expect("row F");
        assert_eq!(row_f.lane, 0);

        let row_g = graph.row(1).expect("row G");
        assert_eq!(row_g.lane, 1);
        assert_eq!(row_g.edges.len(), 2);
        let d_pass = row_g
            .edges
            .iter()
            .find(|e| e.x_above == 0 && e.x_below == 0)
            .expect("D lane passes through row G");
        let g_own = row_g
            .edges
            .iter()
            .find(|e| e.x_above == 1 && e.x_below == 1)
            .expect("G continues toward E lane");
        assert_eq!(g_own.color, row_g.color);

        let row_d = graph.row(2).expect("row D");
        assert_eq!(row_d.lane, 0);
        let pass_through = row_d
            .edges
            .iter()
            .find(|e| e.x_above == 1 && e.x_below == 1)
            .expect("E lane passes through row D");
        assert_eq!(pass_through.color, g_own.color);
        assert_ne!(pass_through.color, d_pass.color);

        let row_e = graph.row(3).expect("row E");
        let merge_edge = row_e
            .edges
            .iter()
            .find(|e| e.x_above == 1 && e.x_below == 0)
            .expect("E merges into B lane");
        assert_eq!(merge_edge.x_below, 0);
    }

    #[test]
    fn isolated_root_has_no_edges() {
        let a = commit("A", &[]);
        let graph = build_graph(&[a]);
        assert_eq!(graph.rows.len(), 1);
        assert!(graph.rows[0].edges.is_empty());
        assert_eq!(graph.lane_count, 1);
    }

    #[test]
    fn colors_stay_within_palette() {
        let mut commits = Vec::new();
        for i in 0..(MAX_COLORS + 4) {
            let id = format!("c{i}");
            let parents: Vec<String> = if i > 0 {
                vec![format!("c{}", i - 1)]
            } else {
                Vec::new()
            };
            let refs: Vec<&str> = parents.iter().map(|p| p.as_str()).collect();
            commits.push(commit(&id, &refs));
        }
        let graph = build_graph(&commits);
        assert_eq!(graph.rows.len(), MAX_COLORS + 4);
        assert!(graph.rows.iter().all(|r| r.color < MAX_COLORS));
    }
}
