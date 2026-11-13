//! 幅優先探索（4近傍。壁以外は歩行可能）。

use std::collections::VecDeque;

use super::types::{Floor, Pos};

const DIRS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
const NONE: usize = usize::MAX;

/// `from` からの BFS。(距離, 直前セル添字) を返す。未到達は `u32::MAX` / `NONE`。
fn bfs(floor: &Floor, from: Pos, stop: Option<Pos>) -> Option<(Vec<u32>, Vec<usize>)> {
    if !floor.is_walkable(from) {
        return None;
    }
    let n = floor.tiles.len();
    let mut dist = vec![u32::MAX; n];
    let mut prev = vec![NONE; n];
    let mut queue = VecDeque::new();
    dist[floor.index(from)] = 0;
    queue.push_back(from);
    while let Some(cur) = queue.pop_front() {
        if Some(cur) == stop {
            break;
        }
        let ci = floor.index(cur);
        for (dx, dy) in DIRS {
            let next = Pos::new(cur.x + dx, cur.y + dy);
            if !floor.is_walkable(next) {
                continue;
            }
            let ni = floor.index(next);
            if dist[ni] == u32::MAX {
                dist[ni] = dist[ci] + 1;
                prev[ni] = ci;
                queue.push_back(next);
            }
        }
    }
    Some((dist, prev))
}

/// `from` から各セルへの最短歩数。未到達・壁は `None`。
pub fn distance_map(floor: &Floor, from: Pos) -> Vec<Option<u32>> {
    match bfs(floor, from, None) {
        Some((dist, _)) => dist.into_iter().map(|d| (d != u32::MAX).then_some(d)).collect(),
        None => vec![None; floor.tiles.len()],
    }
}

/// 最短経路（`from` と `to` を含む）。到達不能なら `None`。
pub fn shortest_path(floor: &Floor, from: Pos, to: Pos) -> Option<Vec<Pos>> {
    if !floor.is_walkable(to) {
        return None;
    }
    let (dist, prev) = bfs(floor, from, Some(to))?;
    let mut i = floor.index(to);
    if dist[i] == u32::MAX {
        return None;
    }
    let mut path = Vec::with_capacity(dist[i] as usize + 1);
    loop {
        path.push(Pos::new(i as i32 % floor.width, i as i32 / floor.width));
        if prev[i] == NONE {
            break;
        }
        i = prev[i];
    }
    path.reverse();
    Some(path)
}
