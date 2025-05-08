use std::num::NonZero;

use image::{ImageBuffer, Rgba};
use wgpu::{
    wgc::instance, BindGroupLayoutDescriptor, BindGroupLayoutEntry, Buffer, ComputePipeline, ComputePipelineDescriptor, Device, Instance, InstanceDescriptor, PipelineCompilationOptions, PipelineLayoutDescriptor, Queue, RenderPipeline, RequestAdapterOptionsBase, Texture, TextureView
};

fn main() {
    pollster::block_on(async {
        let gpu = Gpu::new().await;
        gpu.run().await;
    });
}

struct Gpu {
    instance: Instance,
    device: Device,
    queue: Queue,
    pipeline: ComputePipeline,
    tex: Texture,
    tex_view: TextureView,
    output_buffer: Buffer,
}

impl Gpu {
    pub async fn new() -> Self {
        let instance = Instance::new(&InstanceDescriptor::default());
    
        let adapter = instance
            .request_adapter(&RequestAdapterOptionsBase::default())
            .await
            .unwrap();
    
        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
            ..Default::default()
        }).await.unwrap();
    
        let texture_size = 256u32;
        let buffer_size = (texture_size * texture_size * std::mem::size_of::<u32>() as u32) as wgpu::BufferAddress;
    
        // Create storage buffer
        let storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Storage Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    
        // Output buffer for readback
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            label: Some("Output Buffer"),
            mapped_at_creation: false,
        });
    
        // Shader
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
    
        // Create bind group layout for storage buffer
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Storage Buffer Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
    
        // Bind group layout -> pipeline layout
        let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
    
        // Create compute pipeline
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            cache: None,
            compilation_options: PipelineCompilationOptions::default(),
            entry_point: Some("compute_main"),
            label: Some("Compute Pipeline"),
            layout: Some(&layout),
            module: &shader,
        });
    
        // You don't need tex and tex_view if you're not using a render pipeline or sampling a texture
        let dummy_tex = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_SRC,
            label: None,
            view_formats: &[],
        });
        let dummy_tex_view = dummy_tex.create_view(&Default::default());
    
        Self {
            instance,
            device,
            queue,
            pipeline,
            tex: dummy_tex,
            tex_view: dummy_tex_view,
            output_buffer,
        }
    }
    

    pub async fn run(&self) {
        let texture_size = 256;
        let buffer_size = (texture_size * texture_size * std::mem::size_of::<u32>()) as u64;

        // Create a buffer for the compute shader to write to
        let storage_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Storage Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = self.pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: storage_buffer.as_entire_binding(),
            }],
            label: None,
        });

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Compute Encoder"),
        });

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(32, 32, 1); // 256 / 8 = 32
        }

        // Copy to output buffer
        encoder.copy_buffer_to_buffer(
            &storage_buffer,
            0,
            &self.output_buffer,
            0,
            buffer_size,
        );

        self.queue.submit(Some(encoder.finish()));

        // Map and read buffer
        let buffer_slice = self.output_buffer.slice(..);
        let (tx, rx) = futures_intrusive::channel::shared::oneshot_channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
        self.device.poll(wgpu::PollType::Wait).unwrap();
        rx.receive().await.unwrap().unwrap();
        
        {
            let data = buffer_slice.get_mapped_range();

            let image: ImageBuffer<Rgba<u8>, _> =
                ImageBuffer::from_raw(texture_size as _, texture_size as _, data.to_vec()).unwrap();

            image.save("gradient.png").unwrap();
            println!("Saved gradient.png");
        }
        self.output_buffer.unmap();
    }
}
