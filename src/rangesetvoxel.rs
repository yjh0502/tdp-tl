use super::{BoundingBox, Model, Voxel, VoxelIdx};
use rangemap::RangeSet;

#[derive(Default)]
pub struct RangeSetVoxel {
    ranges: RangeSet<VoxelIdx>,
    bb: BoundingBox,
}

impl Voxel for RangeSetVoxel {
    fn blocks(&self) -> usize {
        let mut count = 0usize;
        for r in self.ranges.iter() {
            count += (r.end[2] - r.start[2]) as usize;
        }
        count
    }

    fn ranges(&self) -> usize {
        self.ranges.iter().count()
    }

    fn bounding_box(&self) -> &BoundingBox {
        &self.bb
    }

    fn occupied(&self, coord: VoxelIdx) -> bool {
        self.ranges.contains(&coord)
    }

    fn add(&mut self, coord: VoxelIdx) -> bool {
        if self.occupied(coord) {
            return false;
        }

        let end = coord + VoxelIdx::new([0, 0, 1]);
        self.ranges.insert(coord..end);
        self.bb.add(coord);
        true
    }

    fn to_model(&self) -> Model {
        let mut model = Model::default();

        for range in self.ranges.iter() {
            assert_eq!(range.start.xy(), range.end.xy());
            let x = range.start[0];
            let y = range.start[1];

            let range_z = range.start[2]..range.end[2];

            let up = VoxelIdx::from([1, 1, 0]);
            model.add_face([x, y, range_z.start].into(), up);
            model.add_face([x, y, range_z.end].into(), up);

            let faces = [
                ([1, 0], [1, 1, 1], [0, -1, -1]),
                ([-1, 0], [0, 0, 0], [0, 1, 1]),
                ([0, 1], [1, 1, 1], [-1, 0, -1]),
                ([0, -1], [0, 0, 0], [1, 0, 1]),
            ];

            for ([dx, dy], offset, dir) in faces {
                for z in range_z.clone() {
                    if !self.occupied([x + dx, y + dy, z].into()) {
                        model.add_face(
                            [x + offset[0], y + offset[1], z + offset[2]].into(),
                            dir.into(),
                        );
                    }
                }
            }
        }

        model
    }
}
