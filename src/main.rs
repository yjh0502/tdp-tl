use anyhow::Result;
use argh::FromArgs;
use std::collections::BTreeMap;
use std::fs::File;
use std::ops::Range;

#[derive(FromArgs)]
/// Reach new heights.
struct Opt {
    /// frames
    #[argh(switch)]
    frames: bool,

    /// frames
    #[argh(switch)]
    frames_constz: bool,

    /// bruteforce
    #[argh(switch)]
    bruteforce: bool,

    /// bruteforce
    #[argh(switch)]
    shell: bool,

    /// inject
    #[argh(switch)]
    inject: bool,
}

// RLE, over Z axis,
struct MonotonicVoxel {
    ranges: BTreeMap<[i32; 2], Vec<Range<i32>>>,
}

impl MonotonicVoxel {
    fn new() -> Self {
        Self {
            ranges: BTreeMap::new(),
        }
    }

    fn occupied(&self, coord: [i32; 3]) -> bool {
        if let Some(ranges) = self.ranges.get(&[coord[0], coord[1]]) {
            for range in ranges {
                if range.contains(&coord[2]) {
                    return true;
                }
            }
        }
        false
    }

    fn add(&mut self, coord: [i32; 3]) -> bool {
        let z = coord[2];
        use std::collections::btree_map::Entry;

        match self.ranges.entry([coord[0], coord[1]]) {
            Entry::Vacant(v) => {
                v.insert(vec![z..z + 1]);
                true
            }
            Entry::Occupied(mut v) => {
                let r = v.get_mut();
                for r in r {
                    if r.contains(&z) {
                        return false;
                    } else if r.start == z + 1 {
                        *r = z..r.end;
                        return true;
                    } else if r.end == z {
                        *r = r.start..(z + 1);
                        return true;
                    }
                }

                let r = v.get_mut();
                r.push(z..(z + 1));
                true
            }
        }
    }

    fn to_model(&self) -> Model {
        let mut model = Model::default();

        for (coord, ranges) in self.ranges.iter() {
            for range in ranges {
                if Range::is_empty(range) {
                    continue;
                }

                let x = coord[0];
                let y = coord[1];

                model.add_face([x, y, range.start], [1, 1, 0]);
                model.add_face([x, y, range.end], [1, 1, 0]);

                let faces = [
                    ([1, 0], [1, 0, 0], [0, 1, 1]),
                    ([-1, 0], [0, 0, 0], [0, 1, 1]),
                    ([0, 1], [0, 1, 0], [1, 0, 1]),
                    ([0, -1], [0, 0, 0], [1, 0, 1]),
                ];

                for ([dx, dy], offset, dir) in faces {
                    for z in range.clone() {
                        if !self.occupied([x + dx, y + dy, z]) {
                            model.add_face([x + offset[0], y + offset[1], z + offset[2]], dir);
                        }
                    }
                }
            }
        }

        model
    }
}

#[derive(Default)]
struct Model {
    vertices: Vec<[i32; 3]>,
    faces: Vec<[usize; 4]>,
}

impl Model {
    fn add_vert(&mut self, coord: [i32; 3]) -> usize {
        let len = self.vertices.len();
        self.vertices.push(coord);
        return len;
    }

    fn add_face(&mut self, coord: [i32; 3], dir: [i32; 3]) {
        let (i0, i1, i2, i3) = if dir[0] == 0 {
            let i0 = self.add_vert([coord[0], coord[1], coord[2]]);
            let i1 = self.add_vert([coord[0], coord[1] + dir[1], coord[2]]);
            let i2 = self.add_vert([coord[0], coord[1] + dir[1], coord[2] + dir[2]]);
            let i3 = self.add_vert([coord[0], coord[1], coord[2] + dir[2]]);
            (i0, i1, i2, i3)
        } else if dir[1] == 0 {
            let i0 = self.add_vert([coord[0], coord[1], coord[2]]);
            let i1 = self.add_vert([coord[0] + dir[0], coord[1], coord[2]]);
            let i2 = self.add_vert([coord[0] + dir[0], coord[1], coord[2] + dir[2]]);
            let i3 = self.add_vert([coord[0], coord[1], coord[2] + dir[2]]);
            (i0, i1, i2, i3)
        } else {
            let i0 = self.add_vert([coord[0], coord[1], coord[2]]);
            let i1 = self.add_vert([coord[0] + dir[0], coord[1], coord[2]]);
            let i2 = self.add_vert([coord[0] + dir[0], coord[1] + dir[1], coord[2]]);
            let i3 = self.add_vert([coord[0], coord[1] + dir[1], coord[2]]);
            (i0, i1, i2, i3)
        };

        self.faces.push([i0, i1, i2, i3]);
    }

    fn add_cube(&mut self, coord: [i32; 3]) {
        self.add_face(coord, [1, 1, 0]);
        self.add_face(coord, [1, 0, 1]);
        self.add_face(coord, [0, 1, 1]);

        let coord = [coord[0] + 1, coord[1] + 1, coord[2] + 1];

        self.add_face(coord, [-1, -1, 0]);
        self.add_face(coord, [-1, 0, -1]);
        self.add_face(coord, [0, -1, -1]);
    }

    fn serialize(&self, path: &str) -> Result<()> {
        use std::io::Write;

        let w = File::create(path)?;
        let mut w = std::io::BufWriter::new(w);

        for [x, y, z] in &self.vertices {
            write!(&mut w, "v {} {} {}\n", x, y, z)?;
        }
        for [i0, i1, i2, i3] in &self.faces {
            write!(&mut w, "f {} {} {} {}\n", i0 + 1, i1 + 1, i2 + 1, i3 + 1)?;
        }

        Ok(())
    }
}

const SIZE: i32 = 100i32;
fn test(x: i32, y: i32, z: i32) -> bool {
    return x * x + y * y + z * z < SIZE * SIZE;
}

fn generate_brute_force() -> Model {
    let mut m = Model::default();

    for z in -SIZE..=SIZE {
        for y in -SIZE..=SIZE {
            for x in -SIZE..=SIZE {
                if test(x, y, z) {
                    m.add_cube([x, y, z]);
                }
            }
        }
    }

    m
}

fn generate_shell() -> Model {
    let mut m = Model::default();

    const NEIGHBORS: [[i32; 3]; 6] = [
        [1, 0, 0],
        [-1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
    ];

    fn emit(x: i32, y: i32, z: i32) -> bool {
        let r0 = test(x, y, z);
        for [dx, dy, dz] in &NEIGHBORS {
            let r1 = test(x + dx, y + dy, z + dz);
            if r0 != r1 {
                return true;
            }
        }
        return false;
    }

    for z in -SIZE..=SIZE {
        for y in -SIZE..=SIZE {
            for x in -SIZE..=SIZE {
                if emit(x, y, z) {
                    m.add_cube([x, y, z]);
                }
            }
        }
    }

    m
}

fn generate_face_only() -> Model {
    let mut mv = MonotonicVoxel::new();

    for z in -SIZE..=SIZE {
        for y in -SIZE..=SIZE {
            for x in -SIZE..=SIZE {
                if test(x, y, z) {
                    mv.add([x, y, z]);
                }
            }
        }
    }
    mv.to_model()
}

fn generate_frames_constz() {
    let mut mv = MonotonicVoxel::new();

    let mut idx = 0;
    for z in -SIZE..=SIZE {
        for y in -SIZE..=SIZE {
            for x in -SIZE..=SIZE {
                if test(x, y, z) {
                    mv.add([x, y, z]);
                }
            }
        }
        if z < 0 {
            continue;
        }

        let model = mv.to_model();
        let filename = format!("constz/test_{:03}.obj", idx);
        model.serialize(&filename).unwrap();
        idx += 1;
    }
}

impl MonotonicVoxel {
    fn inject_at(&mut self, zlow: i32, zhigh: i32, pos0: [i32; 3], mut n: usize) {
        use std::collections::BinaryHeap;

        #[derive(Clone, Copy, Ord, PartialEq, Eq, Debug)]
        struct HeapItem {
            dist: usize,
            depth: usize,
            pos: [i32; 3],
        }
        impl std::cmp::PartialOrd for HeapItem {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(other.dist.cmp(&self.dist))
            }
        }

        let mut candidates = BinaryHeap::new();
        let mut visited = MonotonicVoxel::new();
        candidates.push(HeapItem {
            dist: 0,
            depth: 100,
            pos: pos0,
        });

        while let Some(HeapItem {
            dist: _dist,
            depth,
            pos,
        }) = candidates.pop()
        {
            if depth == 0 {
                continue;
            }
            if visited.occupied(pos) {
                continue;
            }
            visited.add(pos);

            if !self.occupied(pos) {
                self.add(pos);

                n -= 1;
                if n == 0 {
                    break;
                }
            }

            let directions = [
                [1, 0, 0],
                [-1, 0, 0],
                [0, 1, 0],
                [0, -1, 0],
                [0, 0, 1],
                [0, 0, -1],
            ];

            for dir in directions {
                let next = [pos[0] + dir[0], pos[1] + dir[1], pos[2] + dir[2]];
                if next[2] < zlow || next[2] > zhigh {
                    continue;
                }
                if visited.occupied(next) {
                    continue;
                }

                let dx = pos0[0] - next[0];
                let dy = pos0[1] - next[1];
                let dz = pos0[2] - next[2];
                let dist = (dx * dx + dy * dy + dz * dz) as usize;
                candidates.push(HeapItem {
                    dist,
                    depth: depth - 1,
                    pos: next,
                });
            }
        }
    }
}

fn generate_inject() {
    let mut mv = MonotonicVoxel::new();

    // unit: 0.02mm, layer thickness: 0.2mm, nozzle size: 0.4mm
    // 20mm

    let inject_per_dist = 200;
    let dist_per_step = 5;
    let dist = 100;
    for step in 0..(dist / dist_per_step) {
        mv.inject_at(
            -5,
            5,
            [step * dist_per_step, 0, 0],
            (inject_per_dist * dist_per_step) as usize,
        );
    }

    let model = mv.to_model();
    model.serialize("inject.obj").unwrap();
}

fn generate_frames() {
    let mut mv = MonotonicVoxel::new();

    let mut count: usize = 0;

    let mut idx = 0;
    for z in -SIZE..=SIZE {
        for y in -SIZE..=SIZE {
            for x in -SIZE..=SIZE {
                if test(x, y, z) {
                    mv.add([x, y, z]);

                    count += 1;
                    if count % 20000 == 0 {
                        eprintln!("render={:?}", (x, y, z));
                        let model = mv.to_model();
                        let filename = format!("constblock/test_{:03}.obj", idx);
                        model.serialize(&filename).unwrap();
                        idx += 1;
                    }
                }
            }
        }
    }
}

fn main() {
    let opt: Opt = argh::from_env();

    if opt.frames_constz {
        generate_frames_constz();
    } else if opt.frames {
        generate_frames();
    } else if opt.inject {
        generate_inject();
    } else {
        let model = if opt.bruteforce {
            generate_brute_force()
        } else if opt.shell {
            generate_shell()
        } else {
            generate_face_only()
        };

        model.serialize("test.obj").unwrap();
    }
}
