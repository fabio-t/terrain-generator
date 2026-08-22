defmodule TerrainGeneratorTest do
  use ExUnit.Case, async: true

  alias TerrainGenerator.Terrain

  test "generates a terrain with correctly sized buffers" do
    {:ok, terrain} = TerrainGenerator.generate(width: 64, height: 32, seed: 1)

    assert terrain.width == 64
    assert terrain.height == 32
    assert byte_size(terrain.heightmap) == 64 * 32 * 4
    assert byte_size(terrain.flow) == 64 * 32 * 4
    assert byte_size(terrain.rivers) == 64 * 32
    assert byte_size(terrain.lakes) == 64 * 32
    assert byte_size(terrain.ocean) == 64 * 32
    assert byte_size(terrain.flow_accumulation) == 64 * 32 * 4
  end

  test "ocean is border-connected sea, and lakes are never ocean" do
    {:ok, terrain} = TerrainGenerator.generate(width: 96, height: 96, seed: 3)

    assert Terrain.ocean?(terrain, 0, 0), "map corner should be sea"

    for y <- 0..95, x <- 0..95, Terrain.lake?(terrain, x, y) do
      refute Terrain.ocean?(terrain, x, y)
    end
  end

  test "raw flow accumulation is at least one cell everywhere" do
    {:ok, terrain} = TerrainGenerator.generate(width: 48, height: 48, seed: 4)

    for y <- 0..47, x <- 0..47 do
      assert Terrain.flow_accumulation_at(terrain, x, y) >= 1.0
    end
  end

  test "archipelago mode is accepted and differs from a single island" do
    opts = [width: 96, height: 96, seed: 5]
    {:ok, single} = TerrainGenerator.generate(opts)
    {:ok, many} = TerrainGenerator.generate(opts ++ [archipelago: 4])

    refute single.heightmap == many.heightmap
    assert byte_size(many.heightmap) == 96 * 96 * 4
  end

  test "absolute river threshold and basin breaching are accepted" do
    {:ok, terrain} =
      TerrainGenerator.generate(
        width: 96,
        height: 96,
        seed: 6,
        river_min_upstream: 200,
        breach_basins: 0.002
      )

    assert byte_size(terrain.rivers) == 96 * 96
  end

  test "generation is deterministic for a seed" do
    {:ok, a} = TerrainGenerator.generate(width: 64, height: 64, seed: 7)
    {:ok, b} = TerrainGenerator.generate(width: 64, height: 64, seed: 7)

    assert a.heightmap == b.heightmap
    assert a.rivers == b.rivers
  end

  test "different seeds produce different terrain" do
    {:ok, a} = TerrainGenerator.generate(width: 64, height: 64, seed: 1)
    {:ok, b} = TerrainGenerator.generate(width: 64, height: 64, seed: 2)

    refute a.heightmap == b.heightmap
  end

  test "heights are in the unit range" do
    {:ok, terrain} = TerrainGenerator.generate(width: 64, height: 64, seed: 3)

    for v <- Terrain.heightmap_list(terrain) do
      assert v >= 0.0 and v <= 1.0
    end
  end

  test "island: false and erosion tuning are accepted" do
    {:ok, terrain} =
      TerrainGenerator.generate(
        width: 32,
        height: 32,
        island: false,
        erosion: 1000,
        rivers: false
      )

    assert terrain.rivers == :binary.copy(<<0>>, 32 * 32)
  end

  test "unknown options raise" do
    assert_raise ArgumentError, fn ->
      TerrainGenerator.generate(bogus: 1)
    end
  end

  test "invalid option values raise with descriptive messages" do
    assert_raise ArgumentError, ~r/width/, fn ->
      TerrainGenerator.generate(width: -1)
    end

    assert_raise ArgumentError, ~r/sea_level/, fn ->
      TerrainGenerator.generate(sea_level: 1.5)
    end

    assert_raise ArgumentError, ~r/erosion/, fn ->
      TerrainGenerator.generate(erosion: -3)
    end

    assert_raise ArgumentError, ~r/rivers/, fn ->
      TerrainGenerator.generate(rivers: 2.0)
    end

    assert_raise ArgumentError, ~r/archipelago/, fn ->
      TerrainGenerator.generate(archipelago: 0)
    end

    assert_raise ArgumentError, ~r/breach_basins/, fn ->
      TerrainGenerator.generate(breach_basins: 5.0)
    end
  end

  test "island: true and rivers: true select the defaults" do
    {:ok, terrain} =
      TerrainGenerator.generate(width: 32, height: 32, island: true, rivers: true, erosion: 500)

    assert terrain.width == 32
  end

  @tag :tmp_dir
  test "renders a png", %{tmp_dir: tmp_dir} do
    path = Path.join(tmp_dir, "map.png")
    assert :ok = TerrainGenerator.render_png(path, width: 64, height: 64)
    assert <<0x89, "PNG", _::binary>> = File.read!(path)
  end
end
