use std::mem;
use std::os::raw::c_void;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicUsize, Ordering};

use lazy_static::lazy_static;
use prometheus::{register_int_counter_vec, IntCounterVec};
use pyo3::ffi::{PyMemAllocatorEx, PyMem_GetAllocator, PyMem_SetAllocator};

struct OrigPymallocAllocators {
    raw: PyMemAllocatorEx,
    mem: PyMemAllocatorEx,
    obj: PyMemAllocatorEx,
}

static mut ORIG_PYMALLOC_ALLOCATORS: OrigPymallocAllocators = unsafe { mem::zeroed() };

lazy_static! {
    static ref MALLOC_COUNTER: IntCounterVec =
        register_int_counter_vec!("mallocs", "help", &["allocator"]).unwrap();
    static ref CALLOC_COUNTER: IntCounterVec =
        register_int_counter_vec!("callocs", "help", &["allocator"]).unwrap();
    static ref REALLOC_COUNTER: IntCounterVec =
        register_int_counter_vec!("reallocs", "help", &["allocator"]).unwrap();
    static ref FREE_COUNTER: IntCounterVec =
        register_int_counter_vec!("frees", "help", &["allocator"]).unwrap();
}

unsafe fn get_allocator_str(alloc: *mut PyMemAllocatorEx) -> &'static str {
    if alloc == &mut ORIG_PYMALLOC_ALLOCATORS.raw {
        "raw"
    } else if alloc == &mut ORIG_PYMALLOC_ALLOCATORS.mem {
        "mem"
    } else if alloc == &mut ORIG_PYMALLOC_ALLOCATORS.obj {
        "obj"
    } else {
        "unk"
    }
}

extern "C" fn pymalloc_malloc(ctx: *mut c_void, size: usize) -> *mut c_void {
    unsafe {
        let alloc: *mut PyMemAllocatorEx = ctx as *mut PyMemAllocatorEx;
        MALLOC_COUNTER
            .with_label_values(&[get_allocator_str(alloc)])
            .inc();
        return (*alloc).malloc.unwrap()(ctx, size);
    };
}

extern "C" fn pymalloc_calloc(ctx: *mut c_void, nelem: usize, elsize: usize) -> *mut c_void {
    unsafe {
        let alloc: *mut PyMemAllocatorEx = ctx as *mut PyMemAllocatorEx;
        CALLOC_COUNTER
            .with_label_values(&[get_allocator_str(alloc)])
            .inc();
        return (*alloc).calloc.unwrap()(ctx, nelem, elsize);
    };
}

extern "C" fn pymalloc_realloc(ctx: *mut c_void, ptr: *mut c_void, size: usize) -> *mut c_void {
    unsafe {
        let alloc: *mut PyMemAllocatorEx = ctx as *mut PyMemAllocatorEx;
        REALLOC_COUNTER
            .with_label_values(&[get_allocator_str(alloc)])
            .inc();
        return (*alloc).realloc.unwrap()(ctx, ptr, size);
    };
}

extern "C" fn pymalloc_free(ctx: *mut c_void, ptr: *mut c_void) {
    unsafe {
        let alloc: *mut PyMemAllocatorEx = ctx as *mut PyMemAllocatorEx;
        FREE_COUNTER
            .with_label_values(&[get_allocator_str(alloc)])
            .inc();
        return (*alloc).free.unwrap()(ctx, ptr);
    };
}

pub unsafe fn setup_allocators() {
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
