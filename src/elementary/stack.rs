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
}
