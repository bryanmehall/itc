//! # Interval Tree Clocks
//!
//! The itc crate implements Interval Tree Clocks as described in
//! http://gsd.di.uminho.pt/members/cbm/ps/itc2008.pdf
//!
//! # Usage:
//!
//! ```
//! use itc::*;
//!
//! let s = Stamp::seed();
//!
//! let (s1, s2) = s.fork();
//! let s1prime = s1.event();
//! let s2prime = s2.event();
//! let s3 = s2prime.join(&s1);
//!
//! assert!(s.leq(&s1));
//! assert!(s1.leq(&s1prime));
//! assert!(!s1prime.leq(&s2prime));
//! assert!(s2prime.leq(&s3));
//! ```
//!
//! This module implements the 4 verbs: fork, event, join, peek,
//! the 3 derived verbs: send, receive and sync,
//! and a partial ordering to establish causality / the happens-before relation.
//! Also in the box is a simple ascii coding representation suitable
//! for printing to stdout and use in protocols.

use std::borrow::Cow;
use std::cmp;

//pub mod ascii_coding;
pub mod cost;

use cost::*;

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum IdTree {
    Leaf {
        i: bool,
    },
    Node {
        left: Box<IdTree>,
        right: Box<IdTree>,
    },
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq )]
pub enum EventTree {
    Leaf {
        n: u32,
    },
    Node {
        n: u32,
        left: Box<EventTree>,
        right: Box<EventTree>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stamp {
    i: IdTree,
    e: EventTree,
}

impl IdTree {
    pub fn leaf(i: bool) -> IdTree {
        IdTree::Leaf { i: i }
    }

    pub fn zero() -> IdTree {
        IdTree::Leaf { i: false }
    }

    pub fn one() -> IdTree {
        IdTree::Leaf { i: true }
    }

    pub fn node(left: Box<IdTree>, right: Box<IdTree>) -> IdTree {
        IdTree::Node {
            left: left,
            right: right,
        }
    }
}

impl EventTree {
    pub fn zero() -> EventTree {
        EventTree::leaf(0)
    }

    pub fn leaf(n: u32) -> EventTree {
        EventTree::Leaf { n: n }
    }

    pub fn node(n: u32, left: Box<EventTree>, right: Box<EventTree>) -> EventTree {
        EventTree::Node {
            n: n,
            left: left,
            right: right,
        }
    }

    pub fn n(&self) -> u32 {
        match self {
            &EventTree::Leaf { n } => n,
            &EventTree::Node { n, .. } => n,
        }
    }

    pub fn lift(self, m: u32) -> EventTree {
        match self {
            EventTree::Leaf { n } => EventTree::leaf(n + m),
            EventTree::Node { n, left, right } => EventTree::node(n + m, left, right),
        }
    }

    pub fn sink(self, m: u32) -> EventTree {
        match self {
            EventTree::Leaf { n } => EventTree::leaf(n - m),
            EventTree::Node { n, left, right } => EventTree::node(n - m, left, right),
        }
    }

    pub fn join(&self, other: &EventTree) -> EventTree {
        match *self {
            EventTree::Leaf { n: n1 } => match *other {
                EventTree::Leaf { n: n2 } => EventTree::leaf(cmp::max(n1, n2)),
                EventTree::Node { .. } => {
                    let new_left = EventTree::node(
                        n1,
                        Box::new(EventTree::zero()),
                        Box::new(EventTree::zero()),
                    );
                    new_left.join(other)
                }
            },
            EventTree::Node {
                n: n1,
                left: ref left1,
                right: ref right1,
            } => match *other {
                EventTree::Leaf { n: n2 } => {
                    let new_right = EventTree::node(
                        n2,
                        Box::new(EventTree::zero()),
                        Box::new(EventTree::zero()),
                    );
                    self.join(&new_right)
                }
                EventTree::Node {
                    n: n2,
                    left: ref left2,
                    right: ref right2,
                } => {
                    if n1 > n2 {
                        other.join(self)
                    } else {
                        let new_left = left1.join(&left2.clone().lift(n2 - n1));
                        let new_right = right1.join(&right2.clone().lift(n2 - n1));
                        EventTree::node(n1, Box::new(new_left), Box::new(new_right)).norm()
                    }
                }
            },
        }
    }
}

impl Stamp {
    pub fn seed() -> Stamp {
        Stamp::new(IdTree::one(), EventTree::zero())
    }

    pub fn new(i: IdTree, e: EventTree) -> Stamp {
        Stamp { i: i, e: e }
    }

    pub fn fill<'a>(&'a self) -> Cow<'a, EventTree> {
        if self.i == IdTree::zero() {
            Cow::Borrowed(&self.e)
        } else if self.i == IdTree::one() {
            Cow::Owned(EventTree::leaf(self.e.max()))
        } else if let EventTree::Leaf { .. } = self.e {
            Cow::Borrowed(&self.e)
        } else {
            if let IdTree::Node {
                left: ref i_left,
                right: ref i_right,
            } = self.i
            {
                if let EventTree::Node {
                    n,
                    left: ref e_left,
                    right: ref e_right,
                } = self.e
                {
                    if i_left.as_ref() == &IdTree::one() {
                        let eprime_right =
                            Stamp::new(i_right.as_ref().clone(), e_right.as_ref().clone())
                                .fill()
                                .into_owned();
                        let new_left = EventTree::leaf(cmp::max(e_left.max(), eprime_right.min()));
                        Cow::Owned(
                            EventTree::node(n, Box::new(new_left), Box::new(eprime_right)).norm(),
                        )
                    } else if i_right.as_ref() == &IdTree::one() {
                        let eprime_left =
                            Stamp::new(i_left.as_ref().clone(), e_left.as_ref().clone())
                                .fill()
                                .into_owned();
                        let new_right = EventTree::leaf(cmp::max(e_right.max(), eprime_left.min()));
                        Cow::Owned(
                            EventTree::node(n, Box::new(eprime_left), Box::new(new_right)).norm(),
                        )
                    } else {
                        let new_left = Stamp::new(i_left.as_ref().clone(), e_left.as_ref().clone())
                            .fill()
                            .into_owned();
                        let new_right =
                            Stamp::new(i_right.as_ref().clone(), e_right.as_ref().clone())
                                .fill()
                                .into_owned();
                        Cow::Owned(
                            EventTree::node(n, Box::new(new_left), Box::new(new_right)).norm(),
                        )
                    }
                } else {
                    unreachable!()
                }
            } else {
                unreachable!()
            }
        }
    }

    // returns event tree and cost
    pub fn grow(&self) -> (EventTree, Cost) {
        match self.e {
            EventTree::Leaf { n } => {
                if self.i == IdTree::one() {
                    (EventTree::leaf(n + 1), Cost::zero())
                } else {
                    let new_e = EventTree::node(
                        n,
                        Box::new(EventTree::zero()),
                        Box::new(EventTree::zero()),
                    );
                    let (eprime, c) = Stamp::new(self.i.clone(), new_e).grow();
                    (eprime, c.shift())
                }
            }
            EventTree::Node {
                n,
                left: ref e_left,
                right: ref e_right,
            } => {
                if let IdTree::Node {
                    left: ref i_left,
                    right: ref i_right,
                } = self.i
                {
                    if **i_left == IdTree::zero() {
                        let (eprime_right, c_right) =
                            Stamp::new(i_right.as_ref().clone(), e_right.as_ref().clone()).grow();
                        (
                            EventTree::node(n, e_left.clone(), Box::new(eprime_right)),
                            c_right + 1,
                        )
                    } else if **i_right == IdTree::zero() {
                        let (eprime_left, c_left) =
                            Stamp::new(*i_left.clone(), *e_left.clone()).grow();
                        (
                            EventTree::node(n, Box::new(eprime_left), e_right.clone()),
                            c_left + 1,
                        )
                    } else {
                        let (eprime_right, c_right) =
                            Stamp::new(*i_right.clone(), *e_right.clone()).grow();
                        let (eprime_left, c_left) =
                            Stamp::new(*i_left.clone(), *e_left.clone()).grow();
                        if c_left < c_right {
                            (
                                EventTree::node(n, Box::new(eprime_left), e_right.clone()),
                                c_left + 1,
                            )
                        } else {
                            (
                                EventTree::node(n, e_left.clone(), Box::new(eprime_right)),
                                c_right + 1,
                            )
                        }
                    }
                } else {
                    // corrupted tree?
                    unreachable!()
                }
            }
        }
    }

    pub fn id_tree(&self) -> IdTree {
        self.i.clone()
    }
    pub fn event_tree(&self) -> EventTree {
        self.e.clone()
    }
}

pub trait Min<T> {
    fn min(&self) -> T;
}

pub trait Max<T> {
    fn max(&self) -> T;
}

impl Min<u32> for EventTree {
    fn min(&self) -> u32 {
        match *self {
            EventTree::Leaf { n } => n,
            EventTree::Node {
                n,
                ref left,
                ref right,
            } => n + cmp::min(left.min(), right.min()),
        }
    }
}

impl Max<u32> for EventTree {
    fn max(&self) -> u32 {
        match *self {
            EventTree::Leaf { n } => n,
            EventTree::Node {
                n,
                ref left,
                ref right,
            } => n + cmp::max(left.max(), right.max()),
        }
    }
}

pub trait Normalisable {
    fn norm(self) -> Self;
}

impl Normalisable for IdTree {
    #[allow(non_shorthand_field_patterns)]
    fn norm(self) -> IdTree {
        match self {
            IdTree::Leaf { i: _ } => {
                return self;
            }
            IdTree::Node { left, right } => {
                let norm_left = left.norm();
                let norm_right = right.norm();

                if let IdTree::Leaf { i: i1 } = norm_left {
                    if let IdTree::Leaf { i: i2 } = norm_right {
                        if i1 == i2 {
                            return norm_left;
                        }
                    }
                }

                return IdTree::node(Box::new(norm_left), Box::new(norm_right));
            }
        };
    }
}

impl Normalisable for EventTree {
    fn norm(self) -> EventTree {
        match self {
            EventTree::Leaf { n: _ } => {
                return self;
            }
            EventTree::Node { n, left, right } => {
                let norm_left = left.norm();
                let norm_right = right.norm();

                if let EventTree::Leaf { n: m1 } = norm_left {
                    if let EventTree::Leaf { n: m2 } = norm_right {
                        if m1 == m2 {
                            return EventTree::leaf(n + m1);
                        }
                    }
                }

                // normalised trees have min == n
                let min_left = norm_left.n();
                let min_right = norm_right.n();

                let m = cmp::min(min_left, min_right);

                return EventTree::node(
                    n + m,
                    Box::new(norm_left.sink(m)),
                    Box::new(norm_right.sink(m)),
                );
            }
        }
    }
}

impl Normalisable for Stamp {
    fn norm(self) -> Stamp {
        Stamp::new(self.i.norm(), self.e.norm())
    }
}

pub trait LessThanOrEqual {
    fn leq(&self, other: &Self) -> bool;
}

impl LessThanOrEqual for Stamp {
    fn leq(&self, other: &Stamp) -> bool {
        self.e.leq(&other.e)
    }
}

impl LessThanOrEqual for EventTree {
    #[allow(non_shorthand_field_patterns)]
    fn leq(&self, other: &EventTree) -> bool {
        match *self {
            EventTree::Leaf { n: n1 } => match *other {
                EventTree::Leaf { n: n2 } => n1 <= n2,
                EventTree::Node { n: n2, .. } => n1 <= n2,
            },
            EventTree::Node {
                n: n1,
                left: ref left1,
                right: ref right1,
            } => match *other {
                EventTree::Leaf { n: n2 } => {
                    (n1 <= n2)
                        && left1.clone().lift(n1).leq(&EventTree::leaf(n2))
                        && right1.clone().lift(n1).leq(&EventTree::leaf(n2))
                }
                EventTree::Node {
                    n: n2,
                    left: ref left2,
                    right: ref right2,
                } => {
                    (n1 <= n2)
                        && left1.clone().lift(n1).leq(&left2.clone().lift(n2))
                        && right1.clone().lift(n1).leq(&right2.clone().lift(n2))
                }
            },
        }
    }
}

pub trait Split {
    fn split(&self) -> Self;
}

impl Split for IdTree {
    fn split(&self) -> IdTree {
        match *self {
            IdTree::Leaf { i } => {
                if !i {
                    IdTree::node(Box::new(IdTree::zero()), Box::new(IdTree::zero()))
                } else {
                    let new_left = Box::new(IdTree::node(
                        Box::new(IdTree::one()),
                        Box::new(IdTree::zero()),
                    ));
                    let new_right = Box::new(IdTree::node(
                        Box::new(IdTree::zero()),
                        Box::new(IdTree::one()),
                    ));
                    IdTree::node(new_left, new_right)
                }
            }
            IdTree::Node {
                ref left,
                ref right,
            } => {
                if *left.as_ref() == IdTree::zero() {
                    // split always returns a Node, not a Leaf
                    if let IdTree::Node {
                        left: i1,
                        right: i2,
                    } = right.split()
                    {
                        let new_left = Box::new(IdTree::node(Box::new(IdTree::zero()), i1));
                        let new_right = Box::new(IdTree::node(Box::new(IdTree::zero()), i2));
                        IdTree::node(new_left, new_right)
                    } else {
                        unreachable!()
                    }
                } else if *right.as_ref() == IdTree::zero() {
                    if let IdTree::Node {
                        left: i1,
                        right: i2,
                    } = left.split()
                    {
                        let new_left = Box::new(IdTree::node(i1, Box::new(IdTree::zero())));
                        let new_right = Box::new(IdTree::node(i2, Box::new(IdTree::zero())));
                        IdTree::node(new_left, new_right)
                    } else {
                        unreachable!()
                    }
                } else {
                    let new_left = Box::new(IdTree::node(left.clone(), Box::new(IdTree::zero())));
                    let new_right = Box::new(IdTree::node(Box::new(IdTree::zero()), right.clone()));
                    IdTree::node(new_left, new_right)
                }
            }
        }
    }
}

pub trait Sum {
    fn sum(&self, other: &Self) -> Self;
}

impl Sum for IdTree {
    #[allow(non_shorthand_field_patterns)]
    fn sum(&self, other: &IdTree) -> IdTree {
        if *self == IdTree::zero() {
            return other.clone();
        } else if *other == IdTree::zero() {
            return self.clone();
        }

        if let IdTree::Node {
            left: ref left1,
            right: ref right1,
        } = *self
        {
            if let IdTree::Node {
                left: ref left2,
                right: ref right2,
            } = *other
            {
                let new_left = Box::new(left1.sum(left2));
                let new_right = Box::new(right1.sum(right2));
                return IdTree::node(new_left, new_right).norm();
            } else {
                // corrupted tree?
                unreachable!();
            }
        } else {
            // corrupted tree?
            unreachable!();
        }
    }
}

pub trait IntervalTreeClock
where
    Self: Sized,
{
    fn fork(&self) -> (Self, Self);
    fn peek(&self) -> (Self, Self);
    fn join(&self, other: &Self) -> Self;
    fn event(&self) -> Self;

    fn send(&self) -> (Self, Self);
    fn receive(&self, other: &Self) -> Self;
    fn sync(&self, other: &Self) -> (Self, Self);
}

impl IntervalTreeClock for Stamp {
    fn peek(&self) -> (Stamp, Stamp) {
        let s1 = Stamp::new(IdTree::zero(), self.e.clone());
        let s2 = Stamp::new(self.i.clone(), self.e.clone());
        return (s1, s2);
    }

    fn fork(&self) -> (Stamp, Stamp) {
        if let IdTree::Node { left, right } = self.i.split() {
            let s1 = Stamp::new(*left, self.e.clone());
            let s2 = Stamp::new(*right, self.e.clone());
            (s1, s2)
        } else {
            unreachable!()
        }
    }

    fn join(&self, other: &Stamp) -> Stamp {
        let sum_i = self.i.sum(&other.i);
        let join_e = self.e.join(&other.e);
        Stamp::new(sum_i, join_e)
    }

    fn event(&self) -> Stamp {
        let filled_e = self.fill();

        if filled_e.as_ref() != &self.e {
            Stamp::new(self.i.clone(), filled_e.into_owned())
        } else {
            let (eprime, _c) = self.grow();

            Stamp::new(self.i.clone(), eprime)
        }
    }

    fn send(&self) -> (Stamp, Stamp) {
        self.event().peek()
    }

    fn receive(&self, other: &Stamp) -> Stamp {
        self.join(other).event()
    }

    fn sync(&self, other: &Stamp) -> (Stamp, Stamp) {
        self.join(other).fork()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn norm_id_one_is_one() {
        let idt = IdTree::one();
        let nidt = idt.norm();
        assert_eq!(nidt, IdTree::one());
    }

    #[test]
    fn norm_id_zero_is_zero() {
        let idt = IdTree::zero();
        let nidt = idt.norm();
        assert_eq!(nidt, IdTree::zero());
    }

    #[test]
    fn norm_id_0_0_is_0() {
        let idt = IdTree::node(Box::new(IdTree::zero()), Box::new(IdTree::zero()));
        let nidt = idt.norm();
        assert_eq!(nidt, IdTree::zero());
    }

    #[test]
    fn norm_id_1_1_is_1() {
        let idt = IdTree::node(Box::new(IdTree::one()), Box::new(IdTree::one()));
        let nidt = idt.norm();
        assert_eq!(nidt, IdTree::one());
    }

    #[test]
    fn norm_id_0_1_is_0_1() {
        let idt = IdTree::node(Box::new(IdTree::one()), Box::new(IdTree::zero()));
        let nidt = idt.clone().norm();
        assert_eq!(nidt, idt);
    }

    #[test]
    fn norm_id_1_1_1_is_1() {
        let idt = IdTree::node(
            Box::new(IdTree::one()),
            Box::new(IdTree::node(
                Box::new(IdTree::one()),
                Box::new(IdTree::one()),
            )),
        );
        let nidt = idt.clone().norm();
        assert_eq!(nidt, IdTree::one());
    }

    // (2, 1, 1) ~=~ 3
    #[test]
    fn norm_e_one() {
        let et = EventTree::node(
            2,
            Box::new(EventTree::leaf(1)),
            Box::new(EventTree::leaf(1)),
        );
        let net = et.clone().norm();
        assert_eq!(net, EventTree::leaf(3));
    }

    // (2, (2, 1, 0), 3) ~=~ (4, (0, 1, 0), 1)
    #[test]
    fn norm_e_two() {
        let a = Box::new(EventTree::node(
            2,
            Box::new(EventTree::leaf(1)),
            Box::new(EventTree::leaf(0)),
        ));
        let b = Box::new(EventTree::leaf(3));
        let et = EventTree::node(2, a, b);

        let expected_a = Box::new(EventTree::node(
            0,
            Box::new(EventTree::leaf(1)),
            Box::new(EventTree::leaf(0)),
        ));
        let expected_b = Box::new(EventTree::leaf(1));
        let expected = EventTree::node(4, expected_a, expected_b);

        let net = et.norm();

        assert_eq!(net, expected);
    }

    #[test]
    fn split_test() {
        assert_eq!(
            IdTree::one().split(),
            IdTree::node(
                Box::new(IdTree::node(
                    Box::new(IdTree::one()),
                    Box::new(IdTree::zero())
                )),
                Box::new(IdTree::node(
                    Box::new(IdTree::zero()),
                    Box::new(IdTree::one())
                ))
            )
        );
    }

    #[test]
    fn example() {
        let seed = Stamp::seed();
        let (l, r) = seed.fork();

        assert_eq!(
            l,
            Stamp::new(
                IdTree::node(Box::new(IdTree::one()), Box::new(IdTree::zero())),
                EventTree::zero()
            )
        );
        assert_eq!(
            r,
            Stamp::new(
                IdTree::node(Box::new(IdTree::zero()), Box::new(IdTree::one())),
                EventTree::zero()
            )
        );

        let le = l.event();
        let re = r.event();

        assert_eq!(
            le,
            Stamp::new(
                IdTree::node(Box::new(IdTree::one()), Box::new(IdTree::zero())),
                EventTree::node(0, Box::new(EventTree::leaf(1)), Box::new(EventTree::zero()))
            )
        );
        assert_eq!(
            re,
            Stamp::new(
                IdTree::node(Box::new(IdTree::zero()), Box::new(IdTree::one())),
                EventTree::node(0, Box::new(EventTree::zero()), Box::new(EventTree::leaf(1)))
            )
        );

        let (lel, ler) = le.fork();

        assert_eq!(
            lel,
            Stamp::new(
                IdTree::node(
                    Box::new(IdTree::node(
                        Box::new(IdTree::one()),
                        Box::new(IdTree::zero())
                    )),
                    Box::new(IdTree::zero())
                ),
                EventTree::node(0, Box::new(EventTree::leaf(1)), Box::new(EventTree::zero()))
            )
        );
        assert_eq!(
            ler,
            Stamp::new(
                IdTree::node(
                    Box::new(IdTree::node(
                        Box::new(IdTree::zero()),
                        Box::new(IdTree::one())
                    )),
                    Box::new(IdTree::zero())
                ),
                EventTree::node(0, Box::new(EventTree::leaf(1)), Box::new(EventTree::zero()))
            )
        );

        let ree = re.event();

        assert_eq!(
            ree,
            Stamp::new(
                IdTree::node(Box::new(IdTree::zero()), Box::new(IdTree::one())),
                EventTree::node(0, Box::new(EventTree::zero()), Box::new(EventTree::leaf(2)))
            )
        );

        let lele = lel.event();

        assert_eq!(
            lele,
            Stamp::new(
                IdTree::node(
                    Box::new(IdTree::node(
                        Box::new(IdTree::one()),
                        Box::new(IdTree::zero())
                    )),
                    Box::new(IdTree::zero())
                ),
                EventTree::node(
                    0,
                    Box::new(EventTree::node(
                        1,
                        Box::new(EventTree::leaf(1)),
                        Box::new(EventTree::zero())
                    )),
                    Box::new(EventTree::zero())
                )
            )
        );

        let lerjree = ler.join(&ree);

        assert_eq!(
            lerjree,
            Stamp::new(
                IdTree::node(
                    Box::new(IdTree::node(
                        Box::new(IdTree::zero()),
                        Box::new(IdTree::one())
                    )),
                    Box::new(IdTree::one())
                ),
                EventTree::node(1, Box::new(EventTree::zero()), Box::new(EventTree::leaf(1)))
            )
        );

        let (lerjreel, lerjreer) = lerjree.fork();

        assert_eq!(
            lerjreel,
            Stamp::new(
                IdTree::node(
                    Box::new(IdTree::node(
                        Box::new(IdTree::zero()),
                        Box::new(IdTree::one())
                    )),
                    Box::new(IdTree::zero())
                ),
                EventTree::node(1, Box::new(EventTree::zero()), Box::new(EventTree::leaf(1)))
            )
        );
        assert_eq!(
            lerjreer,
            Stamp::new(
                IdTree::node(Box::new(IdTree::zero()), Box::new(IdTree::one())),
                EventTree::node(1, Box::new(EventTree::zero()), Box::new(EventTree::leaf(1)))
            )
        );

        let lelejlerjreel = lele.join(&lerjreel);

        assert_eq!(
            lelejlerjreel,
            Stamp::new(
                IdTree::node(Box::new(IdTree::one()), Box::new(IdTree::zero())),
                EventTree::node(
                    1,
                    Box::new(EventTree::node(
                        0,
                        Box::new(EventTree::leaf(1)),
                        Box::new(EventTree::zero())
                    )),
                    Box::new(EventTree::leaf(1))
                )
            )
        );

        let lelejlerjreele = lelejlerjreel.event();

        assert_eq!(
            lelejlerjreele,
            Stamp::new(
                IdTree::node(Box::new(IdTree::one()), Box::new(IdTree::zero())),
                EventTree::leaf(2)
            )
        );
    }
}
