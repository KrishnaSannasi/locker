pub struct Slab<T> {
    entries: Vec<Entry<T>>,
    len: usize,
    next: usize,
}

#[derive(Clone)]
enum Entry<T> {
    Vacant(usize),
    Occupied(T),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Index(usize);

impl<T> Slab<T> {
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            len: 0,
            next: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn insert(&mut self, value: T) -> Index {
        let index = self.next;
        self.len += 1;
        if let Some(entry) = self.entries.get_mut(self.next) {
            match *entry {
                Entry::Vacant(next) => self.next = next,
                Entry::Occupied(_) => panic!("self.next was in an invalid state"),
            }

            *entry = Entry::Occupied(value);
        } else {
            debug_assert_eq!(self.next, self.entries.len());

            self.entries.push(Entry::Occupied(value));
        }

        Index(index)
    }

    pub fn remove(&mut self, Index(index): Index) -> T {
        let entry = &mut self.entries[index];

        let entry = std::mem::replace(entry, Entry::Vacant(self.next));
        self.next = index;

        match entry {
            Entry::Vacant(_) => panic!("tried to remove from an empty slot"),
            Entry::Occupied(value) => value,
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            inner: self.entries.iter_mut().enumerate(),
            len: self.len,
        }
    }
}

pub struct IterMut<'a, T> {
    inner: std::iter::Enumerate<std::slice::IterMut<'a, Entry<T>>>,
    len: usize,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (Index, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        let len = &mut self.len;
        self.inner.by_ref().find_map(|(index, entry)| {
            *len -= 1;
            match entry {
                Entry::Occupied(value) => Some((Index(index), value)),
                Entry::Vacant(_) => None,
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}
