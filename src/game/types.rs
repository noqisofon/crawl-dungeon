//! パーティ・メンバー・向き。

use crate::dungeon::{ItemKind, Pos};

use super::event::MemberSnapshot;
use super::consts::{INITIAL_PARTY, class_stats};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    North,
    East,
    South,
    West,
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::North, Dir::East, Dir::South, Dir::West];

    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::North => (0, -1),
            Dir::East => (1, 0),
            Dir::South => (0, 1),
            Dir::West => (-1, 0),
        }
    }

    fn index(self) -> usize {
        self as usize
    }

    pub fn turn_right(self) -> Dir {
        Dir::ALL[(self.index() + 1) % 4]
    }

    pub fn turn_left(self) -> Dir {
        Dir::ALL[(self.index() + 3) % 4]
    }

    /// `self` から `to` へ時計回りに何回転か（0..=3）。
    pub fn clockwise_steps_to(self, to: Dir) -> usize {
        (to.index() + 4 - self.index()) % 4
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Dir::North => "north",
            Dir::East => "east",
            Dir::South => "south",
            Dir::West => "west",
        }
    }

    pub fn from_symbol(s: &str) -> Option<Dir> {
        Dir::ALL.into_iter().find(|d| d.symbol() == s)
    }

    pub fn step(self, p: Pos) -> Pos {
        let (dx, dy) = self.delta();
        Pos::new(p.x + dx, p.y + dy)
    }

    /// 隣接する2セル間の向き。隣接していなければ `None`。
    pub fn between(from: Pos, to: Pos) -> Option<Dir> {
        Dir::ALL.into_iter().find(|d| d.step(from) == to)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Fighter,
    Thief,
    Mage,
    Priest,
}

impl Class {
    pub fn symbol(self) -> &'static str {
        match self {
            Class::Fighter => "fighter",
            Class::Thief => "thief",
            Class::Mage => "mage",
            Class::Priest => "priest",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    pub class: Class,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
}

impl Member {
    pub fn new(name: &str, class: Class) -> Self {
        let (max_hp, max_mp) = class_stats(class);
        Self { name: name.to_string(), class, hp: max_hp, max_hp, mp: max_mp, max_mp }
    }

    pub fn is_alive(&self) -> bool {
        self.hp > 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Party {
    pub pos: Pos,
    /// 階番号（B1F = 1）。
    pub floor: usize,
    pub facing: Dir,
    pub members: Vec<Member>,
    pub items: Vec<ItemKind>,
    pub turn: u32,
}

impl Party {
    /// 初期パーティ。`entrance` は B1F の上り階段。向きは南。
    pub fn new(entrance: Pos) -> Self {
        Self {
            pos: entrance,
            floor: 1,
            facing: Dir::South,
            members: INITIAL_PARTY.iter().map(|&(n, c)| Member::new(n, c)).collect(),
            items: Vec::new(),
            turn: 0,
        }
    }

    pub fn alive_count(&self) -> usize {
        self.members.iter().filter(|m| m.is_alive()).count()
    }

    /// 全員の現在HP合計 / 最大HP合計。
    ///
    /// 注意: 倒れたメンバー（復活しない）の最大HPも分母に入るので、
    /// 誰かが倒れると、休んでも 1.0 には戻らない。
    pub fn hp_ratio(&self) -> f64 {
        let max: i32 = self.members.iter().map(|m| m.max_hp).sum();
        let cur: i32 = self.members.iter().map(|m| m.hp).sum();
        if max == 0 { 0.0 } else { cur as f64 / max as f64 }
    }

    /// 生存メンバーのみの現在HP合計 / 最大HP合計。全滅時は 0.0。
    pub fn alive_hp_ratio(&self) -> f64 {
        let max: i32 = self.members.iter().filter(|m| m.is_alive()).map(|m| m.max_hp).sum();
        let cur: i32 = self.members.iter().filter(|m| m.is_alive()).map(|m| m.hp).sum();
        if max == 0 { 0.0 } else { cur as f64 / max as f64 }
    }

    /// 全メンバーの現在の HP/MP。
    pub fn snapshot(&self) -> Vec<MemberSnapshot> {
        self.members.iter().map(|m| MemberSnapshot { hp: m.hp, mp: m.mp }).collect()
    }

    pub fn has_item(&self, kind: ItemKind) -> bool {
        self.items.contains(&kind)
    }
}
