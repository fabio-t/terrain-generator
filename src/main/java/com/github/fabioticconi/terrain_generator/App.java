package com.github.fabioticconi.terrain_generator;

/**
 * Hello world!
 *
 */
public class App
{
    public static void main(final String[] args)
    {
        float map[][];

        final int width = 1600;
        final int height = 1050;

        final ImageWriter image = new ImageWriter(width, height, false);

        for (int octaves = 5; octaves < 8; octaves++)
        {
            for (float roughness = 0.1f; roughness < 1f; roughness = roughness + 0.1f)
            {
                // map = generateOctavedSimplexNoise(1000, 1000, octaves, roughness);
                map = SimplexNoise.generateOctavedSimplexNoise(width, height, octaves, roughness, 0.003f);

                System.out.print(octaves + " ");
                image.savePng(String.format("images/octaves%d_roughness%.1f.png", octaves, roughness), map);
            }

            // map = generateOctavedSimplexNoise(width, height, octaves);
            //
            // System.out.print(octaves + " ");
            // image.savePng(String.format("images/octaves%d.png", octaves), map);
        }
    }
}
