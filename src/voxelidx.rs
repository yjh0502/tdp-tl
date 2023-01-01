#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VoxelIdx {
    pub idx: [i32; 3],
}

impl VoxelIdx {
    pub fn unit() -> Self {
        Self::new([1, 1, 1])
    }
    pub fn new(idx: [i32; 3]) -> Self {
        Self { idx }
    }

    pub fn x(&self) -> VoxelIdx {
        Self {
            idx: [self.idx[0], 0, 0],
        }
    }
    pub fn y(&self) -> VoxelIdx {
        Self {
            idx: [0, self.idx[1], 0],
        }
    }
    pub fn z(&self) -> VoxelIdx {
        Self {
            idx: [0, 0, self.idx[2]],
        }
    }

    pub fn xy(&self) -> VoxelIdx {
        let mut other = self.clone();
        other.idx[2] = 0;
        other
    }

    pub fn xz(&self) -> VoxelIdx {
        let mut other = self.clone();
        other.idx[1] = 0;
        other
    }

    pub fn yz(&self) -> VoxelIdx {
        let mut other = self.clone();
        other.idx[0] = 0;
        other
    }

    pub fn magnitude_squared(&self) -> usize {
        let [x, y, z] = self.idx;
        (x * x + y * y + z * z) as usize
    }
}

impl std::convert::From<[i32; 3]> for VoxelIdx {
    fn from(idx: [i32; 3]) -> Self {
        Self { idx }
    }
}

impl std::convert::From<VoxelIdx> for [i32; 3] {
    fn from(idx: VoxelIdx) -> Self {
        idx.idx
    }
}

impl std::ops::Add for VoxelIdx {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            idx: [
                self.idx[0] + rhs.idx[0],
                self.idx[1] + rhs.idx[1],
                self.idx[2] + rhs.idx[2],
            ],
        }
    }
}

impl std::ops::AddAssign for VoxelIdx {
    fn add_assign(&mut self, rhs: Self) {
        self.idx[0] += rhs.idx[0];
        self.idx[1] += rhs.idx[1];
        self.idx[2] += rhs.idx[2];
    }
}

impl std::ops::Sub for VoxelIdx {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            idx: [
                self.idx[0] - rhs.idx[0],
                self.idx[1] - rhs.idx[1],
                self.idx[2] - rhs.idx[2],
            ],
        }
    }
}

impl std::ops::SubAssign for VoxelIdx {
    fn sub_assign(&mut self, rhs: Self) {
        self.idx[0] -= rhs.idx[0];
        self.idx[1] -= rhs.idx[1];
        self.idx[2] -= rhs.idx[2];
    }
}
