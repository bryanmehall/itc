// Mirror `IdTree`, `EventTree`, and `Stamp` types for nice json serialization in the form [4, [0, 1, 0], 1] etc

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{EventTree, IdTree, Stamp};

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum TupleIdTree {
    Leaf(u8),
    Node(Box<TupleIdTree>, Box<TupleIdTree>),
}

impl From<&IdTree> for TupleIdTree {
    fn from(id_tree: &IdTree) -> Self {
        match id_tree {
            IdTree::Leaf { i } => TupleIdTree::Leaf(*i as u8),
            IdTree::Node { left, right } => TupleIdTree::Node(
                Box::new(TupleIdTree::from(left.as_ref())),
                Box::new(TupleIdTree::from(right.as_ref())),
            ),
        }
    }
}

impl From<&TupleIdTree> for IdTree {
    fn from(tuple_id_tree: &TupleIdTree) -> Self {
        match tuple_id_tree {
            TupleIdTree::Leaf(i) => IdTree::Leaf { i: *i == 1 }, // convert the integer back to bool
            TupleIdTree::Node(left, right) => IdTree::Node {
                left: Box::new(IdTree::from(left.as_ref())),
                right: Box::new(IdTree::from(right.as_ref())),
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum TupleEventTree {
    Leaf(u32),
    Node(Box<TupleEventTree>, u32, Box<TupleEventTree>),
}

impl From<&EventTree> for TupleEventTree {
    fn from(event_tree: &EventTree) -> Self {
        match event_tree {
            EventTree::Leaf { n } => TupleEventTree::Leaf(*n),
            EventTree::Node { n, left, right } => TupleEventTree::Node(
                Box::new(TupleEventTree::from(left.as_ref())),
                *n,
                Box::new(TupleEventTree::from(right.as_ref())),
            ),
        }
    }
}

impl From<&TupleEventTree> for EventTree {
    fn from(tuple_event_tree: &TupleEventTree) -> Self {
        match tuple_event_tree {
            TupleEventTree::Leaf(n) => EventTree::Leaf { n: *n },
            TupleEventTree::Node(left, n, right) => EventTree::Node {
                n: *n,
                left: Box::new(EventTree::from(left.as_ref())),
                right: Box::new(EventTree::from(right.as_ref())),
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
struct TupleStamp {
    id: TupleIdTree,
    event: TupleEventTree,
}

impl Serialize for Stamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        TupleStamp {
            id: TupleIdTree::from(&self.i),
            event: TupleEventTree::from(&self.e),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Stamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(|TupleStamp { id, event }| Stamp {
            i: IdTree::from(&id),
            e: EventTree::from(&event),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    /// Expect that serializing the empty stamp gives the expected string and
    fn empty() {
        let stamp = Stamp::seed();
        let serialized = serde_json::to_string(&stamp).unwrap();
        assert_eq!(serialized, "{\"id\":1,\"event\":0}");
        let new_stamp: Stamp = serde_json::from_str(&serialized).unwrap();
        assert_eq!(stamp, new_stamp);
    }

    #[test]
    fn complex() {
        let stamp = Stamp::new(
            IdTree::node(
                Box::new(IdTree::node(
                    Box::new(IdTree::one()),
                    Box::new(IdTree::zero()),
                )),
                Box::new(IdTree::zero()),
            ),
            EventTree::node(
                0,
                Box::new(EventTree::node(
                    1,
                    Box::new(EventTree::leaf(1)),
                    Box::new(EventTree::zero()),
                )),
                Box::new(EventTree::zero()),
            ),
        );
        let serialized = serde_json::to_string(&stamp).unwrap();
        assert_eq!(serialized, "{\"id\":[[1,0],0],\"event\":[[1,1,0],0,0]}");
        let new_stamp: Stamp = serde_json::from_str(&serialized).unwrap();
        assert_eq!(stamp, new_stamp);
    }
}
