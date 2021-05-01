use std::fmt;
use std::time;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A type that provides "nicer" Display and Serialize impls for
/// std::time::Duration. The serialization format should actually be compatible
/// with the Deserialize impl for std::time::Duration, since this type only
/// adds new fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NiceDuration(pub time::Duration);

impl fmt::Display for NiceDuration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:0.6}s", self.fractional_seconds())
    }
}

impl NiceDuration {
    /// Returns the number of seconds in this duration in fraction form.
    /// The number to the left of the decimal point is the number of seconds,
    /// and the number to the right is the number of milliseconds.
    fn fractional_seconds(&self) -> f64 {
        let fractional = (self.0.subsec_nanos() as f64) / 1_000_000_000.0;
        self.0.as_secs() as f64 + fractional
    }
}

impl Serialize for NiceDuration {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut state = ser.serialize_struct("Duration", 2)?;
        state.serialize_field("secs", &self.0.as_secs())?;
        state.serialize_field("nanos", &self.0.subsec_nanos())?;
        state.serialize_field("human", &format!("{}", self))?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for NiceDuration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct StdDuration {
            secs: u64,
            nanos: u32,
        }

        let deserialized = StdDuration::deserialize(deserializer)?;

        Ok(NiceDuration(time::Duration::new(
            deserialized.secs,
            deserialized.nanos,
        )))
    }
}

#[test]
fn test_nice_duration_serde() {
    let duration = time::Duration::new(10, 20);
    let nice_duration = NiceDuration(duration);
    let seralized = serde_json::to_string(&nice_duration).unwrap();
    let deserialized: NiceDuration = serde_json::from_str(&seralized).unwrap();
    assert_eq!(deserialized, NiceDuration(time::Duration::new(10, 20)));
}
