use std::convert::{TryFrom, TryInto};
use std::mem;

use byteorder::{NetworkEndian, ReadBytesExt};

use crate::decode::Decode;
use crate::encode::{Encode, IsNull};
use crate::error::BoxDynError;
use crate::postgres::{PgArgumentBuffer, PgTypeInfo, PgValueFormat, PgValueRef, Postgres};
use crate::types::Type;

// `PgInterval` is available for direct access to the INTERVAL type

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct PgInterval {
    pub months: i32,
    pub days: i32,
    pub microseconds: i64,
}

impl PgInterval {
    pub fn from_std(value: std::time::Duration) -> Result<Self, BoxDynError> {
        Self::try_from(value)
    }

    pub fn to_std(self) -> Result<std::time::Duration, BoxDynError> {
        self.try_into()
    }
}

impl Type<Postgres> for PgInterval {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::INTERVAL
    }
}

impl Type<Postgres> for [PgInterval] {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::INTERVAL_ARRAY
    }
}

impl<'de> Decode<'de, Postgres> for PgInterval {
    fn decode(value: PgValueRef<'de>) -> Result<Self, BoxDynError> {
        match value.format() {
            PgValueFormat::Binary => {
                let mut buf = value.as_bytes()?;
                let microseconds = buf.read_i64::<NetworkEndian>()?;
                let days = buf.read_i32::<NetworkEndian>()?;
                let months = buf.read_i32::<NetworkEndian>()?;

                Ok(PgInterval {
                    months,
                    days,
                    microseconds,
                })
            }

            // TODO: Implement parsing of text mode
            PgValueFormat::Text => {
                Err("not implemented: decode `INTERVAL` in text mode (unprepared queries)".into())
            }
        }
    }
}

impl Encode<'_, Postgres> for PgInterval {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> IsNull {
        buf.extend(&self.microseconds.to_be_bytes());
        buf.extend(&self.days.to_be_bytes());
        buf.extend(&self.months.to_be_bytes());

        IsNull::No
    }

    fn size_hint(&self) -> usize {
        2 * mem::size_of::<i64>()
    }
}

// We then implement Encode + Type for std Duration, chrono Duration, and time Duration
// This is to enable ease-of-use for encoding when its simple

impl Type<Postgres> for std::time::Duration {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::INTERVAL
    }
}

impl Type<Postgres> for [std::time::Duration] {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::INTERVAL_ARRAY
    }
}

impl Encode<'_, Postgres> for std::time::Duration {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> IsNull {
        PgInterval::try_from(*self)
            .expect("failed to encode `std::time::Duration`")
            .encode_by_ref(buf)
    }

    fn size_hint(&self) -> usize {
        2 * mem::size_of::<i64>()
    }
}

impl TryFrom<std::time::Duration> for PgInterval {
    type Error = BoxDynError;

    /// Convert a `std::time::Duration` to a `PgInterval`
    ///
    /// This returns an error if there is a loss of precision using nanoseconds or if there is a
    /// microsecond overflow.
    fn try_from(value: std::time::Duration) -> Result<Self, BoxDynError> {
        if value.as_nanos() % 1000 != 0 {
            return Err("PostgreSQL `INTERVAL` does not support nanoseconds precision".into());
        }

        Ok(Self {
            months: 0,
            days: 0,
            microseconds: value.as_micros().try_into()?,
        })
    }
}

impl TryInto<std::time::Duration> for PgInterval {
    type Error = BoxDynError;

    /// Convert a `PgInterval` to a `std::time::Duration`
    ///
    /// This returns an error if there is an overflow for (months + days) to seconds or microseconds
    /// to nanoseconds
    fn try_into(self) -> Result<std::time::Duration, BoxDynError> {
        let secs: u64 = u64::try_from(self.months)?
            .checked_mul(30 * 24 * 60 * 60)
            .ok_or("months would overflow in seconds")?
            .checked_add(
                u64::try_from(self.days)?
                    .checked_mul(24 * 60 * 60)
                    .ok_or("days would overflow in seconds")?,
            )
            .ok_or("months + days would overflow in seconds")?
            .checked_add(u64::try_from(self.microseconds / 1_000_000)?)
            .ok_or("months + days + microseconds would overflow in seconds")?;

        let nanos: u32 = u32::try_from((self.microseconds % 1_000_000) * 1_000)?;

        Ok(std::time::Duration::new(secs, nanos))
    }
}

#[cfg(feature = "chrono")]
impl Type<Postgres> for chrono::Duration {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::INTERVAL
    }
}

#[cfg(feature = "chrono")]
impl Type<Postgres> for [chrono::Duration] {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::INTERVAL_ARRAY
    }
}

#[cfg(feature = "chrono")]
impl<'de> Decode<'de, Postgres> for chrono::Duration {
    fn decode(value: PgValueRef<'de>) -> Result<Self, BoxDynError> {
        PgInterval::decode(value)?.try_into()
    }
}

#[cfg(feature = "chrono")]
impl Encode<'_, Postgres> for chrono::Duration {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> IsNull {
        let pg_interval = PgInterval::try_from(*self).expect("Failed to encode chrono::Duration");
        pg_interval.encode_by_ref(buf)
    }

    fn size_hint(&self) -> usize {
        2 * mem::size_of::<i64>()
    }
}

#[cfg(feature = "chrono")]
impl TryFrom<chrono::Duration> for PgInterval {
    type Error = BoxDynError;

    /// Convert a `chrono::Duration` to a `PgInterval`.
    ///
    /// This returns an error if there is a loss of precision using nanoseconds or if there is a
    /// microsecond or nanosecond overflow.
    fn try_from(value: chrono::Duration) -> Result<Self, BoxDynError> {
        value.to_std()?.try_into()
    }
}

#[cfg(feature = "chrono")]
impl TryInto<chrono::Duration> for PgInterval {
    type Error = BoxDynError;

    /// Convert a `PgInterval` to a `chrono::Duration`
    ///
    /// This returns an error if there is a loss of precision using nanoseconds or if there is a
    /// microsecond or nanosecond overflow.
    fn try_into(self) -> Result<chrono::Duration, BoxDynError> {
        chrono::Duration::from_std(self.to_std()?).map_err(|e| Box::new(e) as BoxDynError)
    }
}

#[cfg(feature = "time")]
impl Type<Postgres> for time::Duration {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::INTERVAL
    }
}

#[cfg(feature = "time")]
impl Type<Postgres> for [time::Duration] {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::INTERVAL_ARRAY
    }
}

#[cfg(feature = "time")]
impl<'de> Decode<'de, Postgres> for time::Duration {
    fn decode(value: PgValueRef<'de>) -> Result<Self, BoxDynError> {
        PgInterval::decode(value)?.try_into()
    }
}

#[cfg(feature = "time")]
impl Encode<'_, Postgres> for time::Duration {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> IsNull {
        let pg_interval = PgInterval::try_from(*self).expect("Failed to encode time::Duration");
        pg_interval.encode_by_ref(buf)
    }

    fn size_hint(&self) -> usize {
        2 * mem::size_of::<i64>()
    }
}

#[cfg(feature = "time")]
impl TryFrom<time::Duration> for PgInterval {
    type Error = BoxDynError;

    /// Convert a `time::Duration` to a `PgInterval`.
    ///
    /// This returns an error if there is a loss of precision using nanoseconds or if there is a
    /// microsecond overflow.
    fn try_from(value: time::Duration) -> Result<Self, BoxDynError> {
        if value.whole_nanoseconds() % 1000 != 0 {
            return Err("PostgreSQL `INTERVAL` does not support nanoseconds precision".into());
        }

        Ok(Self {
            months: 0,
            days: 0,
            microseconds: value.whole_microseconds().try_into()?,
        })
    }
}

#[cfg(feature = "time")]
impl TryInto<time::Duration> for PgInterval {
    type Error = BoxDynError;

    /// Convert a `PgInterval` to a `chrono::Duration`
    ///
    /// This returns an error if there is a loss of precision using nanoseconds or if there is a
    /// microsecond or nanosecond overflow.
    fn try_into(self) -> Result<time::Duration, BoxDynError> {
        self.to_std()?
            .try_into()
            .map_err(|e| Box::new(e) as BoxDynError)
    }
}

#[test]
fn test_encode_interval() {
    let mut buf = PgArgumentBuffer::default();

    let interval = PgInterval {
        months: 0,
        days: 0,
        microseconds: 0,
    };
    assert!(matches!(
        Encode::<Postgres>::encode(&interval, &mut buf),
        IsNull::No
    ));
    assert_eq!(&**buf, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    buf.clear();

    let interval = PgInterval {
        months: 0,
        days: 0,
        microseconds: 1_000,
    };
    assert!(matches!(
        Encode::<Postgres>::encode(&interval, &mut buf),
        IsNull::No
    ));
    assert_eq!(&**buf, [0, 0, 0, 0, 0, 0, 3, 232, 0, 0, 0, 0, 0, 0, 0, 0]);
    buf.clear();

    let interval = PgInterval {
        months: 0,
        days: 0,
        microseconds: 1_000_000,
    };
    assert!(matches!(
        Encode::<Postgres>::encode(&interval, &mut buf),
        IsNull::No
    ));
    assert_eq!(&**buf, [0, 0, 0, 0, 0, 15, 66, 64, 0, 0, 0, 0, 0, 0, 0, 0]);
    buf.clear();

    let interval = PgInterval {
        months: 0,
        days: 0,
        microseconds: 3_600_000_000,
    };
    assert!(matches!(
        Encode::<Postgres>::encode(&interval, &mut buf),
        IsNull::No
    ));
    assert_eq!(
        &**buf,
        [0, 0, 0, 0, 214, 147, 164, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    buf.clear();

    let interval = PgInterval {
        months: 0,
        days: 1,
        microseconds: 0,
    };
    assert!(matches!(
        Encode::<Postgres>::encode(&interval, &mut buf),
        IsNull::No
    ));
    assert_eq!(&**buf, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0]);
    buf.clear();

    let interval = PgInterval {
        months: 1,
        days: 0,
        microseconds: 0,
    };
    assert!(matches!(
        Encode::<Postgres>::encode(&interval, &mut buf),
        IsNull::No
    ));
    assert_eq!(&**buf, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    buf.clear();
}

#[test]
fn test_pginterval_std() {
    let interval = PgInterval {
        days: 0,
        months: 0,
        microseconds: 27_000,
    };
    assert_eq!(
        &PgInterval::try_from(std::time::Duration::from_micros(27_000)).unwrap(),
        &interval
    );
}

#[test]
#[cfg(feature = "chrono")]
fn test_pginterval_chrono() {
    let interval = PgInterval {
        days: 0,
        months: 0,
        microseconds: 27_000,
    };
    assert_eq!(
        &PgInterval::try_from(chrono::Duration::microseconds(27_000)).unwrap(),
        &interval
    );
}

#[test]
#[cfg(feature = "time")]
fn test_pginterval_time() {
    let interval = PgInterval {
        days: 0,
        months: 0,
        microseconds: 27_000,
    };
    assert_eq!(
        &PgInterval::try_from(time::Duration::microseconds(27_000)).unwrap(),
        &interval
    );
}
