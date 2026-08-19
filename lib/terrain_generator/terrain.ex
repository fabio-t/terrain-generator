defmodule TerrainGenerator.Terrain do
  @moduledoc """
  A generated terrain.

  All per-cell buffers are row-major binaries indexed as `y * width + x`:

    * `:heightmap` — 32-bit little-endian floats, elevation in `[0, 1]`
    * `:flow` — 32-bit little-endian floats, log-normalised flow
      accumulation in `[0, 1]` (a useful moisture proxy)
    * `:rivers` — one byte per cell, `1` where a river runs
    * `:lakes` — one byte per cell, `1` where a lake holds standing water

  Binaries are cheap to pass around (they live outside the BEAM heap) and
  can be consumed wholesale — e.g. uploaded to a GPU or sent over a socket
  — or queried per cell with the accessors below.
  """

  @enforce_keys [:width, :height, :sea_level, :heightmap, :flow, :rivers, :lakes]
  defstruct [:width, :height, :sea_level, :heightmap, :flow, :rivers, :lakes]

  @type t :: %__MODULE__{
          width: pos_integer(),
          height: pos_integer(),
          sea_level: float(),
          heightmap: binary(),
          flow: binary(),
          rivers: binary(),
          lakes: binary()
        }

  @doc """
  Elevation at `{x, y}`, in `[0, 1]`.
  """
  @spec height_at(t(), non_neg_integer(), non_neg_integer()) :: float()
  def height_at(%__MODULE__{} = t, x, y) do
    <<v::float-32-little>> = binary_part(t.heightmap, (y * t.width + x) * 4, 4)
    v
  end

  @doc """
  Log-normalised flow accumulation at `{x, y}`, in `[0, 1]`.
  """
  @spec flow_at(t(), non_neg_integer(), non_neg_integer()) :: float()
  def flow_at(%__MODULE__{} = t, x, y) do
    <<v::float-32-little>> = binary_part(t.flow, (y * t.width + x) * 4, 4)
    v
  end

  @doc """
  Whether a river runs through `{x, y}`.
  """
  @spec river?(t(), non_neg_integer(), non_neg_integer()) :: boolean()
  def river?(%__MODULE__{} = t, x, y) do
    binary_part(t.rivers, y * t.width + x, 1) == <<1>>
  end

  @doc """
  Whether `{x, y}` lies in a lake.
  """
  @spec lake?(t(), non_neg_integer(), non_neg_integer()) :: boolean()
  def lake?(%__MODULE__{} = t, x, y) do
    binary_part(t.lakes, y * t.width + x, 1) == <<1>>
  end

  @doc """
  Whether `{x, y}` is covered by water: at or below sea level, in a lake,
  or on a river.
  """
  @spec water?(t(), non_neg_integer(), non_neg_integer()) :: boolean()
  def water?(%__MODULE__{} = t, x, y) do
    height_at(t, x, y) <= t.sea_level or lake?(t, x, y) or river?(t, x, y)
  end

  @doc """
  The heightmap as a flat list of floats, row by row.

  Convenient for exploration, but for large maps prefer working with the
  binary directly.
  """
  @spec heightmap_list(t()) :: [float()]
  def heightmap_list(%__MODULE__{} = t) do
    for <<v::float-32-little <- t.heightmap>>, do: v
  end
end
