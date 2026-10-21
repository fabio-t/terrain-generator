defmodule TerrainGenerator.MixProject do
  use Mix.Project

  @version "0.3.1"
  @source_url "https://github.com/fabio-t/terrain-generator"

  def project do
    [
      app: :terrain_generator,
      version: @version,
      elixir: "~> 1.15",
      start_permanent: Mix.env() == :prod,
      deps: deps(),
      description:
        "Procedural terrain generator: fractal noise, hydraulic erosion and rivers (Rust NIF)",
      package: package(),
      docs: docs()
    ]
  end

  def application do
    [
      extra_applications: [:logger]
    ]
  end

  defp deps do
    [
      {:rustler, "~> 0.38"},
      {:ex_doc, "~> 0.40", only: :dev, runtime: false}
    ]
  end

  defp package do
    [
      licenses: ["Apache-2.0"],
      links: %{"GitHub" => @source_url},
      # Cargo builds a crate only when every member of its workspace is
      # present, so this lists all of them: the NIF, the core library and
      # the C ABI crate. CI builds the NIF from the unpacked package to
      # keep this list honest. The GUI is a workspace of its own and not
      # needed.
      files: [
        "lib",
        "native/terrain_generator_nif/src",
        "native/terrain_generator_nif/Cargo.toml",
        "native/terrain_generator_ffi/src",
        "native/terrain_generator_ffi/Cargo.toml",
        "src",
        "Cargo.toml",
        "Cargo.lock",
        "mix.exs",
        "README.md",
        "LICENSE"
      ]
    ]
  end

  defp docs do
    [
      main: "TerrainGenerator",
      source_url: @source_url,
      extras: ["docs/elixir_api.md"]
    ]
  end
end
