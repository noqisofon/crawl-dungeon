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
///
/// # Panics
/// `level` が `1..=FLOOR_COUNT` の範囲外、または `forced_up` が外周壁を含めた範囲外の場合。
pub fn generate_floor(seed: u64, level: usize, forced_up: Option<Pos>) -> Floor {
    assert!((1..=FLOOR_COUNT).contains(&level), "invalid floor level: {level}");
    if let Some(p) = forced_up {
        assert!(
            p.x >= 1 && p.x < FLOOR_WIDTH - 1 && p.y >= 1 && p.y < FLOOR_HEIGHT - 1,
            "forced_up out of interior bounds: {p:?}"
        );
    }
    let base = floor_seed(seed, level);
    for attempt in 0..MAX_ATTEMPTS {
        let mut rng = ChaCha8Rng::seed_from_u64(attempt_seed(base, attempt));
        if let Some(floor) = try_generate_floor(&mut rng, level, forced_up)
            && validate(&floor)
        {
            return floor;
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
        carve_corridor(rng, &mut floor, &rooms, rooms[a].center(), rooms[b].center());
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

/// 線分 `from..=to` がいずれかの部屋の壁リング上を並走するか。
fn grazes_room_wall(rooms: &[Room], from: Pos, to: Pos) -> bool {
    if from.y == to.y && from.x != to.x {
        let (x0, x1) = (from.x.min(to.x), from.x.max(to.x));
        rooms.iter().any(|r| {
            (from.y == r.y - 1 || from.y == r.y + r.h) && x0.max(r.x) <= x1.min(r.x + r.w - 1)
        })
    } else if from.x == to.x && from.y != to.y {
        let (y0, y1) = (from.y.min(to.y), from.y.max(to.y));
        rooms.iter().any(|r| {
            (from.x == r.x - 1 || from.x == r.x + r.w) && y0.max(r.y) <= y1.min(r.y + r.h - 1)
        })
    } else {
        false
    }
}

/// `a` から `b` へ L 字の通路を掘る（部屋の壁リング上を並走しない向きを優先し、両方可なら乱数）。
fn carve_corridor(rng: &mut impl Rng, floor: &mut Floor, rooms: &[Room], a: Pos, b: Pos) {
    let c1 = Pos::new(b.x, a.y);
    let c2 = Pos::new(a.x, b.y);
    let ok1 = !grazes_room_wall(rooms, a, c1) && !grazes_room_wall(rooms, c1, b);
    let ok2 = !grazes_room_wall(rooms, a, c2) && !grazes_room_wall(rooms, c2, b);
    let coin = rng.random_bool(0.5);
    let corner = match (ok1, ok2) {
        (true, false) => c1,
        (false, true) => c2,
        _ => if coin { c1 } else { c2 },
    };
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

#[derive(Clone, Copy)]
struct WallCell {
    pos: Pos,
    tangent: (i32, i32),
    normal: (i32, i32),
}

/// 部屋 `r` の4辺の壁リング（四隅を除く）のセルと、壁に沿う方向・外向き法線を返す。
fn room_wall_ring(r: &Room) -> Vec<WallCell> {
    let mut ring = Vec::with_capacity((2 * (r.w + r.h)) as usize);
    for x in r.x..r.x + r.w {
        ring.push(WallCell { pos: Pos::new(x, r.y - 1), tangent: (1, 0), normal: (0, -1) });
        ring.push(WallCell { pos: Pos::new(x, r.y + r.h), tangent: (1, 0), normal: (0, 1) });
    }
    for y in r.y..r.y + r.h {
        ring.push(WallCell { pos: Pos::new(r.x - 1, y), tangent: (0, 1), normal: (-1, 0) });
        ring.push(WallCell { pos: Pos::new(r.x + r.w, y), tangent: (0, 1), normal: (1, 0) });
    }
    ring
}

fn on_room_wall_ring(r: &Room, p: Pos) -> bool {
    let on_horiz = (p.y == r.y - 1 || p.y == r.y + r.h) && p.x >= r.x && p.x < r.x + r.w;
    let on_vert = (p.x == r.x - 1 || p.x == r.x + r.w) && p.y >= r.y && p.y < r.y + r.h;
    on_horiz || on_vert
}

/// 通路が部屋の壁を貫く場所を Door にする。
/// 同じ部屋の壁リング上で隣り合うセル（角を挟む隣接を含む）が壁であり、
/// かつ部屋の外側へ通じている1マス幅の開口部を Door にする。
fn place_doors(floor: &mut Floor, rooms: &[Room]) {
    for r in rooms {
        let ring = room_wall_ring(r);
        let is_wall_on_ring = |q: Pos| !on_room_wall_ring(r, q) || floor.tile(q) == Some(Tile::Wall);
        let mut doors = Vec::new();
        for &WallCell { pos: p, tangent: (tx, ty), normal: (nx, ny) } in &ring {
            if floor.tile(p) != Some(Tile::Floor) {
                continue;
            }
            // 同じ辺の隣、および角を挟んで接する隣辺の端が壁であること。
            let adj_ok = is_wall_on_ring(Pos::new(p.x - tx, p.y - ty))
                && is_wall_on_ring(Pos::new(p.x + tx, p.y + ty))
                && is_wall_on_ring(Pos::new(p.x - tx - nx, p.y - ty - ny))
                && is_wall_on_ring(Pos::new(p.x + tx - nx, p.y + ty - ny));
            // 部屋の外側（法線方向、または角の外側）の少なくとも1方向へ通路が続いていること。
            let leads_outside = [(nx, ny), (-tx, -ty), (tx, ty)].into_iter().any(|(dx, dy)| {
                let q = Pos::new(p.x + dx, p.y + dy);
                !r.contains(q) && !on_room_wall_ring(r, q) && floor.is_walkable(q)
            });
            if adj_ok && leads_outside {
                doors.push(p);
            }
        }
        for p in doors {
            let i = floor.index(p);
            floor.tiles[i] = Tile::Door;
        }
    }
}

/// 上り階段から、下り階段・魔導書・全部屋の中心に到達でき、
/// かつすべての部屋の壁リングに未変換の床（幅2以上の壁欠損など）がないか検証する。
fn validate(floor: &Floor) -> bool {
    let dist = distance_map(floor, floor.stairs_up);
    let reachable = |p: Pos| dist[floor.index(p)].is_some();
    let walls_intact = floor.rooms.iter().all(|r| {
        room_wall_ring(r).into_iter().all(|c| matches!(floor.tile(c.pos), Some(Tile::Wall | Tile::Door)))
    });
    walls_intact
        && floor.stairs_down.is_none_or(reachable)
        && floor.items.iter().all(|it| reachable(it.pos))
        && floor.rooms.iter().all(|r| reachable(r.center()))
}
