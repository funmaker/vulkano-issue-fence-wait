// `FenceSignalFuture::wait(Some(timeout))` marks the future as `Cleaned` *before* waiting on the
// fence. When the wait times out, the `?` returns early and the inner future (here a
// `CommandBufferExecFuture`) is dropped without `signal_finished()` having been called. Its `Drop`
// impl then calls `flush()`, whose `submitted` flag is still `false` (the real submission went
// through the parent's `build_submission`), so the command buffer is submitted a second time.

use std::{sync::Arc, time::{Duration, Instant}};
use vulkano::{
	buffer::{Buffer, BufferCreateInfo, BufferUsage},
	command_buffer::{allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, CommandBufferUsage},
	device::{Device, DeviceCreateInfo, QueueCreateInfo, QueueFlags},
	instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
	memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
	sync::{self, GpuFuture},
	VulkanLibrary,
};

// Enough GPU work that the fence is still unsignaled right after submission.
const BUFFER_LEN: u64 = 64 * 1024 * 1024; // 64Mi u32 = 256 MiB
const FILLS: u32 = 64;

fn main() {
	let library = unsafe { VulkanLibrary::new() }.unwrap();
	let instance = Instance::new(&library, &InstanceCreateInfo {
		flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
		..Default::default()
	}).unwrap();

	let (physical_device, queue_family_index) = instance.enumerate_physical_devices()
	                                                    .unwrap()
	                                                    .into_iter()
	                                                    .find_map(|p| {
		                                                    p.queue_family_properties()
		                                                     .iter()
		                                                     .position(|q| q.queue_flags.intersects(QueueFlags::TRANSFER | QueueFlags::GRAPHICS | QueueFlags::COMPUTE))
		                                                     .map(|i| (p, i as u32))
	                                                    })
	                                                    .unwrap();

	println!("Using device: {}", physical_device.properties().device_name);

	let (device, queues) = Device::new(&physical_device, &DeviceCreateInfo {
		queue_create_infos: &[QueueCreateInfo { queue_family_index, ..Default::default() }],
		..Default::default()
	}).unwrap();
	let [queue] = queues.try_into().unwrap();

	let memory_allocator = Arc::new(StandardMemoryAllocator::new(&device, &Default::default()));
	let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(&device, &Default::default()));

	let buffer = Buffer::new_slice::<u32>(&memory_allocator,
	                                      &BufferCreateInfo { usage: BufferUsage::TRANSFER_DST, ..Default::default() },
	                                      &AllocationCreateInfo { memory_type_filter: MemoryTypeFilter::PREFER_DEVICE, ..Default::default() },
	                                      BUFFER_LEN).unwrap();

	let mut builder = AutoCommandBufferBuilder::primary(command_buffer_allocator, queue_family_index, CommandBufferUsage::OneTimeSubmit).unwrap();
	for i in 0..FILLS {
		builder.fill_buffer(buffer.clone(), i).unwrap();
	}
	let command_buffer = builder.build().unwrap();

	let future = sync::now(device.clone())
		.then_execute(queue.clone(), command_buffer)
		.unwrap()
		.then_signal_fence_and_flush()
		.unwrap();

	println!("submitted, is_signaled = {:?}", future.is_signaled());

	// Poll the fence without blocking. Expected: `Err(Timeout)` and the future stays usable.
	let start = Instant::now();
	let result = future.wait(Some(Duration::ZERO)); // panics in here
	println!("wait(Some(0)) = {result:?}, took {:?}", start.elapsed());

	// Expected: false while the GPU is busy. Actual: the future thinks it is `Cleaned`.
	println!("is_signaled = {:?}", future.is_signaled());

	future.wait(None).unwrap();
	println!("Done");
}
