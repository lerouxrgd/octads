use alloc::boxed::Box;
use core::mem::{self, MaybeUninit};
use core::ptr;

use crate::allocator::BlockAllocator;
use crate::stacks::{BoundedStack, LinkedListStack};
use crate::trees::{TreeElem, TreeNode};

#[derive(Debug)]
pub struct SearchTree<K, V> {
    allocator: BlockAllocator<TreeNode<K, V>>,
    root: *mut TreeNode<K, V>,
    length: usize,
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
                Some(&*(*tmp_node).left.as_leaf())
            } else {
                None
            }
        }
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.length += 1;
        unsafe {
            if (*self.root).is_empty() {
                (*self.root).left = TreeElem::Leaf(Box::into_raw(Box::new(value)));
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
                mem::swap(&mut val_ptr, (*tmp_node).left.as_leaf_mut());
                return Some(*Box::from_raw(val_ptr));
            }

            if (*tmp_node).key.assume_init_ref() < &key {
                let old_leaf = self.allocator.get_node();
                (*old_leaf).left = (*tmp_node).left;
                (*old_leaf).key = MaybeUninit::new((*tmp_node).key.assume_init_read());

                let new_leaf = self.allocator.get_node();
                (*new_leaf).left = TreeElem::Leaf(Box::into_raw(Box::new(value)));
                (*new_leaf).key = MaybeUninit::new(key.clone());

                (*tmp_node).left = TreeElem::Node(old_leaf);
                (*tmp_node).right = new_leaf;
                (*tmp_node).key = MaybeUninit::new(key);
            } else {
                let old_leaf = self.allocator.get_node();
                (*old_leaf).left = (*tmp_node).left;
                (*old_leaf).key = MaybeUninit::new((*tmp_node).key.assume_init_read().clone());

                let new_leaf = self.allocator.get_node();
                (*new_leaf).left = TreeElem::Leaf(Box::into_raw(Box::new(value)));
                (*new_leaf).key = MaybeUninit::new(key);

                (*tmp_node).left = TreeElem::Node(new_leaf);
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

            if (*self.root).has_value() {
                if key == (*self.root).key.assume_init_ref() {
                    (*self.root).key.assume_init_drop();
                    let val_ptr = mem::take(&mut (*self.root).left).as_leaf();
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
            let val_ptr = mem::take(&mut (*tmp_node).left).as_leaf();
            (*tmp_node).key.assume_init_drop();
            self.allocator.return_node(tmp_node);
            self.allocator.return_node(other_node);
            self.length -= 1;
            Some(*Box::from_raw(val_ptr))
        }
    }

    pub fn find<'a, 'b>(&'a self, min_key: &'b K, max_key: &'b K) -> SearchTreeFind<'a, 'b, K, V> {
        let mut stack = LinkedListStack::new(64, 8);
        stack.push(self.root);
        SearchTreeFind {
            _tree: self,
            stack,
            min_key,
            max_key,
        }
    }

    pub fn iter(&self) -> SearchTreeIter<'_, K, V> {
        let mut stack = LinkedListStack::new(64, 8);
        if unsafe { !(*self.root).is_empty() } {
            stack.push(self.root);
        }
        SearchTreeIter { _tree: self, stack }
    }

    /// Top-down contruction of an optimal SearchTree.
    /// It is assumed that `iter` is sorted (by K).
    pub fn from_iter<I>(iter: I) -> Self
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

        let mut allocator: BlockAllocator<TreeNode<K, V>> = BlockAllocator::new(256, 32);
        let mut stack = BoundedStack::new(length.ilog2() as usize + 1);

        // Put root node on stack
        let root = allocator.get_node();
        current.node1 = root;
        current.number = length; // root expands to length leaves
        stack.push(current);

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
                unsafe { (*current.node1).left = TreeElem::Node(left.node1) };
                unsafe { (*current.node1).right = right.node1 };
                stack.push(right);
                stack.push(left);
            } else
            // Reached a leaf, must be filled with list item
            {
                let (key, value) = iter.next().unwrap();
                let val_ptr = TreeElem::Leaf(Box::into_raw(Box::new(value)));
                if !current.node2.is_null() {
                    unsafe { (*current.node2).key = MaybeUninit::new(key.clone()) };
                }
                unsafe {
                    (*current.node1).left = val_ptr;
                    (*current.node1).key = MaybeUninit::new(key);
                    (*current.node1).right = ptr::null_mut();
                }
            }
        }

        Self {
            allocator,
            root,
            length,
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
                if (*(*current_node).left.as_node()).has_value() {
                    let leaf_node = (*current_node).left.as_node();
                    let val_ptr = (*leaf_node).left.as_leaf();
                    drop(*Box::from_raw(val_ptr));
                    (*leaf_node).key.assume_init_drop();
                    self.allocator.return_node(leaf_node);

                    let tmp = (*current_node).right;
                    (*current_node).key.assume_init_drop();
                    self.allocator.return_node(current_node);
                    current_node = tmp;
                } else {
                    let tmp = (*current_node).left.as_node();
                    (*current_node).left = TreeElem::Node((*tmp).right);
                    (*tmp).right = current_node;
                    current_node = tmp;
                }
            }
            (*current_node).key.assume_init_drop();
            let val_ptr = (*current_node).left.as_leaf();
            drop(*Box::from_raw(val_ptr));
        }
    }
}

pub struct SearchTreeIter<'a, K, V> {
    _tree: &'a SearchTree<K, V>,
    stack: LinkedListStack<*mut TreeNode<K, V>>,
}

impl<'a, K, V> Iterator for SearchTreeIter<'a, K, V>
where
    K: Ord,
{
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        while !self.stack.is_empty() {
            unsafe {
                let node = self.stack.pop();
                if (*node).has_value() {
                    return Some(((*node).key.assume_init_ref(), &*(*node).left.as_leaf()));
                } else {
                    self.stack.push((*node).left.as_node());
                    self.stack.push((*node).right);
                }
            }
        }
        None
    }
}

pub struct SearchTreeFind<'a, 'b, K, V> {
    _tree: &'a SearchTree<K, V>,
    stack: LinkedListStack<*mut TreeNode<K, V>>,
    min_key: &'b K,
    max_key: &'b K,
}

impl<'a, 'b, K, V> Iterator for SearchTreeFind<'a, 'b, K, V>
where
    K: Ord,
{
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        while !self.stack.is_empty() {
            let node = self.stack.pop();
            unsafe {
                if (*node).has_value() {
                    if self.min_key <= (*node).key.assume_init_ref()
                        && (*node).key.assume_init_ref() < self.max_key
                    {
                        return Some(((*node).key.assume_init_ref(), &*(*node).left.as_leaf()));
                    }
                } else if self.max_key <= (*node).key.assume_init_ref() {
                    self.stack.push((*node).left.as_node());
                } else if (*node).key.assume_init_ref() <= self.min_key {
                    self.stack.push((*node).right);
                } else {
                    self.stack.push((*node).left.as_node());
                    self.stack.push((*node).right);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_tree_ok() {
        let mut tree = SearchTree::new(32, 8);
        tree.insert(5, 50);
        tree.insert(3, 30);
        tree.insert(1, 10);
        tree.insert(2, 20);
        tree.insert(4, 40);
        assert_eq!(Some(&20), tree.get(&2));
        assert_eq!(5, tree.len());
        assert_eq!(4, tree.find(&1, &5).count());
        for ((&k, &v), i) in tree.find(&1, &5).zip((1..5).rev()) {
            assert_eq!((k, v), (i, i * 10));
        }
        assert_eq!(Some(30), tree.remove(&3));
        assert_eq!(None, tree.remove(&3));
        assert_eq!(None, tree.get(&3));
        assert_eq!(4, tree.len());
        assert_eq!(3, tree.find(&1, &5).count());

        tree = SearchTree::new(32, 8);
        drop(tree);

        let tree = SearchTree::from_iter([(1, 10), (2, 20), (3, 30), (4, 40)]);
        assert_eq!(Some(&30), tree.get(&3));
        assert_eq!(4, tree.len());
        assert_eq!(3, tree.find(&2, &5).count());
        for ((&k, &v), i) in tree.iter().zip((1..5).rev()) {
            assert_eq!((k, v), (i, i * 10));
        }
    }
}
