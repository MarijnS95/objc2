use crate::CFUUIDBytes;

impl From<[u8; 16]> for CFUUIDBytes {
    #[inline]
    fn from(value: [u8; 16]) -> Self {
        Self {
            byte0: value[0],
            byte1: value[1],
            byte2: value[2],
            byte3: value[3],
            byte4: value[4],
            byte5: value[5],
            byte6: value[6],
            byte7: value[7],
            byte8: value[8],
            byte9: value[9],
            byte10: value[10],
            byte11: value[11],
            byte12: value[12],
            byte13: value[13],
            byte14: value[14],
            byte15: value[15],
        }
    }
}

impl From<CFUUIDBytes> for [u8; 16] {
    #[inline]
    fn from(value: CFUUIDBytes) -> Self {
        [
            value.byte0,
            value.byte1,
            value.byte2,
            value.byte3,
            value.byte4,
            value.byte5,
            value.byte6,
            value.byte7,
            value.byte8,
            value.byte9,
            value.byte10,
            value.byte11,
            value.byte12,
            value.byte13,
            value.byte14,
            value.byte15,
        ]
    }
}

#[cfg(test)]
#[cfg(feature = "CFBase")]
mod tests {
    use crate::{CFUUIDCreate, CFUUIDCreateFromUUIDBytes, CFUUIDGetUUIDBytes};

    #[test]
    fn eq() {
        let uuid0 = CFUUIDCreateFromUUIDBytes(None, [0; 16].into()).unwrap();
        let uuid1 = CFUUIDCreateFromUUIDBytes(None, [1; 16].into()).unwrap();
        assert_eq!(uuid0, uuid0);
        assert_ne!(uuid0, uuid1);
    }

    #[test]
    fn roundtrip() {
        let uuid = CFUUIDCreate(None).unwrap();
        assert_eq!(uuid, uuid);

        let bytes = CFUUIDGetUUIDBytes(&uuid);
        let same_uuid = CFUUIDCreateFromUUIDBytes(None, bytes).unwrap();
        assert_eq!(uuid, same_uuid);
    }
}
