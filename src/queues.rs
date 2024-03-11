use alloc::alloc::{alloc, dealloc, handle_alloc_error, Layout};
use core::{mem::MaybeUninit, ptr};

use crate::allocator::{BiNode, BlockAllocator, Node};

#[derive(Debug)]
pub struct BoundedQueue<T> {
    base: *mut T,
    front: usize,
    rear: usize,
    max_size: usize,
    len: usize,
}

impl<T> BoundedQueue<T> {
    pub fn new(max_size: usize) -> Self {
        let layout = Layout::array::<T>(max_size).expect("Couldn't create memory layout");
        let base = unsafe { alloc(layout) };
        if base.is_null() {
            handle_alloc_error(layout);
        }
        let base = base as *mut _;

        Self {
            base,
            front: 0,
            rear: 0,
            max_size,
            len: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn max_len(&self) -> usize {
        self.max_size
    }

    pub fn enqueue(&mut self, val: T) {
        assert!(
            self.len < self.max_size,
            "overflow: enqueuing to a full queue"
        );
        unsafe { ptr::write(self.base.add(self.rear), val) };
        self.rear = (self.rear + 1) % self.max_size;
        self.len += 1;
    }

    pub fn dequeue(&mut self) -> T {
        assert!(!self.is_empty(), "underflow: dequeuing from an empty queue");
        let tmp = self.front;
        self.front = (self.front + 1) % self.max_size;
        self.len -= 1;
        unsafe { ptr::read(self.base.add(tmp)) }
    }

    pub fn peek(&self) -> &T {
        assert!(!self.is_empty(), "underflow: peeking at an empty queue");
        unsafe {
            let peek = self.base.add(self.front);
            &*peek
        }
    }
}

impl<T> Drop for BoundedQueue<T> {
    fn drop(&mut self) {
        while !self.is_empty() {
            self.dequeue();
        }
        let layout = Layout::array::<T>(self.max_size).unwrap();
        unsafe { dealloc(self.base as *mut u8, layout) };
    }
}

#[derive(Debug)]
pub struct LinkedListQueue<T> {
    allocator: BlockAllocator<Node<T>>,
    len: usize,
    remove: *mut Node<T>,
    insert: *mut Node<T>,
}

impl<T> Default for LinkedListQueue<T> {
    fn default() -> Self {
        Self::new(
            BlockAllocator::<Node<T>>::DEFAULT_BLOCK_SIZE,
            BlockAllocator::<Node<T>>::DEFAULT_BLOCK_CAP,
        )
    }
}

impl<T> LinkedListQueue<T> {
    pub fn new(block_size: usize, blocks_cap: usize) -> Self {
        Self {
            allocator: BlockAllocator::new(block_size, blocks_cap),
            len: 0,
            remove: ptr::null_mut(),
            insert: ptr::null_mut(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn enqueue(&mut self, val: T) {
        let tmp = self.allocator.get_node();
        unsafe { (*tmp).val = MaybeUninit::new(val) };
        if !self.is_empty() {
            unsafe { (*self.insert).next = tmp };
            self.insert = tmp;
        } else {
            self.remove = tmp;
            self.insert = tmp;
        }
        self.len += 1;
    }

    pub fn dequeue(&mut self) -> T {
        assert!(!self.is_empty(), "underflow: dequeuing from an empty queue");
        let tmp = self.remove;
        unsafe {
            self.remove = (*tmp).next;
            let val = (*tmp).val.assume_init_read();
            self.allocator.return_node(tmp);
            self.len -= 1;
            val
        }
    }

    pub fn peek(&self) -> &T {
        assert!(!self.is_empty(), "underflow: peeking at an empty queue");
        unsafe { (*self.remove).val.assume_init_ref() }
    }
}

impl<T> Drop for LinkedListQueue<T> {
    fn drop(&mut self) {
        let mut next = self.remove;
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
pub struct CircularLinkedQueue<T> {
    allocator: BlockAllocator<Node<T>>,
    len: usize,
    entry: *mut Node<T>,
}

impl<T> Default for CircularLinkedQueue<T> {
    fn default() -> Self {
        Self::new(
            BlockAllocator::<Node<T>>::DEFAULT_BLOCK_SIZE,
            BlockAllocator::<Node<T>>::DEFAULT_BLOCK_CAP,
        )
    }
}

impl<T> CircularLinkedQueue<T> {
    pub fn new(block_size: usize, blocks_cap: usize) -> Self {
        let mut allocator = BlockAllocator::new(block_size, blocks_cap);
        let entry: *mut Node<_> = allocator.get_node();
        unsafe { (*entry).next = entry };
        Self {
            allocator,
            len: 0,
            entry,
        }
    }

    pub fn is_empty(&self) -> bool {
        unsafe { self.entry == (*self.entry).next }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn enqueue(&mut self, val: T) {
        let node = self.allocator.get_node();
        unsafe {
            (*node).val = MaybeUninit::new(val);
            let tmp = self.entry;
            self.entry = node;
            (*node).next = (*tmp).next;
            (*tmp).next = node;
        }
        self.len += 1;
    }

    pub fn dequeue(&mut self) -> T {
        assert!(!self.is_empty(), "underflow: dequeuing from an empty queue");
        unsafe {
            let tmp = (*(*self.entry).next).next;
            (*(*self.entry).next).next = (*tmp).next;
            if tmp == self.entry {
                self.entry = (*tmp).next;
            }
            let val = (*tmp).val.assume_init_read();
            self.allocator.return_node(tmp);
            self.len -= 1;
            val
        }
    }

    pub fn peek(&self) -> &T {
        assert!(!self.is_empty(), "underflow: peeking at an empty queue");
        unsafe { (*(*(*self.entry).next).next).val.assume_init_ref() }
    }
}

impl<T> Drop for CircularLinkedQueue<T> {
    fn drop(&mut self) {
        while !self.is_empty() {
            self.dequeue();
        }
        unsafe { self.allocator.return_node(self.entry) }
    }
}

#[derive(Debug)]
pub struct DoubleLinkedQueue<T> {
    allocator: BlockAllocator<BiNode<T>>,
    len: usize,
    entry: *mut BiNode<T>,
}

impl<T> Default for DoubleLinkedQueue<T> {
    fn default() -> Self {
        Self::new(
            BlockAllocator::<Node<T>>::DEFAULT_BLOCK_SIZE,
            BlockAllocator::<Node<T>>::DEFAULT_BLOCK_CAP,
        )
    }
}

impl<T> DoubleLinkedQueue<T> {
    pub fn new(block_size: usize, blocks_cap: usize) -> Self {
        let mut allocator = BlockAllocator::new(block_size, blocks_cap);
        let entry: *mut BiNode<_> = allocator.get_node();
        unsafe { (*entry).next = entry };
        unsafe { (*entry).prev = entry };
        Self {
            allocator,
            len: 0,
            entry,
        }
    }

    pub fn is_empty(&self) -> bool {
        unsafe { self.entry == (*self.entry).next }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn enqueue(&mut self, val: T) {
        let node = self.allocator.get_node();
        unsafe {
            (*node).val = MaybeUninit::new(val);
            (*node).next = (*self.entry).next;
            (*self.entry).next = node;
            (*(*node).next).prev = node;
            (*node).prev = self.entry;
        }
        self.len += 1;
    }

    pub fn dequeue(&mut self) -> T {
        assert!(!self.is_empty(), "underflow: dequeuing from an empty queue");
        unsafe {
            let tmp = (*self.entry).prev;
            let val = (*tmp).val.assume_init_read();
            (*(*tmp).prev).next = self.entry;
            (*self.entry).prev = (*tmp).prev;
            self.allocator.return_node(tmp);
            self.len -= 1;
            val
        }
    }

    pub fn peek(&self) -> &T {
        assert!(!self.is_empty(), "underflow: peeking at an empty queue");
        unsafe { (*(*self.entry).prev).val.assume_init_ref() }
    }
}

impl<T> Drop for DoubleLinkedQueue<T> {
    fn drop(&mut self) {
        while !self.is_empty() {
            self.dequeue();
        }
        unsafe { self.allocator.return_node(self.entry) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_queue_ok() {
        let mut q = BoundedQueue::new(6);
        q.enqueue(3);
        q.enqueue(2);
        q.enqueue(1);
        assert_eq!(&3, q.peek());
        assert_eq!(3, q.len());
        assert_eq!(3, q.dequeue());

        q.dequeue();
        q.dequeue();
        assert!(q.is_empty());

        let range = 4..=9;
        for (j, i) in range.clone().enumerate() {
            assert_eq!(j, q.len());
            q.enqueue(i);
        }
        assert_eq!(range.clone().count(), q.len());
        for i in range {
            assert_eq!(i, q.dequeue());
        }
        assert!(q.is_empty());
    }

    #[test]
    #[should_panic(expected = "underflow: dequeuing from an empty queue")]
    fn bounded_queue_underflow() {
        let mut q = BoundedQueue::new(1);
        q.enqueue(1);
        q.dequeue();
        assert!(q.is_empty());
        q.dequeue();
    }

    #[test]
    #[should_panic(expected = "overflow: enqueuing to a full queue")]
    fn bounded_queue_overflow() {
        let mut q = BoundedQueue::new(1);
        q.enqueue(1);
        q.enqueue(2);
    }

    #[test]
    fn linked_list_queue_ok() {
        let mut q = LinkedListQueue::new(2, 1);
        q.enqueue(3);
        q.enqueue(2);
        q.enqueue(1);
        assert_eq!(&3, q.peek());
        assert_eq!(3, q.len());
        assert_eq!(3, q.dequeue());

        q.dequeue();
        q.dequeue();
        assert!(q.is_empty());

        let range = 4..=9;
        for (j, i) in range.clone().enumerate() {
            assert_eq!(j, q.len());
            q.enqueue(i);
        }
        assert_eq!(range.clone().count(), q.len());
        for i in range {
            assert_eq!(i, q.dequeue());
        }
        assert!(q.is_empty());
    }

    #[test]
    #[should_panic(expected = "underflow: dequeuing from an empty queue")]
    fn linked_list_queue_underflow() {
        let mut q = LinkedListQueue::new(4, 2);
        q.enqueue(1);
        q.dequeue();
        assert!(q.is_empty());
        q.dequeue();
    }

    #[test]
    fn circular_linked_queue_ok() {
        let mut q = CircularLinkedQueue::new(2, 1);
        q.enqueue(3);
        q.enqueue(2);
        q.enqueue(1);
        assert_eq!(&3, q.peek());
        assert_eq!(3, q.len());
        assert_eq!(3, q.dequeue());

        q.dequeue();
        q.dequeue();
        assert!(q.is_empty());

        let range = 4..=9;
        for (j, i) in range.clone().enumerate() {
            assert_eq!(j, q.len());
            q.enqueue(i);
        }
        assert_eq!(range.clone().count(), q.len());
        for i in range {
            assert_eq!(i, q.dequeue());
        }
        assert!(q.is_empty());
    }

    #[test]
    #[should_panic(expected = "underflow: dequeuing from an empty queue")]
    fn circular_linked_queue_underflow() {
        let mut q = CircularLinkedQueue::new(4, 2);
        q.enqueue(1);
        q.dequeue();
        assert!(q.is_empty());
        q.dequeue();
    }

    #[test]
    fn double_linked_queue_ok() {
        let mut q = DoubleLinkedQueue::new(2, 1);
        q.enqueue(3);
        q.enqueue(2);
        q.enqueue(1);
        assert_eq!(&3, q.peek());
        assert_eq!(3, q.len());
        assert_eq!(3, q.dequeue());

        q.dequeue();
        q.dequeue();
        assert!(q.is_empty());

        let range = 4..=9;
        for (j, i) in range.clone().enumerate() {
            assert_eq!(j, q.len());
            q.enqueue(i);
        }
        assert_eq!(range.clone().count(), q.len());
        for i in range {
            assert_eq!(i, q.dequeue());
        }
        assert!(q.is_empty());
    }

    #[test]
    #[should_panic(expected = "underflow: dequeuing from an empty queue")]
    fn double_linked_queue_underflow() {
        let mut q = DoubleLinkedQueue::new(4, 2);
        q.enqueue(1);
        q.dequeue();
        assert!(q.is_empty());
        q.dequeue();
    }
}
