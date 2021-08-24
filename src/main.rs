use wgpu::util::DeviceExt;

use std::{convert::TryInto, num::NonZeroU64};
use spirv_std::glam::*;

fn opaque_array_to_bytes<T>(arr: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            (arr.as_ptr()) as *const u8,
            std::mem::size_of::<T>() * arr.len(),
        )
    }
}

fn bytes_to_opaque_array<T>(arr: &[u8]) -> &[T] {
    unsafe {
        std::slice::from_raw_parts(
            (arr.as_ptr()) as *const T,
            arr.len() / std::mem::size_of::<T>(),
        )
    }
}

pub async fn execute_kernel<T: Clone>(shader_binary: wgpu::ShaderModuleDescriptor<'static>, input: Vec<T>) -> Option<Vec<T>> {
    // Create wpgu instance
    let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
        })
        .await
        .expect("Failed to find an appropriate adapter");

    // Use instance to create device and command queue
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::default(),
                limits: wgpu::Limits::default(),
            },
            None,
        )
        .await
        .expect("Failed to create device");
    drop(instance);
    drop(adapter);

    // Load shader
    let module = device.create_shader_module(&shader_binary);
    let src = opaque_array_to_bytes(input.as_slice());

    // Create dummy bind group layout since some GPUs don't support empty bind layout group
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                count: None,
                visibility: wgpu::ShaderStage::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    has_dynamic_offset: false,
                    min_binding_size: Some(NonZeroU64::new(1).unwrap()),
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                },
            },
        ],
    });

    // Create pipeline layout from bind group
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    // Create compute pipeline
    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: "main_cs",
    });

    // Create buffer for GPU -> CPU
    let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: src.len() as wgpu::BufferAddress,
        // Can be read to the CPU, and can be copied from the shader's storage buffer
        usage: wgpu::BufferUsage::MAP_READ | wgpu::BufferUsage::COPY_DST,
        mapped_at_creation: false,
    });

    // Create buffer for CPU -> GPU and storage
    let storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: &src,
        usage: wgpu::BufferUsage::STORAGE
            | wgpu::BufferUsage::COPY_DST
            | wgpu::BufferUsage::COPY_SRC,
    });

    // Create bind group for GPU buffer
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: storage_buffer.as_entire_binding(),
        }],
    });

    // Create encoder for CPU - GPU communcation
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    // Begin compute dispatch
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.set_pipeline(&compute_pipeline);
        cpass.dispatch(input.len() as u32 / 64, 1, 1);
    }

    // CPU readback
    encoder.copy_buffer_to_buffer(
        &storage_buffer, 0,
        &readback_buffer, 0,
        src.len() as wgpu::BufferAddress,
    );

    // Wait for GPU to finish
    queue.submit(Some(encoder.finish()));
    let buffer_slice = readback_buffer.slice(..);
    let buffer_future = buffer_slice.map_async(wgpu::MapMode::Read);
    device.poll(wgpu::Maintain::Wait);

    // Fetch result as u32 vec
    if let Ok(_) = buffer_future.await {
        let data = buffer_slice.get_mapped_range();
        let result = bytes_to_opaque_array(&data).to_owned();
        drop(data);
        readback_buffer.unmap();
        Some(result)
    } else {
        None
    }
}

const KERNEL: &[u8] = include_bytes!(env!("compute.spv"));

fn main() {
    let shader_binary = wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::SpirV(std::borrow::Cow::Owned(
            KERNEL
                .chunks(4)
                .map(|x| u32::from_ne_bytes(x.try_into().unwrap()))
                .collect::<Vec<_>>(),
        )),
        flags: wgpu::ShaderFlags::default(),
    };

    let data = (0..64).map(|x| { 
        let id = x as f32;
        shared::Ray {
            origin: vec4(id * 1.0, id * 2.0, id * 3.0, id * 4.0),
            direction: vec4(id * 4.0, id * 3.0, id * 2.0, id * 1.0)
        }
    }).collect::<Vec<_>>();

    match futures::executor::block_on(execute_kernel(shader_binary, data)) {
        Some(result) => println!("Execution result: {:?}", result),
        None => println!("Error executing kernel")
    }
}
