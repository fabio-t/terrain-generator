/*
 * Copyright (c) 2015-2018 Fabio Ticconi
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

use crate::noise::open_simplex::OpenSimplexNoise;

pub struct FractalNoise {
    seed: i32,

    octaves: u32,
    frequency: f32,
    roughness: f32,
    amplitude: f32,
    lacunarity: f32,
}

impl Default for FractalNoise {
    fn default() -> Self {
        Self::new()
    }
}

impl FractalNoise {
    pub fn new() -> Self {
        Self {
            octaves: 7,
            frequency: 0.007,
            roughness: 0.5,
            amplitude: 0.0,
            lacunarity: 0.0,
            seed: 5,
        }
    }

    pub fn seed(&mut self, seed: i32) -> &mut Self {
        self.seed = seed;

        self
    }

    pub fn get_seed(&self) -> i32 {
        self.seed
    }

    pub fn set(
        &mut self,
        octaves: u32,
        roughness: f32,
        frequency: f32,
        amplitude: f32,
    ) -> &mut Self {
        assert!(
            octaves > 0 && roughness > 0.0 && frequency > 0.0 && amplitude > 0.0,
            "all parameters must be positive"
        );

        self.octaves = octaves;
        self.roughness = roughness;
        self.frequency = frequency;
        self.amplitude = amplitude;
        self.lacunarity = 1.0 / roughness;

        self
    }

    pub fn set_with_lacunarity(
        &mut self,
        octaves: u32,
        roughness: f32,
        frequency: f32,
        amplitude: f32,
        lacunarity: f32,
    ) -> &mut Self {
        assert!(
            octaves > 0
                && roughness > 0.0
                && frequency > 0.0
                && amplitude > 0.0
                && lacunarity > 0.0,
            "all parameters must be positive"
        );

        self.octaves = octaves;
        self.roughness = roughness;
        self.frequency = frequency;
        self.amplitude = amplitude;
        self.lacunarity = lacunarity;

        self
    }

    pub fn build(&self, width: usize, height: usize) -> Vec<Vec<f32>> {
        let noise = OpenSimplexNoise::new(self.seed as i64);

        let mut total_noise = vec![vec![0.0f32; height]; width];

        let mut temp_amplitude = self.amplitude;
        let mut temp_frequency = self.frequency;

        // matches the Java version, which starts from Float.MIN_VALUE (the
        // smallest positive float, not the most negative one)
        let mut max_v = f32::from_bits(1);
        let mut min_v = f32::MAX;

        for octave in 0..self.octaves {
            // Calculate single layer/octave of simplex noise, then add it to
            // total noise
            for (x, column) in total_noise.iter_mut().enumerate() {
                for (y, cell) in column.iter_mut().enumerate() {
                    *cell += noise.eval(
                        (x as f32 * temp_frequency) as f64,
                        (y as f32 * temp_frequency) as f64,
                    ) as f32
                        * temp_amplitude;

                    if octave == self.octaves - 1 {
                        if *cell > max_v {
                            max_v = *cell;
                        } else if *cell < min_v {
                            min_v = *cell;
                        }
                    }
                }
            }

            // Increase variables with each incrementing octave
            temp_frequency *= self.lacunarity;
            temp_amplitude *= self.roughness;
        }

        for column in total_noise.iter_mut() {
            for cell in column.iter_mut() {
                *cell = (((*cell - min_v) * 2.0) / (max_v - min_v)) - 1.0;
            }
        }

        total_noise
    }
}
