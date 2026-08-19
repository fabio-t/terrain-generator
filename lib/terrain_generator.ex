defmodule TerrainGenerator do
  @moduledoc """
  Procedural terrain generation with fractal noise, hydraulic erosion and
  river extraction, backed by a Rust NIF.

  The pipeline has four stages, each optional after the first:

    1. **Base heightmap** — fractal (fBm) OpenSimplex noise
    2. **Island mask** — radial falloff with noise-perturbed coastlines
    3. **Hydraulic erosion** — a droplet simulation that carves valleys and
       drainage networks
    4. **Hydrology** — flow routing over the eroded terrain; high-flow
       cells become rivers, filled depressions become lakes

  Generation is fully deterministic for a given seed and option set, and
  runs on a dirty CPU scheduler so it never blocks the BEAM.

  ## Examples

      {:ok, terrain} = TerrainGenerator.generate(width: 512, height: 512, seed: 42)
      TerrainGenerator.Terrain.height_at(terrain, 100, 100)
      #=> 0.53

      # render straight to a PNG file without keeping the data:
      :ok = TerrainGenerator.render_png("map.png", seed: 42)

  ## Options

  All functions take the same option list; omitted keys use the defaults.

    * `:width`, `:height` — map size in cells (default `512 × 512`)
    * `:seed` — generation seed (default `0`)
    * `:sea_level` — sea level in `[0, 1)` (default `0.08`)
    * `:octaves` — noise octaves (default `8`)
    * `:frequency` — number of large features across the map (default `2.5`)
    * `:lacunarity` — frequency multiplier between octaves (default `2.0`)
    * `:persistence` — amplitude multiplier between octaves (default `0.5`)
    * `:island` — island diameter as a fraction of the shorter map
      dimension, or `false` to let terrain reach the borders
      (default `0.9`)
    * `:coast_noise` — coastline perturbation strength (default `0.35`)
    * `:erosion` — `true`/`false`, or a droplet count to override the
      default of one droplet per cell (default `true`)
    * `:rivers` — approximate fraction of land covered by rivers, or
      `false` to disable rivers and lakes (default `0.01`)
  """

  alias TerrainGenerator.{Native, Terrain}

  @defaults %{
    width: 512,
    height: 512,
    seed: 0,
    sea_level: 0.08,
    octaves: 8,
    frequency: 2.5,
    lacunarity: 2.0,
    persistence: 0.5,
    island: 0.9,
    coast_noise: 0.35,
    erosion: true,
    rivers: 0.01
  }

  @type option ::
          {:width, pos_integer()}
          | {:height, pos_integer()}
          | {:seed, non_neg_integer()}
          | {:sea_level, float()}
          | {:octaves, pos_integer()}
          | {:frequency, number()}
          | {:lacunarity, number()}
          | {:persistence, number()}
          | {:island, number() | false}
          | {:coast_noise, number()}
          | {:erosion, boolean() | pos_integer()}
          | {:rivers, number() | false}

  @doc """
  Generates a terrain and returns `{:ok, %TerrainGenerator.Terrain{}}`.

  Raises `ArgumentError` on unknown options.

      {:ok, terrain} = TerrainGenerator.generate(seed: 7, width: 256, height: 256)
      terrain.width
      #=> 256
  """
  @spec generate([option()]) :: {:ok, Terrain.t()}
  def generate(opts \\ []) do
    native_opts = native_options(opts)

    {:ok, {width, height, heightmap, flow, rivers, lakes}} = Native.generate(native_opts)

    {:ok,
     %Terrain{
       width: width,
       height: height,
       sea_level: native_opts.sea_level,
       heightmap: heightmap,
       flow: flow,
       rivers: rivers,
       lakes: lakes
     }}
  end

  @doc """
  Generates a terrain and writes it to `path` as a PNG file.

  `style` is `:shaded` (biome colours with hillshading, the default) or
  `:greyscale` (plain heightmap).

  Returns `:ok`, or `{:error, reason}` if the file cannot be written.

      :ok = TerrainGenerator.render_png("map.png", seed: 3)
      :ok = TerrainGenerator.render_png("height.png", [seed: 3], :greyscale)
  """
  @spec render_png(Path.t(), [option()], :shaded | :greyscale) :: :ok | {:error, String.t()}
  def render_png(path, opts \\ [], style \\ :shaded)
      when style in [:shaded, :greyscale] do
    case Native.render_png(native_options(opts), to_string(path), style) do
      {:ok, _} -> :ok
      {:error, reason} -> {:error, reason}
    end
  end

  defp native_options(opts) do
    opts = Map.new(opts)

    case Map.keys(opts) -- Map.keys(@defaults) do
      [] -> :ok
      unknown -> raise ArgumentError, "unknown options: #{inspect(unknown)}"
    end

    merged = Map.merge(@defaults, opts)

    %{
      width: merged.width,
      height: merged.height,
      seed: merged.seed,
      sea_level: to_float(merged.sea_level),
      octaves: merged.octaves,
      frequency: to_float(merged.frequency),
      lacunarity: to_float(merged.lacunarity),
      persistence: to_float(merged.persistence),
      island: if(merged.island == false, do: 0.0, else: to_float(merged.island)),
      coast_noise: to_float(merged.coast_noise),
      droplets:
        case merged.erosion do
          true -> -1
          false -> 0
          n when is_integer(n) and n > 0 -> n
        end,
      rivers: if(merged.rivers == false, do: 0.0, else: to_float(merged.rivers))
    }
  end

  defp to_float(v) when is_integer(v), do: v * 1.0
  defp to_float(v) when is_float(v), do: v
end
