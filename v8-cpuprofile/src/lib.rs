#![deny(clippy::all, clippy::pedantic)]
#![no_std]
#![feature(option_insert)]
extern crate alloc;

use crate::ser::MakeIter;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::ops::Index;
use core::slice::Chunks;
use core::time::Duration;
use hashbrown::HashMap;
use hashbrown::HashSet;
use serde::Serialize;
use serde_json::value::RawValue;

mod de;
mod ser;

#[derive(Debug, Default, Copy, Clone, Eq)]
pub struct Sample {
    pub node_id: u64,
    pub ts: Duration,
}

/// samples should have unique timestamps and `node_id` is just a foreign key
impl PartialEq for Sample {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.ts == other.ts
    }
}

impl PartialOrd for Sample {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl Ord for Sample {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.ts.cmp(&other.ts)
    }
}

#[derive(Debug)]
pub struct Profile<'raw> {
    pub nodes: Vec<Node<'raw>>,
    pub start_time: Duration,
    pub end_time: Duration,
    pub samples: Vec<Sample>,
    node_index: HashMap<u64, usize>,
}

impl<'raw> Profile<'raw> {
    pub fn parent_ids_iter(&self, node_id: u64) -> impl Iterator<Item = u64> + '_ {
        ParentIter {
            profile: self,
            node_id: Some(node_id),
        }
    }

    #[must_use]
    pub fn chunks<'profile>(&'profile self, chunk_num: usize) -> ProfileChunks<'profile, 'raw> {
        let chunk_size = div_ceil(self.samples.len(), chunk_num);
        ProfileChunks(self, self.samples.chunks(chunk_size))
    }
}

impl<'raw> Index<u64> for Profile<'raw> {
    type Output = Node<'raw>;

    #[inline]
    fn index(&self, node_id: u64) -> &Self::Output {
        &self.nodes[self.node_index[&node_id]]
    }
}

struct ParentIter<'p, 'raw> {
    profile: &'p Profile<'raw>,
    node_id: Option<u64>,
}

impl<'p, 'raw> Iterator for ParentIter<'p, 'raw> {
    type Item = u64;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.node_id.and_then(|node_id| {
            let parent_id = self.profile[node_id].parent_id;
            self.node_id = parent_id;
            parent_id
        })
    }
}

#[derive(Debug)]
pub struct Node<'raw> {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub call_frame: &'raw RawValue,
    pub hit_count: u32,
    pub children: Option<Vec<u64>>,
    pub deopt_reason: Option<&'raw RawValue>,
    pub position_ticks: Option<&'raw RawValue>,
}

#[derive(Debug)]
pub struct ProfileChunk<'profile, 'raw> {
    profile: &'profile Profile<'raw>,
    samples: &'profile [Sample],
    included: HashSet<u64>,
}

impl<'profile, 'raw> ProfileChunk<'profile, 'raw> {
    #[must_use]
    pub fn new(profile: &'profile Profile<'raw>, samples: &'profile [Sample]) -> Self {
        let mut included = HashSet::new();
        for sample in samples {
            let node_id = sample.node_id;
            if included.insert(node_id) {
                for parent_id in profile.parent_ids_iter(node_id) {
                    if !included.insert(parent_id) {
                        break;
                    }
                }
            }
        }
        ProfileChunk {
            profile,
            samples,
            included,
        }
    }

    #[must_use]
    pub fn nodes(
        &self,
    ) -> impl IntoIterator<Item = FilteredNode<'profile, 'raw, '_>> + Serialize + '_ {
        MakeIter::from(move || {
            self.profile.nodes.iter().filter_map(move |node| {
                if self.included.contains(&node.id) {
                    Some(FilteredNode(node, &self.included))
                } else {
                    None
                }
            })
        })
    }
}

pub struct FilteredNode<'profile, 'raw, 'set>(&'profile Node<'raw>, &'set HashSet<u64>);

impl FilteredNode<'_, '_, '_> {
    fn children(&self) -> Option<impl IntoIterator<Item = u64> + Serialize + '_> {
        self.0.children.as_ref().map(move |children| {
            MakeIter::from(move || {
                children
                    .iter()
                    .filter_map(move |id| if self.1.contains(id) { Some(*id) } else { None })
            })
        })
    }
}

pub struct ProfileChunks<'profile, 'raw>(&'profile Profile<'raw>, Chunks<'profile, Sample>);

impl<'profile, 'raw> Iterator for ProfileChunks<'profile, 'raw> {
    type Item = ProfileChunk<'profile, 'raw>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let profile = self.0;
        self.1
            .next()
            .map(move |samples| ProfileChunk::new(profile, samples))
    }
}

fn div_ceil(n: usize, d: usize) -> usize {
    (n + d - 1) / d
}
