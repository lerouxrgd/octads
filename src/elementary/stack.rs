use alloc::alloc::{alloc, dealloc, handle_alloc_error, Layout};
use core::mem::{self, MaybeUninit};
use core::ptr;

use crate::allocator::{BlockAllocator, Node};

#[derive(Debug)]
pub struct ArrayStack<T, const N: usize> {
    stack: [MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> Default for ArrayStack<T, N> {
    fn default() -> Self {
        Self {
            stack: unsafe { MaybeUninit::uninit().assume_init() },
            len: 0,
        }
    }
}

impl<T, const N: usize> ArrayStack<T, N> {
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn max_len(&self) -> usize {
        self.stack.len()
    }

    pub fn push(&mut self, val: T) {
        assert!(
            self.len < self.stack.len(),
            "overflow: pushing to a full stack"
        );
        self.stack[self.len].write(val);
        self.len += 1;
    }

    pub fn pop(&mut self) -> T {
        assert!(!self.is_empty(), "underflow: popping from an empty stack");
        self.len -= 1;
        let mut val = MaybeUninit::uninit();
        mem::swap(&mut self.stack[self.len], &mut val);
        unsafe { val.assume_init() }
    }

    pub fn peek(&self) -> &T {
        assert!(!self.is_empty(), "underflow: peeking at an empty stack");
        let peek = self.len - 1;
        unsafe { self.stack[peek].assume_init_ref() }
    }
}

impl<T, const N: usize> Drop for ArrayStack<T, N> {
    fn drop(&mut self) {
        while !self.is_empty() {
            self.pop();
        }
    }
}

#[derive(Debug)]
pub struct AllocatedStack<T> {
    base: *mut T,
    top: *mut T,
    max_size: usize,
}

impl<T> AllocatedStack<T> {
    pub fn new(max_size: usize) -> Self {
        let layout = Layout::array::<T>(max_size).expect("Couldn't create memory layout");
        let base = unsafe { alloc(layout) };
        if base.is_null() {
            handle_alloc_error(layout);
        }
        let base = base as *mut _;
        let top = base;

        Self {
            base,
            top,
            max_size,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.base == self.top
    }

    pub fn len(&self) -> usize {
        unsafe { self.top.offset_from(self.base) as usize }
    }

    pub fn max_len(&self) -> usize {
        self.max_size as usize
    }

    pub fn push(&mut self, val: T) {
        unsafe {
            assert!(
                self.top < self.base.add(self.max_size),
                "overflow: pushing to a full stack"
            );
            ptr::write(self.top, val);
            self.top = self.top.offset(1);
        }
    }

    pub fn pop(&mut self) -> T {
        unsafe {
            assert!(!self.is_empty(), "underflow: popping from an empty stack");
            self.top = self.top.offset(-1);
            ptr::read(self.top)
        }
    }

    pub fn peek(&self) -> &T {
        unsafe {
            assert!(!self.is_empty(), "underflow: peeking at an empty stack");
            let peek = self.top.offset(-1);
            &*peek
        }
    }
}

impl<T> Drop for AllocatedStack<T> {
    fn drop(&mut self) {
        while !self.is_empty() {
            self.pop();
        }
        let layout = Layout::array::<T>(self.max_size).unwrap();
        unsafe { dealloc(self.base as *mut u8, layout) };
    }
}

#[derive(Debug)]
pub struct LinkedListStack<T> {
    allocator: BlockAllocator<T>,
    len: usize,
    head: *mut Node<T>,
}

impl<T> LinkedListStack<T> {
    pub fn new(block_size: usize, blocks_cap: usize) -> Self {
        Self {
            allocator: BlockAllocator::new(block_size, blocks_cap),
            len: 0,
            head: ptr::null_mut(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push(&mut self, val: T) {
        let tmp = self.allocator.get_node();
        unsafe {
            (*tmp).val = MaybeUninit::new(val);
            (*tmp).next = self.head;
        }
        self.head = tmp;
        self.len += 1;
    }

    pub fn pop(&mut self) -> T {
        assert!(!self.is_empty(), "underflow: popping from an empty stack");
        let tmp = self.head;
        self.head = unsafe { (*tmp).next };
        let mut val = MaybeUninit::uninit();
        unsafe {
            mem::swap(&mut val, &mut (*tmp).val);
            self.allocator.return_node(tmp);
            self.len -= 1;
            val.assume_init()
        }
    }

    pub fn peek(&self) -> &T {
        assert!(!self.is_empty(), "underflow: peeking at an empty stack");
        unsafe { (*self.head).val.assume_init_ref() }
    }
}

impl<T> Drop for LinkedListStack<T> {
    fn drop(&mut self) {
        let mut next = self.head;
        while !next.is_null() {
            let tmp = next;
            unsafe {
                (*tmp).val.assume_init_drop();
                next = (*tmp).next;
                self.allocator.return_node(tmp);
            }
        }
    }
}

#[derive(Debug)]
pub struct LinkedAllocatedStack<T> {
    base: *mut T,
    top: *mut T,
    chunk_size: usize,
    previous: *mut LinkedAllocatedStack<T>,
    len: usize,
}

impl<T> LinkedAllocatedStack<T> {
    pub fn new(chunk_size: usize) -> Self {
        let chunk_layout = Layout::array::<T>(chunk_size).expect("Couldn't create memory layout");
        let base = unsafe { alloc(chunk_layout) };
        if base.is_null() {
            handle_alloc_error(chunk_layout);
        }
        let base = base as *mut _;
        let top = base;

        Self {
            base,
            top,
            chunk_size,
            previous: ptr::null_mut(),
            len: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.base == self.top && self.previous.is_null()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push(&mut self, val: T) {
        if self.top == unsafe { self.base.add(self.chunk_size) } {
            let node_layout = Layout::new::<LinkedAllocatedStack<T>>();
            let new_node = unsafe { alloc(node_layout) };
            if new_node.is_null() {
                handle_alloc_error(node_layout);
            }
            let new_node = new_node as *mut LinkedAllocatedStack<T>;
            unsafe {
                (*new_node).base = self.base;
                (*new_node).top = self.top;
                (*new_node).chunk_size = self.chunk_size;
                (*new_node).previous = self.previous;
            }

            let chunk_layout = Layout::array::<T>(self.chunk_size).unwrap();
            let new_chunk = unsafe { alloc(chunk_layout) };
            if new_chunk.is_null() {
                handle_alloc_error(chunk_layout);
            }
            let new_chunk = new_chunk as *mut _;

            self.previous = new_node;
            self.base = new_chunk;
            self.top = self.base;
        }
        unsafe {
            ptr::write(self.top, val);
            self.top = self.top.offset(1);
            self.len += 1;
        }
    }

    pub fn pop(&mut self) -> T {
        assert!(!self.is_empty(), "underflow: popping from an empty stack");
        if self.base == self.top {
            unsafe {
                let chunk_layout = Layout::array::<T>(self.chunk_size).unwrap();
                dealloc(self.base as *mut u8, chunk_layout);
                let old_node = self.previous;
                self.previous = (*old_node).previous;
                self.base = (*old_node).base;
                self.top = (*old_node).top;
                self.chunk_size = (*old_node).chunk_size;
                let node_layout = Layout::new::<LinkedAllocatedStack<T>>();
                dealloc(old_node as *mut u8, node_layout);
            }
        }
        unsafe {
            self.len -= 1;
            self.top = self.top.offset(-1);
            ptr::read(self.top)
        }
    }

    pub fn peek(&self) -> &T {
        if self.base == self.top {
            unsafe { &*(*self.previous).top.offset(-1) }
        } else {
            unsafe { &*self.top.offset(-1) }
        }
    }
}

impl<T> Drop for LinkedAllocatedStack<T> {
    fn drop(&mut self) {
        while !self.is_empty() {
            self.pop();
        }
        let chunk_layout = Layout::array::<T>(self.chunk_size).unwrap();
        unsafe { dealloc(self.base as *mut u8, chunk_layout) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array_stack_ok() {
        let mut stack: ArrayStack<usize, 10> = ArrayStack::default();
        stack.push(3);
        stack.push(2);
        stack.push(1);
        assert_eq!(&1, stack.peek());
        assert_eq!(3, stack.len());
        assert_eq!(1, stack.pop());

        stack.pop();
        stack.pop();
        assert!(stack.is_empty());

        let range = 4..=9;
        for i in range.clone() {
            stack.push(i);
        }
        assert_eq!(range.clone().count(), stack.len());
        for i in range.rev() {
            assert_eq!(i, stack.pop());
        }
        assert!(stack.is_empty());
    }

    #[test]
    #[should_panic(expected = "underflow: popping from an empty stack")]
    fn array_stack_panic_underflow() {
        let mut stack: ArrayStack<usize, 1> = ArrayStack::default();
        stack.push(1);
        stack.pop();
        assert!(stack.is_empty());
        stack.pop();
    }

    #[test]
    #[should_panic(expected = "overflow: pushing to a full stack")]
    fn array_stack_overflow() {
        let mut stack: ArrayStack<usize, 1> = ArrayStack::default();
        stack.push(1);
        stack.push(2);
    }

    #[test]
    fn allocated_stack_ok() {
        let mut stack = AllocatedStack::new(10);
        stack.push(3);
        stack.push(2);
        stack.push(1);
        assert_eq!(&1, stack.peek());
        assert_eq!(3, stack.len());
        assert_eq!(1, stack.pop());

        stack.pop();
        stack.pop();
        assert!(stack.is_empty());

        let range = 4..=9;
        for i in range.clone() {
            stack.push(i);
        }
        assert_eq!(range.clone().count(), stack.len());
        for i in range.rev() {
            assert_eq!(i, stack.pop());
        }
        assert!(stack.is_empty());
    }

    #[test]
    #[should_panic(expected = "underflow: popping from an empty stack")]
    fn allocated_stack_panic_underflow() {
        let mut stack = AllocatedStack::new(1);
        stack.push(1);
        stack.pop();
        assert!(stack.is_empty());
        stack.pop();
    }

    #[test]
    #[should_panic(expected = "overflow: pushing to a full stack")]
    fn allocated_stack_overflow() {
        let mut stack = AllocatedStack::new(1);
        stack.push(1);
        stack.push(2);
    }

    #[test]
    fn linked_list_stack_ok() {
        let mut stack = LinkedListStack::new(2, 1);
        stack.push(3);
        stack.push(2);
        stack.push(1);
        assert_eq!(&1, stack.peek());
        assert_eq!(3, stack.len());
        assert_eq!(1, stack.pop());

        stack.pop();
        stack.pop();
        assert!(stack.is_empty());

        let range = 4..=9;
        for i in range.clone() {
            stack.push(i);
        }
        assert_eq!(range.clone().count(), stack.len());
        for i in range.rev() {
            assert_eq!(i, stack.pop());
        }
        assert!(stack.is_empty());
    }

    #[test]
    #[should_panic(expected = "underflow: popping from an empty stack")]
    fn linked_list_stack_panic_underflow() {
        let mut stack = LinkedListStack::new(4, 2);
        stack.push(1);
        stack.pop();
        assert!(stack.is_empty());
        stack.pop();
    }

    #[test]
    fn linked_allocated_stack_ok() {
        let mut stack = LinkedAllocatedStack::new(2);
        stack.push(3);
        stack.push(2);
        stack.push(1);
        assert_eq!(&1, stack.peek());
        assert_eq!(3, stack.len());
        assert_eq!(1, stack.pop());

        stack.pop();
        stack.pop();
        assert!(stack.is_empty());

        let range = 4..=9;
        for i in range.clone() {
            stack.push(i);
        }
        assert_eq!(range.clone().count(), stack.len());
        for i in range.rev() {
            assert_eq!(i, stack.pop());
        }
        assert!(stack.is_empty());
    }

    #[test]
    #[should_panic(expected = "underflow: popping from an empty stack")]
    fn linked_allocated_stack_panic_underflow() {
        let mut stack = LinkedAllocatedStack::new(4);
        stack.push(1);
        stack.pop();
        assert!(stack.is_empty());
        stack.pop();
    }
}
