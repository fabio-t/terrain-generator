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

package com.github.fabioticconi.tergen;

import com.github.fabioticconi.tergen.noise.SimplexNoise;

/**
 * Author: Fabio Ticconi
 * Date: 03/12/17
 */
public class HeightMap
{
    private int width;
    private int height;

    private boolean islandShape;
    private float   islandFraction;

    private int   octaves;
    private float frequency;
    private float roughness;
    private float amplitude;
    private float lacunarity;

    private boolean bounds;
    private float lowerBound;
    private float higherBound;

    public HeightMap()
    {
        this.width = 100;
        this.height = 100;

        this.islandShape = false;

        this.octaves = 7;
        this.frequency = 0.007f;
        this.roughness = 0.5f;

        this.bounds = false;
    }

    public HeightMap size(final int width, final int height)
    {
        if (width < 0 || height < 0)
            throw new IllegalArgumentException("width and height must be positive");

        this.width = width;
        this.height = height;

        return this;
    }

    public HeightMap noise(final int octaves, final float roughness, final float frequency, final float amplitude, final float lacunarity)
    {
        if (octaves <= 0 || roughness <= 0f || frequency <= 0f || amplitude <= 0f || lacunarity <= 0f)
            throw new IllegalArgumentException("all parameters must be positive");

        this.octaves = octaves;
        this.roughness = roughness;
        this.frequency = frequency;
        this.amplitude = amplitude;
        this.lacunarity = lacunarity;

        return this;
    }

    public HeightMap island(final float islandFraction)
    {
        if (islandFraction <= 0f || islandFraction >= 1f)
            throw new IllegalArgumentException("island fraction must be in range (0,1), not included");

        this.islandShape = true;
        this.islandFraction = islandFraction;

        return this;
    }

    public HeightMap bounds(final float lower, final float higher)
    {
        this.bounds = true;

        this.lowerBound = lower;
        this.higherBound = higher;

        return this;
    }

    public float[][] build()
    {
        final float[][] heightmap;

        heightmap = SimplexNoise.generateOctavedSimplexNoise(width, height, octaves, roughness, frequency, amplitude, lacunarity);

        // if (bounds)
        // {
        //     for (int x = 0; x < width; x++)
        //     {
        //         for (int y = 0; y < height; y++)
        //         {
        //             heightmap[x][y] = (((heightmap[x][y] + 1f) * (higherBound - lowerBound)) / 2f) + lowerBound;
        //         }
        //     }
        // }

        for (int x = 0; x < width; x++)
        {
            for (int y = 0; y < height; y++)
            {
                // heightmap[x][y] = Math.abs(heightmap[x][y]);
                heightmap[x][y] = 0.5f*(heightmap[x][y] + 1f);
            }
        }

        if (islandShape)
        {
            final float islandSizeX = width * islandFraction;
            final float islandSizeY = height * islandFraction;

            for (int x = 0; x < width; x++)
            {
                for (int y = 0; y < height; y++)
                {
                    final float  distX = Math.abs(x - width * 0.5f);
                    final float  distY = Math.abs(y - height * 0.5f);
                    final double dist  = Math.sqrt(distX * distX + distY * distY);

                    final float  max      = Math.max(islandSizeX, islandSizeY) * 0.5f;
                    final double delta    = dist / max;
                    final double gradient = delta * delta;

                    // System.out.println(heightmap[x][y] + " -> " + (heightmap[x][y] * Math.max(0.0f, 1.0f - (float) gradient)) + "(" + gradient +")");

                    heightmap[x][y] *= Math.max(0.0f, 1.0f - (float) gradient);
                }
            }
        }

        return heightmap;
    }
}

