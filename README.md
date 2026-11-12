# vulkano-issue-fence-wait
Vulkano `FenceSignalFuture::wait` with a timeout marks the future as `Cleaned` before waiting, so a timed-out wait drops the inner future, which resubmits its command buffer.

Steps to reproduce:

```shell
$ cargo run
```

Expected: `wait(Some(Duration::ZERO))` returns `Err(Timeout)` and the future stays pending.

Actual: panics inside `wait()`:

```
a command buffer, or one of the secondary command buffers it executes, was created with the `CommandBufferUsage::OneTimeSubmit` usage, but it has already been submitted in the past

Vulkan VUIDs:
    VUID-vkQueueSubmit2-commandBuffer-03874
```
