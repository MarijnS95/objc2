use core::{cmp::Ordering, ptr};

use crate::{CFDate, CFDateCompare};

impl CFDate {
    /// Create a `CFDate` from a [`SystemTime`].
    ///
    /// Nanosecond precision may be lost.
    ///
    /// [`SystemTime`]: std::time::SystemTime
    #[cfg(feature = "std")]
    pub fn from_system_time(time: &std::time::SystemTime) -> crate::CFRetained<Self> {
        let since_1970 = match time.duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_secs_f64(),
            Err(err) => -err.duration().as_secs_f64(),
        } as core::ffi::c_double;

        let since_2001 = since_1970 - unsafe { crate::kCFAbsoluteTimeIntervalSince1970 };
        crate::CFDateCreate(None, since_2001).expect("failed creating CFDate")
    }

    /// Try to construct a [`SystemTime`] from the `CFDate`.
    ///
    /// Nanosecond precision may be lost.
    ///
    /// Returns `None` if the `CFDate` is too large to fit inside
    /// [`SystemTime`].
    ///
    /// [`SystemTime`]: std::time::SystemTime
    #[cfg(feature = "std")]
    pub fn to_system_time(&self) -> Option<std::time::SystemTime> {
        let since_2001 = crate::CFDateGetAbsoluteTime(self);
        let since_1970 = (since_2001 + unsafe { crate::kCFAbsoluteTimeIntervalSince1970 }) as f64;

        std::time::UNIX_EPOCH.checked_add(std::time::Duration::try_from_secs_f64(since_1970).ok()?)
    }
}

impl PartialOrd for CFDate {
    #[inline]
    #[doc(alias = "CFDateCompare")]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CFDate {
    #[inline]
    #[doc(alias = "CFDateCompare")]
    fn cmp(&self, other: &Self) -> Ordering {
        // Documented that one should pass NULL here.
        let context = ptr::null_mut();
        unsafe { CFDateCompare(self, Some(other), context) }.into()
    }
}

#[cfg(test)]
mod test {
    use core::ffi::c_double;
    use std::time::{Duration, SystemTime};

    use crate::{CFAbsoluteTimeGetCurrent, CFDateCreate, CFDateGetAbsoluteTime};

    use super::*;

    #[test]
    fn cmp() {
        let now = CFDateCreate(None, CFAbsoluteTimeGetCurrent()).unwrap();
        let past = CFDateCreate(None, CFDateGetAbsoluteTime(&now) - 1.0).unwrap();
        assert_eq!(now.cmp(&past), Ordering::Greater);
        assert_eq!(now.cmp(&now), Ordering::Equal);
        assert_eq!(past.cmp(&now), Ordering::Less);

        assert_eq!(now, now);
        assert_ne!(now, past);
    }

    #[test]
    fn system_time_roundtrip() {
        let date1 = CFDateCreate(None, CFAbsoluteTimeGetCurrent()).unwrap();
        let date2 = CFDate::from_system_time(&date1.to_system_time().unwrap());
        let diff = CFDateGetAbsoluteTime(&date1) - CFDateGetAbsoluteTime(&date2);
        assert!(diff <= 1.0); // Some precision is lost
    }

    #[test]
    fn system_time_cmp() {
        let std_now_first = SystemTime::now();
        let cf_now_first = CFDateCreate(None, CFAbsoluteTimeGetCurrent() + 1.0).unwrap();
        let std_now_second = std_now_first.checked_add(Duration::from_secs(2)).unwrap();
        let cf_now_second = CFDateCreate(None, CFAbsoluteTimeGetCurrent() + 3.0).unwrap();

        assert!(std_now_first <= std_now_second);
        assert!(cf_now_first <= cf_now_second);

        assert!(std_now_first <= cf_now_first.to_system_time().unwrap());
        assert!(cf_now_first.to_system_time().unwrap() <= std_now_second);

        assert!(cf_now_first <= CFDate::from_system_time(&std_now_second));
        assert!(CFDate::from_system_time(&std_now_second) <= cf_now_second);
    }

    #[test]
    fn system_time_from_odd() {
        let time = SystemTime::UNIX_EPOCH
            .checked_sub(Duration::from_secs(10))
            .unwrap();
        let _ = CFDate::from_system_time(&time);
    }

    #[test]
    fn system_time_unrepresentable() {
        let date = CFDateCreate(None, c_double::MIN).unwrap();
        assert_eq!(date.to_system_time(), None);

        let date = CFDateCreate(None, c_double::MAX).unwrap();
        assert_eq!(date.to_system_time(), None);
    }
}
