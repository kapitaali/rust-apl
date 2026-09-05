use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕PNG B — PNG image I/O.
///
/// If B is a string (filename), read PNG and return [width height pixels].
/// If B is a rank-3 array (height×width×channels), write PNG to file.
#[cfg(feature = "plugin-png")]
pub fn quad_png(b: &ValueP) -> AplResult<ValueP> {
    use image::{DynamicImage, ImageBuffer, Rgba};

    let cells = b.cells();
    if cells.is_empty() {
        return Err(ErrorCode::DomainError);
    }

    // Check if first cell is a Char (read mode) or Int (write mode)
    match cells[0] {
        crate::cell::Cell::Char(_) => {
            // Read mode: B is a filename string
            let filename: String = cells
                .iter()
                .filter_map(|c| match c {
                    crate::cell::Cell::Char(ch) => char::from_u32(*ch),
                    _ => None,
                })
                .collect();
            let img = image::open(&filename).map_err(|_| ErrorCode::DomainError)?;
            let w = img.width();
            let h = img.height();
            let pixels = img.to_rgba8();
            let shape: Vec<i64> = vec![h as i64, w as i64, 4];
            let data: Vec<i64> = pixels.iter().map(|p| *p as i64).collect();
            Ok(ValueP::from_parts(
                crate::shape::Shape::from_dims(&shape)?,
                crate::value::ValueP::int_vector(&data).cells().to_vec(),
            )
            .unwrap_or_else(|_| ValueP::int_vector(&data)))
        }
        crate::cell::Cell::Int(_) => {
            // Write mode: expects [height width channels] data
            let shape = b.shape();
            if shape.get_rank() != 3 {
                return Err(ErrorCode::DomainError);
            }
            let h = shape.get_shape_item(0) as u32;
            let w = shape.get_shape_item(1) as u32;
            let pixels: Vec<u8> = cells
                .iter()
                .filter_map(|c| match c {
                    crate::cell::Cell::Int(i) => Some(*i as u8),
                    _ => None,
                })
                .collect();
            let img =
                ImageBuffer::<Rgba<u8>, _>::from_raw(w, h, pixels).ok_or(ErrorCode::DomainError)?;
            let _ = DynamicImage::ImageRgba8(img);
            Ok(ValueP::char_vector(
                &"ok".chars().map(|c| c as u32).collect::<Vec<_>>(),
            ))
        }
        _ => Err(ErrorCode::DomainError),
    }
}

/// ⎕PNG B — disabled version.
#[cfg(not(feature = "plugin-png"))]
pub fn quad_png(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}

/// ⎕PNG plugin struct for registration.
#[cfg(feature = "plugin-png")]
pub struct PngPlugin;

#[cfg(feature = "plugin-png")]
impl PngPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "plugin-png")]
impl crate::plugin_system::AplPlugin for PngPlugin {
    fn info(&self) -> crate::plugin_system::PluginInfo {
        crate::plugin_system::PluginInfo {
            name: "png".into(),
            version: "0.1.0".into(),
            description: "PNG image I/O (⎕PNG)".into(),
        }
    }

    fn register(&self, reg: &mut crate::plugin_system::PluginRegistrar) -> AplResult<()> {
        reg.sysvars.insert(
            "⎕PNG".into(),
            ValueP::char_vector(
                &"png v0.1.0 (image crate)"
                    .chars()
                    .map(|c| c as u32)
                    .collect::<Vec<_>>(),
            ),
        );
        Ok(())
    }
}

#[cfg(all(test, feature = "plugin-png"))]
mod tests {
    use super::*;

    #[test]
    fn test_png_disabled_read() {
        let v = ValueP::char_vector(&"test.png".chars().map(|c| c as u32).collect::<Vec<_>>());
        assert!(quad_png(&v).is_err());
    }
}
