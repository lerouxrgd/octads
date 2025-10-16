use alloc::boxed::Box;
use core::borrow::Borrow;
use core::iter::FusedIterator;
use core::mem::{self, ManuallyDrop, MaybeUninit};
use core::ops::Range;
use core::ptr;

use crate::allocator::{BlockAllocator, Nodable};
use crate::stacks::LinkedListStack;

#[derive(Debug)]
pub struct HeightBalancedTree<K, V> {
    allocator: BlockAllocator<TreeNode<K, V>>,
    root: *mut TreeNode<K, V>,
    length: usize,
}

impl<K, V> Default for HeightBalancedTree<K, V>
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

impl<K, V> HeightBalancedTree<K, V>
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

    unsafe fn rebalance(mut nodes: LinkedListStack<*mut TreeNode<K, V>>) {
        let mut finished = false;
        while !nodes.is_empty() && !finished {
            let tmp_node = nodes.pop();
            let old_height = (*tmp_node).height;
            if (*(*tmp_node).left.as_node()).height - (*(*tmp_node).right).height == 2 {
                if (*(*(*tmp_node).left.as_node()).left.as_node()).height
                    - (*(*tmp_node).right).height
                    == 1
                {
                    (*tmp_node).right_rotation();
                    (*(*tmp_node).right).height = (*(*(*tmp_node).right).left.as_node()).height + 1;
                    (*tmp_node).height = (*(*tmp_node).right).height + 1;
                } else {
                    (*(*tmp_node).left.as_node()).left_rotation();
                    (*tmp_node).right_rotation();
                    let tmp_height = (*(*(*tmp_node).left.as_node()).left.as_node()).height;
                    (*(*tmp_node).left.as_node()).height = tmp_height + 1;
                    (*(*tmp_node).right).height = tmp_height + 1;
                    (*tmp_node).height = tmp_height + 2;
                }
            } else if (*(*tmp_node).left.as_node()).height - (*(*tmp_node).right).height == -2 {
                if (*(*(*tmp_node).right).right).height - (*(*tmp_node).left.as_node()).height == 1
                {
                    (*tmp_node).left_rotation();
                    (*(*tmp_node).left.as_node()).height =
                        (*(*(*tmp_node).left.as_node()).right).height + 1;
                    (*tmp_node).height = (*(*tmp_node).left.as_node()).height + 1;
                } else {
                    (*(*tmp_node).right).right_rotation();
                    (*tmp_node).left_rotation();
                    let tmp_height = (*(*(*tmp_node).right).right).height;
                    (*(*tmp_node).left.as_node()).height = tmp_height + 1;
                    (*(*tmp_node).right).height = tmp_height + 1;
                    (*tmp_node).height = tmp_height + 2;
                }
            } else {
                #[allow(clippy::collapsible_else_if)]
                if (*(*tmp_node).left.as_node()).height > (*(*tmp_node).right).height {
                    (*tmp_node).height = (*(*tmp_node).left.as_node()).height + 1;
                } else {
                    (*tmp_node).height = (*(*tmp_node).right).height + 1;
                }
            }
            if (*tmp_node).height == old_height {
                finished = true;
            }
        }
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.length += 1;
        unsafe {
            if (*self.root).is_empty() {
                (*self.root).left = TreePtr::Val(Box::into_raw(Box::new(value)));
                (*self.root).key = MaybeUninit::new(key);
                (*self.root).height = 0;
                return None;
            }

            let mut nodes = LinkedListStack::default();
            let mut tmp_node = self.root;
            while !(*tmp_node).right.is_null() {
                nodes.push(tmp_node);
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

            let old_leaf = self.allocator.get_node();
            (*old_leaf).left = (*tmp_node).left;
            (*old_leaf).key = MaybeUninit::new((*tmp_node).key.assume_init_read());
            (*old_leaf).height = 0;

            let new_leaf = self.allocator.get_node();
            (*new_leaf).left = TreePtr::Val(Box::into_raw(Box::new(value)));
            (*new_leaf).key = MaybeUninit::new(key.clone());
            (*new_leaf).height = 0;

            if (*tmp_node).key.assume_init_ref() < &key {
                (*tmp_node).left = TreePtr::Node(old_leaf);
                (*tmp_node).right = new_leaf;
                (*tmp_node).key = MaybeUninit::new(key);
            } else {
                (*tmp_node).left = TreePtr::Node(new_leaf);
                (*tmp_node).right = old_leaf;
            }
            (*tmp_node).height = 1;
            Self::rebalance(nodes);

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

            let mut nodes = LinkedListStack::default();
            let mut upper_node = ptr::null_mut();
            let mut other_node = ptr::null_mut();
            let mut tmp_node = self.root;
            while !(*tmp_node).right.is_null() {
                nodes.push(tmp_node);
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
            (*upper_node).height = (*other_node).height;
            let val_ptr = mem::take(&mut (*tmp_node).left).as_val();
            (*tmp_node).key.assume_init_drop();
            self.allocator.return_node(tmp_node);
            self.allocator.return_node(other_node);
            self.length -= 1;

            nodes.pop();
            Self::rebalance(nodes);

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
}

impl<K, V> Drop for HeightBalancedTree<K, V> {
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
            let val_ptr = (*current_node).left.as_val();
            drop(*Box::from_raw(val_ptr));
            (*current_node).key.assume_init_drop();
            self.allocator.return_node(current_node);
        }
    }
}

impl<K, V> FromIterator<(K, V)> for HeightBalancedTree<K, V>
where
    K: Ord + Clone,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut tree = Self::default();
        for (k, v) in iter {
            tree.insert(k, v);
        }
        tree
    }
}

////////////////////////////////////////////////////////////////////////////////////////

pub struct SearchTreeIter<'a, K, V> {
    _tree: &'a HeightBalancedTree<K, V>,
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
                    debug_assert!((*node).height == 0);
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
                    debug_assert!(
                        (*node).height
                            == 1 + core::cmp::max(
                                (*(*node).left.as_node()).height,
                                (*(*node).right).height
                            )
                    );
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

////////////////////////////////////////////////////////////////////////////////////////

pub struct SearchTreeFind<'a, K, V, Q> {
    _tree: &'a HeightBalancedTree<K, V>,
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

////////////////////////////////////////////////////////////////////////////////////////

impl<K, V> IntoIterator for HeightBalancedTree<K, V>
where
    K: Ord,
{
    type Item = (K, V);
    type IntoIter = SearchTreeIntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        let tree = ManuallyDrop::new(self);
        SearchTreeIntoIter {
            current_node: tree.root,
            tree,
        }
    }
}

pub struct SearchTreeIntoIter<K, V>
where
    K: Ord,
{
    current_node: *mut TreeNode<K, V>,
    tree: ManuallyDrop<HeightBalancedTree<K, V>>,
}

impl<K, V> Iterator for SearchTreeIntoIter<K, V>
where
    K: Ord,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.current_node.is_null() || (*self.current_node).is_empty() {
                return None;
            }
            while (*self.current_node).has_subtrees() {
                if (*(*self.current_node).left.as_node()).is_leaf() {
                    let leaf_node = (*self.current_node).left.as_node();
                    let val_ptr = (*leaf_node).left.as_val();
                    let val = *Box::from_raw(val_ptr);
                    let key = (*leaf_node).key.assume_init_read();
                    self.tree.allocator.return_node(leaf_node);

                    let tmp = (*self.current_node).right;
                    (*self.current_node).key.assume_init_drop();
                    self.tree.allocator.return_node(self.current_node);
                    self.current_node = tmp;

                    return Some((key, val));
                } else {
                    let tmp = (*self.current_node).left.as_node();
                    (*self.current_node).left = TreePtr::Node((*tmp).right);
                    (*tmp).right = self.current_node;
                    self.current_node = tmp;
                }
            }
            let val_ptr = (*self.current_node).left.as_val();
            let val = *Box::from_raw(val_ptr);
            let key = (*self.current_node).key.assume_init_read();
            self.tree.allocator.return_node(self.current_node);
            self.current_node = ptr::null_mut();
            Some((key, val))
        }
    }
}

impl<K, V> Drop for SearchTreeIntoIter<K, V>
where
    K: Ord,
{
    fn drop(&mut self) {
        unsafe {
            while self.next().is_some() {}
            ptr::drop_in_place(&mut self.tree.allocator as *mut _);
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct TreeNode<K, V> {
    pub key: MaybeUninit<K>,
    pub right: *mut TreeNode<K, V>,
    pub left: TreePtr<K, V>,
    pub height: isize,
}

impl<K, V> Default for TreeNode<K, V> {
    fn default() -> Self {
        Self {
            key: MaybeUninit::uninit(),
            right: ptr::null_mut(),
            left: TreePtr::Null,
            height: 0,
        }
    }
}

impl<K, V> Nodable for TreeNode<K, V> {
    fn next(&self) -> *mut Self {
        self.right
    }

    fn next_mut(&mut self) -> &mut *mut Self {
        &mut self.right
    }
}

#[derive(Debug, Default)]
pub enum TreePtr<K, V> {
    #[default]
    Null,
    Node(*mut TreeNode<K, V>),
    Val(*mut V),
}

impl<K, V> Clone for TreePtr<K, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K, V> Copy for TreePtr<K, V> {}

impl<K, V> TreePtr<K, V> {
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn is_val(&self) -> bool {
        matches!(self, Self::Val(_))
    }

    pub fn is_node(&self) -> bool {
        matches!(self, Self::Node(_))
    }

    pub fn as_node(&self) -> *mut TreeNode<K, V> {
        match *self {
            Self::Node(ptr) => ptr,
            _ => panic!("tree pointer is not a node"),
        }
    }

    pub fn as_val(&self) -> *mut V {
        match *self {
            Self::Val(ptr) => ptr,
            _ => panic!("tree pointer is not a value"),
        }
    }

    pub fn as_val_mut(&mut self) -> &mut *mut V {
        match self {
            Self::Val(ptr) => ptr,
            _ => panic!("tree pointer is not a value"),
        }
    }
}

impl<K, V> TreeNode<K, V> {
    pub fn is_empty(&self) -> bool {
        self.left.is_null() && self.right.is_null()
    }

    pub fn is_leaf(&self) -> bool {
        self.left.is_val() && self.right.is_null()
    }

    pub fn has_subtrees(&self) -> bool {
        self.left.is_node() && !self.right.is_null()
    }

    pub fn left_rotation(&mut self) {
        assert!(
            self.has_subtrees() && unsafe { (*self.right).has_subtrees() },
            "invalid left rotation"
        );
        unsafe {
            let tmp_node = self.left;
            let tmp_key = self.key.assume_init_read();
            self.left = TreePtr::Node(self.right);
            self.key = MaybeUninit::new((*self.right).key.assume_init_read());
            self.right = (*(self.left).as_node()).right;
            (*(self.left).as_node()).right = (*(self.left).as_node()).left.as_node();
            (*(self.left).as_node()).left = tmp_node;
            (*(self.left).as_node()).key = MaybeUninit::new(tmp_key);
        }
    }

    pub fn right_rotation(&mut self) {
        assert!(
            self.has_subtrees() && unsafe { (*(self.left).as_node()).has_subtrees() },
            "invalid right rotation"
        );
        unsafe {
            let tmp_node = self.right;
            let tmp_key = self.key.assume_init_read();
            self.right = self.left.as_node();
            self.key = MaybeUninit::new((*self.left.as_node()).key.assume_init_read());
            self.left = (*self.right).left;
            (*self.right).left = TreePtr::Node((*self.right).right);
            (*self.right).right = tmp_node;
            (*self.right).key = MaybeUninit::new(tmp_key);
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_tree_ok() {
        let mut tree = HeightBalancedTree::default();
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
        tree.insert(3, 30);
        assert_eq!(Some(&30), tree.get(&3));
        assert_eq!(4, tree.find(1..5).count());

        tree = HeightBalancedTree::default();
        drop(tree);

        let tree = HeightBalancedTree::from_iter([(2, 20), (1, 10), (3, 30), (4, 40)]);
        assert_eq!(Some(&30), tree.get(&3));
        assert_eq!(4, tree.len());
        assert_eq!(3, tree.find(2..5).count());
    }

    #[test]
    fn search_tree_iter() {
        let tree = HeightBalancedTree::from_iter([(1, 10), (3, 30), (4, 40), (2, 20)]);

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

        for ((k, v), i) in tree.into_iter().zip(1..5) {
            assert_eq!((k, v), (i, i * 10));
        }

        let tree = HeightBalancedTree::from_iter([(4, 40), (1, 10), (2, 20), (3, 30)]);
        let mut iter = tree.into_iter();
        assert_eq!(Some((1, 10)), iter.next());
        drop(iter);

        let tree: HeightBalancedTree<usize, usize> = HeightBalancedTree::default();
        let iter = tree.into_iter();
        drop(iter);

        let mut tree = HeightBalancedTree::from_iter([(5, 50), (3, 30), (1, 10), (2, 20), (4, 40)]);
        for ((&k, &v), i) in tree.iter().zip(1..5) {
            assert_eq!((k, v), (i, i * 10));
        }
        tree.remove(&5);
        for ((&k, &v), i) in tree.iter().zip(1..4) {
            assert_eq!((k, v), (i, i * 10));
        }
        tree.insert(5, 50);
        for ((&k, &v), i) in tree.iter().zip(1..5) {
            assert_eq!((k, v), (i, i * 10));
        }
    }

    #[test]
    fn search_tree_find() {
        let tree = HeightBalancedTree::from_iter([(2, 20), (1, 10), (3, 30), (4, 40)]);
        for ((&k, &v), i) in tree.find(2..5).zip(2..5) {
            assert_eq!((k, v), (i, i * 10));
        }
        assert_eq!(3, tree.find(2..5).count());

        let tree = HeightBalancedTree::from_iter([(5, 50), (1, 10), (2, 20), (3, 30), (4, 40)]);
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
        let tree = HeightBalancedTree::from_iter([
            ("1".to_string(), 10),
            ("4".to_string(), 40),
            ("3".to_string(), 30),
            ("2".to_string(), 20),
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
            HeightBalancedTree::from_iter([(Int(3), 30), (Int(1), 10), (Int(2), 20), (Int(4), 40)]);
        let start = Int(2);
        let end = Int(5);
        for ((k, &v), i) in tree.find(&start..&end).zip(2..5) {
            assert_eq!((k, v), (&Int(i), i * 10));
        }
        assert_eq!(3, tree.find(&start..&end).count());
        assert_eq!(3, tree.find(start..end).count());
    }
}
