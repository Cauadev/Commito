//! Deterministic lane assignment for the commit DAG.
//!
//! Input: commits in display order (children before parents, as produced by
//! `git log --date-order`), each with its full parent list. Output: for every row the
//! lane (column) of the commit node, its colour and the line segments crossing the row.
//!
//! Segments are expressed in row-local coordinates so the UI can draw each virtualised
//! row independently and still get perfectly continuous lines:
//! * `Full`   – straight vertical line in lane `from` (top → bottom).
//! * `Top`    – from lane `from` at the top edge to lane `to` at the row centre.
//! * `Bottom` – from lane `from` at the row centre to lane `to` at the bottom edge.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    Full = 0,
    Top = 1,
    Bottom = 2,
}

/// Compact edge: `[kind, from, to, color, dashed]` (serialised as a JSON array).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct Edge(pub u8, pub u16, pub u16, pub u16, pub u8);

impl Edge {
    fn new(kind: EdgeKind, from: usize, to: usize, color: u16, dashed: bool) -> Self {
        Edge(kind as u8, from as u16, to as u16, color, dashed as u8)
    }
    pub fn kind(&self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RowLayout {
    pub lane: u16,
    pub color: u16,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Debug)]
struct Slot {
    hash: String,
    color: u16,
    dashed: bool,
}

pub struct GraphNode<'a> {
    pub hash: &'a str,
    pub parents: &'a [String],
    /// Virtual nodes (WIP, stash) draw dashed connections to their parent.
    pub virtual_node: bool,
}

pub struct LayoutResult {
    pub rows: Vec<RowLayout>,
    pub max_lanes: usize,
}

pub fn layout(nodes: &[GraphNode<'_>]) -> LayoutResult {
    let mut lanes: Vec<Option<Slot>> = Vec::new();
    let mut next_color: u16 = 0;
    let mut rows = Vec::with_capacity(nodes.len());
    let mut max_lanes = 0usize;

    for node in nodes {
        let matching: Vec<usize> = lanes
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().filter(|s| s.hash == node.hash).map(|_| i))
            .collect();

        let (lane, color) = match matching.first() {
            Some(&i) => (i, lanes[i].as_ref().map(|s| s.color).unwrap_or(0)),
            None => {
                let i = first_free(&lanes, 0, &[]);
                let c = next_color;
                next_color = next_color.wrapping_add(1);
                ensure_len(&mut lanes, i + 1);
                (i, c)
            }
        };

        let mut edges = Vec::new();
        for (i, slot) in lanes.iter().enumerate() {
            if let Some(s) = slot {
                if s.hash == node.hash {
                    edges.push(Edge::new(EdgeKind::Top, i, lane, s.color, s.dashed));
                } else {
                    edges.push(Edge::new(EdgeKind::Full, i, i, s.color, s.dashed));
                }
            }
        }

        // Lanes that converged into this commit are released. They may not be reused by
        // merge parents of this very row (it would draw an X), except the commit's own lane.
        let mut blocked: Vec<usize> = Vec::new();
        for &i in &matching {
            lanes[i] = None;
            if i != lane {
                blocked.push(i);
            }
        }
        lanes[lane] = None;

        for (k, parent) in node.parents.iter().enumerate() {
            let existing = lanes.iter().position(|s| s.as_ref().is_some_and(|s| s.hash == *parent));
            match existing {
                Some(j) => {
                    let c = if k == 0 { color } else { lanes[j].as_ref().map(|s| s.color).unwrap_or(color) };
                    edges.push(Edge::new(EdgeKind::Bottom, lane, j, c, node.virtual_node));
                }
                None if k == 0 => {
                    lanes[lane] = Some(Slot { hash: parent.clone(), color, dashed: node.virtual_node });
                    edges.push(Edge::new(EdgeKind::Bottom, lane, lane, color, node.virtual_node));
                }
                None => {
                    let j = first_free(&lanes, lane + 1, &blocked);
                    ensure_len(&mut lanes, j + 1);
                    let c = next_color;
                    next_color = next_color.wrapping_add(1);
                    lanes[j] = Some(Slot { hash: parent.clone(), color: c, dashed: node.virtual_node });
                    edges.push(Edge::new(EdgeKind::Bottom, lane, j, c, node.virtual_node));
                }
            }
        }

        while matches!(lanes.last(), Some(None)) {
            lanes.pop();
        }
        let width = edges.iter().map(|e| e.1.max(e.2) as usize + 1).max().unwrap_or(0).max(lane + 1);
        max_lanes = max_lanes.max(width);
        rows.push(RowLayout { lane: lane as u16, color, edges });
    }

    LayoutResult { rows, max_lanes }
}

fn first_free(lanes: &[Option<Slot>], from: usize, blocked: &[usize]) -> usize {
    (from..lanes.len())
        .find(|i| lanes[*i].is_none() && !blocked.contains(i))
        .unwrap_or_else(|| lanes.len().max(from))
}

fn ensure_len(lanes: &mut Vec<Option<Slot>>, len: usize) {
    while lanes.len() < len {
        lanes.push(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(spec: &[(&str, &[&str])]) -> LayoutResult {
        let owned: Vec<(String, Vec<String>)> =
            spec.iter().map(|(h, p)| (h.to_string(), p.iter().map(|s| s.to_string()).collect())).collect();
        let nodes: Vec<GraphNode> =
            owned.iter().map(|(h, p)| GraphNode { hash: h, parents: p, virtual_node: false }).collect();
        layout(&nodes)
    }

    #[test]
    fn linear_history_stays_in_one_lane() {
        let r = run(&[("c", &["b"]), ("b", &["a"]), ("a", &[])]);
        assert!(r.rows.iter().all(|row| row.lane == 0));
        assert_eq!(r.max_lanes, 1);
        assert_eq!(r.rows[0].edges, vec![Edge(2, 0, 0, 0, 0)]);
        assert_eq!(r.rows[1].edges, vec![Edge(1, 0, 0, 0, 0), Edge(2, 0, 0, 0, 0)]);
        assert_eq!(r.rows[2].edges, vec![Edge(1, 0, 0, 0, 0)]);
    }

    #[test]
    fn merge_opens_and_closes_second_lane() {
        // m merges feature (f2 -> f1) into main (b), both from a.
        let r = run(&[("m", &["b", "f2"]), ("f2", &["f1"]), ("b", &["a"]), ("f1", &["a"]), ("a", &[])]);
        let lanes: Vec<u16> = r.rows.iter().map(|x| x.lane).collect();
        assert_eq!(lanes, vec![0, 1, 0, 1, 0]);
        assert_eq!(r.max_lanes, 2);
        // merge commit emits two bottom edges
        let bottoms: Vec<_> = r.rows[0].edges.iter().filter(|e| e.kind() == 2).collect();
        assert_eq!(bottoms.len(), 2);
        // f1 joins the lane already waiting for `a`, so the root only receives lane 0
        assert!(r.rows[3].edges.contains(&Edge(2, 1, 0, r.rows[3].color, 0)));
        let tops: Vec<_> = r.rows[4].edges.iter().filter(|e| e.kind() == 1).collect();
        assert_eq!(tops.len(), 1);
        assert_eq!(tops[0].2, 0);
        // colors differ between the two branches
        assert_ne!(r.rows[0].color, r.rows[1].color);
    }

    #[test]
    fn divergent_tips_get_separate_lanes() {
        let r = run(&[("x", &["a"]), ("y", &["a"]), ("a", &[])]);
        assert_eq!(r.rows[0].lane, 0);
        assert_eq!(r.rows[1].lane, 1);
        assert_eq!(r.rows[2].lane, 0);
        // x's lane passes through row y
        assert!(r.rows[1].edges.contains(&Edge(0, 0, 0, 0, 0)));
    }

    #[test]
    fn octopus_merge_allocates_lanes_to_the_right() {
        let r = run(&[("m", &["a", "b", "c"]), ("c", &["r"]), ("b", &["r"]), ("a", &["r"]), ("r", &[])]);
        let bottoms: Vec<u16> = r.rows[0].edges.iter().filter(|e| e.kind() == 2).map(|e| e.2).collect();
        assert_eq!(bottoms, vec![0, 1, 2]);
        assert_eq!(r.max_lanes, 3);
    }

    #[test]
    fn shared_parent_reuses_waiting_lane() {
        // Two children whose first parent is the same commit: second child connects into existing lane.
        let r = run(&[("x", &["a"]), ("y", &["a"]), ("a", &[])]);
        let y_bottom: Vec<_> = r.rows[1].edges.iter().filter(|e| e.kind() == 2).collect();
        assert_eq!(y_bottom.len(), 1);
        assert_eq!((y_bottom[0].1, y_bottom[0].2), (1, 0));
    }

    #[test]
    fn layout_is_deterministic() {
        let spec: &[(&str, &[&str])] = &[("m", &["b", "f"]), ("f", &["a"]), ("b", &["a"]), ("a", &[])];
        assert_eq!(run(spec).rows, run(spec).rows);
    }

    #[test]
    fn virtual_nodes_are_dashed() {
        let parents = vec!["a".to_string()];
        let nodes = vec![
            GraphNode { hash: "WIP", parents: &parents, virtual_node: true },
            GraphNode { hash: "a", parents: &[], virtual_node: false },
        ];
        let r = layout(&nodes);
        assert_eq!(r.rows[0].edges, vec![Edge(2, 0, 0, 0, 1)]);
        assert_eq!(r.rows[1].edges, vec![Edge(1, 0, 0, 0, 1)]);
    }
}
