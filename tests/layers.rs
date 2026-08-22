//! Multi-layer file output: one generation, one file per data layer.

use std::str::FromStr;
use terrain_generator::{Layer, Terrain};

fn decode(path: &std::path::Path) -> (png::OutputInfo, Vec<u8>) {
    let file = std::io::BufReader::new(std::fs::File::open(path).unwrap());
    let decoder = png::Decoder::new(file);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    (info, buf)
}

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tergen-layers-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn layer_names_round_trip() {
    for layer in Layer::ALL {
        assert_eq!(Layer::from_str(layer.name()).unwrap(), layer);
    }
    assert!(Layer::from_str("nope").is_err());
}

#[test]
fn height_layer_is_16_bit_and_preserves_elevation() {
    let dir = tmp_dir("height");
    let t = Terrain::builder().size(64, 64).seed(2).generate();
    let path = dir.join("h.png");
    t.save_layer_png(Layer::Height, &path).unwrap();

    let (info, buf) = decode(&path);
    assert_eq!(info.bit_depth, png::BitDepth::Sixteen);
    assert_eq!(info.color_type, png::ColorType::Grayscale);
    assert_eq!(info.width, 64);
    assert_eq!(info.height, 64);

    // 16 bits quantise to ~1.5e-5; 8 bits would lose 100x more, which is
    // the whole reason this layer is not written as an ordinary greyscale
    for (i, chunk) in buf.chunks_exact(2).enumerate() {
        let stored = u16::from_be_bytes([chunk[0], chunk[1]]) as f32 / 65535.0;
        let original = t.heightmap.as_slice()[i];
        assert!(
            (stored - original).abs() < 2e-5,
            "cell {i}: {stored} vs {original}"
        );
    }
}

#[test]
fn mask_layers_are_black_and_white_and_match_the_terrain() {
    let dir = tmp_dir("masks");
    let t = Terrain::builder().size(64, 64).seed(2).generate();

    for (layer, mask) in [
        (Layer::Rivers, &t.rivers),
        (Layer::Lakes, &t.lakes),
        (Layer::Ocean, &t.ocean),
    ] {
        let path = dir.join(format!("{layer}.png"));
        t.save_layer_png(layer, &path).unwrap();

        let (info, buf) = decode(&path);
        assert_eq!(info.bit_depth, png::BitDepth::Eight, "{layer}");
        assert_eq!(info.color_type, png::ColorType::Grayscale, "{layer}");
        assert_eq!(buf.len(), mask.len(), "{layer}");

        for (i, byte) in buf.iter().enumerate() {
            let expected = if mask.as_slice()[i] == 0 { 0u8 } else { 255 };
            assert_eq!(*byte, expected, "{layer} cell {i}");
        }
    }
}

#[test]
fn every_layer_writes_a_readable_file() {
    let dir = tmp_dir("all");
    let t = Terrain::builder().size(32, 32).seed(4).generate();
    for layer in Layer::ALL {
        let path = dir.join(format!("{layer}.png"));
        t.save_layer_png(layer, &path).unwrap();
        let (info, _) = decode(&path);
        assert_eq!((info.width, info.height), (32, 32), "{layer}");
    }
}

#[test]
fn layers_come_from_one_generation() {
    // writing several layers must not regenerate (and so must not risk
    // disagreeing between files)
    let dir = tmp_dir("consistent");
    let t = Terrain::builder().size(48, 48).seed(6).generate();
    t.save_layer_png(Layer::Ocean, dir.join("o.png")).unwrap();
    t.save_layer_png(Layer::Rivers, dir.join("r.png")).unwrap();

    let (_, ocean) = decode(&dir.join("o.png"));
    let (_, rivers) = decode(&dir.join("r.png"));
    for i in 0..ocean.len() {
        assert!(
            !(ocean[i] == 255 && rivers[i] == 255),
            "cell {i} is both ocean and river"
        );
    }
}
