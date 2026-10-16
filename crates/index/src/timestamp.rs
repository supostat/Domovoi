use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
pub struct Timestamp(u32);

impl Timestamp {
    pub const UNKNOWN: Self = Self(0);
    const EARLIEST_KNOWN: u32 = 1;

    pub fn from_system_time(time: SystemTime) -> Self {
        let seconds = time.duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
        Self(u32::try_from(seconds).unwrap_or(u32::MAX).max(Self::EARLIEST_KNOWN))
    }

    pub fn is_known(self) -> bool {
        self != Self::UNKNOWN
    }

    pub fn system_time(self) -> Option<SystemTime> {
        self.is_known()
            .then(|| UNIX_EPOCH + Duration::from_secs(u64::from(self.0)))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::Timestamp;

    #[test]
    fn keeps_whole_seconds_and_marks_unknown_dates() {
        let time = UNIX_EPOCH + Duration::new(1_760_000_000, 999_000_000);
        let stamp = Timestamp::from_system_time(time);
        assert_eq!(
            stamp.system_time(),
            Some(UNIX_EPOCH + Duration::from_secs(1_760_000_000))
        );
        assert!(!Timestamp::UNKNOWN.is_known());
        assert!(Timestamp::from_system_time(UNIX_EPOCH - Duration::from_secs(10)).is_known());
    }
}
