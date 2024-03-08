use alloc::alloc::{alloc, dealloc, handle_alloc_error, realloc, Layout};
use core::mem::MaybeUninit;
use core::ptr;

pub trait Nodable: Default {
    fn next(&self) -> *mut Self;
    fn next_mut(&mut self) -> &mut *mut Self;
}

#[derive(Debug)]
pub struct BlockAllocator<N>
where
    N: Nodable,
{
    blocks: *mut *mut N,
    blocks_cap: usize,
    blocks_len: usize,
    cursor: *mut N,
    block_size: usize,
    size_left: usize,
    free_list: *mut N,
}

impl<N> Default for BlockAllocator<N>
where
    N: Nodable,
{
    fn default() -> Self {
        Self::new(Self::DEFAULT_BLOCK_SIZE, Self::DEFAULT_BLOCK_CAP)
    }
}

impl<N> BlockAllocator<N>
where
    N: Nodable,
{
    pub const DEFAULT_BLOCK_SIZE: usize = 256;
    pub const DEFAULT_BLOCK_CAP: usize = 32;

    pub fn new(block_size: usize, blocks_cap: usize) -> Self {
        assert!(block_size > 0, "invalid block size of 0");
        assert!(blocks_cap > 0, "invalid blocks capacity of 0");

        let layout = Layout::array::<*mut N>(blocks_cap).expect("Couldn't create memory layout");
        let blocks = unsafe { alloc(layout) };
        if blocks.is_null() {
            handle_alloc_error(layout);
        }

        Self {
            blocks: blocks as *mut _,
            blocks_len: 0,
            blocks_cap,
            cursor: ptr::null_mut(),
            block_size,
            size_left: 0,
            free_list: ptr::null_mut(),
        }
    }

    pub fn get_node(&mut self) -> *mut N {
        let node;
        if !self.free_list.is_null() {
            node = self.free_list;
            self.free_list = unsafe { (*self.free_list).next() };
        } else {
            if self.cursor.is_null() || self.size_left == 0 {
                let layout =
                    Layout::array::<N>(self.block_size).expect("Couldn't create memory layout");
                let new_block = unsafe { alloc(layout) };
                if new_block.is_null() {
                    handle_alloc_error(layout);
                }
                let new_block = new_block as *mut _;

                if self.blocks_len == self.blocks_cap {
                    let old_layout = Layout::array::<*mut N>(self.blocks_cap).unwrap();
                    self.blocks_cap *= 2;
                    let new_layout = Layout::array::<*mut N>(self.blocks_cap)
                        .expect("Couldn't create memory layout");
                    let blocks =
                        unsafe { realloc(self.blocks as *mut u8, old_layout, new_layout.size()) };
                    if blocks.is_null() {
                        handle_alloc_error(new_layout);
                    }
                    self.blocks = blocks as *mut _;
                }
                unsafe { self.blocks.add(self.blocks_len).write(new_block) };
                self.blocks_len += 1;

                self.cursor = new_block;
                self.size_left = self.block_size;
            }
            node = self.cursor;
            self.cursor = unsafe { self.cursor.add(1) };
            self.size_left -= 1;
        }
        unsafe { ptr::write(node, Default::default()) };
        node
    }

    /// # Safety
    ///
    /// Returned node must have its fields uninit/dropped
    pub unsafe fn return_node(&mut self, node: *mut N) {
        unsafe { *(*node).next_mut() = self.free_list };
        self.free_list = node;
    }
}

impl<N> Drop for BlockAllocator<N>
where
    N: Nodable,
{
    fn drop(&mut self) {
        for i in 0..self.blocks_len {
            let layout = Layout::array::<N>(self.block_size).unwrap();
            unsafe { dealloc(*self.blocks.add(i) as *mut u8, layout) };
        }
        let layout = Layout::array::<*mut N>(self.blocks_cap).unwrap();
        unsafe { dealloc(self.blocks as *mut u8, layout) };
    }
}

#[derive(Debug)]
pub struct Node<T> {
    pub next: *mut Node<T>,
    pub val: MaybeUninit<T>,
}

impl<T> Default for Node<T> {
    fn default() -> Self {
        Self {
            next: ptr::null_mut(),
            val: MaybeUninit::uninit(),
        }
    }
}

impl<T> Nodable for Node<T> {
    fn next(&self) -> *mut Self {
        self.next
    }

    fn next_mut(&mut self) -> &mut *mut Self {
        &mut self.next
    }
}

#[derive(Debug)]
pub struct BiNode<T> {
    pub next: *mut BiNode<T>,
    pub prev: *mut BiNode<T>,
    pub val: MaybeUninit<T>,
}

impl<T> Default for BiNode<T> {
    fn default() -> Self {
        Self {
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
            val: MaybeUninit::uninit(),
        }
    }
}

impl<T> Nodable for BiNode<T> {
    fn next(&self) -> *mut Self {
        self.next
    }

    fn next_mut(&mut self) -> &mut *mut Self {
        &mut self.next
    }
}
