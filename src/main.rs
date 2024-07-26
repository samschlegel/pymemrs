use std::ffi::{c_char, c_int, CString};
use std::io::{stdout, Write};
use std::mem;
use std::os::unix::ffi::OsStringExt;
use std::process::exit;
use std::{env, ptr};

use lazy_static::lazy_static;
use prometheus::{register_int_counter_vec, Encoder, IntCounterVec, TextEncoder};
use pyo3::ffi::{
    self, PyConfig, PyConfig_Clear, PyConfig_InitPythonConfig, PyConfig_SetBytesArgv,
    PyEval_SetProfile, PyFrameObject, PyPreConfig, PyPreConfig_InitPythonConfig, PyStatus,
    PyStatus_Exception, PyStatus_IsExit, PyTrace_CALL, PyTrace_C_CALL, PyTrace_C_EXCEPTION,
    PyTrace_C_RETURN, PyTrace_EXCEPTION, PyTrace_LINE, PyTrace_OPCODE, PyTrace_RETURN,
    Py_ExitStatusException, Py_InitializeFromConfig, Py_PreInitializeFromBytesArgs, Py_RunMain,
};

use crate::memory::setup_allocators;
mod memory;

lazy_static! {
    static ref PY_TRACE_COUNTER: IntCounterVec =
        register_int_counter_vec!("py_trace_count", "help", &["what"]).unwrap();
}

unsafe extern "C" fn pytracefunc(
    _obj: *mut ffi::PyObject,
    _frame: *mut PyFrameObject,
    what: c_int,
    _arg: *mut ffi::PyObject,
) -> c_int {
    // let obj = if obj.is_null() { None } else { Some(obj) };
    // let frame = if frame.is_null() { None } else { Some(frame) };
    // let arg = if arg.is_null() { None } else { Some(arg) };
    let what = match what {
        PyTrace_CALL => "CALL",
        PyTrace_EXCEPTION => "EXCEPTION",
        PyTrace_LINE => "LINE",
        PyTrace_RETURN => "RETURN",
        PyTrace_C_CALL => "C_CALL",
        PyTrace_C_EXCEPTION => "C_EXCEPTION",
        PyTrace_C_RETURN => "C_RETURN",
        PyTrace_OPCODE => "OPCODE",
        _ => "UNKNOWN",
    };
    PY_TRACE_COUNTER.with_label_values(&[what]).inc();

    0
}

fn print_prometheus() {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    stdout().write(&buffer);
}

fn main() {
    unsafe {
        let mut status: PyStatus;
        let mut preconfig: PyPreConfig = mem::zeroed();
        let mut config: PyConfig = mem::zeroed();

        let args = env::args_os();
        let argc = args.len().try_into().unwrap();
        let mut argv: Vec<*mut c_char> = args
            .map(|s| CString::new(s.into_vec()).unwrap().into_raw())
            .collect();

        setup_allocators();

        PyPreConfig_InitPythonConfig(&mut preconfig);
        status = Py_PreInitializeFromBytesArgs(&preconfig, argc, argv.as_mut_ptr());
        if PyStatus_Exception(status) != 0 {
            if PyStatus_IsExit(status) != 0 {
                exit(status.exitcode);
            }
            Py_ExitStatusException(status);
        }

        PyConfig_InitPythonConfig(&mut config);
        PyConfig_SetBytesArgv(&mut config, argc, argv.as_mut_ptr() as *mut *const i8);
        status = Py_InitializeFromConfig(&config);
        if PyStatus_Exception(status) != 0 {
            PyConfig_Clear(&mut config);
            if PyStatus_IsExit(status) != 0 {
                exit(status.exitcode);
            }
            Py_ExitStatusException(status);
        }

        PyEval_SetProfile(Some(pytracefunc), ptr::null_mut());

        let res = Py_RunMain();

        print_prometheus();

        exit(res);
    }
}
