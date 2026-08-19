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

/*
 * OpenSimplex Noise, ported from the Java implementation by Kurt Spencer
 * (v1.1, October 5, 2014). Only the 2D evaluator is ported, as it is the
 * only one used by this project. The seed-based permutation construction
 * uses the same 64-bit LCG as the original, so results match the Java
 * version bit-for-bit.
 */

const STRETCH_CONSTANT_2D: f64 = -0.211324865405187; // (1/sqrt(2+1)-1)/2
const SQUISH_CONSTANT_2D: f64 = 0.366025403784439; // (sqrt(2+1)-1)/2

const NORM_CONSTANT_2D: f64 = 47.0;

// Gradients for 2D. They approximate the directions to the
// vertices of an octagon from the center.
const GRADIENTS_2D: [i8; 16] = [5, 2, 2, 5, -5, 2, -2, 5, 5, -2, 2, -5, -5, -2, -2, -5];

pub struct OpenSimplexNoise {
    perm: [i16; 256],
}

impl OpenSimplexNoise {
    // Initializes the class using a permutation array generated from a 64-bit
    // seed.
    // Generates a proper permutation (i.e. doesn't merely perform N successive
    // pair swaps on a base array)
    // Uses a simple 64-bit LCG.
    pub fn new(seed: i64) -> Self {
        let mut perm = [0i16; 256];
        let mut source: [i16; 256] = [0; 256];
        for (i, v) in source.iter_mut().enumerate() {
            *v = i as i16;
        }

        let mut seed = seed;
        for _ in 0..3 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
        }

        for i in (0..256usize).rev() {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let mut r = (seed.wrapping_add(31) % (i as i64 + 1)) as i32;
            if r < 0 {
                r += i as i32 + 1;
            }
            perm[i] = source[r as usize];
            source[r as usize] = source[i];
        }

        Self { perm }
    }

    fn fast_floor(x: f64) -> i32 {
        let xi = x as i32;
        if x < xi as f64 {
            xi - 1
        } else {
            xi
        }
    }

    // 2D OpenSimplex Noise.
    pub fn eval(&self, x: f64, y: f64) -> f64 {
        // Place input coordinates onto grid.
        let stretch_offset = (x + y) * STRETCH_CONSTANT_2D;
        let xs = x + stretch_offset;
        let ys = y + stretch_offset;

        // Floor to get grid coordinates of rhombus (stretched square)
        // super-cell origin.
        let mut xsb = Self::fast_floor(xs);
        let mut ysb = Self::fast_floor(ys);

        // Skew out to get actual coordinates of rhombus origin. We'll need
        // these later.
        let squish_offset = (xsb + ysb) as f64 * SQUISH_CONSTANT_2D;
        let xb = xsb as f64 + squish_offset;
        let yb = ysb as f64 + squish_offset;

        // Compute grid coordinates relative to rhombus origin.
        let xins = xs - xsb as f64;
        let yins = ys - ysb as f64;

        // Sum those together to get a value that determines which region we're
        // in.
        let in_sum = xins + yins;

        // Positions relative to origin point.
        let mut dx0 = x - xb;
        let mut dy0 = y - yb;

        // We'll be defining these inside the next block and using them
        // afterwards.
        let dx_ext;
        let dy_ext;
        let xsv_ext;
        let ysv_ext;

        let mut value = 0.0;

        // Contribution (1,0)
        let dx1 = dx0 - 1.0 - SQUISH_CONSTANT_2D;
        let dy1 = dy0 - 0.0 - SQUISH_CONSTANT_2D;
        let mut attn1 = 2.0 - dx1 * dx1 - dy1 * dy1;
        if attn1 > 0.0 {
            attn1 *= attn1;
            value += attn1 * attn1 * self.extrapolate(xsb + 1, ysb, dx1, dy1);
        }

        // Contribution (0,1)
        let dx2 = dx0 - 0.0 - SQUISH_CONSTANT_2D;
        let dy2 = dy0 - 1.0 - SQUISH_CONSTANT_2D;
        let mut attn2 = 2.0 - dx2 * dx2 - dy2 * dy2;
        if attn2 > 0.0 {
            attn2 *= attn2;
            value += attn2 * attn2 * self.extrapolate(xsb, ysb + 1, dx2, dy2);
        }

        if in_sum <= 1.0 {
            // We're inside the triangle (2-Simplex) at (0,0)
            let zins = 1.0 - in_sum;
            if zins > xins || zins > yins {
                // (0,0) is one of the closest two triangular vertices
                if xins > yins {
                    xsv_ext = xsb + 1;
                    ysv_ext = ysb - 1;
                    dx_ext = dx0 - 1.0;
                    dy_ext = dy0 + 1.0;
                } else {
                    xsv_ext = xsb - 1;
                    ysv_ext = ysb + 1;
                    dx_ext = dx0 + 1.0;
                    dy_ext = dy0 - 1.0;
                }
            } else {
                // (1,0) and (0,1) are the closest two vertices.
                xsv_ext = xsb + 1;
                ysv_ext = ysb + 1;
                dx_ext = dx0 - 1.0 - 2.0 * SQUISH_CONSTANT_2D;
                dy_ext = dy0 - 1.0 - 2.0 * SQUISH_CONSTANT_2D;
            }
        } else {
            // We're inside the triangle (2-Simplex) at (1,1)
            let zins = 2.0 - in_sum;
            if zins < xins || zins < yins {
                // (0,0) is one of the closest two triangular vertices
                if xins > yins {
                    xsv_ext = xsb + 2;
                    ysv_ext = ysb;
                    dx_ext = dx0 - 2.0 - 2.0 * SQUISH_CONSTANT_2D;
                    dy_ext = dy0 + 0.0 - 2.0 * SQUISH_CONSTANT_2D;
                } else {
                    xsv_ext = xsb;
                    ysv_ext = ysb + 2;
                    dx_ext = dx0 + 0.0 - 2.0 * SQUISH_CONSTANT_2D;
                    dy_ext = dy0 - 2.0 - 2.0 * SQUISH_CONSTANT_2D;
                }
            } else {
                // (1,0) and (0,1) are the closest two vertices.
                dx_ext = dx0;
                dy_ext = dy0;
                xsv_ext = xsb;
                ysv_ext = ysb;
            }
            xsb += 1;
            ysb += 1;
            dx0 = dx0 - 1.0 - 2.0 * SQUISH_CONSTANT_2D;
            dy0 = dy0 - 1.0 - 2.0 * SQUISH_CONSTANT_2D;
        }

        // Contribution (0,0) or (1,1)
        let mut attn0 = 2.0 - dx0 * dx0 - dy0 * dy0;
        if attn0 > 0.0 {
            attn0 *= attn0;
            value += attn0 * attn0 * self.extrapolate(xsb, ysb, dx0, dy0);
        }

        // Extra Vertex
        let mut attn_ext = 2.0 - dx_ext * dx_ext - dy_ext * dy_ext;
        if attn_ext > 0.0 {
            attn_ext *= attn_ext;
            value += attn_ext * attn_ext * self.extrapolate(xsv_ext, ysv_ext, dx_ext, dy_ext);
        }

        value / NORM_CONSTANT_2D
    }

    fn extrapolate(&self, xsb: i32, ysb: i32, dx: f64, dy: f64) -> f64 {
        let index = (self.perm[((self.perm[(xsb & 0xFF) as usize] as i32 + ysb) & 0xFF) as usize]
            & 0x0E) as usize;
        GRADIENTS_2D[index] as f64 * dx + GRADIENTS_2D[index + 1] as f64 * dy
    }
}
