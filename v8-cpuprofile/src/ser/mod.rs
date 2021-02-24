mod util;

use crate::FilteredNode;
use crate::Node;
use crate::Profile;
use crate::ProfileChunk;
use crate::Sample;
use core::time::Duration;
use serde::ser::SerializeMap;
use serde::Serialize;
use serde::Serializer;
use serde_json::value::RawValue;
pub(crate) use util::MakeIter;

impl Serialize for Node<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_node(
            serializer,
            self.id,
            self.call_frame,
            self.hit_count,
            self.children.as_ref(),
            self.deopt_reason,
            self.position_ticks,
        )
    }
}

impl Serialize for Profile<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_profile(
            serializer,
            &self.nodes,
            &self.start_time,
            &self.end_time,
            &self.samples,
        )
    }
}

fn serialize_node<'raw, S, C>(
    serializer: S,
    id: u64,
    call_frame: &'raw RawValue,
    hit_count: u32,
    children: Option<&C>,
    deopt_reason: Option<&'raw RawValue>,
    position_ticks: Option<&'raw RawValue>,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    C: Serialize,
{
    let mut map = serializer.serialize_map(None)?;
    map.serialize_entry(&"id", &id)?;
    map.serialize_entry(&"callFrame", call_frame)?;
    map.serialize_entry(&"hitCount", &hit_count)?;
    if let Some(ref children) = children {
        map.serialize_entry(&"children", children)?;
    }
    if let Some(deopt_reason) = deopt_reason {
        map.serialize_entry(&"deoptReason", deopt_reason)?;
    }
    if let Some(position_ticks) = position_ticks {
        map.serialize_entry(&"positionTicks", position_ticks)?;
    }
    map.end()
}

fn serialize_profile<'raw, 'iter, S, N, I>(
    serializer: S,
    nodes: &N,
    start_time: &Duration,
    end_time: &Duration,
    samples: I,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    N: Serialize,
    I: IntoIterator<Item = &'iter Sample> + Copy,
{
    let mut map = serializer.serialize_map(None)?;
    map.serialize_entry("nodes", &nodes)?;
    map.serialize_entry("startTime", &start_time.as_micros())?;
    map.serialize_entry("endTime", &end_time.as_micros())?;
    let sample_node_ids: MakeIter<_> = (|| samples.into_iter().map(|s| s.node_id)).into();
    map.serialize_entry("samples", &sample_node_ids)?;
    let sample_time_deltas: MakeIter<_> = (|| {
        let mut last = 0;
        samples.into_iter().map(move |sample| {
            let ts = sample.ts.as_micros();
            let delta = ts - last;
            last = ts;
            delta
        })
    })
    .into();
    map.serialize_entry("timeDeltas", &sample_time_deltas)?;
    map.end()
}

impl Serialize for ProfileChunk<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_profile(
            serializer,
            &self.nodes(),
            &self.profile.start_time,
            &self.profile.end_time,
            self.samples,
        )
    }
}

impl Serialize for FilteredNode<'_, '_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_node(
            serializer,
            self.0.id,
            self.0.call_frame,
            self.0.hit_count,
            self.children().as_ref(),
            self.0.deopt_reason,
            self.0.position_ticks,
        )
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    #[test]
    fn round_trip_serialization() {
        const PROFILE: &str = core::include_str!("../../tests/fixture.cpuprofile");

        let profile: crate::Profile<'_> = serde_json::from_str(PROFILE).unwrap();

        let json = serde_json::to_string(&profile).unwrap();

        assert_eq!(profile.samples.len(), 28);

        let node = &profile[profile.samples[0].node_id];

        assert_eq!(node.parent_id, Some(1));

        let parent_ids: Vec<_> = profile.parent_ids_iter(node.id).collect();

        assert_eq!(parent_ids, [1]);

        assert_eq!(json, PROFILE);
    }
}
