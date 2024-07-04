#![cfg(feature = "MTLDevice")]
use objc2_metal::MTLDevice;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {}

#[test]
#[ignore = "doesn't work in CI"]
fn test_create_default() {
    let _ = MTLDevice::system_default();
}

#[test]
#[cfg(target_os = "macos")]
fn get_all() {
    let _ = MTLDevice::all();
}
