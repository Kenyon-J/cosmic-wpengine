## 2024-05-18 - Async Action Feedback
**Learning:** Users lack visual indication of ongoing background processes (like checking for updates), which can make the UI feel unresponsive or broken. Adding standard system spinners (like `process-working-symbolic`) to loading states provides essential feedback without requiring custom animations.
**Action:** Always include a visual loading indicator (e.g., `process-working-symbolic` spinner) for UI states that depend on asynchronous network requests or long-running tasks.
