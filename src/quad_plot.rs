use crate::types::{AplResult, ErrorCode};
use crate::value::ValueP;

/// Configuration for ⎕PLOT.
#[derive(Clone, Debug, Default)]
pub struct PlotConfig {
    pub line_color_rgb: Option<(u8, u8, u8)>,
    pub line_width: Option<u32>,
    pub title: Option<String>,
    pub x_label: Option<String>,
    pub y_label: Option<String>,
    pub background_rgb: Option<(u8, u8, u8)>,
    pub show_grid: Option<bool>,
    pub auto_open: Option<bool>,
}

/// Parse a PlotConfig from a right argument ValueP.
/// The argument can be:
/// - A character vector/matrix (property name=value pairs separated by newlines)
/// - A nested array of properties (e.g., 'title' 'My Plot' 'color' 'blue')
/// - An integer scalar (code for predefined plot type: 1=line, 2=scatter, 3=bar)
fn parse_config(b: &ValueP) -> AplResult<PlotConfig> {
    let cells = b.cells();
    if cells.is_empty() {
        return Err(ErrorCode::DomainError);
    }

    let mut config = PlotConfig::default();

    // Check if it's an integer scalar — plot type code
    if cells.len() == 1 {
        if let crate::cell::Cell::Int(code) = cells[0] {
            // We don't have per-type logic yet, but store the code for future use
            if code >= 1 && code <= 3 {
                return Ok(config);
            }
        }
    }

    // Try to parse as a character matrix with property=value pairs
    // Each line is "key=value"
    if b.rank() == 1 || b.rank() == 2 {
        let (cols, rows) = if b.rank() == 1 {
            (b.get_shape_item(0) as usize, 1)
        } else {
            (b.get_shape_item(0) as usize, b.get_shape_item(1) as usize)
        };
        let mut line = String::new();
        for row in 0..rows {
            line.clear();
            for col in 0..cols {
                let idx = row * cols + col;
                if idx < cells.len() {
                    match &cells[idx] {
                        crate::cell::Cell::Char(c) => {
                            if let Some(ch) = char::from_u32(*c) {
                                line.push(ch);
                            }
                        }
                        _ => break,
                    }
                }
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some((key, val)) = trimmed.split_once('=') {
                let key = key.trim().to_lowercase();
                let val = val.trim();
                match key.as_str() {
                    "color" | "line_color" => {
                        config.line_color_rgb = parse_color(val);
                    }
                    "width" | "line_width" => {
                        config.line_width = val.parse().ok();
                    }
                    "title" => config.title = Some(val.to_string()),
                    "x_label" | "xlabel" => config.x_label = Some(val.to_string()),
                    "y_label" | "ylabel" => config.y_label = Some(val.to_string()),
                    "background" => {
                        config.background_rgb = parse_color(val);
                    }
                    "grid" => {
                        config.show_grid = Some(val == "1" || val.eq_ignore_ascii_case("true"));
                    }
                    "auto_open" => {
                        config.auto_open = Some(val == "1" || val.eq_ignore_ascii_case("true"));
                    }
                    _ => {} // unknown property — ignored
                }
            }
        }
    }

    Ok(config)
}

fn parse_color(name: &str) -> Option<(u8, u8, u8)> {
    match name.to_lowercase().as_str() {
        "red" => Some((255, 0, 0)),
        "green" => Some((0, 255, 0)),
        "blue" => Some((0, 0, 255)),
        "yellow" => Some((255, 255, 0)),
        "cyan" => Some((0, 255, 255)),
        "magenta" => Some((255, 0, 255)),
        "black" => Some((0, 0, 0)),
        "white" => Some((255, 255, 255)),
        "orange" => Some((255, 165, 0)),
        _ => None,
    }
}

fn rgb_to_plotters((r, g, b): (u8, u8, u8)) -> plotters::style::RGBColor {
    plotters::style::RGBColor(r, g, b)
}

/// ⎕PLOT B — plot a vector of numbers (monadic).
#[cfg(feature = "plugin-plot")]
pub fn quad_plot(b: &ValueP) -> AplResult<ValueP> {
    quad_plot_with_config(b, &PlotConfig::default())
}

/// ⎕PLOT B — disabled version (returns error when plugin not compiled in).
#[cfg(not(feature = "plugin-plot"))]
pub fn quad_plot(_b: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}

/// config ⎕PLOT B — dyadic form with plot configuration.
#[cfg(feature = "plugin-plot")]
pub fn quad_plot_dyad(config: &ValueP, data: &ValueP) -> AplResult<ValueP> {
    let cfg = parse_config(config)?;
    quad_plot_with_config(data, &cfg)
}

#[cfg(not(feature = "plugin-plot"))]
pub fn quad_plot_dyad(_config: &ValueP, _data: &ValueP) -> AplResult<ValueP> {
    Err(ErrorCode::DomainError)
}

/// Internal: render a line plot with the given configuration.
#[cfg(feature = "plugin-plot")]
fn quad_plot_with_config(b: &ValueP, config: &PlotConfig) -> AplResult<ValueP> {
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

    // Background
    let bg = config.background_rgb.map(rgb_to_plotters).unwrap_or(WHITE);
    root.fill(&bg).map_err(|_| ErrorCode::DomainError)?;

    let x_range = 0..data.len();
    let y_min = data.iter().cloned().fold(f64::INFINITY, f64::min);
    let y_max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let y_margin = (y_max - y_min) * 0.1;

    let title = config.title.as_deref().unwrap_or("APL Plot");
    let x_label = config.x_label.as_deref().unwrap_or("");
    let y_label = config.y_label.as_deref().unwrap_or("");

    let mut chart = ChartBuilder::on(&root)
        .caption(title, ("sans-serif", 40))
        .margin(10)
        .x_label_area_size(if x_label.is_empty() { 30 } else { 40 })
        .y_label_area_size(if y_label.is_empty() { 30 } else { 40 })
        .build_cartesian_2d(x_range, (y_min - y_margin)..(y_max + y_margin))
        .map_err(|_| ErrorCode::DomainError)?;

    if config.show_grid.unwrap_or(true) {
        chart
            .configure_mesh()
            .draw()
            .map_err(|_| ErrorCode::DomainError)?;
    }

    // Line color
    let line_color = config.line_color_rgb.map(rgb_to_plotters).unwrap_or(RED);
    let line_width = config.line_width.unwrap_or(1);

    chart
        .draw_series(LineSeries::new(
            data.iter().enumerate().map(|(x, y)| (x, *y)),
            ShapeStyle {
                color: line_color.into(),
                filled: false,
                stroke_width: line_width,
            },
        ))
        .map_err(|_| ErrorCode::DomainError)?;

    root.present().map_err(|_| ErrorCode::DomainError)?;

    // Auto-open if configured (default false to avoid URI issues)
    if config.auto_open.unwrap_or(false) {
        let _ = std::process::Command::new(if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        })
        .arg(filename)
        .spawn();
    }

    Ok(ValueP::char_vector(
        &filename.chars().map(|c| c as u32).collect::<Vec<_>>(),
    ))
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

    #[test]
    fn test_quad_plot_with_color() {
        // Test dyadic form: 'color=red' as character matrix
        let color_prop =
            ValueP::char_vector(&"color=red".chars().map(|c| c as u32).collect::<Vec<_>>());
        let data = ValueP::int_vector(&[1, 2, 3, 4, 5]);
        let result = quad_plot_dyad(&color_prop, &data);
        assert!(result.is_ok());
        let _ = std::fs::remove_file("plot.png");
    }

    #[test]
    fn test_quad_plot_with_title() {
        let title = ValueP::char_vector(
            &"title=My Plot"
                .chars()
                .map(|c| c as u32)
                .collect::<Vec<_>>(),
        );
        let data = ValueP::int_vector(&[10, 20, 30]);
        let result = quad_plot_dyad(&title, &data);
        assert!(result.is_ok());
        let _ = std::fs::remove_file("plot.png");
    }

    #[test]
    fn test_parse_config_auto_open() {
        let v = ValueP::char_vector(
            &"auto_open=true"
                .chars()
                .map(|c| c as u32)
                .collect::<Vec<_>>(),
        );
        let config = parse_config(&v).unwrap();
        assert_eq!(config.auto_open, Some(true));

        let v = ValueP::char_vector(
            &"auto_open=false"
                .chars()
                .map(|c| c as u32)
                .collect::<Vec<_>>(),
        );
        let config = parse_config(&v).unwrap();
        assert_eq!(config.auto_open, Some(false));
    }
}
