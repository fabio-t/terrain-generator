/*
 * Copyright (c) 2015-2017 Fabio Ticconi
 *
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not
 * use this file except in compliance with the License. You may obtain a copy of
 * the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
 * WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the
 * License for the specific language governing permissions and limitations under
 * the License.
 */

use crate::noise::fractal_noise::FractalNoise;
use crate::utils::java_random::JavaRandom;

pub struct HeightMap {
    width: usize,
    height: usize,

    island_fraction: f32,

    source_threshold: f32,
    river_threshold: f32,
    river_fraction: f32,
    max_same_direction: i32,

    pub fractal_noise: FractalNoise,
}

impl Default for HeightMap {
    fn default() -> Self {
        Self::new()
    }
}

impl HeightMap {
    pub fn new() -> Self {
        Self {
            width: 100,
            height: 100,

            island_fraction: 0.5,

            source_threshold: 0.0,
            river_threshold: 0.0,
            river_fraction: 0.0,
            max_same_direction: 0,

            fractal_noise: FractalNoise::new(),
        }
    }

    pub fn size(mut self, width: usize, height: usize) -> Self {
        self.width = width;
        self.height = height;

        self
    }

    pub fn island(mut self, island_fraction: f32) -> Self {
        assert!(
            island_fraction > 0.0 && island_fraction < 1.0,
            "island fraction must be in range (0,1), not included"
        );

        self.island_fraction = island_fraction;

        self
    }

    pub fn rivers(
        mut self,
        source_threshold: f32,
        river_threshold: f32,
        river_fraction: f32,
        max_same_direction: i32,
    ) -> Self {
        assert!(
            source_threshold > 0.0 && source_threshold <= 1.0,
            "source threshold must be in range (0,1]"
        );

        assert!(
            (0.0..1.0).contains(&river_threshold),
            "river threshold must be in range [0,1)"
        );

        assert!(
            river_threshold < source_threshold,
            "river threshold must be strictly less than the sourceThreshold"
        );

        assert!(
            river_fraction > 0.0 && river_fraction <= 1.0,
            "river fraction must be in range (0,1]"
        );

        assert!(
            max_same_direction >= 0,
            "max same direction must be a non-negative integer"
        );

        self.source_threshold = source_threshold;
        self.river_threshold = river_threshold;
        self.river_fraction = river_fraction;
        self.max_same_direction = max_same_direction;

        self
    }

    pub fn build(&mut self) -> Vec<Vec<f32>> {
        let width = self.width;
        let height = self.height;

        let mut heightmap = self.fractal_noise.build(width, height);

        for column in heightmap.iter_mut() {
            for cell in column.iter_mut() {
                *cell = 1.0 - cell.abs();
            }
        }

        if self.island_fraction > 0.0 {
            // we make a new "noise map" that we will use to make noisy coasts of this island
            let seed = self.fractal_noise.get_seed();
            self.fractal_noise
                .seed(if seed < i32::MAX { seed + 1 } else { seed - 1 });
            let height_mod = self.fractal_noise.build(width, height);
            self.fractal_noise.seed(seed); // must put it back

            let island_size_x = width as f32 * self.island_fraction;
            let island_size_y = height as f32 * self.island_fraction;

            let max = island_size_x.max(island_size_y) * 0.5;

            for (x, column) in heightmap.iter_mut().enumerate() {
                for (y, cell) in column.iter_mut().enumerate() {
                    let dist_x = (x as f32 - width as f32 * 0.5).abs();
                    let dist_y = (y as f32 - height as f32 * 0.5).abs();
                    let dist = f64::from(dist_x * dist_x + dist_y * dist_y).sqrt();

                    let delta = dist / f64::from(max);

                    // hackish noise gradient
                    let gradient = delta.powf(3.0) + f64::from(height_mod[x][y].abs());

                    *cell = (0.0f64).max(f64::from(*cell) * (1.0 - gradient)) as f32;
                }
            }
        }

        if self.river_fraction > 0.0 {
            // let's make some rivers

            // simple approach: we go through the whole map, and every cell with height > sourceThreshold
            // gets a chance (equal to riverFraction) of becoming the source of a river.

            let mut r = JavaRandom::new(self.fractal_noise.get_seed() as i64);

            for x in 0..width {
                for y in 0..height {
                    if heightmap[x][y] >= self.source_threshold
                        && r.next_float() < self.river_fraction
                    {
                        self.make_river(&mut heightmap, x as i32, y as i32, -1, 0, -1);
                    }
                }
            }
        }

        heightmap
    }

    fn make_river(
        &self,
        heightmap: &mut [Vec<f32>],
        start_x: i32,
        start_y: i32,
        start_skip_neighbour: i32,
        start_same: i32,
        start_prev: i32,
    ) {
        let width = self.width as i32;
        let height = self.height as i32;

        let mut x = start_x;
        let mut y = start_y;
        let mut skip_neighbour = start_skip_neighbour;
        let mut same = start_same;
        let mut prev = start_prev;

        // the recursion in the original implementation is a tail call, so it
        // becomes a loop here
        loop {
            // the "source" cannot be lower than the "river depth"
            if heightmap[x as usize][y as usize] <= self.river_threshold {
                return;
            }

            let mut new_x = -1;
            let mut new_y = -1;
            let mut min_height = f32::MAX;

            // (the erosion of heightmap[x][y] to river depth will always happen here,
            // and the loop will continue on a chosen neighbour)
            heightmap[x as usize][y as usize] = self.river_threshold;

            let xmin = (x - 1).max(0);
            let xmax = (x + 1).min(width - 1);

            let mut chosen = -1;
            let mut neighbour = 0;
            let mut i = -1 - (x - 1).min(0);
            let mut x_t = xmin;
            while x_t <= xmax {
                let z = 1 - i.abs();

                let y1 = y + z;

                if y1 < height
                    && neighbour != skip_neighbour
                    && (same <= self.max_same_direction || prev != neighbour)
                {
                    let value = heightmap[x_t as usize][y1 as usize];

                    if value > self.river_threshold && value < min_height {
                        new_x = x_t;
                        new_y = y1;
                        min_height = value;
                        chosen = neighbour;
                    }
                }

                neighbour += 1;

                let y2 = y - z;

                if y1 != y2 && y2 >= 0 {
                    let value = heightmap[x_t as usize][y2 as usize];

                    if neighbour != skip_neighbour
                        && value > self.river_threshold
                        && value < min_height
                        && (same <= self.max_same_direction || prev != neighbour)
                    {
                        new_x = x_t;
                        new_y = y2;
                        min_height = value;
                        chosen = neighbour;
                    }

                    neighbour += 1;
                }

                x_t += 1;
                i += 1;
            }

            let same_chosen = if chosen == prev { same + 1 } else { 0 };

            // we continue on the chosen neighbour
            if new_x >= 0 {
                if skip_neighbour < 0 && chosen >= 0 {
                    skip_neighbour = 3 - chosen;
                }
                x = new_x;
                y = new_y;
                same = same_chosen;
                prev = chosen;
            } else {
                return;
            }
        }
    }
}
