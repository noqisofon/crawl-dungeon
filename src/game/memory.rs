//! マップ記憶（階ごと）。

use std::collections::VecDeque;

use crate::dungeon::{Floor, Pos, Tile};

use super::types::Dir;

#[derive(Clone, Debug)]
pub struct FloorMemory {
    width: i32,
    height: i32,
    known: Vec<Option<Tile>>,
    visited: Vec<bool>,
}

impl FloorMemory {
    pub fn new(floor: &Floor) -> Self {
        let n = floor.tiles.len();
        Self { width: floor.width, height: floor.height, known: vec![None; n], visited: vec![false; n] }
    }

    fn in_bounds(&self, p: Pos) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.width && p.y < self.height
    }

    fn index(&self, p: Pos) -> usize {
        (p.y * self.width + p.x) as usize
    }

    pub fn known(&self, p: Pos) -> Option<Tile> {
        if self.in_bounds(p) { self.known[self.index(p)] } else { None }
    }

    pub fn visited(&self, p: Pos) -> bool {
        self.in_bounds(p) && self.visited[self.index(p)]
    }

    fn learn(&mut self, floor: &Floor, p: Pos) {
        if let Some(t) = floor.tile(p) {
            let i = self.index(p);
            self.known[i] = Some(t);
        }
    }

    /// `pos` に着いたときの記憶更新。
    /// - 現在地を既知・訪問済みにする
    /// - 4方向に、壁または扉に当たる（当たったセル自身は既知にする）まで既知にする
    /// - 部屋の中なら、部屋全体と外周の壁・扉を既知にする
    pub fn arrive(&mut self, floor: &Floor, pos: Pos) {
        self.learn(floor, pos);
        let i = self.index(pos);
        self.visited[i] = true;
        for d in Dir::ALL {
            let mut p = d.step(pos);
            while floor.in_bounds(p) {
                self.learn(floor, p);
                if matches!(floor.tile(p), Some(Tile::Wall | Tile::Door)) {
                    break;
                }
                p = d.step(p);
            }
        }
        if let Some(room) = floor.rooms.iter().find(|r| r.contains(pos)) {
            for y in room.y - 1..=room.y + room.h {
                for x in room.x - 1..=room.x + room.w {
                    self.learn(floor, Pos::new(x, y));
                }
            }
        }
    }

    /// 既知の歩行可能セルのうち、未知のセルと隣り合うもの（行優先の順）。
    pub fn frontier(&self) -> Vec<Pos> {
        let mut out = Vec::new();
        for y in 0..self.height {
            for x in 0..self.width {
                let p = Pos::new(x, y);
                if !self.known(p).is_some_and(Tile::is_walkable) {
                    continue;
                }
                let touches_unknown =
                    Dir::ALL.into_iter().any(|d| {
                        let q = d.step(p);
                        self.in_bounds(q) && self.known(q).is_none()
                    });
                if touches_unknown {
                    out.push(p);
                }
            }
        }
        out
    }

    fn known_walkable(&self, p: Pos) -> bool {
        self.known(p).is_some_and(Tile::is_walkable)
    }

    /// 既知マップだけを使った最短経路（向きのリスト）。経路がなければ `None`。
    /// 未知セルは壁として扱う。フロアを複製せず、記憶の上で直接 BFS する。
    pub fn path(&self, from: Pos, to: Pos) -> Option<Vec<Dir>> {
        if !self.known_walkable(from) || !self.known_walkable(to) {
            return None;
        }
        let n = self.known.len();
        let (start, goal) = (self.index(from), self.index(to));
        // prev[i] = 直前セルの添字と、そこからの向き。
        let mut prev: Vec<Option<(usize, Dir)>> = vec![None; n];
        let mut seen = vec![false; n];
        seen[start] = true;
        let mut queue = VecDeque::from([from]);
        while let Some(cur) = queue.pop_front() {
            let ci = self.index(cur);
            if ci == goal {
                break;
            }
            for d in Dir::ALL {
                let next = d.step(cur);
                if !self.known_walkable(next) {
                    continue;
                }
                let ni = self.index(next);
                if !seen[ni] {
                    seen[ni] = true;
                    prev[ni] = Some((ci, d));
                    queue.push_back(next);
                }
            }
        }
        if !seen[goal] {
            return None;
        }
        let mut dirs = Vec::new();
        let mut i = goal;
        while let Some((p, d)) = prev[i] {
            dirs.push(d);
            i = p;
        }
        dirs.reverse();
        Some(dirs)
    }
}
