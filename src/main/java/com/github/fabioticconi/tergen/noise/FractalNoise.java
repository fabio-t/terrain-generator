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

package com.github.fabioticconi.tergen.noise;

/**
 * Author: Fabio Ticconi
 * Date: 23/02/18
 */
public class FractalNoise
{
    private int seed;

    private int   octaves;
    private float frequency;
    private float roughness;
    private float amplitude;
    private float lacunarity;

    public FractalNoise()
    {
        this.octaves = 7;
        this.frequency = 0.007f;
        this.roughness = 0.5f;

        this.seed = 5;
    }

    public FractalNoise seed(final int seed)
    {
        this.seed = seed;

        return this;
    }

    public int getSeed()
    {
        return seed;
    }

    public FractalNoise set(final int octaves, final float roughness, final float frequency, final float amplitude)
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

    public FractalNoise set(final int octaves, final float roughness, final float frequency, final float amplitude, final float lacunarity)
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

    // TODO: make a version that keeps a noise instance saved, and can be queried for new noise values?
    public float[][] build(final int width, final int height)
    {
        final OpenSimplexNoise noise = new OpenSimplexNoise(seed);

        final float[][] totalNoise = new float[width][height];

        float tempAmplitude = amplitude;
        float tempFrequency = frequency;
        // float weightSum = 0f;

        float maxV = Float.MIN_VALUE;
        float minV = Float.MAX_VALUE;

        for (int octave = 0; octave < octaves; octave++)
        {
            // Calculate single layer/octave of simplex noise, then add it to
            // total noise
            for (int x = 0; x < width; x++)
            {
                for (int y = 0; y < height; y++)
                {
                    totalNoise[x][y] += (float) noise.eval(x * tempFrequency, y * tempFrequency) * tempAmplitude;

                    if (octave == octaves - 1)
                    {
                        if (totalNoise[x][y] > maxV)
                            maxV = totalNoise[x][y];
                        else if (totalNoise[x][y] < minV)
                            minV = totalNoise[x][y];
                    }
                }
            }

            // Increase variables with each incrementing octave
            tempFrequency *= lacunarity;
            // weightSum += tempAmplitude;
            tempAmplitude *= roughness;
        }

        for (int x = 0; x < width; x++)
        {
            for (int y = 0; y < height; y++)
            {
                // totalNoise[x][y] /= weightSum;

                totalNoise[x][y] = (((totalNoise[x][y] - minV) * 2f) / (maxV - minV)) -1f;
                // System.out.println(totalNoise[x][y]);
            }
        }

        return totalNoise;
    }
}
