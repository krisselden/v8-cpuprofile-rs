use crate::Sample;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::time::Duration;
use hashbrown::HashMap;
use serde::de::Error;
use serde::de::MapAccess;
use serde::de::Visitor;

use super::util::{offset_duration, visit_seq};
use crate::{Node, Profile};

pub(super) fn node<'de: 'raw, 'raw>() -> impl Visitor<'de, Value = Node<'raw>> {
    NodeVisitor(PhantomData)
}

pub(super) fn profile<'de: 'raw, 'raw>() -> impl Visitor<'de, Value = Profile<'raw>> {
    ProfileVisitor(PhantomData)
}

macro_rules! check_missing {
    ($error:ty, $field:ident) => {
        match $field {
            Some(value) => value,
            None => return Err(<$error>::missing_field(stringify!($field))),
        }
    };
    ($error:ty, $field:ident, $name:expr) => {
        match $field {
            Some(value) => value,
            None => return Err(<$error>::missing_field($name)),
        }
    };
}

struct NodeVisitor<'a>(PhantomData<fn() -> Node<'a>>);

const NODE_FIELDS: &[&str] = &[
    "id",
    "callFrame",
    "hitCount",
    "children",
    "deoptReason",
    "positionTicks",
];

impl<'de: 'raw, 'raw> Visitor<'de> for NodeVisitor<'raw> {
    type Value = Node<'raw>;

    fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        formatter.write_str("v8 profile node json")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut id = None;
        let mut call_frame = None;
        let mut hit_count = None;
        let mut children = None;
        let mut deopt_reason = None;
        let mut position_ticks = None;

        while let Some(key) = access.next_key()? {
            match key {
                "id" => {
                    id = access.next_value()?;
                }
                "callFrame" => {
                    call_frame = access.next_value()?;
                }
                "hitCount" => {
                    hit_count = access.next_value()?;
                }
                "children" => {
                    children = access.next_value()?;
                }
                "deoptReason" => {
                    deopt_reason = access.next_value()?;
                }
                "positionTicks" => {
                    position_ticks = access.next_value()?;
                }
                _ => {
                    return Err(M::Error::unknown_field(key, NODE_FIELDS));
                }
            }
        }

        let id = check_missing!(M::Error, id);
        let call_frame = check_missing!(M::Error, call_frame, "callFrame");
        let hit_count = check_missing!(M::Error, hit_count, "hitCount");

        Ok(Node {
            id,
            parent_id: None,
            call_frame,
            hit_count,
            children,
            deopt_reason,
            position_ticks,
        })
    }
}

const PROFILE_FIELDS: &[&str] = &["nodes", "startTime", "endTime", "samples", "timeDeltas"];

struct ProfileVisitor<'raw>(PhantomData<fn() -> Profile<'raw>>);

impl<'de: 'raw, 'raw> Visitor<'de> for ProfileVisitor<'raw> {
    type Value = Profile<'raw>;

    fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        formatter.write_str("v8 cpuprofile json object")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut node_index: HashMap<u64, usize> = HashMap::new();
        let mut parent_ids: Vec<(u64, u64)> = Vec::new();
        let mut nodes: Option<Vec<Node<'raw>>> = None;
        let mut start_time = None;
        let mut end_time = None;
        let mut samples: Vec<Sample> = Vec::new();
        let mut has_samples = false;
        let mut has_time_deltas = false;
        let mut current = Duration::default();
        while let Some(key) = access.next_key()? {
            match key {
                "nodes" => {
                    let inner = nodes.insert(Vec::new());
                    access.next_value_seed(visit_seq(
                        |node: Node, index| {
                            node_index.insert(node.id, index);
                            if let Some(ref children) = node.children {
                                parent_ids
                                    .extend(children.iter().map(|&child_id| (node.id, child_id)));
                            }
                            inner.push(node);
                        },
                        "a sequence of v8 profile nodes",
                    ))?;
                }
                "startTime" => {
                    start_time = access.next_value()?;
                }
                "endTime" => {
                    end_time = access.next_value()?;
                }
                "samples" => {
                    has_samples = true;
                    access.next_value_seed(visit_seq(
                        |node_id: u64, index| {
                            if let Some(sample) = samples.get_mut(index) {
                                sample.node_id = node_id;
                            } else {
                                samples.insert(
                                    index,
                                    Sample {
                                        node_id,
                                        ts: Duration::default(),
                                    },
                                );
                            }
                        },
                        "a sequence of node ids",
                    ))?;
                }
                "timeDeltas" => {
                    has_time_deltas = true;
                    access.next_value_seed(visit_seq(
                        |delta: i32, index| {
                            current = offset_duration(current, delta);
                            if let Some(sample) = samples.get_mut(index) {
                                sample.ts = current;
                            } else {
                                samples.insert(
                                    index,
                                    Sample {
                                        node_id: 0,
                                        ts: current,
                                    },
                                );
                            }
                        },
                        "a sequence of time deltas",
                    ))?;
                }
                _ => {
                    return Err(M::Error::unknown_field(key, PROFILE_FIELDS));
                }
            }
        }
        let mut nodes = check_missing!(M::Error, nodes);
        let start_time = check_missing!(M::Error, start_time, "startTime");
        let end_time = check_missing!(M::Error, end_time, "endTime");

        for (parent_id, ref node_id) in parent_ids {
            let node = &mut nodes[node_index[node_id]];
            node.parent_id = Some(parent_id);
        }

        if !has_samples {
            return Err(M::Error::missing_field("samples"));
        }

        if !has_time_deltas {
            return Err(M::Error::missing_field("timeDeltas"));
        }

        samples.sort();

        Ok(Profile {
            nodes,
            start_time: Duration::from_micros(start_time),
            end_time: Duration::from_micros(end_time),
            samples,
            node_index,
        })
    }
}
