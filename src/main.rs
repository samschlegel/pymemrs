use std::env;
use std::ffi::{c_char, CString};
use std::mem;
use std::os::unix::ffi::OsStringExt;
use std::process::exit;

use memory::print_metrics;
use pyo3::ffi::{
    PyConfig, PyConfig_Clear, PyConfig_InitPythonConfig, PyConfig_SetBytesArgv, PyPreConfig,
    PyPreConfig_InitPythonConfig, PyStatus, PyStatus_Exception, PyStatus_IsExit,
    Py_ExitStatusException, Py_InitializeFromConfig, Py_PreInitializeFromBytesArgs, Py_RunMain,
};

use crate::memory::setup_allocators;

mod memory;

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

        PyPreConfig_InitPythonConfig(&mut preconfig);
        status = Py_PreInitializeFromBytesArgs(&preconfig, argc, argv.as_mut_ptr());
        if PyStatus_Exception(status) != 0 {
            if PyStatus_IsExit(status) != 0 {
                exit(status.exitcode);
            }
            Py_ExitStatusException(status);
        }

        setup_allocators();

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

        let res = Py_RunMain();

        print_metrics();

        exit(res);
    }
}
