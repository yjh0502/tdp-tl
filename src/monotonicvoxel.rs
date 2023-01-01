use super::{BoundingBox, Model, Voxel, VoxelIdx};
use std::collections::BTreeMap;
use std::ops::Range;

// RLE, over Z axis,
#[derive(Default)]
pub struct MonotonicVoxel {
    ranges: BTreeMap<[i32; 2], Vec<Range<i32>>>,
    bb: BoundingBox,
}

impl Voxel for MonotonicVoxel {
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

    fn ranges(&self) -> usize {
        let mut count = 0;
        for ranges in self.ranges.values() {
            count += ranges.len();
        }
        count
    }

    fn bounding_box(&self) -> &BoundingBox {
        &self.bb
    }

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

    fn add(&mut self, coord: VoxelIdx) -> bool {
        use ordslice::Ext;
        let z = coord[2];
        use std::collections::btree_map::Entry;

        match self.ranges.entry([coord[0], coord[1]]) {
            Entry::Vacant(v) => {
                v.insert(vec![z..z + 1]);
            }
            Entry::Occupied(mut v) => {
                let mut updated = false;
                for r in v.get_mut() {
                    if r.contains(&z) {
                        return false;
                    }
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
                    let idx = r.upper_bound_by(|r| z.cmp(&r.start));
                    r.insert(idx, z..(z + 1));
                }
            }
        };

        self.bb.add(coord);
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
                /*
                if !self.occupied([x, y, range.start - 1].into()) {
                    model.add_face([x, y, range.start].into(), up);
                }
                */
                if !self.occupied([x, y, range.end].into()) {
                    model.add_face([x, y, range.end].into(), up);
                }

                let faces = [
                    ([1, 0], [1, 0, 0], [0, 1, 1]),
                    // ([-1, 0], [0, 0, 0], [0, 1, 1]),
                    ([0, 1], [0, 1, 0], [1, 0, 1]),
                    // ([0, -1], [0, 0, 0], [1, 0, 1]),
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
