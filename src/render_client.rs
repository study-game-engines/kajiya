use crate::{
    asset::mesh::{PackedTriangleMesh, PackedVertex},
    backend::{self, image::*, shader::*, RenderBackend},
    dynamic_constants::DynamicConstants,
    render_passes::{RasterMeshesData, UploadedTriMesh},
    renderer::*,
    rg,
    rg::RetiredRenderGraph,
    viewport::ViewConstants,
    FrameState,
};
use backend::buffer::{Buffer, BufferDesc};
use byte_slice_cast::AsByteSlice;
use glam::Vec2;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use slingshot::{ash::vk, backend::device, vk_sync};
use std::{mem::size_of, sync::Arc};
use winit::VirtualKeyCode;

#[repr(C)]
#[derive(Copy, Clone)]
struct FrameConstants {
    view_constants: ViewConstants,
    mouse: [f32; 4],
    frame_idx: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct GpuMesh {
    vertex_core_offset: u32,
    vertex_aux_offset: u32,
}

const MAX_GPU_MESHES: usize = 1024;
const VERTEX_BUFFER_CAPACITY: usize = 1024 * 1024 * 128;

pub struct VickiRenderClient {
    device: Arc<device::Device>,
    raster_simple_render_pass: Arc<RenderPass>,
    //sdf_img: TemporalImage,
    //cube_index_buffer: Arc<Buffer>,
    meshes: Vec<UploadedTriMesh>,
    mesh_buffer: Arc<Buffer>,
    vertex_buffer: Arc<Buffer>,
    vertex_buffer_size: usize,
    frame_idx: u32,
}

fn as_byte_slice_unchecked<T: Copy>(v: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * size_of::<T>()) }
}

impl VickiRenderClient {
    pub fn new(backend: &RenderBackend) -> anyhow::Result<Self> {
        /*let cube_indices = cube_indices();
        let cube_index_buffer = backend.device.create_buffer(
            BufferDesc {
                size: cube_indices.len() * 4,
                usage: vk::BufferUsageFlags::INDEX_BUFFER,
            },
            Some((&cube_indices).as_byte_slice()),
        )?;*/

        let raster_simple_render_pass = create_render_pass(
            &*backend.device,
            RenderPassDesc {
                color_attachments: &[RenderPassAttachmentDesc::new(
                    vk::Format::R16G16B16A16_SFLOAT,
                )
                .garbage_input()],
                depth_attachment: Some(RenderPassAttachmentDesc::new(
                    vk::Format::D24_UNORM_S8_UINT,
                )),
            },
        )?;

        let mesh_buffer = Arc::new(
            backend
                .device
                .create_buffer(
                    BufferDesc {
                        size: MAX_GPU_MESHES * size_of::<GpuMesh>(),
                        usage: vk::BufferUsageFlags::STORAGE_BUFFER,
                        mapped: true,
                    },
                    None,
                )
                .unwrap(),
        );

        let vertex_buffer = Arc::new(
            backend
                .device
                .create_buffer(
                    BufferDesc {
                        size: VERTEX_BUFFER_CAPACITY,
                        usage: vk::BufferUsageFlags::STORAGE_BUFFER,
                        mapped: true,
                    },
                    None,
                )
                .unwrap(),
        );

        Ok(Self {
            raster_simple_render_pass,

            //sdf_img: TemporalImage::new(Arc::new(sdf_img)),
            //cube_index_buffer: Arc::new(cube_index_buffer),
            device: backend.device.clone(),
            meshes: Default::default(),
            mesh_buffer,
            vertex_buffer,
            vertex_buffer_size: 0,
            frame_idx: 0u32,
        })
    }

    pub fn add_mesh(&mut self, mesh: PackedTriangleMesh) {
        let mesh_idx = self.meshes.len();

        let index_buffer = Arc::new(
            self.device
                .create_buffer(
                    BufferDesc {
                        size: mesh.indices.len() * 4,
                        usage: vk::BufferUsageFlags::INDEX_BUFFER,
                        mapped: false,
                    },
                    Some((&mesh.indices).as_byte_slice()),
                )
                .unwrap(),
        );

        let vertex_core_offset;
        let vertex_aux_offset;

        unsafe {
            let vertex_buffer_dst = self.vertex_buffer.allocation_info.get_mapped_data();

            {
                vertex_core_offset = self.vertex_buffer_size as _;
                let dst = std::slice::from_raw_parts_mut(
                    vertex_buffer_dst.add(self.vertex_buffer_size) as *mut PackedVertex,
                    mesh.verts.len(),
                );
                dst.copy_from_slice(&mesh.verts);
                self.vertex_buffer_size += mesh.verts.len() * size_of::<PackedVertex>();
            }

            {
                vertex_aux_offset = self.vertex_buffer_size as _;
                let dst = std::slice::from_raw_parts_mut(
                    vertex_buffer_dst.add(self.vertex_buffer_size) as *mut [f32; 4],
                    mesh.colors.len(),
                );
                dst.copy_from_slice(&mesh.colors);
                self.vertex_buffer_size += mesh.colors.len() * size_of::<[f32; 4]>();
            }
        }

        let mesh_buffer_dst = unsafe {
            let mesh_buffer_dst =
                self.mesh_buffer.allocation_info.get_mapped_data() as *mut GpuMesh;
            assert!(!mesh_buffer_dst.is_null());
            std::slice::from_raw_parts_mut(mesh_buffer_dst, MAX_GPU_MESHES)
        };

        mesh_buffer_dst[mesh_idx] = GpuMesh {
            vertex_core_offset,
            vertex_aux_offset,
        };

        self.meshes.push(UploadedTriMesh {
            index_buffer,
            index_count: mesh.indices.len() as _,
        });
    }
}

impl RenderClient<FrameState> for VickiRenderClient {
    fn prepare_render_graph(
        &mut self,
        rg: &mut crate::rg::RenderGraph,
        frame_state: &FrameState,
    ) -> rg::ExportedHandle<Image> {
        /*let mut sdf_img = rg.import_image(self.sdf_img.resource.clone(), self.sdf_img.access_type);
        let cube_index_buffer = rg.import_buffer(
            self.cube_index_buffer.clone(),
            vk_sync::AccessType::TransferWrite,
        );*/

        let mut depth_img = crate::render_passes::create_image(
            rg,
            ImageDesc::new_2d(vk::Format::D24_UNORM_S8_UINT, frame_state.window_cfg.dims()),
        );
        crate::render_passes::clear_depth(rg, &mut depth_img);
        /*crate::render_passes::edit_sdf(rg, &mut sdf_img, self.frame_idx == 0);

        let sdf_raster_bricks: SdfRasterBricks =
            crate::render_passes::calculate_sdf_bricks_meta(rg, &sdf_img);*/
        /*let mut tex = crate::render_passes::raymarch_sdf(
            rg,
            &sdf_img,
            ImageDesc::new_2d(
                vk::Format::R16G16B16A16_SFLOAT,
                frame_state.window_cfg.dims(),
            ),
        );*/

        let mut tex = crate::render_passes::create_image(
            rg,
            ImageDesc::new_2d(
                vk::Format::R16G16B16A16_SFLOAT,
                frame_state.window_cfg.dims(),
            ),
        );
        crate::render_passes::clear_color(rg, &mut tex, [0.1, 0.2, 0.5, 1.0]);

        let mesh_buffer = rg.import_buffer(
            self.mesh_buffer.clone(),
            vk_sync::AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer,
        );

        let vertex_buffer = rg.import_buffer(
            self.vertex_buffer.clone(),
            vk_sync::AccessType::AnyShaderReadSampledImageOrUniformTexelBuffer,
        );

        crate::render_passes::raster_meshes(
            rg,
            self.raster_simple_render_pass.clone(),
            &mut depth_img,
            &mut tex,
            RasterMeshesData {
                meshes: self.meshes.as_slice(),
                mesh_buffer: &mesh_buffer,
                vertex_buffer: &vertex_buffer,
            },
        );

        /*crate::render_passes::raster_sdf(
            rg,
            self.raster_simple_render_pass.clone(),
            &mut depth_img,
            &mut tex,
            crate::render_passes::RasterSdfData {
                sdf_img: &sdf_img,
                brick_inst_buffer: &sdf_raster_bricks.brick_inst_buffer,
                brick_meta_buffer: &sdf_raster_bricks.brick_meta_buffer,
                cube_index_buffer: &cube_index_buffer,
            },
        );*/

        //let tex = crate::render_passes::blur(rg, &tex);
        //self.sdf_img.last_rg_handle = Some(rg.export_image(sdf_img, vk::ImageUsageFlags::empty()));

        rg.export_image(tex, vk::ImageUsageFlags::SAMPLED)
    }

    fn prepare_frame_constants(
        &mut self,
        dynamic_constants: &mut DynamicConstants,
        frame_state: &FrameState,
    ) {
        let width = frame_state.window_cfg.width;
        let height = frame_state.window_cfg.height;

        dynamic_constants.push(FrameConstants {
            view_constants: ViewConstants::builder(frame_state.camera_matrices, width, height)
                .build(),
            mouse: gen_shader_mouse_state(&frame_state),
            frame_idx: self.frame_idx,
        });
    }

    fn retire_render_graph(&mut self, retired_rg: &RetiredRenderGraph) {
        /*if let Some(handle) = self.sdf_img.last_rg_handle.take() {
            self.sdf_img.access_type = retired_rg.get_image(handle).1;
        }*/

        self.frame_idx = self.frame_idx.overflowing_add(1).0;
    }
}

// Vertices: bits 0, 1, 2, map to +/- X, Y, Z
fn cube_indices() -> Vec<u32> {
    let mut res = Vec::with_capacity(6 * 2 * 3);

    for (ndim, dim0, dim1) in [(1, 2, 4), (2, 4, 1), (4, 1, 2)].iter().copied() {
        for (nbit, dim0, dim1) in [(0, dim1, dim0), (ndim, dim0, dim1)].iter().copied() {
            res.push(nbit);
            res.push(nbit + dim0);
            res.push(nbit + dim1);

            res.push(nbit + dim1);
            res.push(nbit + dim0);
            res.push(nbit + dim0 + dim1);
        }
    }

    res
}

fn gen_shader_mouse_state(frame_state: &FrameState) -> [f32; 4] {
    let pos = frame_state.input.mouse.pos
        / Vec2::new(
            frame_state.window_cfg.width as f32,
            frame_state.window_cfg.height as f32,
        );

    [
        pos.x(),
        pos.y(),
        if (frame_state.input.mouse.button_mask & 1) != 0 {
            1.0
        } else {
            0.0
        },
        if frame_state.input.keys.is_down(VirtualKeyCode::LShift) {
            -1.0
        } else {
            1.0
        },
    ]
}

struct TemporalImage {
    resource: Arc<Image>,
    access_type: vk_sync::AccessType,
    last_rg_handle: Option<rg::ExportedHandle<Image>>,
}

impl TemporalImage {
    pub fn new(resource: Arc<Image>) -> Self {
        Self {
            resource,
            access_type: vk_sync::AccessType::Nothing,
            last_rg_handle: None,
        }
    }
}
