use alloc::alloc::{alloc, dealloc, handle_alloc_error, Layout};
use alloc::vec::Vec;
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
        self.len -= 1;
        let mut val = MaybeUninit::uninit();
        mem::swap(&mut self.stack[self.len], &mut val);
        unsafe { val.assume_init() }
    }

    pub fn peek(&self) -> &T {
        let peek = self.len - 1;
        unsafe { self.stack[peek].assume_init_ref() }
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

    pub fn max_ln(&self) -> usize {
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
            self.top = self.top.offset(-1);
            assert!(
                self.top >= self.base,
                "underflow: popping from an empty stack"
            );
            ptr::read(self.top)
        }
    }

    pub fn peek(&self) -> &T {
        unsafe {
            let peek = self.top.offset(-1);
            assert!(peek >= self.base, "underflow: peeking from an empty stack");
            &*peek
        }
    }
}

impl<T> Drop for AllocatedStack<T> {
    fn drop(&mut self) {
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
    pub fn new(block_size: usize) -> Self {
        Self {
            allocator: BlockAllocator::new(block_size),
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
        unsafe { (*self.head).val.assume_init_ref() }
    }
}

impl<T> Drop for LinkedListStack<T> {
    fn drop(&mut self) {
        let mut next = self.head;
        while !next.is_null() {
            let tmp = next;
            unsafe {
                #[cfg(test)]
                libc_print::libc_println!("Hello !");
                (*tmp).val.assume_init_drop();
                next = (*tmp).next;
                self.allocator.return_node(tmp);
            }
        }
    }
}

#[derive(Debug)]
pub struct VecStack<T> {
    stack: Vec<T>,
}

impl<T> Default for VecStack<T> {
    fn default() -> Self {
        Self {
            stack: Vec::default(),
        }
    }
}

impl<T> VecStack<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            stack: Vec::with_capacity(capacity),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn push(&mut self, val: T) {
        self.stack.push(val);
    }

    pub fn pop(&mut self) -> T {
        self.stack.pop().expect("attempt to pop an empty stack")
    }

    pub fn peek(&self) -> &T {
        self.stack.last().expect("attempt to peek an empty stack")
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
    #[should_panic(expected = "attempt to subtract with overflow")]
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

    // TODO: investigate cargo miri test allocated_stack
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
    fn vec_stack_ok() {
        let mut stack = VecStack::default();
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
    #[should_panic(expected = "attempt to pop an empty stack")]
    fn vec_stack_panic_underflow() {
        let mut stack = VecStack::default();
        stack.push(1);
        stack.pop();
        assert!(stack.is_empty());
        stack.pop();
    }

    #[test]
    fn linked_list_stack_ok() {
        let mut stack = LinkedListStack::new(2);
        stack.push(3);
        stack.push(2);
        // stack.push(1); // TODO: this triggers a bug on Drop due to bad realloc usage
        // assert_eq!(&1, stack.peek());
        // assert_eq!(3, stack.len());
        // assert_eq!(1, stack.pop());

        // stack.pop();
        // stack.pop();
        // assert!(stack.is_empty());

        // let range = 4..=9;
        // for i in range.clone() {
        //     stack.push(i);
        // }
        // assert_eq!(range.clone().count(), stack.len());
        // for i in range.rev() {
        //     assert_eq!(i, stack.pop());
        // }
        // assert!(stack.is_empty());
    }
}
