pub mod search_tree;

use core::mem::MaybeUninit;
use core::ptr;

use crate::allocator::Nodable;

#[derive(Debug)]
pub struct TreeNode<K, V> {
    pub key: MaybeUninit<K>,
    pub right: *mut TreeNode<K, V>,
    pub left: TreeElem<K, V>,
}

impl<K, V> Default for TreeNode<K, V> {
    fn default() -> Self {
        Self {
            key: MaybeUninit::uninit(),
            right: ptr::null_mut(),
            left: TreeElem::Null,
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
pub enum TreeElem<K, V> {
    #[default]
    Null,
    Node(*mut TreeNode<K, V>),
    Leaf(*mut V), // TODO: rename to Value ?!
}

impl<K, V> Clone for TreeElem<K, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K, V> Copy for TreeElem<K, V> {}

impl<K, V> TreeElem<K, V> {
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn is_leaf(&self) -> bool {
        matches!(self, Self::Leaf(_))
    }

    pub fn is_node(&self) -> bool {
        matches!(self, Self::Node(_))
    }

    pub fn as_node(&self) -> *mut TreeNode<K, V> {
        match *self {
            Self::Node(ptr) => ptr,
            _ => panic!("tree element is not a node"),
        }
    }

    pub fn as_leaf(&self) -> *mut V {
        match *self {
            Self::Leaf(ptr) => ptr,
            _ => panic!("tree element is not a leaf"),
        }
    }

    pub fn as_leaf_mut(&mut self) -> &mut *mut V {
        match self {
            Self::Leaf(ptr) => ptr,
            _ => panic!("tree element is not a leaf"),
        }
    }
}

impl<K, V> TreeNode<K, V> {
    pub fn is_empty(&self) -> bool {
        self.left.is_null() && self.right.is_null()
    }

    pub fn has_value(&self) -> bool {
        self.left.is_leaf() && self.right.is_null()
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
            self.left = TreeElem::Node(self.right);
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
            (*self.right).left = TreeElem::Node((*self.right).right);
            (*self.right).right = tmp_node;
            (*self.right).key = MaybeUninit::new(tmp_key);
        }
    }
}
