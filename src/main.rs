use std::ffi::{c_char, c_int, CString};
use std::mem;
use std::os::unix::ffi::OsStringExt;
use std::process::exit;
use std::{env, ptr};

use memory::print_metrics;
use pyo3::ffi::{
    self, PyConfig, PyConfig_Clear, PyConfig_InitPythonConfig, PyConfig_SetBytesArgv,
    PyEval_SetProfile, PyFrameObject, PyPreConfig, PyPreConfig_InitPythonConfig, PyStatus,
    PyStatus_Exception, PyStatus_IsExit, PyTrace_CALL, PyTrace_C_CALL, PyTrace_C_EXCEPTION,
    PyTrace_C_RETURN, PyTrace_EXCEPTION, PyTrace_LINE, PyTrace_OPCODE, PyTrace_RETURN,
    Py_ExitStatusException, Py_InitializeFromConfig, Py_PreInitializeFromBytesArgs, Py_RunMain,
};

use crate::memory::setup_allocators;

mod memory;

unsafe extern "C" fn pytracefunc(
    obj: *mut ffi::PyObject,
    frame: *mut PyFrameObject,
    what: c_int,
    arg: *mut ffi::PyObject,
) -> c_int {
    // let obj = if obj.is_null() { None } else { Some(obj) };
    // let frame = if frame.is_null() { None } else { Some(frame) };
    // let arg = if arg.is_null() { None } else { Some(arg) };
    // let what = match what {
    //     PyTrace_CALL => "CALL",
    //     PyTrace_EXCEPTION => "EXCEPTION",
    //     PyTrace_LINE => "LINE",
    //     PyTrace_RETURN => "RETURN",
    //     PyTrace_C_CALL => "C_CALL",
    //     PyTrace_C_EXCEPTION => "C_EXCEPTION",
    //     PyTrace_C_RETURN => "C_RETURN",
    //     PyTrace_OPCODE => "OPCODE",
    //     _ => "UNKNOWN",
    // };

    0
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

        print_metrics();

        exit(res);
    }
}
