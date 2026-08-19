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

use terrain_generator::heightmap::HeightMap;
use terrain_generator::utils::image_writer::ImageWriter;

fn main() {
    let width = 1024;
    let height = 1024;

    let image = ImageWriter::new(width, height, false);

    let mut height_map = HeightMap::new()
        .size(width, height)
        .island(0.8)
        .rivers(0.8, 0.03, 0.001, 1);

    height_map
        .fractal_noise
        .set(16, 0.5, 3.0 / width.max(height) as f32, 1.0);

    for seed in 0..10 {
        println!("seed: {}", seed);

        height_map.fractal_noise.seed(seed);

        image.save_png(format!("map{}.png", seed), &height_map.build());
    }
}
