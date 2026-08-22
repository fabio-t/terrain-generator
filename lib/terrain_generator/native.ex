defmodule TerrainGenerator.Native do
  @moduledoc false
  # Raw NIF stubs; the real implementations live in
  # native/terrain_generator_nif. Use TerrainGenerator instead of calling
  # this module directly.

  use Rustler, otp_app: :terrain_generator, crate: "terrain_generator_nif"

  def generate(_opts), do: :erlang.nif_error(:nif_not_loaded)
  def render_png(_opts, _path, _style), do: :erlang.nif_error(:nif_not_loaded)
end
