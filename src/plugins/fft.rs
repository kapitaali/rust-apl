use crate::cell::Cell;
use crate::types::{AplResult, ErrorCode, ShapeItem};
use crate::value::ValueP;

/// ⎕FFT B — Fast Fourier Transform.
#[cfg(feature = "plugin-fft")]
pub fn quad_fft(b: &ValueP) -> AplResult<ValueP> {
    use rustfft::{num_complex::Complex, FftPlanner};

    let cells = b.cells();
    if cells.is_empty() {
        return Err(ErrorCode::DomainError);
    }

    let mut input: Vec<Complex<f64>> = cells
        .iter()
        .map(|c| match c {
            crate::cell::Cell::Int(n) => Ok(Complex::new(*n as f64, 0.0)),
            crate::cell::Cell::Float(f) => Ok(Complex::new(*f, 0.0)),
            _ => Err(ErrorCode::DomainError),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(input.len());
    fft.process(&mut input);

    let shape = crate::shape::Shape::vector(input.len() as ShapeItem * 2);
    let ravel: Vec<crate::cell::Cell> = input
        .iter()
        .flat_map(|c| vec![Cell::Float(c.re), Cell::Float(c.im)])
        .collect();
    ValueP::from_parts(shape, ravel)
}

/// ⎕FFT B — disabled version.
#[cfg(not(feature = "plugin-fft"))]
pub fn quad_fft(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}

/// ⎕FFT plugin struct for registration.
#[cfg(feature = "plugin-fft")]
pub struct FftPlugin;

#[cfg(feature = "plugin-fft")]
impl FftPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "plugin-fft")]
impl crate::plugin_system::AplPlugin for FftPlugin {
    fn info(&self) -> crate::plugin_system::PluginInfo {
        crate::plugin_system::PluginInfo {
            name: "fft".into(),
            version: "0.1.0".into(),
            description: "Fast Fourier Transform (⎕FFT)".into(),
        }
    }

    fn register(&self, reg: &mut crate::plugin_system::PluginRegistrar) -> AplResult<()> {
        reg.sysvars.insert(
            "⎕FFT".into(),
            ValueP::char_vector(
                &"fft v0.1.0 (rustfft)"
                    .chars()
                    .map(|c| c as u32)
                    .collect::<Vec<_>>(),
            ),
        );
        Ok(())
    }
}

#[cfg(all(test, feature = "plugin-fft"))]
mod tests {
    use super::*;

    #[test]
    fn test_fft_empty() {
        let v = ValueP::int_vector(&[]);
        assert!(quad_fft(&v).is_err());
    }

    #[test]
    fn test_fft_simple() {
        let v = ValueP::int_vector(&[1, 2, 3, 4]);
        let result = quad_fft(&v);
        assert!(result.is_ok());
    }
}
