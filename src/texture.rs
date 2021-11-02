use super::{renderer::Renderer, sprite::Sprite};
use std::num::NonZeroU32;

pub struct TextureBundle {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub format: wgpu::TextureFormat,
}

pub struct Texture {
    pub data: Sprite,
    pub texture_bundle: Option<TextureBundle>,
}

impl Texture {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            label: None,
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            data: Sprite::new(width, height),
            texture_bundle: Some(TextureBundle {
                texture,
                view,
                format,
            }),
        }
    }
    pub fn new_from_sprite(
        renderer: &Renderer,
        data: Sprite,
        format: wgpu::TextureFormat,
    ) -> Self {
        let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: data.width,
                height: data.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            label: None,
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut tex = Self {
            data,
            texture_bundle: Some(TextureBundle {
                texture,
                view,
                format,
            }),
        };
        tex.update_internal(renderer);
        tex
    }

    pub fn uninitialized(data: Sprite) -> Self {
        Self { data, texture_bundle: None }
    }

    pub fn initialize(&mut self, renderer: &Renderer, format: wgpu::TextureFormat) {
        let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: self.data.width,
                height: self.data.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            label: None,
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.texture_bundle = Some(TextureBundle {
            texture,
            view,
            format,
        });
        self.update_internal(renderer);
    }

    fn update_internal(&mut self, renderer: &Renderer) {
        let size = wgpu::Extent3d {
            width: self.data.width,
            height: self.data.height,
            depth_or_array_layers: 1,
        };
        renderer.queue.write_texture(
            wgpu::ImageCopyTexture {
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                texture: &self.texture_bundle.as_ref().unwrap().texture,
                aspect: wgpu::TextureAspect::All,
            },
            self.data.get_data(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(4 * self.data.width as u32),
                rows_per_image: NonZeroU32::new(0),
            },
            size,
        );
    }

    pub fn update(&mut self, queue: &wgpu::Queue, spr: &Sprite) {
        let size = wgpu::Extent3d {
            width: spr.width,
            height: spr.height,
            depth_or_array_layers: 1,
        };
        queue.write_texture(
            wgpu::ImageCopyTexture {
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                texture: &self.texture_bundle.as_ref().unwrap().texture,
                aspect: wgpu::TextureAspect::All,
            },
            spr.get_data(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(4 * spr.width as u32),
                rows_per_image: NonZeroU32::new(0),
            },
            size,
        );
    }
}
