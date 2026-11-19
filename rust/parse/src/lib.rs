//! Python bindings for `bilibili_cli.py`: generated from the signatures in `port.rs` by rust-anything.
//! Regenerate with `rust-anything refresh` after changing them.

use pyo3::prelude::*;

pub mod js;
pub mod port;
pub mod pyconv;
pub mod pynum;

#[allow(unused_imports)]
use pynum::Num;

fn error(message: String) -> PyErr {
    use pyo3::exceptions as e;
    let (kind, text) = message.split_once(": ").unwrap_or(("", message.as_str()));
    let text = text.to_string();
    match kind {
        "TypeError" => e::PyTypeError::new_err(text),
        "KeyError" => e::PyKeyError::new_err(text),
        "IndexError" => e::PyIndexError::new_err(text),
        "OverflowError" => e::PyOverflowError::new_err(text),
        "ZeroDivisionError" => e::PyZeroDivisionError::new_err(text),
        "AttributeError" => e::PyAttributeError::new_err(text),
        "RuntimeError" => e::PyRuntimeError::new_err(text),
        "NotImplementedError" => e::PyNotImplementedError::new_err(text),
        "LookupError" => e::PyLookupError::new_err(text),
        "ArithmeticError" => e::PyArithmeticError::new_err(text),
        "UnicodeError" => e::PyUnicodeError::new_err(text),
        "ValueError" => e::PyValueError::new_err(text),
        _ => e::PyValueError::new_err(message),
    }
}

fn parse_target_inner(t: String) -> Result<serde_json::Value, String> {
    let out = port::parse_target(t)?;
    Ok(out)
}

/// `parse_target`.
#[pyfunction]
#[pyo3(name = "parse_target", signature = (t))]
fn parse_target(py: Python<'_>, t: String) -> PyResult<Py<PyAny>> {
    parse_target_inner(t)
        .map_err(error)
        .and_then(|v| crate::pyconv::ToPy::to_py(&v, py))
}

fn _qn_value_inner(name: Option<String>) -> Result<i64, String> {
    let out = port::_qn_value(name)?;
    Ok(out)
}

/// `_qn_value`.
#[pyfunction]
#[pyo3(name = "_qn_value", signature = (name=None))]
fn _qn_value(name: Option<String>) -> PyResult<i64> {
    _qn_value_inner(name).map_err(error)
}

fn iso_inner(ts: i64) -> Result<String, String> {
    let out = port::iso(ts)?;
    Ok(out)
}

/// `iso`.
#[pyfunction]
#[pyo3(name = "iso", signature = (ts))]
fn iso(ts: i64) -> PyResult<String> {
    iso_inner(ts).map_err(error)
}

fn fmt_dur_inner(sec: f64) -> Result<String, String> {
    let out = port::fmt_dur(sec)?;
    Ok(out)
}

/// `fmt_dur`.
#[pyfunction]
#[pyo3(name = "fmt_dur", signature = (sec))]
fn fmt_dur(sec: f64) -> PyResult<String> {
    fmt_dur_inner(sec).map_err(error)
}

fn _fmt_count_inner(n: i64) -> Result<String, String> {
    let out = port::_fmt_count(n)?;
    Ok(out)
}

/// `_fmt_count`.
#[pyfunction]
#[pyo3(name = "_fmt_count", signature = (n))]
fn _fmt_count(n: i64) -> PyResult<String> {
    _fmt_count_inner(n).map_err(error)
}

fn safe_name_inner(s: String, limit: i64) -> Result<String, String> {
    let out = port::safe_name(s, limit)?;
    Ok(out)
}

/// `safe_name`.
#[pyfunction]
#[pyo3(name = "safe_name", signature = (s, limit=80))]
fn safe_name(s: String, limit: i64) -> PyResult<String> {
    safe_name_inner(s, limit).map_err(error)
}

#[pymodule]
#[pyo3(name = "_native")]
fn native_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_target, m)?)?;
    m.add_function(wrap_pyfunction!(_qn_value, m)?)?;
    m.add_function(wrap_pyfunction!(iso, m)?)?;
    m.add_function(wrap_pyfunction!(fmt_dur, m)?)?;
    m.add_function(wrap_pyfunction!(_fmt_count, m)?)?;
    m.add_function(wrap_pyfunction!(safe_name, m)?)?;
    Ok(())
}
