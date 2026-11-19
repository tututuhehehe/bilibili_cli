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

fn sign_app_inner(
    params: Bound<'_, PyAny>,
    key: Option<Bound<'_, PyAny>>,
    sec: Option<Bound<'_, PyAny>>,
) -> Result<String, String> {
    let params: serde_json::Value = crate::pyconv::FromPy::from_py(&params)?;
    let key: Option<serde_json::Value> = match &key {
        None => None,
        Some(v) => Some(crate::pyconv::FromPy::from_py(v)?),
    };
    let sec: Option<serde_json::Value> = match &sec {
        None => None,
        Some(v) => Some(crate::pyconv::FromPy::from_py(v)?),
    };
    let out = port::sign_app(params, key, sec)?;
    Ok(out)
}

/// `sign_app`.
#[pyfunction]
#[pyo3(name = "sign_app", signature = (params, key, sec))]
fn sign_app(
    params: Bound<'_, PyAny>,
    key: Option<Bound<'_, PyAny>>,
    sec: Option<Bound<'_, PyAny>>,
) -> PyResult<String> {
    sign_app_inner(params, key, sec).map_err(error)
}

fn cookie_header_inner(c: Bound<'_, PyAny>) -> Result<String, String> {
    let c: serde_json::Value = crate::pyconv::FromPy::from_py(&c)?;
    let out = port::cookie_header(c)?;
    Ok(out)
}

/// `cookie_header`.
#[pyfunction]
#[pyo3(name = "cookie_header", signature = (c))]
fn cookie_header(c: Bound<'_, PyAny>) -> PyResult<String> {
    cookie_header_inner(c).map_err(error)
}

fn _read_jar_inner(path: String) -> Result<serde_json::Value, String> {
    let out = port::_read_jar(path)?;
    Ok(out)
}

/// `_read_jar`.
#[pyfunction]
#[pyo3(name = "_read_jar", signature = (path))]
fn _read_jar(py: Python<'_>, path: String) -> PyResult<Py<PyAny>> {
    _read_jar_inner(path)
        .map_err(error)
        .and_then(|v| crate::pyconv::ToPy::to_py(&v, py))
}

#[pymodule]
#[pyo3(name = "_native")]
fn native_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(sign_app, m)?)?;
    m.add_function(wrap_pyfunction!(cookie_header, m)?)?;
    m.add_function(wrap_pyfunction!(_read_jar, m)?)?;
    Ok(())
}
