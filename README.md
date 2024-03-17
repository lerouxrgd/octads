# Octads

Rust **no_std** and **generic** implementations of [Advanced Data Structures][ads] by
[Peter Brass][brass].

The original [C implementations][dstest] are provided by the author. The [Rust
implementations][rdoc] are documented on GitHub pages.

The implementations use **unsafe** Rust and are tested with [Miri][] on the CI.

## Data Structures

#### Allocation

The [BlockAllocator][] struct provides dynamic allocations through the [Nodable][]
trait.

Nodable structs: [Node][], [BiNode][], [TreeNode][]

#### Stacks

Stack implementations: [ArrayStack][], [BoundedStack][], [UnboundedStack][],
[LinkedListStack][], [ShadowCopyStack][]

#### Queues

Queue implementations: [BoundedQueue][], [LinkedListQueue][], [CircularLinkedQueue][],
[DoubleLinkedQueue][]

#### Trees

Tree implementations: [SearchTree][]

[ads]: https://www.cambridge.org/core/books/advanced-data-structures/D56E2269D7CEE969A3B8105AD5B9254C
[brass]: http://www-cs.engr.ccny.cuny.edu/~peter/
[dstest]: http://www-cs.engr.ccny.cuny.edu/~peter/dstest.html

[ord]: https://doc.rust-lang.org/std/cmp/trait.Ord.html
[clone]: https://doc.rust-lang.org/std/clone/trait.Clone.html
[miri]: https://github.com/rust-lang/miri

[rdoc]: https://lerouxrgd.github.io/octads/

[blockallocator]: https://lerouxrgd.github.io/octads/octads/allocator/struct.BlockAllocator.html
[nodable]: https://lerouxrgd.github.io/octads/octads/allocator/trait.Nodable.html
[node]: https://lerouxrgd.github.io/octads/octads/allocator/struct.Node.html
[binode]: https://lerouxrgd.github.io/octads/octads/allocator/struct.BiNode.html
[treenode]: https://lerouxrgd.github.io/octads/octads/trees/struct.TreeNode.html

[arraystack]: https://lerouxrgd.github.io/octads/octads/stacks/struct.ArrayStack.html
[boundedstack]: https://lerouxrgd.github.io/octads/octads/stacks/struct.BoundedStack.html
[unboundedstack]: https://lerouxrgd.github.io/octads/octads/stacks/struct.UnboundedStack.html
[linkedliststack]: https://lerouxrgd.github.io/octads/octads/stacks/struct.LinkedListStack.html
[shadowcopystack]: https://lerouxrgd.github.io/octads/octads/stacks/struct.ShadowCopyStack.html

[boundedqueue]: https://lerouxrgd.github.io/octads/octads/queues/struct.BoundedQueue.html
[linkedlistqueue]: https://lerouxrgd.github.io/octads/octads/queues/struct.LinkedListQueue.html
[circularlinkedqueue]: https://lerouxrgd.github.io/octads/octads/queues/struct.CircularLinkedQueue.html
[doublelinkedqueue]: https://lerouxrgd.github.io/octads/octads/queues/struct.DoubleLinkedQueue.html

[searchtree]: https://lerouxrgd.github.io/octads/octads/trees/search_tree/struct.SearchTree.html