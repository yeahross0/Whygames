#![allow(dead_code)]

mod fy {
    //! Implementation of Fisher-Yates algorithm.
    //! This is modified version of https://github.com/adambudziak/shuffle/blob/master/src/fy.rs

    use super::RandomRange;

    /// Implementation of Fisher-Yates algorithm.
    #[derive(Debug, Default)]
    pub struct FisherYates {
        buffer: [u8; std::mem::size_of::<usize>()],
    }

    impl FisherYates {
        pub fn shuffle<T>(&mut self, rng: &mut super::SeededRng, data: &mut [T]) {
            for i in 1..data.len() {
                let j = self.gen_range(rng, i);
                data.swap(i, j);
            }
        }

        fn gen_range(&mut self, seeded_rng: &mut super::SeededRng, top: usize) -> usize {
            const USIZE_BYTES: usize = std::mem::size_of::<usize>();
            let bit_width = USIZE_BYTES * 8 - top.leading_zeros() as usize;
            let byte_count = (bit_width - 1) / 8 + 1;
            loop {
                for i in 0..byte_count {
                    self.buffer[i] = seeded_rng.number_in_range(0, 255);
                }
                let result = usize::from_le_bytes(self.buffer);
                let result = result & ((1 << bit_width) - 1);
                if result < top {
                    break result;
                }
            }
        }
    }
}

const DEFAULT_INC: u64 = 1442695040888963407;
const MULTIPLIER: u64 = 6364136223846793005;

#[derive(Debug, Clone)]
pub struct SeededRng {
    seed: u64,
    state: u64,
}

impl SeededRng {
    pub fn new(seed: u64) -> SeededRng {
        let mut rng = SeededRng { seed, state: 0 };
        let old_state = rng.state;
        rng.rand();
        rng.state = old_state.wrapping_add(seed);
        rng.rand();
        rng
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    fn rand(&mut self) -> u32 {
        let oldstate = self.state;
        self.state = self
            .state
            .wrapping_mul(MULTIPLIER)
            .wrapping_add(DEFAULT_INC);
        let xorshifted: u32 = (((oldstate >> 18) ^ oldstate) >> 27) as u32;
        let rot: u32 = (oldstate >> 59) as u32;
        xorshifted.rotate_right(rot)
    }
}

pub trait RandomRange<T> {
    fn number_in_range(&mut self, low: T, high: T) -> T;
}

impl RandomRange<u8> for SeededRng {
    fn number_in_range(&mut self, low: u8, high: u8) -> u8 {
        let r = self.rand() as f32 / std::u32::MAX as f32;
        let r = low as f32 + (high as f32 - low as f32) * r;
        r as u8
    }
}

impl RandomRange<u32> for SeededRng {
    fn number_in_range(&mut self, low: u32, high: u32) -> u32 {
        let r = self.rand() as f32 / std::u32::MAX as f32;
        let r = low as f32 + (high as f32 - low as f32) * r;
        r as u32
    }
}

impl RandomRange<i32> for SeededRng {
    fn number_in_range(&mut self, low: i32, high: i32) -> i32 {
        let r = self.rand() as f32 / std::u32::MAX as f32;
        let r = low as f32 + (high as f32 - low as f32) * r;
        r as i32
    }
}

impl RandomRange<f32> for SeededRng {
    fn number_in_range(&mut self, low: f32, high: f32) -> f32 {
        let r = self.rand() as f32 / std::u32::MAX as f32;
        low + (high - low) * r
    }
}

impl RandomRange<u64> for SeededRng {
    fn number_in_range(&mut self, low: u64, high: u64) -> u64 {
        let r = self.rand() as f32 / std::u32::MAX as f32;
        let r = low as f32 + (high as f32 - low as f32) * r;
        r as u64
    }
}

impl RandomRange<usize> for SeededRng {
    fn number_in_range(&mut self, low: usize, high: usize) -> usize {
        let r = self.rand() as f32 / std::u32::MAX as f32;
        let r = low as f32 + (high as f32 - low as f32) * r;
        r as usize
    }
}

pub struct VecChooseIter<'a, T> {
    source: &'a Vec<T>,
    indices: std::vec::IntoIter<usize>,
}

impl<'a, T> Iterator for VecChooseIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        self.indices.next().map(|ix| &self.source[ix])
    }
}

pub trait ChooseRandom<T> {
    fn shuffle(&mut self, rng: &mut SeededRng);
    fn choose(&self, rng: &mut SeededRng) -> Option<&T>;
    fn choose_mut(&mut self, rng: &mut SeededRng) -> Option<&mut T>;
    fn choose_multiple(&self, _amount: usize) -> VecChooseIter<T>;
}

impl<T> ChooseRandom<T> for Vec<T> {
    fn shuffle(&mut self, rng: &mut SeededRng) {
        let mut fy = fy::FisherYates::default();

        fy.shuffle(rng, self);
    }

    fn choose(&self, rng: &mut SeededRng) -> Option<&T> {
        let ix = rng.number_in_range(0, self.len());
        self.get(ix)
    }

    fn choose_mut(&mut self, rng: &mut SeededRng) -> Option<&mut T> {
        let ix = rng.number_in_range(0, self.len());
        self.get_mut(ix)
    }

    fn choose_multiple(&self, amount: usize) -> VecChooseIter<T> {
        let mut indices = (0..self.len())
            .enumerate()
            .map(|(i, _)| i)
            .collect::<Vec<usize>>();

        indices.resize(amount, 0);

        VecChooseIter {
            source: self,
            indices: indices.into_iter(),
        }
    }
}
