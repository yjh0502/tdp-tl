use anyhow::Result;
use argh::FromArgs;
use std::fs::File;
use std::ops::Range;

#[derive(FromArgs)]
/// Reach new heights.
struct Opt {
    /// frames
    #[argh(switch)]
    frames: bool,

    /// bruteforce
    #[argh(switch)]
    bruteforce: bool,

    /// bruteforce
    #[argh(switch)]
    shell: bool,
}

// single range

const MONOTONIC_VOXEL_SIZE: usize = 300;

// RLE, over Z axis,
struct MonotonicVoxel {
    ranges: Box<[Range<i32>]>,
}

impl MonotonicVoxel {
    fn new() -> Self {
        let l = MONOTONIC_VOXEL_SIZE * MONOTONIC_VOXEL_SIZE;
        let mut ranges = Vec::with_capacity(l);
        for _ in 0..l {
            ranges.push(0i32..0i32);
        }

        Self {
            ranges: ranges.into_boxed_slice(),
        }
    }

    fn idx(coord: [i32; 3]) -> Option<usize> {
        let i = coord[0] + MONOTONIC_VOXEL_SIZE as i32 / 2;
        let j = coord[1] + MONOTONIC_VOXEL_SIZE as i32 / 2;
        if i < 0 || j < 0 || i >= MONOTONIC_VOXEL_SIZE as i32 || j >= MONOTONIC_VOXEL_SIZE as i32 {
            return None;
        }

        let idx = (i * MONOTONIC_VOXEL_SIZE as i32 + j) as usize;
        Some(idx)
    }

    fn occupied(&self, coord: [i32; 3]) -> bool {
        if let Some(idx) = Self::idx(coord) {
            let range = &self.ranges[idx];
            range.contains(&coord[2])
        } else {
            false
        }
    }

    fn add(&mut self, coord: [i32; 3]) -> bool {
        let z = coord[2];

        let idx = Self::idx(coord).unwrap();

        let r = &mut self.ranges[idx];

        if r.contains(&z) {
            false
        } else if Range::is_empty(r) {
            *r = z..(z + 1);
            true
        } else if r.start == z + 1 {
            *r = z..r.end;
            true
        } else if r.end == z {
            *r = r.start..(z + 1);
            true
        } else {
            panic!("coord={:?}, range={:?}", coord, r);
        }
    }

    fn to_model(&self) -> Model {
        let mut model = Model::default();

        for (idx, range) in self.ranges.iter().enumerate() {
            if Range::is_empty(range) {
                continue;
            }

            let i = idx / MONOTONIC_VOXEL_SIZE;
            let j = idx % MONOTONIC_VOXEL_SIZE;

            let x = i as i32 - MONOTONIC_VOXEL_SIZE as i32 / 2;
            let y = j as i32 - MONOTONIC_VOXEL_SIZE as i32 / 2;

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
                    if count % 40000 == 0 {
                        eprintln!("render={:?}", (x, y, z));
                        let model = mv.to_model();
                        let filename = format!("test_{:03}.obj", idx);
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

    if opt.frames {
        generate_frames();
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
