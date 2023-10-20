use std::{
    fmt::Display,
    iter::FusedIterator,
    ops::{Index, IndexMut},
};

#[derive(Clone, Debug)]
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    first_free: u32,
    free_count: u32,
}
impl<T> Arena<T> {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            first_free: 0,
            free_count: 0,
        }
    }

    pub fn clear(&mut self) {
        self.slots.clear();
        self.free_count = 0;
    }
    pub fn reserve(&mut self, additional: usize) {
        let free = self.free_count as usize;
        if additional <= free {
            return;
        }
        self.slots.reserve(free - additional);
    }
    pub fn reserve_exact(&mut self, additional: usize) {
        let free = self.free_count as usize;
        if additional <= free {
            return;
        }
        self.slots.reserve_exact(free - additional);
    }

    pub fn len(&self) -> usize {
        let free_count = self.free_count as usize;
        self.slots.len() - free_count
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn insert<I: GenIndex<Item = T>>(&mut self, t: T) -> I {
        let raw = self.insert_raw(t);
        I::from(raw)
    }
    pub fn remove<I: GenIndex<Item = T>>(&mut self, id: I) -> Option<T> {
        self.remove_raw(id.get_id())
    }
    pub fn contains<I: GenIndex<Item = T>>(&self, id: I) -> bool {
        self.exists_raw(id.get_id())
    }

    pub fn get<I: GenIndex<Item = T>>(&mut self, id: I) -> Option<&T> {
        self.get_raw(id.get_id())
    }
    pub fn get_mut<I: GenIndex<Item = T>>(&mut self, id: I) -> Option<&mut T> {
        self.get_mut_raw(id.get_id())
    }

    pub fn insert_raw(&mut self, t: T) -> Id {
        let index = self.free_index();
        self.slots[index.index as usize].entry = Entry::Present(t);

        index
    }
    pub fn remove_raw(&mut self, id: Id) -> Option<T> {
        if !self.exists_raw(id) {
            return None;
        };

        let item = &mut self.slots[id.index as usize];
        item.generation += 1;

        let old = item.entry.take(self.first_free);
        self.first_free = id.index;
        self.free_count += 1;

        old
    }
    pub fn exists_raw(&self, id: Id) -> bool {
        let index = id.index as usize;
        if index >= self.slots.len() {
            return false;
        }

        let item = &self.slots[index];
        id.generation == item.generation
    }

    pub fn get_raw(&self, id: Id) -> Option<&T> {
        let (index, generation) = (id.index() as usize, id.generation());
        let slot = self.slots.get(index)?;
        if generation != slot.generation {
            return None;
        }

        let Entry::Present(item) = &slot.entry else {
            return None;
        };
        Some(item)
    }
    pub fn get_mut_raw(&mut self, id: Id) -> Option<&mut T> {
        let (index, generation) = (id.index() as usize, id.generation());
        let slot = self.slots.get_mut(index)?;
        if generation != slot.generation {
            return None;
        }

        let Entry::Present(item) = &mut slot.entry else {
            return None;
        };
        Some(item)
    }

    pub fn iter(&self) -> Iter<T> {
        Iter {
            length: self.len() as u32,
            returned: 0,
            slots: self.slots.iter(),
        }
    }
    pub fn iter_mut(&mut self) -> IterMut<T> {
        IterMut {
            length: self.len() as u32,
            returned: 0,
            slots: self.slots.iter_mut(),
        }
    }

    fn free_index(&mut self) -> Id {
        if self.free_count > 0 {
            let index = self.first_free;
            let item = &self.slots[index as usize];
            let generation = item.generation;
            let Entry::Free { next_free } = item.entry else {
                unreachable!()
            };
            self.first_free = next_free;
            self.free_count -= 1;
            Id { index, generation }
        } else {
            let index = self.slots.len();
            self.slots.push(Slot {
                generation: 0,
                entry: Entry::Free { next_free: 0 },
            });
            Id {
                index: index as u32,
                generation: 0,
            }
        }
    }
}
impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> IntoIterator for Arena<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            length: self.len() as u32,
            returned: 0,
            slots: self.slots.into_iter(),
        }
    }
}
impl<'a, T> IntoIterator for &'a Arena<T> {
    type IntoIter = Iter<'a, T>;
    type Item = &'a T;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T> IntoIterator for &'a mut Arena<T> {
    type IntoIter = IterMut<'a, T>;
    type Item = &'a mut T;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
impl<A> FromIterator<A> for Arena<A> {
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self {
        let slots = iter
            .into_iter()
            .map(|a| Slot {
                generation: 0,
                entry: Entry::Present(a),
            })
            .collect();

        Self {
            slots,
            first_free: 0,
            free_count: 0,
        }
    }
}
impl<T> Index<Id> for Arena<T> {
    type Output = T;
    fn index(&self, index: Id) -> &Self::Output {
        match self.get_raw(index) {
            Some(item) => item,
            None => panic!("Index {index} does not exist in Arena"),
        }
    }
}
impl<T> IndexMut<Id> for Arena<T> {
    fn index_mut(&mut self, index: Id) -> &mut Self::Output {
        match self.get_mut_raw(index) {
            Some(item) => item,
            None => panic!("Index {index} does not exist in Arena"),
        }
    }
}
impl<T, I> Index<I> for Arena<T>
where
    I: GenIndex<Item = T>,
{
    type Output = T;
    fn index(&self, index: I) -> &Self::Output {
        self.index(index.get_id())
    }
}
impl<T, I> IndexMut<I> for Arena<T>
where
    I: GenIndex<Item = T>,
{
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        self.index_mut(index.get_id())
    }
}

impl<T> Extend<T> for Arena<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            self.insert_raw(item);
        }
    }
}

#[derive(Clone, Debug)]
struct Slot<T> {
    generation: u32,
    entry: Entry<T>,
}

#[derive(Clone, Debug)]
enum Entry<T> {
    Present(T),
    Free { next_free: u32 },
}
impl<T> Entry<T> {
    fn take(&mut self, next_free: u32) -> Option<T> {
        let old = std::mem::replace(self, Entry::Free { next_free });
        let Entry::Present(t) = old else { return None };
        Some(t)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Id {
    index: u32,
    generation: u32,
}
impl Id {
    pub fn index(self) -> u32 {
        self.index
    }
    pub fn generation(self) -> u32 {
        self.generation
    }
}
impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Id({}, {})", self.index, self.generation)
    }
}

pub trait GenIndex: From<Id> {
    type Item;
    fn get_id(&self) -> Id;
}

#[derive(Clone, Debug)]
pub struct Iter<'a, T> {
    slots: std::slice::Iter<'a, Slot<T>>,
    length: u32,
    returned: u32,
}
impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let slot = self.slots.next()?;
            if let Entry::Present(item) = &slot.entry {
                self.returned += 1;
                return Some(item);
            }
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let length = (self.length - self.returned) as usize;
        (length, Some(length))
    }
}
impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            let slot = self.slots.next_back()?;
            if let Entry::Present(item) = &slot.entry {
                self.returned += 1;
                return Some(item);
            }
        }
    }
}
impl<'a, T> ExactSizeIterator for Iter<'a, T> {}
impl<'a, T> FusedIterator for Iter<'a, T> {}

#[derive(Debug)]
pub struct IterMut<'a, T> {
    slots: std::slice::IterMut<'a, Slot<T>>,
    length: u32,
    returned: u32,
}
impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let slot = self.slots.next()?;
            if let Entry::Present(item) = &mut slot.entry {
                self.returned += 1;
                return Some(item);
            }
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let length = (self.length - self.returned) as usize;
        (length, Some(length))
    }
}
impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            let slot = self.slots.next_back()?;
            if let Entry::Present(item) = &mut slot.entry {
                self.returned += 1;
                return Some(item);
            }
        }
    }
}
impl<'a, T> ExactSizeIterator for IterMut<'a, T> {}
impl<'a, T> FusedIterator for IterMut<'a, T> {}

#[derive(Clone, Debug)]
pub struct IntoIter<T> {
    slots: std::vec::IntoIter<Slot<T>>,
    length: u32,
    returned: u32,
}
impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let slot = self.slots.next()?;
            if let Entry::Present(item) = slot.entry {
                self.returned += 1;
                return Some(item);
            }
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let length = (self.length - self.returned) as usize;
        (length, Some(length))
    }
}
impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            let slot = self.slots.next_back()?;
            if let Entry::Present(item) = slot.entry {
                self.returned += 1;
                return Some(item);
            }
        }
    }
}
impl<T> ExactSizeIterator for IntoIter<T> {}
impl<T> FusedIterator for IntoIter<T> {}
