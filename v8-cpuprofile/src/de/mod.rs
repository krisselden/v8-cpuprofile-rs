mod util;
mod visitors;

use crate::Node;
use crate::Profile;
use serde::Deserialize;
use serde::Deserializer;

impl<'de: 'r, 'r> Deserialize<'de> for Profile<'r> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(visitors::profile())
    }
}

impl<'de: 'r, 'r> Deserialize<'de> for Node<'r> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(visitors::node())
    }
}
