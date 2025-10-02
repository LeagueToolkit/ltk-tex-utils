// =============================================================================
// CLASS FACTORY (following Microsoft's CClassFactory pattern)
// =============================================================================

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::AtomicI32;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::core::*;

use crate::{DllAddRef, DllRelease, CLSID_TEX_THUMB_HANDLER};
use crate::thumbnail_provider::CTexThumbProvider_CreateInstance;

pub type PfnCreateInstance = fn(*const GUID, *mut *mut c_void) -> HRESULT;

pub struct ClassObjectInit {
    pub pClsid: &'static GUID,
    pub pfnCreate: PfnCreateInstance,
}

/// Registry of class objects supported by this DLL
pub const C_RGCLASSOBJECTINIT: &[ClassObjectInit] = &[ClassObjectInit {
    pClsid: &CLSID_TEX_THUMB_HANDLER,
    pfnCreate: CTexThumbProvider_CreateInstance,
}];

#[implement(IClassFactory)]
pub struct CClassFactory {
    #[allow(dead_code)]
    cRef: AtomicI32,
    pfnCreate: PfnCreateInstance,
}

impl CClassFactory {
    fn new(pfnCreate: PfnCreateInstance) -> Self {
        DllAddRef();
        Self {
            cRef: AtomicI32::new(1),
            pfnCreate,
        }
    }

    pub fn create_instance(
        clsid: &GUID,
        class_inits: &[ClassObjectInit],
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT {
        unsafe {
            *ppv = ptr::null_mut();
        }

        // Find matching CLSID
        for init in class_inits {
            if clsid == init.pClsid {
                let factory = CClassFactory::new(init.pfnCreate);
                let unknown: IUnknown = factory.into();
                return unsafe { unknown.query(riid, ppv) };
            }
        }

        CLASS_E_CLASSNOTAVAILABLE
    }
}

impl IClassFactory_Impl for CClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkOuter: Option<&IUnknown>,
        riid: *const GUID,
        ppvObject: *mut *mut c_void,
    ) -> Result<()> {
        if punkOuter.is_some() {
            return Err(Error::from(CLASS_E_NOAGGREGATION));
        }
        let hr = (self.pfnCreate)(riid, ppvObject);
        if hr.is_ok() {
            Ok(())
        } else {
            Err(Error::from(hr))
        }
    }

    fn LockServer(&self, fLock: BOOL) -> Result<()> {
        if fLock.as_bool() {
            DllAddRef();
        } else {
            DllRelease();
        }
        Ok(())
    }
}

impl Drop for CClassFactory {
    fn drop(&mut self) {
        DllRelease();
    }
}

