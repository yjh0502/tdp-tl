use anyhow::Result;
use argh::FromArgs;
use log::*;
use std::fs::File;
use stopwatch::Stopwatch;

mod voxelidx;
use voxelidx::VoxelIdx;

mod rangesetvoxel;
use rangesetvoxel::RangeSetVoxel;

mod monotonicvoxel;
use monotonicvoxel::MonotonicVoxel;

#[derive(FromArgs)]
/// toplevel
struct TopLevel {
    #[argh(subcommand)]
    nested: SubCommandEnum,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
enum SubCommandEnum {
    DemoSphereFrames(DemoSphereFrames),
    DemoSphere(DemoSphere),
    DemoInject(DemoInject),
    Gcode(SubCommandGcode),
    GcodeLayers(SubCommandGcodeLayers),
}

#[derive(FromArgs, PartialEq, Debug)]
/// sphere frames
#[argh(subcommand, name = "demo-sphere-frames")]
struct DemoSphereFrames {
    /// const-z mode
    #[argh(option)]
    constz: bool,

    /// output directory
    #[argh(option)]
    outdir: String,
}

#[derive(FromArgs, PartialEq, Debug)]
/// sphere frames
#[argh(subcommand, name = "demo-sphere")]
struct DemoSphere {
    /// bruteforce
    #[argh(option)]
    bruteforce: bool,

    /// shell-only
    #[argh(option)]
    shell: bool,

    /// output filename
    #[argh(option)]
    out: String,
}

#[derive(FromArgs, PartialEq, Debug)]
/// demo: inject
#[argh(subcommand, name = "demo-inject")]
struct DemoInject {
    /// output filename
    #[argh(option)]
    out: String,
}

#[derive(FromArgs, PartialEq, Debug)]
/// gcode to obj
#[argh(subcommand, name = "gcode")]
struct SubCommandGcode {
    /// input filename
    #[argh(option)]
    gcode: String,

    /// output filename
    #[argh(option)]
    out: String,

    /// target number of layers
    #[argh(option)]
    layer: Option<usize>,
}

#[derive(FromArgs, PartialEq, Debug)]
/// gcode layers to obj
#[argh(subcommand, name = "gcode-layers")]
struct SubCommandGcodeLayers {
    /// input filename
    #[argh(option)]
    gcode: String,

    /// output directory
    #[argh(option)]
    outdir: String,

    /// use rangeset data structure
    #[argh(switch)]
    rangeset: bool,
}

impl std::ops::Index<usize> for VoxelIdx {
    type Output = i32;

    fn index(&self, index: usize) -> &Self::Output {
        &self.idx[index]
    }
}

#[derive(Default, Debug)]
pub struct BoundingBox {
    bound_min: VoxelIdx,
    bound_max: VoxelIdx,

    count: usize,
}

impl BoundingBox {
    fn add(&mut self, coord: VoxelIdx) {
        // first block
        if self.count == 0 {
            self.bound_min = coord;
            self.bound_max = coord;
        } else {
            self.bound_min = coord.bb_min(&self.bound_min);
            self.bound_max = coord.bb_max(&self.bound_max);
        }
        self.count += 1;
    }
}

pub trait Voxel {
    fn blocks(&self) -> usize;
    fn ranges(&self) -> usize;
    fn bounding_box(&self) -> &BoundingBox;
    fn occupied(&self, coord: VoxelIdx) -> bool;
    fn add(&mut self, coord: VoxelIdx) -> bool;
    fn to_model(&self) -> Model;
}

#[derive(Default)]
pub struct Model {
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

fn generate_frames_constz(outdir: &String) -> Result<()> {
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
        let filename = format!("{}/out_{:03}.obj", outdir, idx);
        model.serialize(&filename, [0f32; 3], 1f32)?;
        idx += 1;
    }
    Ok(())
}

fn inject_at<V: Voxel>(v: &mut V, zlow: i32, zhigh: i32, pos0: VoxelIdx, mut n: usize) {
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

        if v.add(pos) {
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

fn generate_inject(out: &str) -> Result<()> {
    let mut mv = MonotonicVoxel::default();

    // unit: 0.02mm, layer thickness: 0.2mm, nozzle size: 0.4mm
    // 20mm

    let inject_per_dist = 200;
    let dist_per_step = 5;
    let dist = 100;
    for step in 0..(dist / dist_per_step) {
        inject_at(
            &mut mv,
            -5,
            5,
            [step * dist_per_step, 0, 0].into(),
            (inject_per_dist * dist_per_step) as usize,
        );
    }

    let model = mv.to_model();
    model.serialize(out, [0f32; 3], 1f32)
}

fn generate_frames(outdir: &str) -> Result<()> {
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
                        let filename = format!("{}/out_{:03}.obj", outdir, idx);
                        model.serialize(&filename, [0f32; 3], 1f32)?;
                        idx += 1;
                    }
                }
            }
        }
    }
    Ok(())
}

fn generate_gcode<V: Voxel + Default>(
    filename: &str,
    out_filename: &str,
    layer: usize,
    out_layers: bool,
) -> Result<()> {
    use nalgebra::Vector3;
    use nom_gcode::{GCodeLine::*, Mnemonic};

    // unit: 0.02mm, layer thickness: 0.2mm, nozzle size: 0.4mm
    // 20mm
    const UNIT: f32 = 0.04f32;

    let mut mv = V::default();

    let gcode = std::fs::read_to_string(filename)?;

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
        match nom_gcode::parse_gcode(&line)? {
            (_, Some(Comment(comment))) => {
                let prefix = "LAYER:";
                if !comment.0.starts_with(prefix) {
                    continue;
                }
                let layer_idx = comment.0[prefix.len()..].parse::<usize>()?;
                if layer_idx == 0 {
                    continue;
                }

                if layer_idx == layer {
                    break;
                }

                if out_layers {
                    let sw = Stopwatch::start_new();
                    let model = mv.to_model();
                    info!("to_model: took={}ms", sw.elapsed_ms());

                    let sw = Stopwatch::start_new();
                    let out_filename = format!("{}/gcode_{:03}.obj", out_filename, layer_idx);
                    model.serialize(&out_filename, [-90f32, -90f32, 0f32], UNIT)?;
                    info!(
                        "Model::Serialize: took={}ms, filename={}",
                        sw.elapsed_ms(),
                        out_filename
                    );
                }
            }
            (_, Some(GCode(code))) => {
                debug!("{}", line);
                if code.mnemonic != Mnemonic::General {
                    continue;
                }
                if code.major == 0 {
                    for (letter, value) in code.arguments() {
                        let letter = *letter;
                        let v = match value {
                            Some(v) => *v,
                            None => continue,
                        };

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
                        let v = match value {
                            Some(v) => *v,
                            None => continue,
                        };

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
                        inject_at(&mut mv, z - 20, z, next_pos, blocks_per_step);
                        cursor = next;
                        blocks -= blocks_per_step;
                    }
                    {
                        let next_pos = to_intpos([dst[0], dst[1], dst[2]]);
                        let z = next_pos[2];
                        inject_at(&mut mv, z - 20, z, next_pos, blocks);
                    }

                    pos = dst;
                    e = dst_e;
                }
            }
            (_, _) => (),
        }
    }

    let blocks = mv.blocks();
    info!(
        "voxel construction: took={}ms, blocks={}/{}, bps={}",
        sw.elapsed_ms(),
        blocks,
        mv.ranges(),
        blocks * 1000 / sw.elapsed_ms() as usize
    );

    info!("bounding box: {:?}", mv.bounding_box());

    if !out_layers {
        let sw = Stopwatch::start_new();
        let model = mv.to_model();
        info!("to_model: took={}ms", sw.elapsed_ms());

        let sw = Stopwatch::start_new();
        model.serialize(&out_filename, [-90f32, -90f32, 0f32], UNIT)?;
        info!(
            "Model::Serialize: took={}ms, filename={}",
            sw.elapsed_ms(),
            out_filename
        );
    }

    Ok(())
}

fn main() -> Result<()> {
    env_logger::init();

    let opt: TopLevel = argh::from_env();

    match opt.nested {
        SubCommandEnum::DemoSphereFrames(opt) => {
            if opt.constz {
                generate_frames_constz(&opt.outdir)
            } else {
                generate_frames(&opt.outdir)
            }
        }

        SubCommandEnum::DemoSphere(opt) => {
            let model = if opt.bruteforce {
                generate_brute_force()
            } else if opt.shell {
                generate_shell()
            } else {
                generate_face_only()
            };

            model.serialize(&opt.out, [0f32; 3], 1f32)?;
            Ok(())
        }

        SubCommandEnum::DemoInject(opt) => generate_inject(&opt.out),

        SubCommandEnum::Gcode(opt) => {
            let layer = opt.layer.unwrap_or(std::usize::MAX);
            generate_gcode::<MonotonicVoxel>(&opt.gcode, &opt.out, layer, false)
        }

        SubCommandEnum::GcodeLayers(opt) => {
            let layer = std::usize::MAX;
            if opt.rangeset {
                generate_gcode::<RangeSetVoxel>(&opt.gcode, &opt.outdir, layer, true)
            } else {
                generate_gcode::<MonotonicVoxel>(&opt.gcode, &opt.outdir, layer, true)
            }
        }
    }
}
