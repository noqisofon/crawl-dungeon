//! ダンジョンのデータ構造。

/// グリッド座標。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Wall,
    Floor,
    Door,
    StairsUp,
    StairsDown,
}

impl Tile {
    /// 歩行可能か（壁以外）。
    pub fn is_walkable(self) -> bool {
        self != Tile::Wall
    }
}

/// 罠（構造のみ。生成は未実装）。Floor / Door に付随するレイヤーとして持つ。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trap {
    Spike,
    Pit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomKind {
    Normal,
    /// 上り階段のある部屋。
    Start,
    /// B5F の魔導書の部屋。
    Goal,
}

/// 部屋。`(x, y)` から `w × h` の内部（床）領域。壁は外側の1マス。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Room {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub kind: RoomKind,
}

impl Room {
    pub fn center(&self) -> Pos {
        Pos::new(self.x + self.w / 2, self.y + self.h / 2)
    }

    pub fn contains(&self, p: Pos) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }

    /// 内部領域どうしの間隔が `gap` 未満なら true。
    pub fn too_close(&self, other: &Room, gap: i32) -> bool {
        self.x < other.x + other.w + gap
            && other.x < self.x + self.w + gap
            && self.y < other.y + other.h + gap
            && other.y < self.y + self.h + gap
    }

    pub fn cells(&self) -> impl Iterator<Item = Pos> + '_ {
        (self.y..self.y + self.h).flat_map(move |y| (self.x..self.x + self.w).map(move |x| Pos::new(x, y)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    MagicBook,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemPlacement {
    pub pos: Pos,
    pub kind: ItemKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Floor {
    /// 階番号（B1F = 1）。
    pub level: usize,
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<Tile>,
    /// 罠レイヤー（`tiles` と同じ添字）。
    pub traps: Vec<Option<Trap>>,
    pub rooms: Vec<Room>,
    pub stairs_up: Pos,
    pub stairs_down: Option<Pos>,
    pub items: Vec<ItemPlacement>,
}

impl Floor {
    pub fn in_bounds(&self, p: Pos) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.width && p.y < self.height
    }

    pub fn index(&self, p: Pos) -> usize {
        (p.y * self.width + p.x) as usize
    }

    pub fn tile(&self, p: Pos) -> Option<Tile> {
        self.in_bounds(p).then(|| self.tiles[self.index(p)])
    }

    pub fn trap(&self, p: Pos) -> Option<Trap> {
        if self.in_bounds(p) { self.traps[self.index(p)] } else { None }
    }

    pub fn is_walkable(&self, p: Pos) -> bool {
        self.tile(p).is_some_and(Tile::is_walkable)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dungeon {
    pub seed: u64,
    pub floors: Vec<Floor>,
}
