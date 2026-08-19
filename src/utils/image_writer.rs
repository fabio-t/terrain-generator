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

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

type Color = [u8; 3];

pub struct ImageWriter {
    grey: bool,
    width: usize,
    height: usize,
}

// same rounding as java.awt.Color's float constructor
fn float_color(r: f32, g: f32, b: f32) -> Color {
    [
        (r * 255.0 + 0.5) as u8,
        (g * 255.0 + 0.5) as u8,
        (b * 255.0 + 0.5) as u8,
    ]
}

impl ImageWriter {
    pub fn new(width: usize, height: usize, grey: bool) -> Self {
        Self {
            grey,
            width,
            height,
        }
    }

    #[allow(dead_code)]
    fn get_color(&self, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);

        let water = 0.3;

        if t < water {
            if t < water * 0.7 {
                float_color(0.2, 0.5, 0.9)
            } else {
                float_color(0.4, 0.7, 1.0)
            }
        } else {
            // normalize val so 0 is at water level
            let val = (t - water) / (1.0 - water);

            // set color based on above the sea level
            // beach, plain, forest, mountains etc

            if val < 0.1 {
                [255, 255, 0] // yellow
            } else if val < 0.3 {
                [0, 255, 0] // green
            } else if val < 0.55 {
                float_color(0.1, 0.8, 0.2)
            } else if val < 0.7 {
                [128, 128, 128] // gray
            } else {
                [255, 255, 255] // white
            }
        }
    }

    fn get_color2(&self, t: f32) -> Color {
        if t < 0.01 {
            [51, 102, 153]
        } else if t < 0.05 {
            [20, 152, 204]
        } else if t < 0.08 {
            [184, 134, 11]
        } else if t < 0.1 {
            [170, 170, 0]
        } else if t < 0.4 {
            [31, 138, 19]
        } else if t < 0.7 {
            [74, 105, 4]
        } else if t < 0.8 {
            [119, 93, 61]
        } else if t < 0.9 {
            [76, 70, 50]
        } else {
            [255, 240, 220]
        }
    }

    pub fn save_png<P: AsRef<Path>>(&self, filename: P, map: &[Vec<f32>]) {
        let mut data = Vec::with_capacity(self.width * self.height * 3);

        for y in 0..self.height {
            for column in map.iter().take(self.width) {
                let mut value = column[y];

                let col = if self.grey {
                    if value > 1.0 {
                        println!("error: {}", value);
                        value = 1.0;
                    }
                    if value < 0.0 {
                        println!("error: {}", value);
                        value = 0.0;
                    }
                    float_color(value, value, value)
                } else {
                    self.get_color2(value)
                };

                data.extend_from_slice(&col);
            }
        }

        let file = File::create(filename).expect("Failed to create the file");
        let writer = BufWriter::new(file);

        let mut encoder = png::Encoder::new(writer, self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);

        let mut png_writer = encoder.write_header().expect("Failed to create the file");
        png_writer
            .write_image_data(&data)
            .expect("Failed to create the file");
    }
}
