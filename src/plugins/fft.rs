use crate::types::{AplResult, ErrorCode};
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

    let shape = crate::shape::Shape::vector(input.len() as crate::shape::ShapeItem * 2);
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
