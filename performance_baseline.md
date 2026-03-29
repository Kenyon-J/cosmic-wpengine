# Performance Baseline - MPRIS Module

## Identified Inefficiency
The current MPRIS implementation in `src/modules/mpris.rs` uses long-lived blocking loops that occupy dedicated threads indefinitely.

### 1. Position Polling Loop (Line 79)
- **Mechanism:** `tokio::task::spawn_blocking(move || { loop { ... std::thread::sleep(...) } })`
- **Impact:** Occupies one thread from the Tokio blocking thread pool for the entire lifetime of the application. The blocking thread pool has a limited size, and although it can grow, keeping a thread occupied for a simple periodic poll is inefficient.
- **Resource Usage:** 1 blocking pool thread.

### 2. Main MPRIS Event Loop (Line 106)
- **Mechanism:** `std::thread::spawn(move || { loop { ... } })`
- **Impact:** Occupies one dedicated OS thread for the entire lifetime of the application. This thread spends most of its time either sleeping (`std::thread::sleep`) or waiting for MPRIS events in a blocking manner.
- **Resource Usage:** 1 dedicated OS thread.

## Total Baseline Resource Consumption
- 1 Blocking Pool Thread (indefinite)
- 1 Dedicated OS Thread (indefinite)
- CPU wakeups every 1-3 seconds via `std::thread::sleep`.

## Optimization Goal
- Replace long-lived blocking loops with async loops using `tokio::spawn`.
- Use `tokio::time::sleep` instead of `std::thread::sleep` to allow the thread to be returned to the executor during wait periods.
- Confine blocking MPRIS library calls to short-lived `spawn_blocking` tasks.
- Reduce the application's thread footprint by 2 threads.
