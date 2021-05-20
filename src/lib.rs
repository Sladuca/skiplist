#![feature(maybe_uninit_array_assume_init)]
use std::ptr::{self, NonNull};
use std::fmt::Debug;
use std::fmt;

// NUM_LEVELS must be <= std::mem::size_of<usize>()

// INVARIANT: if a link is Some, it must point to a SkipListNode
type Link<T, const NUM_LEVELS: usize> = Option<NonNull<SkipListNode<T, NUM_LEVELS>>>;

pub struct SkipList<T: PartialOrd + PartialEq + Debug, const NUM_LEVELS: usize> {
    head: Box<SkipListNode<T, NUM_LEVELS>>,
    rng: fastrand::Rng,
    len: usize,
}

impl<T: PartialOrd + PartialEq + Debug, const NUM_LEVELS: usize> Debug for SkipList<T, NUM_LEVELS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut args = Vec::new();
        let mut node = self.head.as_ref();
        loop {
            args.push(format!("{:?}", node));
            match node.next(0) {
                Some(next) => node = next,
                None => break
            }
        };
        f.debug_list().entries(args.iter()).finish()
    }
}

#[derive(Debug)]
pub struct SkipListNode<T: PartialOrd + PartialEq + Debug, const NUM_LEVELS: usize> {
    level: usize,
    val: Option<T>,
    prev: Link<T, NUM_LEVELS>,
    next: [Link<T, NUM_LEVELS>; NUM_LEVELS],
}

impl<T: PartialOrd + PartialEq + Debug, const NUM_LEVELS: usize> Drop for SkipListNode<T, NUM_LEVELS> {
    fn drop(&mut self) {
        let mut node = self.next[0].take();
        while let Some(next) = node {
            let mut next = unsafe { Box::from_raw(next.as_ptr()) };
            node = next.next[0].take();
        }
    }
}

impl<T: PartialOrd + PartialEq + Debug, const NUM_LEVELS: usize> SkipListNode<T, NUM_LEVELS> {
    fn val(&self) -> Option<&T> {
        self.val.as_ref()
    }

    fn next(&self, level: usize) -> Option<&Self> {
        assert!(level < NUM_LEVELS);

        // SAFETY: If a link is some, it points to a SkipListNode
        unsafe { self.next[level].map(|p| p.as_ref()) }
    }

    fn next_mut(&mut self, level: usize) -> Option<&mut Self> {
        assert!(level < NUM_LEVELS);

        // SAFETY: If a link is Some, it points to SkipListNode
        unsafe { self.next[level].as_mut().map(|p| p.as_mut()) }
    }

    fn next_if(&self, level: usize, f: impl FnOnce(&Self, &Self) -> bool) -> Result<&Self, &Self> {
        assert!(level < NUM_LEVELS);

        // SAFETY: If a link is Some, it points to SkipListNode
        let next = unsafe { self.next[level].map(|p| p.as_ref()) };
        match next {
            Some(next) if f(self, next) => Ok(next),
            _ => Err(self),
        }
    }

    fn prev(&self) -> Option<&Self> {
        // SAFETY: If a link is Some, it points to a SkipListNode
        unsafe { self.prev.map(|p| p.as_ref()) }
    }

    fn prev_mut(&mut self) -> Option<&mut Self> {
        // SAFETY: If a link is Some, it points to a SkipList
        unsafe { self.prev.as_mut().map(|p| p.as_mut()) }
    }

    fn next_if_mut(
        &mut self,
        level: usize,
        f: impl FnOnce(&Self, &Self) -> bool,
    ) -> Result<&mut Self, &mut Self> {
        assert!(level < NUM_LEVELS);

        // SAFETY: If a link is some, it points to SkipListNode
        let next = unsafe { self.next[level].as_mut().map(|p| p.as_mut()) };
        match next {
            Some(next) if f(self, next) => Ok(next),
            _ => Err(self),
        }
    }

    fn proceed_at_level_while(
        &self,
        level: usize,
        mut f: impl FnMut(&Self, &Self) -> bool,
    ) -> &Self {
        assert!(level < NUM_LEVELS);

        let mut curr = self;
        loop {
            match curr.next_if(level, &mut f) {
                Ok(next) => {
                    curr = next;
                }
                Err(curr) => return curr,
            }
        }
    }

    fn proceed_at_level_while_mut(
        &mut self,
        level: usize,
        mut f: impl FnMut(&Self, &Self) -> bool,
    ) -> &mut Self {
        assert!(level < NUM_LEVELS);

        let mut curr = self;
        loop {
            match curr.next_if_mut(level, &mut f) {
                Ok(next) => {
                    curr = next;
                }
                Err(curr) => return curr,
            }
        }
    }
}

impl<T: PartialOrd + PartialEq + Debug, const NUM_LEVELS: usize> SkipListNode<T, NUM_LEVELS> {
    fn new_head() -> SkipListNode<T, NUM_LEVELS> {
        SkipListNode {
            level: NUM_LEVELS - 1,
            val: None,
            prev: None,
            next: [None; NUM_LEVELS],
        }
    }
    fn new(val: T, level: usize, prev: Link<T, NUM_LEVELS>) -> SkipListNode<T, NUM_LEVELS> {
        SkipListNode {
            level,
            val: Some(val),
            prev: prev,
            next: [None; NUM_LEVELS],
        }
    }

    fn is_head(&self) -> bool {
        self.prev.is_none()
    }
}

impl<T: PartialOrd + PartialEq + Debug, const NUM_LEVELS: usize> SkipList<T, NUM_LEVELS> {
    pub fn new() -> Self {
        let head = Box::new(SkipListNode::<T, NUM_LEVELS>::new_head());
        SkipList { head, rng: fastrand::Rng::new(), len: 0 }
    }

    pub fn gen_level(&self) -> usize {
        let max_level = NUM_LEVELS - 1;
        let mask = (1 << max_level) - 1;
        let rand = self.rng.usize(..);
        let jawn = rand & mask;
        jawn.trailing_ones() as usize
    }
    

    pub fn find(&self, item: &T) -> Option<&T> {
        let node = self.find_node(item);
        
        match node.val() {
            Some(val) => {
                if val == item {
                    Some(val)
                } else {
                    None
                }
            }
            None => None,
        }
    }

    pub fn find_node(&self, item: &T) -> &SkipListNode<T, NUM_LEVELS> {
        let mut node = self.head.as_ref();
        for level in (0..NUM_LEVELS).rev() {
            node = node.proceed_at_level_while(level, move |_, next| {
                next.val().map_or(false, |v2| item >= v2)
            });
        }
        node
    }

    pub fn find_node_mut(&mut self, item: &T) -> &mut SkipListNode<T, NUM_LEVELS> {
        let mut node = self.head.as_mut();
        for level in (0..NUM_LEVELS).rev() {
            node = node.proceed_at_level_while_mut(level, move |_, next| {
                next.val().map_or(false, |v2| item >= v2)
            })
        }
        node
    }

    pub fn contains(&self, item: &T) -> bool {
        self.find(item).is_some()
    }

    pub fn insert(&mut self, item: T) {
        let new_node_level = self.gen_level();
        
        let new_node = Box::new(SkipListNode::<T, NUM_LEVELS>::new(
            item,
            new_node_level,
            None
        ));


        // SAFETY: box never null, so NonNull::new_unchecked is ok
        let mut new_node = unsafe { NonNull::new_unchecked(Box::into_raw(new_node)) };
        let item = unsafe { new_node.as_ref().val().unwrap() };

        let mut node = self.head.as_mut();
        let mut level = NUM_LEVELS;
        let old_next = loop {
            level -= 1;

            node = node.proceed_at_level_while_mut(level, move |_, next| {
                next.val().map_or(false, |v2| item >= v2)
            });

            
            if level <= new_node_level {
                let old_next = node.next[level].replace(new_node);

                // SAFETY: new_node hasn't been deleted yet since we're still inserting it
                unsafe { new_node.as_mut().next[level] = old_next };
                
                if level == 0 {
                    break old_next;
                }
            }
        };

        unsafe { new_node.as_mut().prev = Some(node.into())}

        match old_next {
            // SAFETY: old_next.as_mut() ok because a link is Some iff it points to a valid SkipListNode
            Some(mut old_next) => unsafe {
                old_next.as_mut().prev = Some(new_node);
            }
            None => {}
        }
    }
}



#[cfg(test)]
mod tests {
    use super::SkipList;
    use criterion::{criterion_group, criterion_main, black_box, Criterion};

    #[test]
    fn insert_and_lookup_same_order() {
        let mut l = SkipList::<usize, 8>::new();
        for i in 0..10 {
            l.insert(i);
        }
        

        for i in 0..10 {
            assert!(l.contains(&i));
        }
    }

    #[test]
    fn insert_and_lookup_different_order() {
        let mut l = SkipList::<i32, 9>::new();
        let mut nums = Vec::new();
        for _ in 0..200 {
            let i = fastrand::i32(..);
            l.insert(i);
            nums.push(i);
        }
        fastrand::shuffle(nums.as_mut());

        for i in nums.into_iter() {
            assert!(l.contains(&i));
        }
    }

}