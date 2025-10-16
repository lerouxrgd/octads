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

            // TODO: impl that
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
}

////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct TreeNode<K, V> {
    pub key: MaybeUninit<K>,
    pub right: *mut TreeNode<K, V>,
    pub left: TreePtr<K, V>,
    pub height: usize,
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
