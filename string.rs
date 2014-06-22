use std::mem;
use std::str::raw::c_str_to_static_slice;

use runtime::{Messageable, Object, Sel, objc_msgSend};
use id::{class, ClassName, Id, FromId};
use super::INSObject;

pub trait INSCopying<T: FromId> : INSObject {
	fn copy(&self) -> T {
		let copy = Sel::register("copy");
		unsafe {
			let obj = objc_msgSend(self.as_ptr(), copy);
			FromId::from_retained_ptr(obj)
		}
	}
}

pub trait INSString : INSObject {
	fn as_str<'a>(&'a self) -> &'a str {
		let utf8_string = Sel::register("UTF8String");
		unsafe {
			let result = objc_msgSend(self.as_ptr(), utf8_string);
			c_str_to_static_slice(mem::transmute(result))
		}
	}
}

#[deriving(Clone)]
pub struct NSString {
	ptr: Id,
}

impl Messageable for NSString {
	unsafe fn as_ptr(&self) -> *Object {
		self.ptr.as_ptr()
	}
}

impl FromId for NSString {
	unsafe fn from_id(id: Id) -> NSString {
		NSString { ptr: id }
	}
}

impl INSObject for NSString {
	fn class_name() -> ClassName<NSString> {
		ClassName::from_str("NSString")
	}
}

impl INSString for NSString { }

impl INSCopying<NSString> for NSString { }

impl NSString {
	pub fn from_str(string: &str) -> NSString {
		let class = class::<NSString>();
		let alloc = Sel::register("alloc");
		let init_with_bytes = Sel::register("initWithBytes:length:encoding:");
		let utf8_encoding = 4u;
		unsafe {
			let obj = objc_msgSend(class.as_ptr(), alloc);
			let obj = objc_msgSend(obj, init_with_bytes, string.as_ptr(),
				string.len(), utf8_encoding);
			FromId::from_retained_ptr(obj)
		}
	}
}

impl Str for NSString {
	fn as_slice<'a>(&'a self) -> &'a str {
		self.as_str()
	}
}