/*
 * Copyright (c) 2017 Fabio Ticconi
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

package com.github.fabioticconi.terrain_generator;

import com.github.fabioticconi.terrain_generator.noise.SimplexNoise;

/**
 *
 */
public class Main
{
    public static void main(final String[] args)
    {
        float map[][];

        final int width  = 1024;
        final int height = 1024;

        // final OpenSimplexNoise noise = new OpenSimplexNoise();

        final ImageWriter image = new ImageWriter(width, height, false);

        for (int octaves = 2; octaves < 9; octaves++)
        {
            for (float roughness = 0.1f; roughness <= 1f; roughness = roughness + 0.1f)
            // final float roughness = 0.5f;
            {
                for (float frequency = 0.005f; frequency < 0.01f; frequency = frequency + 0.001f)
                // final float frequency = 0.007f;
                {
                    map = SimplexNoise.generateOctavedSimplexNoise(width, height, octaves, roughness, frequency);
                    // map = OpenSimplexNoise.generateOctavedSimplexNoise(noise,
                    // width, height, octaves, roughness,
                    // frequency);

                    final String filename = String.format("images/octaves%d_roughness%.1f_frequency%.4f.png",
                                                          octaves,
                                                          roughness,
                                                          frequency);
                    image.savePng(filename, map);
                }
            }
        }
    }
}
