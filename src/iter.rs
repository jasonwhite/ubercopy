// Copyright (c) 2017 Jason White
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

/// Represents a *change*. That is, if an item was added or removed.
#[derive(Ord, PartialOrd, Eq, PartialEq, Debug)]
pub enum Change
{
    Added,
    Removed,
    None,
}

/// An iterator for stepping through two iterators (whose items are sorted).
/// Yields a `Change` for each item.
pub struct Changes<I: Iterator>
{
    a: I,
    b: I,

    next_a: Option<I::Item>,
    next_b: Option<I::Item>,
}

impl<I> Changes<I>
    where I: Iterator
{
    pub fn new(a: I, b: I) -> Self {
        let mut changes = Changes {
            a: a,
            b: b,
            next_a: None,
            next_b: None,
        };

        changes.next_a = changes.a.next();
        changes.next_b = changes.b.next();

        changes
    }
}

impl<I> Iterator for Changes<I>
    where I: Iterator,
          I::Item: PartialOrd
{
    type Item = (I::Item, Change);

    fn next(&mut self) -> Option<Self::Item> {
        match (self.next_a.take(), self.next_b.take()) {
            // Elements remaining on both sides
            (Some(a), Some(b)) => {
                if a < b {
                    self.next_a = self.a.next();
                    self.next_b = Some(b);
                    Some((a, Change::Removed))
                }
                else if b < a {
                    self.next_a = Some(a);
                    self.next_b = self.b.next();
                    Some((b, Change::Added))
                }
                else {
                    // No change. Elements are the same.
                    self.next_a = self.a.next();
                    self.next_b = self.b.next();
                    Some((a, Change::None))
                }
            },

            // Only elements remaining are on left side
            (Some(a), None) => {
                self.next_a = self.a.next();
                Some((a, Change::Removed))
            },

            // Only elements remaining are on right side
            (None, Some(b)) => {
                self.next_b = self.b.next();
                Some((b, Change::Added))
            },

            // No elements on either left or right side
            (None, None) => None,
        }
    }
}

/// An iterator adaptor that counts the number of times an element occurs
/// consecutively in an iterator.
pub struct Adjacent<I: Iterator>
{
    iter: I,
    prev: Option<I::Item>,
}

impl<I> Adjacent<I>
    where I: Iterator
{
    pub fn new(iter: I) -> Self {
        let mut adj = Adjacent {
            iter: iter,
            prev: None,
        };

        adj.prev = adj.iter.next();
        adj
    }
}

impl<I> Iterator for Adjacent<I>
    where I: Iterator,
          I::Item: PartialEq
{
    type Item = (I::Item, usize);

    fn next(&mut self) -> Option<Self::Item> {

        if self.prev.is_none() {
            return None;
        }

        let mut count = 1;

        loop {
            let elem = self.iter.next();

            if self.prev == elem {
                count += 1;
            }
            else {
                let ret = Some((self.prev.take().unwrap(), count));
                self.prev = elem;
                return ret;
            }
        }
    }
}

pub struct Unique<I>
    where I: Iterator
{
    iter: Adjacent<I>,
}

impl<I> Unique<I>
    where I: Iterator
{
    pub fn new(iter: I) -> Self {
        Unique { iter: Adjacent::new(iter) }
    }
}

impl<I> Iterator for Unique<I>
    where I: Iterator,
          I::Item: PartialEq
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|e| e.0)
    }
}

pub trait IterExt : Iterator {
    fn changes(self, other: Self) -> Changes<Self>
        where Self : Sized
    {
        Changes::new(self, other)
    }

    fn adjacent(self) -> Adjacent<Self>
        where Self : Sized
    {
        Adjacent::new(self)
    }

    fn unique(self) -> Unique<Self>
        where Self : Sized
    {
        Unique::new(self)
    }
}

impl<T: ?Sized> IterExt for T where T: Iterator { }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change() {
        assert_eq!(Change::Added, Change::Added);
        assert_ne!(Change::Added, Change::Removed);
        assert_eq!(Change::Removed, Change::Removed);
    }

    #[test]
    fn changes_base_case() {
        let a : Vec<u32> = vec![];
        let b : Vec<u32> = vec![];

        let mut c = a.iter().changes(b.iter());
        assert_eq!(c.next(), None);
    }

    #[test]
    fn changes_left_only() {
        let a : Vec<u32> = vec![1];
        let b : Vec<u32> = vec![];

        let mut c = a.iter().changes(b.iter());
        assert_eq!(c.next(), Some((&1, Change::Removed)));
        assert_eq!(c.next(), None);
    }

    #[test]
    fn changes_right_only() {
        let a : Vec<u32> = vec![];
        let b : Vec<u32> = vec![1];

        let mut c = a.iter().changes(b.iter());
        assert_eq!(c.next(), Some((&1, Change::Added)));
        assert_eq!(c.next(), None);
    }

    #[test]
    fn changes_both_sides() {
        let a = vec![1, 2, 3, 4, 6];
        let b = vec![1, 3, 4, 5];

        let mut c = a.iter().changes(b.iter());
        assert_eq!(c.next(), Some((&1, Change::None)));
        assert_eq!(c.next(), Some((&2, Change::Removed)));
        assert_eq!(c.next(), Some((&3, Change::None)));
        assert_eq!(c.next(), Some((&4, Change::None)));
        assert_eq!(c.next(), Some((&5, Change::Added)));
        assert_eq!(c.next(), Some((&6, Change::Removed)));
        assert_eq!(c.next(), None);
    }

    #[test]
    fn adjacent_base_case() {
        let v : Vec<u32> = vec![];
        let mut adj = v.iter().adjacent();
        assert_eq!(adj.next(), None);
        assert_eq!(adj.next(), None);
    }

    #[test]
    fn adjacent_case_1() {
        let v = vec![1];
        let mut adj = v.iter().adjacent();
        assert_eq!(adj.next(), Some((&1, 1)));
        assert_eq!(adj.next(), None);
    }

    #[test]
    fn adjacent_case_2() {
        let v = vec![1, 2, 2, 3, 4, 4, 4];

        let mut adj = v.iter().adjacent();
        assert_eq!(adj.next(), Some((&1, 1)));
        assert_eq!(adj.next(), Some((&2, 2)));
        assert_eq!(adj.next(), Some((&3, 1)));
        assert_eq!(adj.next(), Some((&4, 3)));
        assert_eq!(adj.next(), None);
    }

    #[test]
    fn unique_case_1() {
        let v = vec![1, 1, 2, 3, 3, 4, 5, 5, 5];
        let mut u = v.iter().unique();
        assert_eq!(u.next(), Some(&1));
        assert_eq!(u.next(), Some(&2));
        assert_eq!(u.next(), Some(&3));
        assert_eq!(u.next(), Some(&4));
        assert_eq!(u.next(), Some(&5));
        assert_eq!(u.next(), None);
    }
}
