use std::mem::{self, MaybeUninit};

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

    pub fn capacity(&self) -> usize {
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
}

pub struct VecStack<T> {
    stack: Vec<T>,
}

impl<T> Default for VecStack<T> {
    fn default() -> Self {
        Self { stack: vec![] }
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
        assert_eq!(1, stack.pop());

        stack.pop();
        stack.pop();
        assert!(stack.is_empty());

        let range = 4..=9;
        for i in range.clone() {
            stack.push(i);
        }
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
    fn vec_stack_ok() {
        let mut stack = VecStack::default();
        stack.push(3);
        stack.push(2);
        stack.push(1);
        assert_eq!(1, stack.pop());

        stack.pop();
        stack.pop();
        assert!(stack.is_empty());

        let range = 4..=9;
        for i in range.clone() {
            stack.push(i);
        }
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
