use alloc::alloc::{alloc, dealloc, handle_alloc_error, realloc, Allocator, Global, Layout};
use core::mem::MaybeUninit;
use core::ptr;

#[derive(Debug)]
pub struct BlockAllocator<T> {
    blocks_ptr: *mut Node<T>,
    nb_blocks: usize,
    block_size: usize,
    size_left: usize,
    free_list: *mut Node<T>,
}

impl<T> BlockAllocator<T> {
    pub fn new(block_size: usize) -> Self {
        Self {
            blocks_ptr: ptr::null_mut(),
            nb_blocks: 0,
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
            if self.blocks_ptr.is_null() {
                self.nb_blocks = 1;
                let layout = Layout::array::<Node<T>>(self.nb_blocks * self.block_size)
                    .expect("Couldn't create memory layout");
                // TODO: expolore this instead:
                // let blocks_ptr = Global.allocate(layout);
                let blocks_ptr = unsafe { alloc(layout) };
                if blocks_ptr.is_null() {
                    handle_alloc_error(layout);
                }
                self.blocks_ptr = blocks_ptr as *mut _;
                self.size_left = self.block_size;
            } else if self.size_left == 0 {
                let old_layout =
                    Layout::array::<Node<T>>(self.nb_blocks * self.block_size).unwrap();
                self.nb_blocks += 1;
                let new_layout = Layout::array::<Node<T>>(self.nb_blocks * self.block_size)
                    .expect("Couldn't create memory layout");
                // TODO: this realloc invalidates previously issued pointers
                let blocks_ptr =
                    unsafe { realloc(self.blocks_ptr as *mut u8, old_layout, new_layout.size()) };
                if blocks_ptr.is_null() {
                    handle_alloc_error(new_layout);
                }
                self.blocks_ptr = blocks_ptr as *mut _;
                self.size_left = self.block_size;
            }
            let ptr_offset =
                (self.nb_blocks - 1) * self.block_size + (self.block_size - self.size_left);
            node = unsafe { self.blocks_ptr.add(ptr_offset) };
            self.size_left -= 1;
        }
        unsafe {
            (*node).next = ptr::null_mut();
            (*node).val = MaybeUninit::uninit();
        }
        node
    }

    // Safety: returned node must contain a MaybeUninit::uninit() value
    pub unsafe fn return_node(&mut self, node: *mut Node<T>) {
        (*node).next = self.free_list;
        self.free_list = node;
    }
}

impl<T> Drop for BlockAllocator<T> {
    fn drop(&mut self) {
        if !self.blocks_ptr.is_null() {
            let layout = Layout::array::<T>(self.nb_blocks * self.block_size).unwrap();
            unsafe { dealloc(self.blocks_ptr as *mut u8, layout) };
        }
    }
}

#[derive(Debug)]
pub struct Node<T> {
    pub next: *mut Node<T>,
    pub val: MaybeUninit<T>,
}
