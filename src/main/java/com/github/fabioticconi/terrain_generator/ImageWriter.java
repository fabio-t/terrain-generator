/**
 * Copyright 2015 Fabio Ticconi
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
package com.github.fabioticconi.terrain_generator;

import java.awt.Color;
import java.awt.image.BufferedImage;
import java.io.File;
import java.io.IOException;

import javax.imageio.ImageIO;

/**
 *
 * @author Fabio Ticconi
 */
public class ImageWriter
{
    boolean grey;

    int width;
    int height;

    final BufferedImage image;

    public ImageWriter(final int width, final int height, final boolean grey)
    {
        this.width = width;
        this.height = height;

        this.grey = grey;

        image = new BufferedImage(width, height, BufferedImage.TYPE_INT_RGB);
    }

    Color getColor(final float t)
    {
        final float water = 0.3f;

        if (t < water)
        {
            if (t < water * 0.7f)
                return new Color(0.2f, 0.5f, 0.9f);
            else
                return new Color(0.4f, 0.7f, 1f);
        }
        else
        {
            // final float val = t;
            // normalize val so 0 is at water level
            final float val = (t - water) / (1.0f - water);

            // set color based on above the see level
            // beach, plain, forest, mountains etc

            if (val < 0.1f)
                return Color.YELLOW;
            else if (val < 0.3f)
                return Color.GREEN;
            else if (val < 0.55f)
                return new Color(.1f, 0.8f, .2f);
            else if (val < 0.8f)
                return Color.GRAY;
            else
                return Color.WHITE;
        }
    }

    public void savePng(final String filename, final float[][] map)
    {
        // this takes an array of float between -1 and 1 and generates a grey scale image from them

        float min = Float.MAX_VALUE;
        float max = -Float.MAX_VALUE;
        float value;
        for (int y = 0; y < height; y++)
        {
            for (int x = 0; x < width; x++)
            {
                if (map[x][y] < min)
                {
                    min = map[x][y];
                }

                if (map[x][y] > max)
                {
                    max = map[x][y];
                }

                value = (1f + map[x][y]) / 2f;
                // value = 0.5f + map[x][y];

                if (value > 1f)
                {
                    // System.out.println("error: " + value);
                    value = 1f;
                }
                if (value < 0f)
                {
                    // System.out.println("error: " + value);
                    value = 0f;
                }

                Color col;

                if (grey)
                {
                    col = new Color(value, value, value);
                }
                else
                {
                    col = getColor(value);
                }

                image.setRGB(x, y, col.getRGB());
            }
        }

        System.out.println(min + " " + max);

        try
        {
            // retrieve image
            final File outputfile = new File(filename);
            outputfile.createNewFile();

            ImageIO.write(image, "png", outputfile);
        } catch (final IOException e)
        {
            throw new RuntimeException("I didn't handle this very well");
        }
    }
}
