//! 生成アルゴリズム本体。

use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use super::path::distance_map;
use super::types::*;

pub const FLOOR_WIDTH: i32 = 40;
pub const FLOOR_HEIGHT: i32 = 40;
pub const FLOOR_COUNT: usize = 5;
pub const MIN_ROOMS: usize = 5;
pub const MAX_ROOMS: usize = 9;
pub const MIN_ROOM_SIZE: i32 = 3;
pub const MAX_ROOM_SIZE: i32 = 7;
/// 部屋の内部領域どうしの最小間隔。
pub const ROOM_GAP: i32 = 3;
/// MST に加えて引くループ用通路の本数。
pub const EXTRA_LOOPS: usize = 2;
/// 部屋配置の試行回数（1回の生成あたり）。
pub const ROOM_PLACEMENT_TRIES: usize = 300;
/// 検証に失敗したときの再生成上限（1階あたり）。
pub const MAX_ATTEMPTS: u32 = 200;

fn splitmix64(z: u64) -> u64 {
    let mut z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// 階ごとのシード。「シード + 階番号」を素朴に足すと、隣り合うシードの階が
/// 重複するので、先にシードをハッシュしてから階番号を足す。
pub fn floor_seed(seed: u64, level: usize) -> u64 {
    splitmix64(splitmix64(seed).wrapping_add(level as u64))
}

fn attempt_seed(floor_seed: u64, attempt: u32) -> u64 {
    splitmix64(floor_seed.wrapping_add(attempt as u64))
}

/// 5階分を B1F から順に生成する。同じシードなら同じ結果。
///
/// 階どうしは階段座標でのみ依存する（B(n) の下り階段 = B(n+1) の上り階段）。
///
/// # Panics
/// `MAX_ATTEMPTS` 回再生成しても検証を通らない場合（実用上起きない）。
pub fn generate_dungeon(seed: u64) -> Dungeon {
    let mut floors: Vec<Floor> = Vec::with_capacity(FLOOR_COUNT);
    let mut forced_up = None;
    for level in 1..=FLOOR_COUNT {
        let floor = generate_floor(seed, level, forced_up);
        forced_up = floor.stairs_down;
        floors.push(floor);
    }
    Dungeon { seed, floors }
}

/// 1階分を生成する。`forced_up` を指定すると、上り階段をその座標にする。
pub fn generate_floor(seed: u64, level: usize, forced_up: Option<Pos>) -> Floor {
    let base = floor_seed(seed, level);
    for attempt in 0..MAX_ATTEMPTS {
        let mut rng = ChaCha8Rng::seed_from_u64(attempt_seed(base, attempt));
        if let Some(floor) = try_generate_floor(&mut rng, level, forced_up) {
            if validate(&floor) {
                return floor;
            }
        }
    }
    panic!("failed to generate floor {level} for seed {seed} in {MAX_ATTEMPTS} attempts");
}

fn random_cell(rng: &mut impl Rng, room: &Room) -> Pos {
    Pos::new(rng.random_range(room.x..room.x + room.w), rng.random_range(room.y..room.y + room.h))
}

fn try_generate_floor(rng: &mut ChaCha8Rng, level: usize, forced_up: Option<Pos>) -> Option<Floor> {
    let mut rooms = place_rooms(rng, forced_up)?;
    let n = (FLOOR_WIDTH * FLOOR_HEIGHT) as usize;
    let mut floor = Floor {
        level,
        width: FLOOR_WIDTH,
        height: FLOOR_HEIGHT,
        tiles: vec![Tile::Wall; n],
        traps: vec![None; n],
        stairs_up: Pos::new(0, 0),
        stairs_down: None,
        items: Vec::new(),
        rooms: Vec::new(),
    };

    for room in &rooms {
        for p in room.cells() {
            let i = floor.index(p);
            floor.tiles[i] = Tile::Floor;
        }
    }

    // 最小全域木 + ループ用の追加通路。
    let mst = mst_edges(&rooms);
    let extra = extra_edges(rng, &rooms, &mst);
    for &(a, b) in mst.iter().chain(extra.iter()) {
        carve_corridor(rng, &mut floor, rooms[a].center(), rooms[b].center());
    }
    place_doors(&mut floor, &rooms);

    // 階段。rooms[0] が Start 部屋。
    let up = forced_up.unwrap_or_else(|| random_cell(rng, &rooms[0]));
    floor.stairs_up = up;
    let i = floor.index(up);
    floor.tiles[i] = Tile::StairsUp;

    if level < FLOOR_COUNT {
        let room = &rooms[rng.random_range(1..rooms.len())];
        let down = random_cell(rng, room);
        floor.stairs_down = Some(down);
        let i = floor.index(down);
        floor.tiles[i] = Tile::StairsDown;
    } else {
        // 魔導書の部屋: 上り階段から経路距離が最も遠い部屋（同距離なら添字が小さい方）。
        let dist = distance_map(&floor, up);
        let goal = (1..rooms.len())
            .filter_map(|r| dist[floor.index(rooms[r].center())].map(|d| (d, std::cmp::Reverse(r))))
            .max()
            .map(|(_, std::cmp::Reverse(r))| r)?;
        rooms[goal].kind = RoomKind::Goal;
        let pos = random_cell(rng, &rooms[goal]);
        floor.items.push(ItemPlacement { pos, kind: ItemKind::MagicBook });
    }

    floor.rooms = rooms;
    Some(floor)
}

/// 重ならない部屋を配置する。`rooms[0]` は Start 部屋（`forced_up` があればそれを含む）。
fn place_rooms(rng: &mut impl Rng, forced_up: Option<Pos>) -> Option<Vec<Room>> {
    let target = rng.random_range(MIN_ROOMS..=MAX_ROOMS);
    let mut rooms: Vec<Room> = Vec::new();

    if let Some(p) = forced_up {
        let w = rng.random_range(MIN_ROOM_SIZE..=MAX_ROOM_SIZE);
        let h = rng.random_range(MIN_ROOM_SIZE..=MAX_ROOM_SIZE);
        // p を含み、外周1マスの壁を残す範囲。
        let x = rng.random_range((p.x - w + 1).max(1)..=p.x.min(FLOOR_WIDTH - 1 - w));
        let y = rng.random_range((p.y - h + 1).max(1)..=p.y.min(FLOOR_HEIGHT - 1 - h));
        rooms.push(Room { x, y, w, h, kind: RoomKind::Start });
    }

    for _ in 0..ROOM_PLACEMENT_TRIES {
        if rooms.len() >= target {
            break;
        }
        let w = rng.random_range(MIN_ROOM_SIZE..=MAX_ROOM_SIZE);
        let h = rng.random_range(MIN_ROOM_SIZE..=MAX_ROOM_SIZE);
        let x = rng.random_range(1..=FLOOR_WIDTH - 1 - w);
        let y = rng.random_range(1..=FLOOR_HEIGHT - 1 - h);
        let room = Room { x, y, w, h, kind: if rooms.is_empty() { RoomKind::Start } else { RoomKind::Normal } };
        if rooms.iter().all(|r| !r.too_close(&room, ROOM_GAP)) {
            rooms.push(room);
        }
    }
    (rooms.len() >= MIN_ROOMS).then_some(rooms)
}

fn dist2(a: Pos, b: Pos) -> i64 {
    let (dx, dy) = ((a.x - b.x) as i64, (a.y - b.y) as i64);
    dx * dx + dy * dy
}

/// 部屋中心どうしの距離による最小全域木（Prim 法。同距離は添字順で決定的）。
fn mst_edges(rooms: &[Room]) -> Vec<(usize, usize)> {
    let n = rooms.len();
    let mut in_tree = vec![false; n];
    in_tree[0] = true;
    let mut edges = Vec::with_capacity(n - 1);
    while edges.len() < n - 1 {
        let mut best: Option<(i64, usize, usize)> = None;
        for i in (0..n).filter(|&i| in_tree[i]) {
            for j in (0..n).filter(|&j| !in_tree[j]) {
                let cand = (dist2(rooms[i].center(), rooms[j].center()), i, j);
                if best.is_none_or(|b| cand < b) {
                    best = Some(cand);
                }
            }
        }
        let (_, i, j) = best.expect("graph is complete");
        in_tree[j] = true;
        edges.push((i, j));
    }
    edges
}

/// MST にない部屋ペアのうち近いもの（上位 `EXTRA_LOOPS*3`）から `EXTRA_LOOPS` 本を選ぶ。
fn extra_edges(rng: &mut impl Rng, rooms: &[Room], mst: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut cands: Vec<(i64, usize, usize)> = Vec::new();
    for i in 0..rooms.len() {
        for j in i + 1..rooms.len() {
            if !mst.iter().any(|&(a, b)| (a, b) == (i, j) || (a, b) == (j, i)) {
                cands.push((dist2(rooms[i].center(), rooms[j].center()), i, j));
            }
        }
    }
    cands.sort();
    cands.truncate(EXTRA_LOOPS * 3);
    cands.shuffle(rng);
    cands.into_iter().take(EXTRA_LOOPS).map(|(_, i, j)| (i, j)).collect()
}

/// `a` から `b` へ L 字の通路を掘る（曲がる向きは乱数）。
fn carve_corridor(rng: &mut impl Rng, floor: &mut Floor, a: Pos, b: Pos) {
    let corner = if rng.random_bool(0.5) { Pos::new(b.x, a.y) } else { Pos::new(a.x, b.y) };
    for (from, to) in [(a, corner), (corner, b)] {
        let (dx, dy) = ((to.x - from.x).signum(), (to.y - from.y).signum());
        let mut p = from;
        loop {
            let i = floor.index(p);
            if floor.tiles[i] == Tile::Wall {
                floor.tiles[i] = Tile::Floor;
            }
            if p == to {
                break;
            }
            p = Pos::new(p.x + dx, p.y + dy);
        }
    }
}

/// 通路が部屋の壁に接する場所を Door にする。両隣が壁のままのセルだけが対象で、
/// 壁沿いを通り抜けるだけの通路は Door にしない。
fn place_doors(floor: &mut Floor, rooms: &[Room]) {
    for r in rooms {
        // (壁リング上のセル, 壁に沿う方向)
        let mut ring: Vec<(Pos, (i32, i32))> = Vec::new();
        for x in r.x..r.x + r.w {
            ring.push((Pos::new(x, r.y - 1), (1, 0)));
            ring.push((Pos::new(x, r.y + r.h), (1, 0)));
        }
        for y in r.y..r.y + r.h {
            ring.push((Pos::new(r.x - 1, y), (0, 1)));
            ring.push((Pos::new(r.x + r.w, y), (0, 1)));
        }
        for (p, (tx, ty)) in ring {
            let walled = |q: Pos| floor.tile(q).is_none_or(|t| t == Tile::Wall);
            if floor.tile(p) == Some(Tile::Floor)
                && walled(Pos::new(p.x - tx, p.y - ty))
                && walled(Pos::new(p.x + tx, p.y + ty))
            {
                let i = floor.index(p);
                floor.tiles[i] = Tile::Door;
            }
        }
    }
}

/// 上り階段から、下り階段・魔導書・全部屋の中心に到達できるか（BFS）。
fn validate(floor: &Floor) -> bool {
    let dist = distance_map(floor, floor.stairs_up);
    let reachable = |p: Pos| dist[floor.index(p)].is_some();
    floor.stairs_down.is_none_or(reachable)
        && floor.items.iter().all(|it| reachable(it.pos))
        && floor.rooms.iter().all(|r| reachable(r.center()))
}
