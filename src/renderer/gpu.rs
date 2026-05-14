use wgpu::InstanceDescriptor;

pub struct Gpu {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub surface_format: wgpu::TextureFormat,
}

impl Gpu {
    // Calculate the aspect ratio based on the current surface configuration
    pub fn aspect_ratio(&self) -> f32 {
        self.surface_config.width as f32 / self.surface_config.height.max(1) as f32
    }

    // Resize the surface configuration and reconfigure the surface when the window size changes
    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    // Create a depth texture for use in rendering, with the specified width and height
    pub fn create_depth_texture(&self, width: u32, height: u32) -> wgpu::TextureView {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(wgpu::TextureFormat::Depth32Float),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
            usage: None,
        })
    }

    // Asynchronously create a new Gpu instance with a surface for rendering, given a window and its dimensions
    pub async fn new_async(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Self {
        let instance =
            wgpu::Instance::new(InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to request adapter!");

        let (device, queue) = {
            log::info!("WGPU Adapter Features: {:#?}", adapter.features());
            adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("WGPU Device"),
                    memory_hints: wgpu::MemoryHints::default(),
                    required_features: wgpu::Features::default(),
                    required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                    trace: wgpu::Trace::Off,
                })
                .await
                .expect("Failed to request a device!")
        };

        let surface_capabilities = surface.get_capabilities(&adapter);

        let surface_format = surface_capabilities
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(surface_capabilities.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: surface_capabilities.present_modes[0],
            alpha_mode: surface_capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_config);

        Self {
            surface,
            device,
            queue,
            surface_config,
            surface_format,
        }
    }
    
    // Asynchronously create a new Gpu instance without a surface, for headless rendering or compute tasks
    pub async fn new_async_headless() -> Self {
        let instance =
            wgpu::Instance::new(InstanceDescriptor::new_without_display_handle_from_env());

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to request adapter!");

        let (device, queue) = {
            log::info!("WGPU Adapter Features: {:#?}", adapter.features());
            adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("WGPU Device"),
                    memory_hints: wgpu::MemoryHints::default(),
                    required_features: wgpu::Features::default(),
                    required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                    trace: wgpu::Trace::Off,
                })
                .await
                .expect("Failed to request a device!")
        };

        let surface_format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: 1,
            height: 1,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let dummy_surface = unsafe {
            let handle =
                wgpu::rwh::RawDisplayHandle::Windows(wgpu::rwh::WindowsDisplayHandle::new());
            let window_handle = wgpu::rwh::RawWindowHandle::Win32(
                wgpu::rwh::Win32WindowHandle::new(std::num::NonZeroIsize::new(1).unwrap()),
            );
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(handle),
                    raw_window_handle: window_handle,
                })
                .unwrap()
        };

        Self {
            surface: dummy_surface,
            device,
            queue,
            surface_config,
            surface_format,
        }
    }
}
