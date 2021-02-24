use core::marker::PhantomData;
use core::time::Duration;
use serde::de::DeserializeSeed;
use serde::de::SeqAccess;
use serde::de::Visitor;
use serde::Deserialize;
use serde::Deserializer;

pub fn visit_seq<'de, F, V>(callback: F, expecting: &'static str) -> VisitSeq<F, V>
where
    F: FnMut(V, usize),
    V: Deserialize<'de>,
{
    VisitSeq::new(callback, expecting)
}

pub struct VisitSeq<F, V> {
    callback: F,
    expecting: &'static str,
    _marker: PhantomData<fn(V)>,
}

impl<F, V> VisitSeq<F, V>
where
    F: FnMut(V, usize),
{
    fn new(callback: F, expecting: &'static str) -> Self {
        VisitSeq {
            callback,
            expecting,
            _marker: PhantomData,
        }
    }
}

impl<'de, F, V> Visitor<'de> for VisitSeq<F, V>
where
    F: FnMut(V, usize),
    V: Deserialize<'de>,
{
    type Value = ();

    fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(formatter, "{}", self.expecting)
    }

    fn visit_seq<S>(mut self, mut seq: S) -> Result<(), S::Error>
    where
        S: SeqAccess<'de>,
    {
        let mut index = 0;
        while let Some(value) = seq.next_element()? {
            (self.callback)(value, index);
            index += 1;
        }
        Ok(())
    }
}

impl<'de, F, V> DeserializeSeed<'de> for VisitSeq<F, V>
where
    F: FnMut(V, usize),
    V: Deserialize<'de>,
{
    type Value = ();

    fn deserialize<T>(self, deserializer: T) -> Result<Self::Value, T::Error>
    where
        T: Deserializer<'de>,
    {
        deserializer.deserialize_seq(self)
    }
}

pub fn offset_duration(duration: Duration, offset_micros: i32) -> Duration {
    let abs_offset = Duration::from_micros(offset_micros.abs() as u64);
    if offset_micros.is_negative() {
        duration - abs_offset
    } else {
        duration + abs_offset
    }
}
