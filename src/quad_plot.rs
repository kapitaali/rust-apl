use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// ⎕PLOT B — plot a vector of numbers.
#[cfg(feature = "plugin-plot")]
pub fn quad_plot(b: &ValueP) -> AplResult<ValueP> {
    use plotters::prelude::*;

    let cells = b.cells();
    if cells.is_empty() {
        return Err(ErrorCode::DomainError);
    }

    let data: Vec<f64> = cells
        .iter()
        .map(|c| match c {
            crate::cell::Cell::Int(n) => Ok(*n as f64),
            crate::cell::Cell::Float(f) => Ok(*f),
            _ => Err(ErrorCode::DomainError),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let filename = "plot.png";
    let root = BitMapBackend::new(filename, (800, 600)).into_drawing_area();
    root.fill(&WHITE).map_err(|_| ErrorCode::DomainError)?;

    let x_range = 0..data.len();
    let y_min = data.iter().cloned().fold(f64::INFINITY, f64::min);
    let y_max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let y_margin = (y_max - y_min) * 0.1;

    let mut chart = ChartBuilder::on(&root)
        .caption("APL Plot", ("sans-serif", 40))
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(x_range, (y_min - y_margin)..(y_max + y_margin))
        .map_err(|_| ErrorCode::DomainError)?;

    chart.configure_mesh().draw().map_err(|_| ErrorCode::DomainError)?;
    chart.draw_series(LineSeries::new(data.iter().enumerate().map(|(x, y)| (x, *y)), &RED)).map_err(|_| ErrorCode::DomainError)?;
    root.present().map_err(|_| ErrorCode::DomainError)?;

    Ok(ValueP::char_vector(&filename.chars().map(|c| c as u32).collect::<Vec<_>>()))
}

/// ⎕PLOT B — disabled version (returns error when plugin not compiled in).
#[cfg(not(feature = "plugin-plot"))]
pub fn quad_plot(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}

#[cfg(all(test, feature = "plugin-plot"))]
mod tests {
    use super::*;

    #[test]
    fn test_quad_plot_empty() {
        let v = ValueP::int_vector(&[]);
        assert!(quad_plot(&v).is_err());
    }

    #[test]
    fn test_quad_plot_simple() {
        let v = ValueP::int_vector(&[1, 2, 3, 4, 5]);
        let result = quad_plot(&v);
        assert!(result.is_ok());
        let _ = std::fs::remove_file("plot.png");
    }
}
