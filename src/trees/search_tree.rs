use alloc::boxed::Box;
use core::borrow::Borrow;
use core::iter::FusedIterator;
use core::mem::{self, MaybeUninit};
use core::ops::Range;
use core::ptr;

use crate::allocator::BlockAllocator;
use crate::stacks::{BoundedStack, LinkedListStack};
use crate::trees::{TreeNode, TreePtr};

#[derive(Debug)]
pub struct SearchTree<K, V> {
    allocator: BlockAllocator<TreeNode<K, V>>,
    root: *mut TreeNode<K, V>,
    length: usize,
}

impl<K, V> Default for SearchTree<K, V>
where
    K: Ord + Clone,
{
    fn default() -> Self {
        Self::new(
            BlockAllocator::<TreeNode<K, V>>::DEFAULT_BLOCK_SIZE,
            BlockAllocator::<TreeNode<K, V>>::DEFAULT_BLOCK_CAP,
        )
    }
}

impl<K, V> SearchTree<K, V>
where
    K: Ord + Clone,
{
    pub fn new(block_size: usize, blocks_cap: usize) -> Self {
        let mut allocator = BlockAllocator::new(block_size, blocks_cap);
        let root = allocator.get_node();
        Self {
            allocator,
            root,
            length: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        unsafe {
            if (*self.root).is_empty() {
                return None;
            }

            let mut tmp_node = self.root;
            while !(*tmp_node).right.is_null() {
                if key < (*tmp_node).key.assume_init_ref() {
                    tmp_node = (*tmp_node).left.as_node();
                } else {
                    tmp_node = (*tmp_node).right;
                }
            }

            if key == (*tmp_node).key.assume_init_ref() {
                Some(&*(*tmp_node).left.as_val())
            } else {
                None
            }
        }
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.length += 1;
        unsafe {
            if (*self.root).is_empty() {
                (*self.root).left = TreePtr::Val(Box::into_raw(Box::new(value)));
                (*self.root).key = MaybeUninit::new(key);
                return None;
            }

            let mut tmp_node = self.root;
            while !(*tmp_node).right.is_null() {
                if &key < (*tmp_node).key.assume_init_ref() {
                    tmp_node = (*tmp_node).left.as_node();
                } else {
                    tmp_node = (*tmp_node).right;
                }
            }

            if &key == (*tmp_node).key.assume_init_ref() {
                let mut val_ptr = Box::into_raw(Box::new(value));
                mem::swap(&mut val_ptr, (*tmp_node).left.as_val_mut());
                return Some(*Box::from_raw(val_ptr));
            }

            if (*tmp_node).key.assume_init_ref() < &key {
                let old_leaf = self.allocator.get_node();
                (*old_leaf).left = (*tmp_node).left;
                (*old_leaf).key = MaybeUninit::new((*tmp_node).key.assume_init_read());

                let new_leaf = self.allocator.get_node();
                (*new_leaf).left = TreePtr::Val(Box::into_raw(Box::new(value)));
                (*new_leaf).key = MaybeUninit::new(key.clone());

                (*tmp_node).left = TreePtr::Node(old_leaf);
                (*tmp_node).right = new_leaf;
                (*tmp_node).key = MaybeUninit::new(key);
            } else {
                let old_leaf = self.allocator.get_node();
                (*old_leaf).left = (*tmp_node).left;
                (*old_leaf).key = MaybeUninit::new((*tmp_node).key.assume_init_read().clone());

                let new_leaf = self.allocator.get_node();
                (*new_leaf).left = TreePtr::Val(Box::into_raw(Box::new(value)));
                (*new_leaf).key = MaybeUninit::new(key);

                (*tmp_node).left = TreePtr::Node(new_leaf);
                (*tmp_node).right = old_leaf;
            }
            None
        }
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        unsafe {
            if (*self.root).is_empty() {
                return None;
            }

            if (*self.root).is_leaf() {
                if key == (*self.root).key.assume_init_ref() {
                    (*self.root).key.assume_init_drop();
                    let val_ptr = mem::take(&mut (*self.root).left).as_val();
                    self.length -= 1;
                    return Some(*Box::from_raw(val_ptr));
                } else {
                    return None;
                }
            }

            let mut upper_node = ptr::null_mut();
            let mut other_node = ptr::null_mut();
            let mut tmp_node = self.root;
            while !(*tmp_node).right.is_null() {
                upper_node = tmp_node;
                if key < (*tmp_node).key.assume_init_ref() {
                    tmp_node = (*upper_node).left.as_node();
                    other_node = (*upper_node).right;
                } else {
                    tmp_node = (*upper_node).right;
                    other_node = (*upper_node).left.as_node();
                }
            }

            if key != (*tmp_node).key.assume_init_ref() {
                return None;
            }

            (*upper_node).key.assume_init_drop();
            (*upper_node).key = MaybeUninit::new((*other_node).key.assume_init_read());
            (*upper_node).left = (*other_node).left;
            (*upper_node).right = (*other_node).right;
            let val_ptr = mem::take(&mut (*tmp_node).left).as_val();
            (*tmp_node).key.assume_init_drop();
            self.allocator.return_node(tmp_node);
            self.allocator.return_node(other_node);
            self.length -= 1;
            Some(*Box::from_raw(val_ptr))
        }
    }

    pub fn find<Q>(&self, range: Range<Q>) -> SearchTreeFind<'_, K, V, Q>
    where
        Q: Borrow<K>,
    {
        let mut iter_stack = LinkedListStack::default();
        let mut rev_stack = LinkedListStack::default();
        iter_stack.push(self.root);
        rev_stack.push(self.root);
        SearchTreeFind {
            _tree: self,
            iter_stack,
            rev_stack,
            last_iter_key: None,
            last_rev_key: None,
            range,
        }
    }

    pub fn iter(&self) -> SearchTreeIter<'_, K, V> {
        let mut iter_stack = LinkedListStack::default();
        let mut rev_stack = LinkedListStack::default();
        if unsafe { !(*self.root).is_empty() } {
            iter_stack.push(self.root);
            rev_stack.push(self.root);
        }
        SearchTreeIter {
            _tree: self,
            iter_stack,
            rev_stack,
            last_iter_key: None,
            last_rev_key: None,
        }
    }

    /// Top-down contruction of an optimal `SearchTree`.
    ///
    /// # Panics
    ///
    /// Panics if `iter` is not sorted (by `K`) or if it contains duplicates.
    pub fn from_sorted<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        I::IntoIter: ExactSizeIterator,
    {
        struct TreeBuilder<K, V> {
            node1: *mut TreeNode<K, V>,
            node2: *mut TreeNode<K, V>,
            number: usize,
        }
        impl<K, V> Clone for TreeBuilder<K, V> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<K, V> Copy for TreeBuilder<K, V> {}

        let [mut current, mut left, mut right] = [TreeBuilder {
            node1: ptr::null_mut(),
            node2: ptr::null_mut(),
            number: 0,
        }; 3];

        let mut iter = iter.into_iter();
        let length = iter.len();

        let mut allocator: BlockAllocator<TreeNode<K, V>> = BlockAllocator::default();
        let mut stack = BoundedStack::new(length.ilog2() as usize + 1);

        // Put root node on stack
        let root = allocator.get_node();
        current.node1 = root;
        current.number = length; // root expands to length leaves
        stack.push(current);

        let mut prev_key = None;
        let mut is_valid = true;
        while !stack.is_empty()
        // There is still unexpanded nodes
        {
            current = stack.pop();
            if current.number > 1
            // Create (empty) tree nodes
            {
                left.node1 = allocator.get_node();
                left.node2 = current.node2;
                left.number = current.number / 2;
                right.node1 = allocator.get_node();
                right.node2 = current.node1;
                right.number = current.number - left.number;
                unsafe { (*current.node1).left = TreePtr::Node(left.node1) };
                unsafe { (*current.node1).right = right.node1 };
                stack.push(right);
                stack.push(left);
            }
            // Reached a leaf, must be filled with list item
            else {
                let (key, value) = iter.next().unwrap();
                let val_ptr = TreePtr::Val(Box::into_raw(Box::new(value)));
                if !current.node2.is_null() {
                    unsafe { (*current.node2).key = MaybeUninit::new(key.clone()) };
                }
                unsafe {
                    (*current.node1).left = val_ptr;
                    (*current.node1).key = MaybeUninit::new(key);
                    (*current.node1).right = ptr::null_mut();
                    // Check whether iter is valid
                    let key = (*current.node1).key.assume_init_ref();
                    if let Some(prev_key) = prev_key.take() {
                        if prev_key >= key {
                            is_valid = false;
                        }
                    }
                    prev_key = Some(key);
                }
            }
        }

        let tree = Self {
            allocator,
            root,
            length,
        };
        if !is_valid {
            panic!("iterator keys are not sorted or unique");
        } else {
            tree
        }
    }
}

impl<K, V> Drop for SearchTree<K, V> {
    fn drop(&mut self) {
        unsafe {
            if (*self.root).is_empty() {
                self.allocator.return_node(self.root);
                return;
            }
            let mut current_node = self.root;
            while (*current_node).has_subtrees() {
                if (*(*current_node).left.as_node()).is_leaf() {
                    let leaf_node = (*current_node).left.as_node();
                    let val_ptr = (*leaf_node).left.as_val();
                    drop(*Box::from_raw(val_ptr));
                    (*leaf_node).key.assume_init_drop();
                    self.allocator.return_node(leaf_node);

                    let tmp = (*current_node).right;
                    (*current_node).key.assume_init_drop();
                    self.allocator.return_node(current_node);
                    current_node = tmp;
                } else {
                    let tmp = (*current_node).left.as_node();
                    (*current_node).left = TreePtr::Node((*tmp).right);
                    (*tmp).right = current_node;
                    current_node = tmp;
                }
            }
            (*current_node).key.assume_init_drop();
            let val_ptr = (*current_node).left.as_val();
            drop(*Box::from_raw(val_ptr));
        }
    }
}

pub struct SearchTreeIter<'a, K, V> {
    _tree: &'a SearchTree<K, V>,
    iter_stack: LinkedListStack<*mut TreeNode<K, V>>,
    rev_stack: LinkedListStack<*mut TreeNode<K, V>>,
    last_iter_key: Option<&'a K>,
    last_rev_key: Option<&'a K>,
}

impl<'a, K, V> Iterator for SearchTreeIter<'a, K, V>
where
    K: Ord,
{
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        while !self.iter_stack.is_empty() {
            unsafe {
                let node = self.iter_stack.pop();
                if (*node).is_leaf() {
                    let node_key = (*node).key.assume_init_ref();
                    match self.last_rev_key {
                        Some(last_rev_key) if last_rev_key <= node_key => {
                            return None;
                        }
                        _ => {
                            self.last_iter_key = Some(node_key);
                            return Some((node_key, &*(*node).left.as_val()));
                        }
                    }
                } else {
                    self.iter_stack.push((*node).right);
                    self.iter_stack.push((*node).left.as_node());
                }
            }
        }
        None
    }
}

impl<'a, K, V> DoubleEndedIterator for SearchTreeIter<'a, K, V>
where
    K: Ord,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        while !self.rev_stack.is_empty() {
            unsafe {
                let node = self.rev_stack.pop();
                if (*node).is_leaf() {
                    let node_key = (*node).key.assume_init_ref();
                    match self.last_iter_key {
                        Some(last_iter_key) if last_iter_key >= node_key => {
                            return None;
                        }
                        _ => {
                            self.last_rev_key = Some(node_key);
                            return Some((node_key, &*(*node).left.as_val()));
                        }
                    }
                } else {
                    self.rev_stack.push((*node).left.as_node());
                    self.rev_stack.push((*node).right);
                }
            }
        }
        None
    }
}

impl<'a, K, V> FusedIterator for SearchTreeIter<'a, K, V> where K: Ord {}

pub struct SearchTreeFind<'a, K, V, Q> {
    _tree: &'a SearchTree<K, V>,
    iter_stack: LinkedListStack<*mut TreeNode<K, V>>,
    rev_stack: LinkedListStack<*mut TreeNode<K, V>>,
    last_iter_key: Option<&'a K>,
    last_rev_key: Option<&'a K>,
    range: Range<Q>,
}

impl<'a, K, V, Q> Iterator for SearchTreeFind<'a, K, V, Q>
where
    Q: Borrow<K>,
    K: Ord,
{
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        while !self.iter_stack.is_empty() {
            let node = self.iter_stack.pop();
            unsafe {
                let node_key = (*node).key.assume_init_ref().borrow();
                if (*node).is_leaf() {
                    if self.range.start.borrow() <= node_key && node_key < self.range.end.borrow() {
                        match self.last_rev_key {
                            Some(last_rev_key) if last_rev_key <= node_key => {
                                return None;
                            }
                            _ => {
                                self.last_iter_key = Some(node_key);
                                return Some((node_key, &*(*node).left.as_val()));
                            }
                        }
                    }
                } else if self.range.end.borrow() <= node_key {
                    self.iter_stack.push((*node).left.as_node());
                } else if node_key <= self.range.start.borrow() {
                    self.iter_stack.push((*node).right);
                } else {
                    self.iter_stack.push((*node).right);
                    self.iter_stack.push((*node).left.as_node());
                }
            }
        }
        None
    }
}

impl<'a, K, V, Q> DoubleEndedIterator for SearchTreeFind<'a, K, V, Q>
where
    Q: Borrow<K>,
    K: Ord,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        while !self.rev_stack.is_empty() {
            let node = self.rev_stack.pop();
            unsafe {
                let node_key = (*node).key.assume_init_ref().borrow();
                if (*node).is_leaf() {
                    if self.range.start.borrow() <= node_key && node_key < self.range.end.borrow() {
                        match self.last_iter_key {
                            Some(last_iter_key) if last_iter_key >= node_key => {
                                return None;
                            }
                            _ => {
                                self.last_rev_key = Some(node_key);
                                return Some((node_key, &*(*node).left.as_val()));
                            }
                        }
                    }
                } else if self.range.end.borrow() <= node_key {
                    self.rev_stack.push((*node).left.as_node());
                } else if node_key <= self.range.start.borrow() {
                    self.rev_stack.push((*node).right);
                } else {
                    self.rev_stack.push((*node).left.as_node());
                    self.rev_stack.push((*node).right);
                }
            }
        }
        None
    }
}

impl<'a, K, V, Q> FusedIterator for SearchTreeFind<'a, K, V, Q>
where
    Q: Borrow<K>,
    K: Ord,
{
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_tree_ok() {
        let mut tree = SearchTree::default();
        tree.insert(5, 50);
        tree.insert(3, 30);
        tree.insert(1, 10);
        tree.insert(2, 20);
        tree.insert(4, 40);
        assert_eq!(Some(&20), tree.get(&2));
        assert_eq!(5, tree.len());
        assert_eq!(4, tree.find(1..5).count());
        assert_eq!(Some(30), tree.remove(&3));
        assert_eq!(None, tree.remove(&3));
        assert_eq!(None, tree.get(&3));
        assert_eq!(4, tree.len());
        assert_eq!(3, tree.find(1..5).count());

        tree = SearchTree::default();
        drop(tree);

        let tree = SearchTree::from_sorted([(1, 10), (2, 20), (3, 30), (4, 40)]);
        assert_eq!(Some(&30), tree.get(&3));
        assert_eq!(4, tree.len());
        assert_eq!(3, tree.find(2..5).count());
    }

    #[test]
    #[should_panic(expected = "iterator keys are not sorted or unique")]
    fn search_tree_unsorted() {
        let _tree = SearchTree::from_sorted([(3, 30), (1, 10), (4, 40), (2, 20)]);
    }

    #[test]
    fn search_tree_iter() {
        let tree = SearchTree::from_sorted([(1, 10), (2, 20), (3, 30), (4, 40)]);

        for ((&k, &v), i) in tree.iter().zip(1..5) {
            assert_eq!((k, v), (i, i * 10));
        }
        for ((&k, &v), i) in tree.iter().rev().zip((1..5).rev()) {
            assert_eq!((k, v), (i, i * 10));
        }
        let mut iter = tree.iter();
        assert_eq!(Some((&1, &10)), iter.next());
        assert_eq!(Some((&2, &20)), iter.next());
        assert_eq!(Some((&4, &40)), iter.next_back());
        assert_eq!(Some((&3, &30)), iter.next_back());
        assert_eq!(None, iter.next_back());
        assert_eq!(None, iter.next());
        assert_eq!(None, iter.next_back());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn search_tree_find() {
        let tree = SearchTree::from_sorted([(1, 10), (2, 20), (3, 30), (4, 40)]);
        for ((&k, &v), i) in tree.find(2..5).zip(2..5) {
            assert_eq!((k, v), (i, i * 10));
        }
        assert_eq!(3, tree.find(2..5).count());

        let tree = SearchTree::from_sorted([(1, 10), (2, 20), (3, 30), (4, 40), (5, 50)]);
        let mut iter = tree.find(2..6);
        assert_eq!(Some((&2, &20)), iter.next());
        assert_eq!(Some((&3, &30)), iter.next());
        assert_eq!(Some((&5, &50)), iter.next_back());
        assert_eq!(Some((&4, &40)), iter.next_back());
        assert_eq!(None, iter.next_back());
        assert_eq!(None, iter.next());
        assert_eq!(None, iter.next_back());
        assert_eq!(None, iter.next());

        use alloc::string::ToString;
        let tree = SearchTree::from_sorted([
            ("1".to_string(), 10),
            ("2".to_string(), 20),
            ("3".to_string(), 30),
            ("4".to_string(), 40),
        ]);
        let start = "2".to_string();
        let end = "5".to_string();
        for ((k, &v), i) in tree.find(&start..&end).zip(2..5) {
            assert_eq!((k.as_str(), v), (i.to_string().as_str(), i * 10));
        }
        assert_eq!(3, tree.find(&start..&end).count());
        assert_eq!(3, tree.find(start..end).count());

        #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
        struct Int(usize);
        let tree =
            SearchTree::from_sorted([(Int(1), 10), (Int(2), 20), (Int(3), 30), (Int(4), 40)]);
        let start = Int(2);
        let end = Int(5);
        for ((k, &v), i) in tree.find(&start..&end).zip(2..5) {
            assert_eq!((k, v), (&Int(i), i * 10));
        }
        assert_eq!(3, tree.find(&start..&end).count());
        assert_eq!(3, tree.find(start..end).count());
    }
}
