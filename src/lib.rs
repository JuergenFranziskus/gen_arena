use std::{
    fmt::Display,
    iter::FusedIterator,
    ops::{Index, IndexMut},
};

#[derive(Clone, Debug)]
pub struct Arena<T> {
    items: Vec<Slot<T>>,
    first_free: u32,
    free_count: u32,
}
impl<T> Arena<T> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            first_free: 0,
            free_count: 0,
        }
    }

    pub fn len(&self) -> usize {
        let free_count = self.free_count as usize;
        self.items.len() - free_count
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn insert(&mut self, t: T) -> Id {
        let index = self.free_index();
        self.items[index.index as usize].entry = Entry::Present(t);

        index
    }
    pub fn remove(&mut self, i: Id) -> Option<T> {
        if !self.exists(i) {
            return None;
        };

        let item = &mut self.items[i.index as usize];
        item.generation += 1;

        let old = item.entry.take(self.first_free);
        self.first_free = i.index;
        self.free_count += 1;

        old
    }
    pub fn exists(&self, i: Id) -> bool {
        let index = i.index as usize;
        if index >= self.items.len() {
            return false;
        }

        let item = &self.items[index];
        i.generation == item.generation
    }

    pub fn get(&self, index: Id) -> Option<&T> {
        let (index, generation) = (index.index() as usize, index.generation());
        let slot = self.items.get(index)?;
        if generation != slot.generation {
            return None;
        }

        let Entry::Present(item) = &slot.entry else {
            return None;
        };
        Some(item)
    }
    pub fn get_mut(&mut self, index: Id) -> Option<&mut T> {
        let (index, generation) = (index.index() as usize, index.generation());
        let slot = self.items.get_mut(index)?;
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
            slots: self.items.iter(),
        }
    }
    pub fn iter_mut(&mut self) -> IterMut<T> {
        IterMut {
            length: self.len() as u32,
            returned: 0,
            slots: self.items.iter_mut(),
        }
    }

    fn free_index(&mut self) -> Id {
        if self.free_count > 0 {
            let index = self.first_free;
            let item = &self.items[index as usize];
            let generation = item.generation;
            let Entry::Free { next_free } = item.entry else {
                unreachable!()
            };
            self.first_free = next_free;
            self.free_count -= 1;
            Id { index, generation }
        } else {
            let index = self.items.len();
            self.items.push(Slot {
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
impl<T> IntoIterator for Arena<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            length: self.len() as u32,
            returned: 0,
            slots: self.items.into_iter(),
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
impl<T> Index<Id> for Arena<T> {
    type Output = T;
    fn index(&self, index: Id) -> &Self::Output {
        match self.get(index) {
            Some(item) => item,
            None => panic!("Index {index} does not exist in Arena"),
        }
    }
}
impl<T> IndexMut<Id> for Arena<T> {
    fn index_mut(&mut self, index: Id) -> &mut Self::Output {
        match self.get_mut(index) {
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

pub trait GenIndex {
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
