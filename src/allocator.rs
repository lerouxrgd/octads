use alloc::alloc::{alloc, dealloc, handle_alloc_error, realloc, Layout};
use core::mem::MaybeUninit;
use core::ptr;

#[derive(Debug)]
pub struct BlockAllocator<T> {
    blocks: *mut *mut Node<T>,
    blocks_cap: usize,
    blocks_len: usize,
    cursor: *mut Node<T>,
    block_size: usize,
    size_left: usize,
    free_list: *mut Node<T>,
}

impl<T> BlockAllocator<T> {
    pub fn new(block_size: usize, blocks_cap: usize) -> Self {
        assert!(block_size > 0, "invalid block size of 0");
        assert!(blocks_cap > 0, "invalid blocks capacity of 0");

        let layout =
            Layout::array::<*mut Node<T>>(blocks_cap).expect("Couldn't create memory layout");
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

    pub fn get_node(&mut self) -> *mut Node<T> {
        let node;
        if !self.free_list.is_null() {
            node = self.free_list;
            self.free_list = unsafe { (*self.free_list).next };
        } else {
            if self.cursor.is_null() || self.size_left == 0 {
                let layout = Layout::array::<Node<T>>(self.block_size)
                    .expect("Couldn't create memory layout");
                let new_block = unsafe { alloc(layout) };
                if new_block.is_null() {
                    handle_alloc_error(layout);
                }
                let new_block = new_block as *mut _;

                if self.blocks_len == self.blocks_cap {
                    let old_layout = Layout::array::<*mut Node<T>>(self.blocks_cap).unwrap();
                    self.blocks_cap *= 2;
                    let new_layout = Layout::array::<*mut Node<T>>(self.blocks_cap)
                        .expect("Couldn't create memory layout");
                    let blocks =
                        unsafe { realloc(self.blocks as *mut u8, old_layout, new_layout.size()) };
                    if blocks.is_null() {
                        handle_alloc_error(layout);
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
        unsafe {
            (*node).next = ptr::null_mut();
            (*node).val = MaybeUninit::uninit();
        }
        node
    }

    /// # Safety
    ///
    /// Returned node must have its val uninit/dropped
    pub unsafe fn return_node(&mut self, node: *mut Node<T>) {
        (*node).next = self.free_list;
        self.free_list = node;
    }
}

impl<T> Drop for BlockAllocator<T> {
    fn drop(&mut self) {
        for i in 0..self.blocks_len {
            let layout = Layout::array::<Node<T>>(self.block_size).unwrap();
            unsafe { dealloc(*self.blocks.add(i) as *mut u8, layout) };
        }
        let layout = Layout::array::<*mut Node<T>>(self.blocks_cap).unwrap();
        unsafe { dealloc(self.blocks as *mut u8, layout) };
    }
}

#[derive(Debug)]
pub struct Node<T> {
    pub next: *mut Node<T>,
    pub val: MaybeUninit<T>,
}
