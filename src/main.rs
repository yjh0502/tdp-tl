use anyhow::Result;
use argh::FromArgs;
use log::*;
use std::collections::BTreeMap;
use std::fs::File;
use std::ops::Range;
use stopwatch::Stopwatch;

mod voxelidx;
use voxelidx::VoxelIdx;

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

    /// gcode
    #[argh(option)]
    gcode_filename: Option<String>,
}

impl std::ops::Index<usize> for VoxelIdx {
    type Output = i32;

    fn index(&self, index: usize) -> &Self::Output {
        &self.idx[index]
    }
}

// RLE, over Z axis,
#[derive(Default)]
struct MonotonicVoxel {
    ranges: BTreeMap<[i32; 2], Vec<Range<i32>>>,
    bound_min: VoxelIdx,
    bound_max: VoxelIdx,

    count: usize,
}

impl MonotonicVoxel {
    fn occupied(&self, coord: VoxelIdx) -> bool {
        if let Some(ranges) = self.ranges.get(&[coord[0], coord[1]]) {
            for range in ranges {
                if range.contains(&coord[2]) {
                    return true;
                }
            }
        }
        false
    }

    fn blocks(&self) -> usize {
        let mut count = 0;
        for ranges in self.ranges.values() {
            for range in ranges {
                assert!(range.start < range.end);
                count += (range.end - range.start) as usize;
            }
        }
        count
    }

    fn add(&mut self, coord: VoxelIdx) -> bool {
        let z = coord[2];
        use std::collections::btree_map::Entry;

        match self.ranges.entry([coord[0], coord[1]]) {
            Entry::Vacant(v) => {
                v.insert(vec![z..z + 1]);
            }
            Entry::Occupied(mut v) => {
                for r in v.get() {
                    if r.contains(&z) {
                        return false;
                    }
                }

                let r = v.get_mut();
                let mut updated = false;
                for r in r {
                    if r.start == z + 1 {
                        r.start -= 1;
                        updated = true;
                        break;
                    } else if r.end == z {
                        r.end += 1;
                        updated = true;
                        break;
                    }
                }

                if !updated {
                    let r = v.get_mut();
                    r.push(z..(z + 1));
                }
            }
        };

        // first block
        if self.count == 0 {
            self.bound_min = coord;
            self.bound_max = coord;
        } else {
            self.bound_min = coord.bb_min(&self.bound_min);
            self.bound_max = coord.bb_max(&self.bound_max);
        }
        self.count += 1;

        true
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

                let up = VoxelIdx::from([1, 1, 0]);

                model.add_face([x, y, range.start].into(), up);
                model.add_face([x, y, range.end].into(), up);

                let faces = [
                    ([1, 0], [1, 0, 0], [0, 1, 1]),
                    ([-1, 0], [0, 0, 0], [0, 1, 1]),
                    ([0, 1], [0, 1, 0], [1, 0, 1]),
                    ([0, -1], [0, 0, 0], [1, 0, 1]),
                ];

                for ([dx, dy], offset, dir) in faces {
                    for z in range.clone() {
                        if !self.occupied([x + dx, y + dy, z].into()) {
                            model.add_face(
                                [x + offset[0], y + offset[1], z + offset[2]].into(),
                                dir.into(),
                            );
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
    vertices: indexmap::IndexSet<VoxelIdx>,
    faces: Vec<[usize; 4]>,
}

impl Model {
    fn add_vert(&mut self, coord: VoxelIdx) -> usize {
        let (idx, _) = self.vertices.insert_full(coord);
        idx
    }

    fn add_face(&mut self, coord: VoxelIdx, dir: VoxelIdx) {
        let (i0, i1, i2, i3) = if dir[0] == 0 {
            let i0 = self.add_vert(coord);
            let i1 = self.add_vert(coord + dir.y());
            let i2 = self.add_vert(coord + dir.yz());
            let i3 = self.add_vert(coord + dir.z());
            (i0, i1, i2, i3)
        } else if dir[1] == 0 {
            let i0 = self.add_vert(coord);
            let i1 = self.add_vert(coord + dir.x());
            let i2 = self.add_vert(coord + dir.xz());
            let i3 = self.add_vert(coord + dir.z());
            (i0, i1, i2, i3)
        } else {
            let i0 = self.add_vert(coord);
            let i1 = self.add_vert(coord + dir.x());
            let i2 = self.add_vert(coord + dir.xy());
            let i3 = self.add_vert(coord + dir.y());
            (i0, i1, i2, i3)
        };

        self.faces.push([i0, i1, i2, i3]);
    }

    fn add_cube(&mut self, coord: VoxelIdx) {
        self.add_face(coord, [1, 1, 0].into());
        self.add_face(coord, [1, 0, 1].into());
        self.add_face(coord, [0, 1, 1].into());

        let coord = coord + VoxelIdx::unit();

        self.add_face(coord, [-1, -1, 0].into());
        self.add_face(coord, [-1, 0, -1].into());
        self.add_face(coord, [0, -1, -1].into());
    }

    fn serialize(&self, path: &str, offset: [f32; 3], scale: f32) -> Result<()> {
        use std::io::Write;

        let w = File::create(path)?;
        let mut w = std::io::BufWriter::new(w);

        for idx in &self.vertices {
            let x = idx[0];
            let y = idx[1];
            let z = idx[2];
            write!(
                &mut w,
                "v {:.2} {:.2} {:.2}\n",
                x as f32 * scale + offset[0],
                y as f32 * scale + offset[1],
                z as f32 * scale + offset[2]
            )?;
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
                    m.add_cube([x, y, z].into());
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
                    m.add_cube([x, y, z].into());
                }
            }
        }
    }

    m
}

fn generate_face_only() -> Model {
    let mut mv = MonotonicVoxel::default();

    for z in -SIZE..=SIZE {
        for y in -SIZE..=SIZE {
            for x in -SIZE..=SIZE {
                if test(x, y, z) {
                    mv.add([x, y, z].into());
                }
            }
        }
    }
    mv.to_model()
}

fn generate_frames_constz() {
    let mut mv = MonotonicVoxel::default();

    let mut idx = 0;
    for z in -SIZE..=SIZE {
        for y in -SIZE..=SIZE {
            for x in -SIZE..=SIZE {
                if test(x, y, z) {
                    mv.add([x, y, z].into());
                }
            }
        }
        if z < 0 {
            continue;
        }

        let model = mv.to_model();
        let filename = format!("constz/test_{:03}.obj", idx);
        model.serialize(&filename, [0f32; 3], 1f32).unwrap();
        idx += 1;
    }
}

impl MonotonicVoxel {
    fn inject_at(&mut self, zlow: i32, zhigh: i32, pos0: VoxelIdx, mut n: usize) {
        use std::collections::BinaryHeap;

        if n == 0 {
            return;
        }

        #[derive(Clone, Copy, Ord, PartialEq, Eq, Debug)]
        struct HeapItem {
            dist: usize,
            depth: usize,
            pos: VoxelIdx,
        }
        impl std::cmp::PartialOrd for HeapItem {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(other.dist.cmp(&self.dist))
            }
        }

        let mut candidates = BinaryHeap::new();
        let mut visited = MonotonicVoxel::default();
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
            if !visited.add(pos) {
                continue;
            }

            if self.add(pos) {
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
                let next: VoxelIdx = pos + dir.into();
                if next[2] < zlow || next[2] > zhigh {
                    continue;
                }
                if visited.occupied(next) {
                    continue;
                }

                let delta = pos0 - next;
                let dist = delta.magnitude_squared();
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
    let mut mv = MonotonicVoxel::default();

    // unit: 0.02mm, layer thickness: 0.2mm, nozzle size: 0.4mm
    // 20mm

    let inject_per_dist = 200;
    let dist_per_step = 5;
    let dist = 100;
    for step in 0..(dist / dist_per_step) {
        mv.inject_at(
            -5,
            5,
            [step * dist_per_step, 0, 0].into(),
            (inject_per_dist * dist_per_step) as usize,
        );
    }

    let model = mv.to_model();
    model.serialize("inject.obj", [0f32; 3], 1f32).unwrap();
}

fn generate_frames() {
    let mut mv = MonotonicVoxel::default();

    let mut count: usize = 0;

    let mut idx = 0;
    for z in -SIZE..=SIZE {
        for y in -SIZE..=SIZE {
            for x in -SIZE..=SIZE {
                if test(x, y, z) {
                    mv.add([x, y, z].into());

                    count += 1;
                    if count % 20000 == 0 {
                        info!("render={:?}", (x, y, z));
                        let model = mv.to_model();
                        let filename = format!("constblock/test_{:03}.obj", idx);
                        model.serialize(&filename, [0f32; 3], 1f32).unwrap();
                        idx += 1;
                    }
                }
            }
        }
    }
}

fn generate_gcode(filename: &str) {
    use nalgebra::Vector3;
    use nom_gcode::{GCodeLine::GCode, Mnemonic};

    // unit: 0.02mm, layer thickness: 0.2mm, nozzle size: 0.4mm
    // 20mm
    const UNIT: f32 = 0.04f32;

    let mut mv = MonotonicVoxel::default();

    let gcode = std::fs::read_to_string(filename).unwrap();

    fn to_intpos(pos: [f32; 3]) -> VoxelIdx {
        return [
            (pos[0] / UNIT).round() as i32,
            (pos[1] / UNIT).round() as i32,
            (pos[2] / UNIT).round() as i32,
        ]
        .into();
    }

    let sw = Stopwatch::start_new();

    let mut pos = Vector3::default();
    let mut e = 0f32;
    for line in gcode.lines() {
        if let (_, Some(GCode(code))) = nom_gcode::parse_gcode(&line).unwrap() {
            debug!("{}", line);
            if code.mnemonic != Mnemonic::General {
                continue;
            }
            if code.major == 0 {
                for (letter, value) in code.arguments() {
                    let letter = *letter;
                    let v = value.unwrap();
                    if letter == 'X' {
                        pos[0] = v;
                    }
                    if letter == 'Y' {
                        pos[1] = v;
                    }
                    if letter == 'Z' {
                        pos[2] = v;
                    }
                }
            } else if code.major == 1 {
                let mut dst = pos;
                let mut dst_e = e;
                for (letter, value) in code.arguments() {
                    let letter = *letter;
                    let v = value.unwrap();
                    if letter == 'X' {
                        dst[0] = v;
                    }
                    if letter == 'Y' {
                        dst[1] = v;
                    }
                    if letter == 'Z' {
                        dst[2] = v;
                    }
                    if letter == 'E' {
                        dst_e = v;
                    }
                }
                if dst_e <= e {
                    pos = dst;
                    continue;
                }

                let dir = (dst - pos).normalize();
                let len = (dst - pos).magnitude();

                let total_blocks = (dst_e - e) * 29000f32;
                let mut blocks = total_blocks as usize;
                let step_size = 0.1;
                let blocks_per_step = (total_blocks * step_size / len) as usize;

                debug!(
                    "{:?} -> {:?}, len={}, e={:?}, blocks={}",
                    pos,
                    dst,
                    len,
                    dst_e - e,
                    total_blocks
                );

                let mut cursor = pos;
                while (cursor - dst).magnitude() > step_size {
                    let next = cursor + dir * step_size;
                    let next_pos = to_intpos([next[0], next[1], next[2]]);
                    let z = next_pos[2];
                    mv.inject_at(z - 20, z, next_pos, blocks_per_step);
                    cursor = next;
                    blocks -= blocks_per_step;
                }
                {
                    let next_pos = to_intpos([dst[0], dst[1], dst[2]]);
                    let z = next_pos[2];
                    mv.inject_at(z - 20, z, next_pos, blocks);
                }

                pos = dst;
                e = dst_e;
            }
        }
    }

    info!(
        "voxel construction: took={}ms, blocks={}/{}, bps={}",
        sw.elapsed_ms(),
        mv.count,
        mv.blocks(),
        mv.count * 1000 / sw.elapsed_ms() as usize
    );

    info!("bounding box: [{:?}, {:?}]", mv.bound_min, mv.bound_max,);

    let sw = Stopwatch::start_new();
    let model = mv.to_model();
    info!("to_model: took={}ms", sw.elapsed_ms());

    let sw = Stopwatch::start_new();
    model
        .serialize("gcode.obj", [-90f32, -90f32, 0f32], UNIT)
        .unwrap();
    info!("Model::Serialize: took={}ms", sw.elapsed_ms());
}

fn main() {
    env_logger::init();

    let opt: Opt = argh::from_env();

    if opt.frames_constz {
        generate_frames_constz();
    } else if opt.frames {
        generate_frames();
    } else if opt.inject {
        generate_inject();
    } else if let Some(filename) = opt.gcode_filename {
        generate_gcode(&filename);
    } else {
        let model = if opt.bruteforce {
            generate_brute_force()
        } else if opt.shell {
            generate_shell()
        } else {
            generate_face_only()
        };

        model.serialize("test.obj", [0f32; 3], 1f32).unwrap();
    }
}
