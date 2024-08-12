# Ore CLI

A command line interface for the Ore program.

## Building

To build the Ore CLI, you will need to have the Rust programming language installed. You can install Rust by following the instructions on the [Rust website](https://www.rust-lang.org/tools/install).

Once you have Rust installed, you can build the Ore CLI by running the following command:

```sh
cargo build --release
```
1. Multithreading and Parallelism:
Implemented Thread Pool: Introduced a thread pool using the threadpool crate to utilize multiple CPU cores for parallel mining.
Dynamic Work Distribution: Each thread in the pool is responsible for processing a portion of the nonce range, ensuring full CPU utilization.
2. Mining Process Timing:
60-Second Mining Window: Ensured that the mining process runs for a full 60 seconds before attempting to submit the best-found hash. This prevents premature submissions and adheres to the required timeframe.
Timing Logic: Used Instant to track the mining duration and correctly stop threads after 60 seconds.
3. Shared State and Synchronization:
Shared Best Result: Implemented a shared RwLock to store and update the best hash, nonce, and difficulty found by the threads.
Thread Synchronization: Used channels (std::sync::mpsc::channel) to coordinate threads and ensure all threads complete their work before submitting the solution.
4. Handling of the Hash Object:
Reference Management: Managed the Hash object using references instead of attempting to clone or move it, which is critical since Hash does not implement the Clone trait.
Efficient Updates: Threads update the best result without causing ownership conflicts or requiring unnecessary data duplication.
5. Error Handling and Debugging:
Resolved Method Usage Errors: Corrected issues related to the use of self in methods, ensuring that they are properly scoped within the impl block.
Debugging Output: Added debugging statements and progress bars to monitor the mining process, helping to ensure that everything is functioning as expected.
Outcome:
Successful Execution: The mining process now runs efficiently across multiple CPU cores, correctly identifies and submits hashes within the required 60-second window, and avoids previous errors related to timing, thread management, and object handling.
Improved Performance: The changes have led to improved CPU utilization and optimized the mining process for better performance.
