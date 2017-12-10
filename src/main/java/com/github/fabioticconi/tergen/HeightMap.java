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

/**
 * Author: Fabio Ticconi
 * Date: 03/12/17
 */
public class HeightMap
{
    private int width;
    private int height;
    private int seed;

    private boolean islandShape;
    private float   islandFraction;

    private int   octaves;
    private float frequency;
    private float roughness;
    private float amplitude;
    private float lacunarity;

    public HeightMap()
    {
        this.width = 100;
        this.height = 100;
        this.seed = 5;

        this.islandShape = false;

        this.octaves = 7;
        this.frequency = 0.007f;
        this.roughness = 0.5f;
    }

    public HeightMap size(final int width, final int height, final int seed)
    {
        if (width < 0 || height < 0)
            throw new IllegalArgumentException("width and height must be positive");

        this.width = width;
        this.height = height;
        this.seed = seed;

        return this;
    }

    public HeightMap noise(final int octaves, final float roughness, final float frequency, final float amplitude)
    {
        if (octaves <= 0 || roughness <= 0f || frequency <= 0f || amplitude <= 0f)
            throw new IllegalArgumentException("all parameters must be positive");

        this.octaves = octaves;
        this.roughness = roughness;
        this.frequency = frequency;
        this.amplitude = amplitude;
        this.lacunarity = 1f / roughness;

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

    public float[][] build()
    {
        final float[][] heightmap;

        heightmap = OpenSimplexNoise.generateOctavedSimplexNoise(
            new OpenSimplexNoise(seed), width, height, octaves, roughness, frequency, amplitude, lacunarity);

        for (int x = 0; x < width; x++)
        {
            for (int y = 0; y < height; y++)
            {
                heightmap[x][y] = 1f-Math.abs(heightmap[x][y]);
                // heightmap[x][y] = Math.abs(heightmap[x][y]);

                // heightmap[x][y] = 0.5f*(heightmap[x][y] + 1f);
                // heightmap[x][y] = 0.5f*(1f-heightmap[x][y]);
            }
        }

        if (islandShape)
        {
            final float[][] heightMod = OpenSimplexNoise.generateOctavedSimplexNoise(
                new OpenSimplexNoise(seed+10), width, height, octaves, roughness, frequency, amplitude, lacunarity);

            final float islandSizeX = width * islandFraction;
            final float islandSizeY = height * islandFraction;

            final float  max = Math.max(islandSizeX, islandSizeY) * 0.5f;

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

                    heightmap[x][y] = (float) Math.max(0.0, heightmap[x][y] * (1d-gradient));
                }
            }
        }

        return heightmap;
    }
}
