use alloc::alloc::{alloc, dealloc, handle_alloc_error, Layout};
use alloc::vec::Vec;
use core::mem::{self, MaybeUninit};
use core::ptr;

#[derive(Debug)]
pub struct ArrayStack<T, const N: usize> {
    stack: [MaybeUninit<T>; N],
    i: usize,
}

impl<T, const N: usize> Default for ArrayStack<T, N> {
    fn default() -> Self {
        Self {
            stack: unsafe { MaybeUninit::uninit().assume_init() },
            i: 0,
        }
    }
}

impl<T, const N: usize> ArrayStack<T, N> {
    pub fn is_empty(&self) -> bool {
        self.i == 0
    }

    pub fn len(&self) -> usize {
        self.i
    }

    pub fn max_capacity(&self) -> usize {
        self.stack.len()
    }

    pub fn push(&mut self, val: T) {
        assert!(self.i < self.stack.len());
        self.stack[self.i].write(val);
        self.i += 1;
    }

    pub fn pop(&mut self) -> T {
        self.i -= 1;
        let mut val = MaybeUninit::uninit();
        mem::swap(&mut self.stack[self.i], &mut val);
        unsafe { val.assume_init() }
    }

    pub fn peek(&self) -> &T {
        let peek = self.i - 1;
        unsafe { self.stack[peek].assume_init_ref() }
    }
}

#[derive(Debug)]
pub struct AllocStack<T> {
    layout: Layout,
    base: *mut T,
    top: *mut T,
    size: isize,
}

impl<T> AllocStack<T> {
    pub fn new(size: usize) -> Self {
        let size = size as isize;
        let layout = Layout::array::<T>(size as usize).expect("Couldn't create memory layout");
        let base = unsafe { alloc(layout) };
        if base.is_null() {
            handle_alloc_error(layout);
        }
        let base = base as *mut _;
        let top = base;

        Self {
            layout,
            base,
            top,
            size,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.base == self.top
    }

    pub fn len(&self) -> usize {
        unsafe { self.top.offset_from(self.base) as usize }
    }

    pub fn max_capacity(&self) -> usize {
        self.size as usize
    }

    pub fn push(&mut self, val: T) {
        unsafe {
            assert!(self.top < self.base.offset(self.size));
            ptr::write(self.top, val);
            self.top = self.top.offset(1);
        }
    }

    pub fn pop(&mut self) -> T {
        unsafe {
            self.top = self.top.offset(-1);
            assert!(self.top >= self.base);
            ptr::read(self.top)
        }
    }

    pub fn peek(&self) -> &T {
        unsafe {
            let peek = self.top.offset(-1);
            assert!(peek >= self.base);
            &*peek
        }
    }
}

impl<T> Drop for AllocStack<T> {
    fn drop(&mut self) {
        unsafe { dealloc(self.base as *mut u8, self.layout) };
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
    pub fn with_capacity(size: usize) -> Self {
        Self {
            stack: Vec::with_capacity(size),
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
    #[should_panic(expected = "assertion failed: self.i < self.stack.len()")]
    fn array_stack_overflow() {
        let mut stack: ArrayStack<usize, 1> = ArrayStack::default();
        stack.push(1);
        stack.push(2);
    }

    #[test]
    fn alloc_stack_ok() {
        let mut stack = AllocStack::new(10);
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
    #[should_panic(expected = "assertion failed: self.top >= self.base")]
    fn alloc_stack_panic_underflow() {
        let mut stack = AllocStack::new(1);
        stack.push(1);
        stack.pop();
        assert!(stack.is_empty());
        stack.pop();
    }

    #[test]
    #[should_panic(expected = "assertion failed: self.top < self.base.offset(self.size)")]
    fn alloc_stack_overflow() {
        let mut stack = AllocStack::new(1);
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
}
