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
