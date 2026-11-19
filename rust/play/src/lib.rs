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

fn entitlement_of_inner(d: Bound<'_, PyAny>) -> Result<serde_json::Value, String> {
    let d: serde_json::Value = crate::pyconv::FromPy::from_py(&d)?;
    let out = port::entitlement_of(d)?;
    Ok(out)
}

/// `entitlement_of`.
#[pyfunction]
#[pyo3(name = "entitlement_of", signature = (d))]
fn entitlement_of(py: Python<'_>, d: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    entitlement_of_inner(d)
        .map_err(error)
        .and_then(|v| crate::pyconv::ToPy::to_py(&v, py))
}

fn access_of_inner(
    probes: Bound<'_, PyAny>,
    ent: Option<Bound<'_, PyAny>>,
) -> Result<serde_json::Value, String> {
    let probes: Vec<serde_json::Value> = crate::pyconv::FromPy::from_py(&probes)?;
    let ent: Option<serde_json::Value> = match &ent {
        None => None,
        Some(v) => Some(crate::pyconv::FromPy::from_py(v)?),
    };
    let out = port::access_of(probes, ent)?;
    Ok(out)
}

/// `access_of`.
#[pyfunction]
#[pyo3(name = "access_of", signature = (probes, ent=None))]
fn access_of(
    py: Python<'_>,
    probes: Bound<'_, PyAny>,
    ent: Option<Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    access_of_inner(probes, ent)
        .map_err(error)
        .and_then(|v| crate::pyconv::ToPy::to_py(&v, py))
}

fn pick_streams_inner(
    dash: Bound<'_, PyAny>,
    want_qn: i64,
    codec: String,
) -> Result<serde_json::Value, String> {
    let dash: serde_json::Value = crate::pyconv::FromPy::from_py(&dash)?;
    let out = port::pick_streams(dash, want_qn, codec)?;
    Ok(out)
}

/// `pick_streams`.
#[pyfunction]
#[pyo3(name = "pick_streams", signature = (dash, want_qn, codec=String::from("avc")))]
fn pick_streams(
    py: Python<'_>,
    dash: Bound<'_, PyAny>,
    want_qn: i64,
    codec: String,
) -> PyResult<Py<PyAny>> {
    pick_streams_inner(dash, want_qn, codec)
        .map_err(error)
        .and_then(|v| crate::pyconv::ToPy::to_py(&v, py))
}

fn _url_of_inner(s: Bound<'_, PyAny>) -> Result<String, String> {
    let s: serde_json::Value = crate::pyconv::FromPy::from_py(&s)?;
    let out = port::_url_of(s)?;
    Ok(out)
}

/// `_url_of`.
#[pyfunction]
#[pyo3(name = "_url_of", signature = (s))]
fn _url_of(s: Bound<'_, PyAny>) -> PyResult<String> {
    _url_of_inner(s).map_err(error)
}

fn _backup_of_inner(s: Bound<'_, PyAny>) -> Result<Vec<serde_json::Value>, String> {
    let s: serde_json::Value = crate::pyconv::FromPy::from_py(&s)?;
    let out = port::_backup_of(s)?;
    Ok(out)
}

/// `_backup_of`.
#[pyfunction]
#[pyo3(name = "_backup_of", signature = (s))]
fn _backup_of(py: Python<'_>, s: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    _backup_of_inner(s)
        .map_err(error)
        .and_then(|v| crate::pyconv::ToPy::to_py(&v, py))
}

#[pymodule]
#[pyo3(name = "_native")]
fn native_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(entitlement_of, m)?)?;
    m.add_function(wrap_pyfunction!(access_of, m)?)?;
    m.add_function(wrap_pyfunction!(pick_streams, m)?)?;
    m.add_function(wrap_pyfunction!(_url_of, m)?)?;
    m.add_function(wrap_pyfunction!(_backup_of, m)?)?;
    Ok(())
}
