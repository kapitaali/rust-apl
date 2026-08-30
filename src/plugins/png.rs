use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕PNG B — PNG image I/O.
///
/// If B is a string (filename), read PNG and return [width height pixels].
/// If B is a rank-3 array (height×width×channels), write PNG to file.
#[cfg(feature = "plugin-png")]
pub fn quad_png(b: &ValueP) -> AplResult<ValueP> {
    use image::{DynamicImage, ImageBuffer, Rgba};

    match b {
        ValueP::Char(s) => {
            let filename: String = s.iter().map(|c| char::from_u32(*c).unwrap()).collect();
            let img = image::open(&filename).map_err(|_| ErrorCode::DomainError)?;
            let (w, h) = img.dimensions();
            let pixels = img.to_rgba8();
            let shape = vec![h as usize, w as usize, 4];
            let data: Vec<i64> = pixels.iter().map(|p| *p as i64).collect();
            Ok(ValueP::array(shape, ValueP::int_vector(&data)))
        }
        ValueP::Int(data) => {
            // Write mode: expects [height width channels] data
            let shape = b.shape();
            if shape.len() != 3 || shape[2] != 4 {
                return Err(ErrorCode::DomainError);
            }
            let h = shape[0] as u32;
            let w = shape[1] as u32;
            let pixels: Vec<u8> = data.iter().map(|p| *p as u8).collect();
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

#[cfg(all(test, feature = "plugin-png"))]
mod tests {
    use super::*;

    #[test]
    fn test_png_disabled_read() {
        let v = ValueP::char_vector(&"test.png".chars().map(|c| c as u32).collect::<Vec<_>>());
        assert!(quad_png(&v).is_err());
    }
}
