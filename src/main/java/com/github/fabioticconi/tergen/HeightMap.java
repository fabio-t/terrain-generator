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

import com.github.fabioticconi.tergen.noise.FractalNoise;

import java.util.ArrayList;
import java.util.Random;

/**
 * Author: Fabio Ticconi
 * Date: 03/12/17
 */
public class HeightMap
{
    private int width;
    private int height;

    private float islandFraction;

    private float sourceThreshold;
    private float riverThreshold;
    private float riverFraction;

    final public FractalNoise fractalNoise;

    public HeightMap()
    {
        this.width = 100;
        this.height = 100;

        this.islandFraction = 0.5f;

        this.sourceThreshold = 0f;
        this.riverThreshold = 0f;
        this.riverFraction = 0f;

        this.fractalNoise = new FractalNoise();
    }

    public HeightMap size(final int width, final int height)
    {
        if (width < 0 || height < 0)
            throw new IllegalArgumentException("width and height must be positive");

        this.width = width;
        this.height = height;

        return this;
    }

    public HeightMap island(final float islandFraction)
    {
        if (islandFraction <= 0f || islandFraction >= 1f)
            throw new IllegalArgumentException("island fraction must be in range (0,1), not included");

        this.islandFraction = islandFraction;

        return this;
    }

    public HeightMap rivers(final float sourceThreshold, final float riverThreshold, final float riverFraction)
    {
        if (sourceThreshold <= 0f || sourceThreshold > 1f)
            throw new IllegalArgumentException("source threshold must be in range (0,1]");

        if (riverThreshold < 0f || riverThreshold >= 1f)
            throw new IllegalArgumentException("river threshold must be in range [0,1)");

        if (riverThreshold >= sourceThreshold)
            throw new IllegalArgumentException("river threshold must be strictly less than the sourceThreshold");

        if (riverFraction <= 0f || riverFraction > 1f)
            throw new IllegalArgumentException("river fraction must be in range (0,1]");

        this.sourceThreshold = sourceThreshold;
        this.riverThreshold = riverThreshold;
        this.riverFraction = riverFraction;

        return this;
    }

    public float[][] build()
    {
        final float[][] heightmap;

        heightmap = fractalNoise.build(width, height);

        for (int x = 0; x < width; x++)
        {
            for (int y = 0; y < height; y++)
            {
                heightmap[x][y] = 1f - Math.abs(heightmap[x][y]);
                // heightmap[x][y] = Math.abs(heightmap[x][y]);

                // heightmap[x][y] = 0.5f*(heightmap[x][y] + 1f);
                // heightmap[x][y] = 0.5f*(1f-heightmap[x][y]);
            }
        }

        if (islandFraction > 0f)
        {
            // we make a new "noise map" that we will use to make noisy coasts of this island
            final int seed = fractalNoise.getSeed();
            fractalNoise.seed(seed < Integer.MAX_VALUE ? seed + 1 : seed - 1);
            final float[][] heightMod = fractalNoise.build(width, height);
            fractalNoise.seed(seed); // must put it back

            final float islandSizeX = width * islandFraction;
            final float islandSizeY = height * islandFraction;

            final float max = Math.max(islandSizeX, islandSizeY) * 0.5f;

            for (int x = 0; x < width; x++)
            {
                for (int y = 0; y < height; y++)
                {
                    final float  distX = Math.abs(x - width * 0.5f);
                    final float  distY = Math.abs(y - height * 0.5f);
                    final double dist  = Math.sqrt(distX * distX + distY * distY);

                    final double delta = dist / max;

                    // circular gradient
                    // final double gradient = delta * delta;

                    // hackish noise gradient
                    final double gradient = Math.pow(delta, 3) + Math.abs(heightMod[x][y]);

                    heightmap[x][y] = (float) Math.max(0.0, heightmap[x][y] * (1d - gradient));
                }
            }
        }

        if (riverFraction > 0f)
        {
            // let's make some rivers

            // simple approach: we go through the whole map, and every cell with height > sourceThreshold
            // gets a chance (equal to riverFraction) of becoming the source of a river.

            final Random r = new Random(fractalNoise.getSeed());

            for (int x = 0; x < width; x++)
            {
                for (int y = 0; y < height; y++)
                {
                    if (heightmap[x][y] >= sourceThreshold && r.nextFloat() < riverFraction)
                    {
                        makeRiver(heightmap, x, y, -1);

                        // return heightmap; // FIXME remove this later
                    }
                }
            }
        }

        return heightmap;
    }

    private void makeRiver(final float heightmap[][], final int x, final int y, final int skipNeighbour)
    {
        // the "source" cannot be lower than the "river depth"
        if (heightmap[x][y] <= riverThreshold)
            return;

        int newX = -1;
        int newY = -1;
        float minHeight = Float.MAX_VALUE;
        // float minHeight = heightmap[x][y];

        // System.out.println("source: " + x + " " + y + " " + heightmap[x][y] + " (skip: " + skipNeighbour + ")");

        // (the erosion of heightmap[x][y] to river depth will always happen here,
        // and the function will be called recursively on a chosen neighbour)
        heightmap[x][y] = riverThreshold;

        final int xmin = Math.max(x - 1, 0);
        final int xmax = Math.min(x + 1, width - 1);

        int chosen = -1;
        int neighbour = 0;
        int i = -1 - Math.min(x - 1, 0);
        for (int x_t = xmin; x_t <= xmax; x_t++, i++)
        {
            final int z = 1 - Math.abs(i);

            final int y1 = y + z;

            if (y1 < height && neighbour != skipNeighbour)
            {
                System.out.println(x_t + " " + y1 + " " + heightmap[x_t][y1]);

                if (heightmap[x_t][y1] > riverThreshold && heightmap[x_t][y1] < minHeight)
                {
                    newX = x_t;
                    newY = y1;
                    minHeight = heightmap[x_t][y1];
                    chosen = neighbour;
                }
            }

            neighbour++;

            final int y2 = y - z;

            if (y1 == y2 || y2 < 0)
            {
                continue;
            }

            System.out.println(x_t + " " + y2 + " " + heightmap[x_t][y2]);

            if (neighbour != skipNeighbour &&
                heightmap[x_t][y2] > riverThreshold &&
                heightmap[x_t][y2] < minHeight)
            {
                newX = x_t;
                newY = y2;
                minHeight = heightmap[x_t][y2];
                chosen = neighbour;
            }

            neighbour++;
        }

        // System.out.println("chosen: " + newX + " " + newY + " " + minHeight + ", #" + chosen);

        // we continue recursively
        if (newX >= 0)
            makeRiver(heightmap, newX, newY, chosen >= 0 ? 3-chosen : chosen);
    }
}
