use std::fmt;

use super::{Descriptor, Encoding, Never};

pub struct Pointer<T>(T) where T: Encoding;

impl<T> Pointer<T> where T: Encoding {
    pub fn new(target: T) -> Pointer<T> {
        Pointer(target)
    }
}

impl<T> Encoding for Pointer<T> where T: Encoding {
    type PointerTarget = T;
    type StructFields = Never;

    fn descriptor(&self) -> Descriptor<T, Never> {
        Descriptor::Pointer(&self.0)
    }

    fn eq_encoding<E: ?Sized + Encoding>(&self, other: &E) -> bool {
        if let Descriptor::Pointer(t) = other.descriptor() {
            self.0.eq_encoding(t)
        } else {
            false
        }
    }
}

impl<T> fmt::Display for Pointer<T> where T: Encoding {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "^{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use encoding::Primitive;
    use super::*;

    #[test]
    fn test_pointer_display() {
        let e = Pointer::new(Primitive::Int);
        assert_eq!(e.to_string(), "^i");
    }

    #[test]
    fn test_eq_encoding() {
        let i = Primitive::Int;

        let p = Pointer::new(i);
        assert!(p.eq_encoding(&p));
        assert!(!p.eq_encoding(&i));
    }
}