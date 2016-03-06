package com.github.fabioticconi.terrain_generator;

/**
 * Hello world!
 *
 */
public class TerrainGenerator
{
    public static void main(final String[] args)
    {
        float map[][];

        final int width = 600;
        final int height = 600;

        final ImageWriter image = new ImageWriter(width, height, false);

        for (int octaves = 2; octaves < 8; octaves++)
        {
            for (float roughness = 0.1f; roughness < 1f; roughness = roughness + 0.1f)
            {
                for (float scale = 0.005f; scale < 0.006f; scale = scale + 0.001f)
                {
                    map = SimplexNoise.generateOctavedSimplexNoise(width, height, octaves, roughness, scale);

                    System.out.print(octaves + " ");
                    final String filename = String.format("images/octaves%d_roughness%.1f_scale%.3f.png", octaves,
                                                          roughness, scale);
                    image.savePng(filename, map);
                }
            }

            // map = SimplexNoise.generateOctavedSimplexNoise(width, height, octaves, 0.5f, 0.005f);
            //
            // System.out.print(octaves + " ");
            // image.savePng(String.format("images/octaves%d_roughness%.1f.png", octaves, 0.5f), map);

            // map = generateOctavedSimplexNoise(width, height, octaves);
            //
            // System.out.print(octaves + " ");
            // image.savePng(String.format("images/octaves%d.png", octaves), map);
        }
    }
}
