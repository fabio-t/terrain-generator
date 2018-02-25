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

package com.github.fabioticconi.tergen;

import com.github.fabioticconi.tergen.utils.ImageWriter;

/**
 *
 */
public class Main
{
    public static void main(final String[] args)
    {
        final int width  = 1024;
        final int height = 1024;

        final ImageWriter image = new ImageWriter(width, height, false);

        final HeightMap heightMap = new HeightMap()
                                        .size(width, height)
                                        .island(0.8f)
                                        .rivers(0.8f, 0.03f, 0.001f);

        heightMap.fractalNoise
            .set(16, 0.5f, 3f / Math.max(width, height), 1f);

        for (int seed = 0; seed < 10; seed++)
        {
            System.out.println("seed: " + seed);

            heightMap.fractalNoise.seed(seed);

            image.savePng("map" + seed + ".png", heightMap.build());
        }
    }
}
