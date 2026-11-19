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

fn _pv_inner(n: i64) -> Result<serde_json::Value, String> {
    let out = port::_pv(n)?;
    Ok(out)
}

/// `_pv`.
#[pyfunction]
#[pyo3(name = "_pv", signature = (n))]
fn _pv(py: Python<'_>, n: i64) -> PyResult<Py<PyAny>> {
    _pv_inner(n)
        .map_err(error)
        .and_then(|v| crate::pyconv::ToPy::to_py(&v, py))
}

fn _ps_inner(num: i64, v: String) -> Result<serde_json::Value, String> {
    let out = port::_ps(num, v)?;
    Ok(out)
}

/// `_ps`.
#[pyfunction]
#[pyo3(name = "_ps", signature = (num, v))]
fn _ps(py: Python<'_>, num: i64, v: String) -> PyResult<Py<PyAny>> {
    _ps_inner(num, v)
        .map_err(error)
        .and_then(|v| crate::pyconv::ToPy::to_py(&v, py))
}

fn _pi_inner(num: i64, n: i64) -> Result<serde_json::Value, String> {
    let out = port::_pi(num, n)?;
    Ok(out)
}

/// `_pi`.
#[pyfunction]
#[pyo3(name = "_pi", signature = (num, n))]
fn _pi(py: Python<'_>, num: i64, n: i64) -> PyResult<Py<PyAny>> {
    _pi_inner(num, n)
        .map_err(error)
        .and_then(|v| crate::pyconv::ToPy::to_py(&v, py))
}

fn _pm_inner(num: i64, body: Bound<'_, PyAny>) -> Result<serde_json::Value, String> {
    let body: serde_json::Value = crate::pyconv::FromPy::from_py(&body)?;
    let out = port::_pm(num, body)?;
    Ok(out)
}

/// `_pm`.
#[pyfunction]
#[pyo3(name = "_pm", signature = (num, body))]
fn _pm(py: Python<'_>, num: i64, body: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    _pm_inner(num, body)
        .map_err(error)
        .and_then(|v| crate::pyconv::ToPy::to_py(&v, py))
}

fn _pread_inner(buf: Bound<'_, PyAny>) -> Result<serde_json::Value, String> {
    let buf: serde_json::Value = crate::pyconv::FromPy::from_py(&buf)?;
    let out = port::_pread(buf)?;
    Ok(out)
}

/// `_pread`.
#[pyfunction]
#[pyo3(name = "_pread", signature = (buf))]
fn _pread(py: Python<'_>, buf: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    _pread_inner(buf)
        .map_err(error)
        .and_then(|v| crate::pyconv::ToPy::to_py(&v, py))
}

fn _ptext_inner(fields: Bound<'_, PyAny>, num: i64, default: String) -> Result<String, String> {
    let fields: serde_json::Value = crate::pyconv::FromPy::from_py(&fields)?;
    let out = port::_ptext(fields, num, default)?;
    Ok(out)
}

/// `_ptext`.
#[pyfunction]
#[pyo3(name = "_ptext", signature = (fields, num, default=String::from("")))]
fn _ptext(fields: Bound<'_, PyAny>, num: i64, default: String) -> PyResult<String> {
    _ptext_inner(fields, num, default).map_err(error)
}

fn _pint_inner(fields: Bound<'_, PyAny>, num: i64, default: i64) -> Result<i64, String> {
    let fields: serde_json::Value = crate::pyconv::FromPy::from_py(&fields)?;
    let out = port::_pint(fields, num, default)?;
    Ok(out)
}

/// `_pint`.
#[pyfunction]
#[pyo3(name = "_pint", signature = (fields, num, default=0))]
fn _pint(fields: Bound<'_, PyAny>, num: i64, default: i64) -> PyResult<i64> {
    _pint_inner(fields, num, default).map_err(error)
}

fn _pmsg_inner(fields: Bound<'_, PyAny>, num: i64) -> Result<serde_json::Value, String> {
    let fields: serde_json::Value = crate::pyconv::FromPy::from_py(&fields)?;
    let out = port::_pmsg(fields, num)?;
    Ok(out)
}

/// `_pmsg`.
#[pyfunction]
#[pyo3(name = "_pmsg", signature = (fields, num))]
fn _pmsg(py: Python<'_>, fields: Bound<'_, PyAny>, num: i64) -> PyResult<Py<PyAny>> {
    _pmsg_inner(fields, num)
        .map_err(error)
        .and_then(|v| crate::pyconv::ToPy::to_py(&v, py))
}

fn _prep_inner(fields: Bound<'_, PyAny>, num: i64) -> Result<Vec<serde_json::Value>, String> {
    let fields: serde_json::Value = crate::pyconv::FromPy::from_py(&fields)?;
    let out = port::_prep(fields, num)?;
    Ok(out)
}

/// `_prep`.
#[pyfunction]
#[pyo3(name = "_prep", signature = (fields, num))]
fn _prep(py: Python<'_>, fields: Bound<'_, PyAny>, num: i64) -> PyResult<Py<PyAny>> {
    _prep_inner(fields, num)
        .map_err(error)
        .and_then(|v| crate::pyconv::ToPy::to_py(&v, py))
}

fn _strip_em_inner(t: String) -> Result<String, String> {
    let out = port::_strip_em(t)?;
    Ok(out)
}

/// `_strip_em`.
#[pyfunction]
#[pyo3(name = "_strip_em", signature = (t))]
fn _strip_em(t: String) -> PyResult<String> {
    _strip_em_inner(t).map_err(error)
}

fn _dur_seconds_inner(text: String) -> Result<i64, String> {
    let out = port::_dur_seconds(text)?;
    Ok(out)
}

/// `_dur_seconds`.
#[pyfunction]
#[pyo3(name = "_dur_seconds", signature = (text))]
fn _dur_seconds(text: String) -> PyResult<i64> {
    _dur_seconds_inner(text).map_err(error)
}

fn _video_from_item_inner(item: Bound<'_, PyAny>) -> Result<serde_json::Value, String> {
    let item: serde_json::Value = crate::pyconv::FromPy::from_py(&item)?;
    let out = port::_video_from_item(item)?;
    Ok(out)
}

/// `_video_from_item`.
#[pyfunction]
#[pyo3(name = "_video_from_item", signature = (item))]
fn _video_from_item(py: Python<'_>, item: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    _video_from_item_inner(item)
        .map_err(error)
        .and_then(|v| crate::pyconv::ToPy::to_py(&v, py))
}

fn _user_from_item_inner(item: Bound<'_, PyAny>) -> Result<serde_json::Value, String> {
    let item: serde_json::Value = crate::pyconv::FromPy::from_py(&item)?;
    let out = port::_user_from_item(item)?;
    Ok(out)
}

/// `_user_from_item`.
#[pyfunction]
#[pyo3(name = "_user_from_item", signature = (item))]
fn _user_from_item(py: Python<'_>, item: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    _user_from_item_inner(item)
        .map_err(error)
        .and_then(|v| crate::pyconv::ToPy::to_py(&v, py))
}

#[pymodule]
#[pyo3(name = "_native")]
fn native_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(_pv, m)?)?;
    m.add_function(wrap_pyfunction!(_ps, m)?)?;
    m.add_function(wrap_pyfunction!(_pi, m)?)?;
    m.add_function(wrap_pyfunction!(_pm, m)?)?;
    m.add_function(wrap_pyfunction!(_pread, m)?)?;
    m.add_function(wrap_pyfunction!(_ptext, m)?)?;
    m.add_function(wrap_pyfunction!(_pint, m)?)?;
    m.add_function(wrap_pyfunction!(_pmsg, m)?)?;
    m.add_function(wrap_pyfunction!(_prep, m)?)?;
    m.add_function(wrap_pyfunction!(_strip_em, m)?)?;
    m.add_function(wrap_pyfunction!(_dur_seconds, m)?)?;
    m.add_function(wrap_pyfunction!(_video_from_item, m)?)?;
    m.add_function(wrap_pyfunction!(_user_from_item, m)?)?;
    Ok(())
}
