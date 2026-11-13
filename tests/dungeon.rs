use crawl_dungeon::dungeon::*;

fn pos_of(f: &Floor, i: usize) -> Pos {
    Pos::new(i as i32 % f.width, i as i32 / f.width)
}

#[test]
fn same_seed_is_identical() {
    for seed in [0, 1, 42, u64::MAX] {
        assert_eq!(generate_dungeon(seed), generate_dungeon(seed));
    }
}

#[test]
fn different_seeds_differ() {
    assert_ne!(generate_dungeon(1), generate_dungeon(2));
    // 隣り合うシードで階が使い回されないこと。
    let (a, b) = (generate_dungeon(1), generate_dungeon(2));
    assert_ne!(a.floors[1].tiles, b.floors[0].tiles);
}

#[test]
fn hundred_seeds_generate_reachable_floors() {
    for seed in 0..100u64 {
        let d = generate_dungeon(seed);
        assert_eq!(d.seed, seed);
        assert_eq!(d.floors.len(), FLOOR_COUNT);
        for f in &d.floors {
            assert_eq!((f.width, f.height), (FLOOR_WIDTH, FLOOR_HEIGHT));
            assert_eq!(f.tiles.len(), f.traps.len());
            assert!((MIN_ROOMS..=MAX_ROOMS).contains(&f.rooms.len()), "seed {seed}");
            for (i, r) in f.rooms.iter().enumerate() {
                assert!((MIN_ROOM_SIZE..=MAX_ROOM_SIZE).contains(&r.w));
                assert!((MIN_ROOM_SIZE..=MAX_ROOM_SIZE).contains(&r.h));
                assert!(r.x >= 1 && r.y >= 1 && r.x + r.w < f.width && r.y + r.h < f.height);
                for o in &f.rooms[i + 1..] {
                    assert!(!r.too_close(o, 1), "rooms overlap: seed {seed}");
                }
                // 部屋の中心は必ず到達可能。
                assert!(shortest_path(f, f.stairs_up, r.center()).is_some());
            }
            if let Some(down) = f.stairs_down {
                let path = shortest_path(f, f.stairs_up, down).expect("up -> down");
                assert_eq!(path.first(), Some(&f.stairs_up));
                assert_eq!(path.last(), Some(&down));
                assert!(path.windows(2).all(|w| (w[0].x - w[1].x).abs() + (w[0].y - w[1].y).abs() == 1));
                assert!(path.iter().all(|&p| f.is_walkable(p)));
            }
        }
    }
}

#[test]
fn stairs_are_consistent() {
    for seed in 0..100u64 {
        let d = generate_dungeon(seed);
        for w in d.floors.windows(2) {
            assert_eq!(w[0].stairs_down, Some(w[1].stairs_up), "seed {seed}");
            let p = w[1].stairs_up;
            assert_eq!(w[0].tile(p), Some(Tile::StairsDown));
            assert_eq!(w[1].tile(p), Some(Tile::StairsUp));
        }
        let last = d.floors.last().unwrap();
        assert_eq!(last.stairs_down, None);
        assert!(!last.tiles.contains(&Tile::StairsDown));
        for (i, f) in d.floors.iter().enumerate() {
            assert_eq!(f.level, i + 1);
            assert_eq!(f.tile(f.stairs_up), Some(Tile::StairsUp));
            assert_eq!(f.tiles.iter().filter(|&&t| t == Tile::StairsUp).count(), 1);
            // 上り階段は Start 部屋の中。
            let start = f.rooms.iter().find(|r| r.kind == RoomKind::Start).unwrap();
            assert!(start.contains(f.stairs_up));
        }
        // 上り階段と下り階段は同じ部屋にない。
        for f in &d.floors[..4] {
            let down = f.stairs_down.unwrap();
            assert!(!f.rooms.iter().any(|r| r.contains(f.stairs_up) && r.contains(down)));
        }
    }
}

#[test]
fn magic_book_room_is_reachable_and_farthest() {
    for seed in 0..100u64 {
        let d = generate_dungeon(seed);
        for f in &d.floors[..4] {
            assert!(f.items.is_empty());
            assert!(f.rooms.iter().all(|r| r.kind != RoomKind::Goal));
        }
        let f = &d.floors[4];
        assert_eq!(f.items.len(), 1);
        let book = f.items[0];
        assert_eq!(book.kind, ItemKind::MagicBook);
        assert!(shortest_path(f, f.stairs_up, book.pos).is_some(), "seed {seed}");

        let goals: Vec<_> = f.rooms.iter().filter(|r| r.kind == RoomKind::Goal).collect();
        assert_eq!(goals.len(), 1);
        assert!(goals[0].contains(book.pos));

        let dist = distance_map(f, f.stairs_up);
        let d_of = |r: &Room| dist[f.index(r.center())].unwrap();
        assert!(f.rooms.iter().all(|r| d_of(r) <= d_of(goals[0])));
    }
}

#[test]
fn doors_sit_in_room_walls() {
    for seed in 0..100u64 {
        for f in &generate_dungeon(seed).floors {
            for (i, &t) in f.tiles.iter().enumerate() {
                if t != Tile::Door {
                    continue;
                }
                let p = pos_of(f, i);
                let near_room = f.rooms.iter().any(|r| {
                    [(0, 1), (0, -1), (1, 0), (-1, 0)].iter().any(|&(dx, dy)| r.contains(Pos::new(p.x + dx, p.y + dy)))
                });
                assert!(near_room && !f.rooms.iter().any(|r| r.contains(p)), "seed {seed} {p:?}");
            }
            for r in &f.rooms {
                let mut ring = Vec::new();
                for x in r.x..r.x + r.w {
                    ring.push(Pos::new(x, r.y - 1));
                    ring.push(Pos::new(x, r.y + r.h));
                }
                for y in r.y..r.y + r.h {
                    ring.push(Pos::new(r.x - 1, y));
                    ring.push(Pos::new(r.x + r.w, y));
                }
                assert!(
                    ring.iter().all(|&p| matches!(f.tile(p), Some(Tile::Wall | Tile::Door))),
                    "seed {seed}: room {r:?} has an un-doored opening in its wall"
                );
                assert!(
                    ring.iter().any(|&p| f.tile(p) == Some(Tile::Door)),
                    "seed {seed}: room {r:?} has no door"
                );
            }
            assert!(f.tiles.contains(&Tile::Door), "seed {seed}: no doors");
        }
    }
}

#[test]
#[should_panic(expected = "forced_up out of interior bounds")]
fn generate_floor_rejects_border_forced_up() {
    generate_floor(0, 1, Some(Pos::new(0, 0)));
}

#[test]
fn shortest_path_edge_cases() {
    let f = &generate_dungeon(7).floors[0];
    assert_eq!(shortest_path(f, f.stairs_up, f.stairs_up), Some(vec![f.stairs_up]));
    assert_eq!(shortest_path(f, Pos::new(0, 0), f.stairs_up), None); // 壁
    assert_eq!(shortest_path(f, f.stairs_up, Pos::new(-1, 3)), None); // 範囲外
}

#[test]
fn ascii_output() {
    let d = generate_dungeon(3);
    let s = to_ascii(&d.floors[4]);
    let lines: Vec<_> = s.lines().collect();
    assert_eq!(lines.len(), FLOOR_HEIGHT as usize);
    assert!(lines.iter().all(|l| l.chars().count() == FLOOR_WIDTH as usize));
    for c in ['#', '.', '+', '<', 'B'] {
        assert!(s.contains(c), "missing {c}");
    }
    assert!(!s.contains('>'));
    assert!(to_ascii(&d.floors[0]).contains('>'));
    println!("{s}");
}
