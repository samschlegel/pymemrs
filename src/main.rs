use std::mem::{self, zeroed};
use std::os::raw::c_void;
use std::process::exit;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicUsize, Ordering};

use pyo3::ffi::{
    PyConfig, PyConfig_Clear, PyConfig_InitPythonConfig, PyMemAllocatorEx, PyMem_GetAllocator,
    PyMem_SetAllocator, PyPreConfig, PyPreConfig_InitPythonConfig, PyStatus, PyStatus_Exception,
    PyStatus_IsExit, Py_ExitStatusException, Py_InitializeFromConfig, Py_PreInitialize, Py_RunMain,
};

struct OrigPymallocAllocators {
    raw: PyMemAllocatorEx,
    mem: PyMemAllocatorEx,
    obj: PyMemAllocatorEx,
}

static mut ORIG_PYMALLOC_ALLOCATORS: OrigPymallocAllocators = unsafe { zeroed() };

static mut MALLOCS: AtomicUsize = AtomicUsize::new(0);
static mut CALLOCS: AtomicUsize = AtomicUsize::new(0);
static mut REALLOCS: AtomicUsize = AtomicUsize::new(0);
static mut FREES: AtomicUsize = AtomicUsize::new(0);

static mut RAW: AtomicUsize = AtomicUsize::new(0);
static mut MEM: AtomicUsize = AtomicUsize::new(0);
static mut OBJ: AtomicUsize = AtomicUsize::new(0);
static mut UNK: AtomicUsize = AtomicUsize::new(0);

extern "C" fn pymalloc_malloc(ctx: *mut c_void, size: usize) -> *mut c_void {
    unsafe {
        let alloc: *mut PyMemAllocatorEx = ctx as *mut PyMemAllocatorEx;
        if alloc == &mut ORIG_PYMALLOC_ALLOCATORS.raw {
            RAW.fetch_add(1, Ordering::SeqCst);
        } else if alloc == &mut ORIG_PYMALLOC_ALLOCATORS.mem {
            MEM.fetch_add(1, Ordering::SeqCst);
        } else if alloc == &mut ORIG_PYMALLOC_ALLOCATORS.obj {
            OBJ.fetch_add(1, Ordering::SeqCst);
        } else {
            UNK.fetch_add(1, Ordering::SeqCst);
        }
        MALLOCS.fetch_add(1, Ordering::SeqCst);
        return (*alloc).malloc.unwrap()(ctx, size);
    };
}

extern "C" fn pymalloc_calloc(ctx: *mut c_void, nelem: usize, elsize: usize) -> *mut c_void {
    unsafe {
        let alloc: *mut PyMemAllocatorEx = ctx as *mut PyMemAllocatorEx;
        CALLOCS.fetch_add(1, Ordering::SeqCst);
        return (*alloc).calloc.unwrap()(ctx, nelem, elsize);
    };
}

extern "C" fn pymalloc_realloc(ctx: *mut c_void, ptr: *mut c_void, size: usize) -> *mut c_void {
    unsafe {
        let alloc: *mut PyMemAllocatorEx = ctx as *mut PyMemAllocatorEx;
        REALLOCS.fetch_add(1, Ordering::SeqCst);
        return (*alloc).realloc.unwrap()(ctx, ptr, size);
    };
}

extern "C" fn pymalloc_free(ctx: *mut c_void, ptr: *mut c_void) {
    unsafe {
        let alloc: *mut PyMemAllocatorEx = ctx as *mut PyMemAllocatorEx;
        FREES.fetch_add(1, Ordering::SeqCst);
        return (*alloc).free.unwrap()(ctx, ptr);
    };
}

unsafe fn setup_allocators() {
    let mut alloc = PyMemAllocatorEx {
        ctx: null_mut(),
        malloc: None,
        calloc: None,
        realloc: None,
        free: None,
    };

    PyMem_GetAllocator(
        pyo3::ffi::PyMemAllocatorDomain::PYMEM_DOMAIN_RAW,
        &mut alloc,
    );

    if alloc.malloc == Some(pymalloc_malloc) {
        return;
    }

    PyMem_GetAllocator(
        pyo3::ffi::PyMemAllocatorDomain::PYMEM_DOMAIN_RAW,
        &mut ORIG_PYMALLOC_ALLOCATORS.raw,
    );
    PyMem_GetAllocator(
        pyo3::ffi::PyMemAllocatorDomain::PYMEM_DOMAIN_MEM,
        &mut ORIG_PYMALLOC_ALLOCATORS.mem,
    );
    PyMem_GetAllocator(
        pyo3::ffi::PyMemAllocatorDomain::PYMEM_DOMAIN_OBJ,
        &mut ORIG_PYMALLOC_ALLOCATORS.obj,
    );

    alloc.malloc = Some(pymalloc_malloc);
    alloc.realloc = Some(pymalloc_realloc);
    alloc.calloc = Some(pymalloc_calloc);
    alloc.free = Some(pymalloc_free);

    alloc.ctx = &mut ORIG_PYMALLOC_ALLOCATORS.raw as *mut _ as *mut c_void;
    PyMem_SetAllocator(
        pyo3::ffi::PyMemAllocatorDomain::PYMEM_DOMAIN_RAW,
        &mut alloc,
    );

    alloc.ctx = &mut ORIG_PYMALLOC_ALLOCATORS.mem as *mut _ as *mut c_void;
    PyMem_SetAllocator(
        pyo3::ffi::PyMemAllocatorDomain::PYMEM_DOMAIN_MEM,
        &mut alloc,
    );

    alloc.ctx = &mut ORIG_PYMALLOC_ALLOCATORS.obj as *mut _ as *mut c_void;
    PyMem_SetAllocator(
        pyo3::ffi::PyMemAllocatorDomain::PYMEM_DOMAIN_OBJ,
        &mut alloc,
    );
}

fn main() {
    unsafe {
        let mut status: PyStatus;
        let mut preconfig: PyPreConfig = mem::zeroed();
        let mut config: PyConfig = mem::zeroed();

        PyPreConfig_InitPythonConfig(&mut preconfig);
        status = Py_PreInitialize(&preconfig);
        if PyStatus_Exception(status) != 0 {
            if PyStatus_IsExit(status) != 0 {
                exit(status.exitcode);
            }
            Py_ExitStatusException(status);
        }

        setup_allocators();

        PyConfig_InitPythonConfig(&mut config);
        status = Py_InitializeFromConfig(&config);
        if PyStatus_Exception(status) != 0 {
            PyConfig_Clear(&mut config);
            if PyStatus_IsExit(status) != 0 {
                exit(status.exitcode);
            }
            Py_ExitStatusException(status);
        }

        let res = Py_RunMain();

        println!("Mallocs: {}", MALLOCS.load(Ordering::SeqCst));
        println!("Callocs: {}", CALLOCS.load(Ordering::SeqCst));
        println!("Reallocs: {}", REALLOCS.load(Ordering::SeqCst));
        println!("Frees: {}", FREES.load(Ordering::SeqCst));
        println!("Raw: {}", RAW.load(Ordering::SeqCst));
        println!("Mem: {}", MEM.load(Ordering::SeqCst));
        println!("Obj: {}", OBJ.load(Ordering::SeqCst));
        println!("Unk: {}", UNK.load(Ordering::SeqCst));

        exit(res);
    }
    // let res = Python::with_gil(|py| {
    //     unsafe { setup_allocators() };
    //     // let sys = py.import_bound("sys")?;
    //     // let version: String = sys.getattr("version")?.extract()?;
    //     //
    //     // let locals = [("os", py.import_bound("os")?)].into_py_dict_bound(py);
    //     // let code = "os.getenv('USER') or os.getenv('USERNAME') or 'Unknown'";
    //     // let user: String = py.eval_bound(code, None, Some(&locals))?.extract()?;
    //     //
    //     // println!("Hello {}, I'm Python {}", user, version);
    //     let gevent = py.import_bound("gevent.monkey")?;
    //     gevent.getattr("patch_all")?.call0()?;
    //
    //     println!("Importing discord.flask.app");
    //     let discord_flask_app = py.import_bound("discord.flask.app")?;
    //     println!("Imported discord_flask.app");
    //     let app_instance_fn = discord_flask_app.getattr("app_instance")?;
    //     println!("Creating app instance");
    //     let app = app_instance_fn.call0()?;
    //     println!("Created app instance, {:#?}", app);
    //     Ok(())
    // });
    //
    // unsafe {
    //     println!("Mallocs: {}", MALLOCS.load(Ordering::SeqCst));
    //     println!("Callocs: {}", CALLOCS.load(Ordering::SeqCst));
    //     println!("Reallocs: {}", REALLOCS.load(Ordering::SeqCst));
    //     println!("Frees: {}", FREES.load(Ordering::SeqCst));
    // }
}
