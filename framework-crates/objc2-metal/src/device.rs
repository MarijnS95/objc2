use objc2::{rc::Id, runtime::ProtocolObject};
use objc2_foundation::NSArray;

use crate::MTLDevice;

pub trait MTLDeviceExt: MTLDevice {
    fn system_default() -> Option<Id<ProtocolObject<dyn MTLDevice>>> {
        unsafe { Id::from_raw(crate::MTLCreateSystemDefaultDevice()) }
    }

    fn all() -> Id<NSArray<ProtocolObject<dyn MTLDevice>>> {
        unsafe { Id::new_nonnull(crate::MTLCopyAllDevices()) }
    }
}

impl<P: MTLDevice> MTLDeviceExt for P {}
