use crate::texture::Texture;

use super::{game_object::GameObject, geometry::*, math_3d::*, transform::*};


pub fn get_game_objects(data: &[u8]) -> (Vec<GameObject>, Vec<gltf::image::Data>)  {
    let (document, buffers, images) = gltf::import_slice(data).unwrap();
    let mut skip_nodes: Vec<usize> = document.nodes().map(get_children_id).flatten().collect();
    (document.nodes()
     .filter(|node| !skip_nodes.contains(&node.index()))
     .map(|node| process_node(node, &buffers, &images))
     .collect(),
     images)
}

pub fn get_game_objects_from_file(data: &str) -> Vec<GameObject> {
    let (document, buffers, images) = gltf::import(data).unwrap();
    let mut skip_nodes: Vec<usize> = document.nodes().map(get_children_id).flatten().collect();
    document
        .nodes()
        .filter(|node| !skip_nodes.contains(&node.index()))
        .map(|node| process_node(node, &buffers, &images))
        .collect()
}

fn get_children_id(node: gltf::Node) -> Vec<usize> {
    node.children().map(get_children_id).flatten().collect()
}

pub fn process_node(
    node: gltf::Node,
    buffers: &[gltf::buffer::Data],
    images: &[gltf::image::Data],
) -> GameObject {
    let (node, transform, meshes) = extract_node(node, buffers, images);
    let mut out_object = GameObject::new(transform, None, meshes);
    for node in node.children() {
        //DONE: Node Transform, MESHES
        //TODO: CAMERAS, PARENT TRANSFORMS, CHILDREN
        out_object.children.insert(
            out_object.children.len(),
            process_node(node, buffers, images),
        );
    }

    out_object
}

pub fn extract_node<'a>(
    node: gltf::Node<'a>,
    buffers: &[gltf::buffer::Data],
    images: &[gltf::image::Data],
) -> (gltf::Node<'a>, Transform3, Vec<Mesh>) {
    use gltf::scene::Transform;
    let mut out_meshes: Vec<Mesh> = vec![];
    let transform: Transform3 = match node.transform() {
        Transform::Matrix { matrix } => {
            //Transform3::default()
            matrix.into()
        }
        Transform::Decomposed {
            translation,
            rotation,
            scale,
        } => {
            //Transform3::default()
            Transform3 {
                rot: Rotor3::from_quat(rotation),
                scale: scale.into(),
                pos: translation.into(),
            }
        }
    };
    if let Some(mesh) = node.mesh() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let material = primitive.material();
            let get_texture =
                |texture: gltf::texture::Texture| -> Option<(crate::sprite::Sprite, usize)> {
                    let image_index = texture.source().index();
                    let image = &images[image_index];
                    let mut spr = crate::sprite::Sprite::new(image.width, image.height);
                    match image.format {
                        gltf::image::Format::R8 => spr.set_data(&image.pixels, 1),
                        gltf::image::Format::R8G8 => spr.set_data(&image.pixels, 2),
                        gltf::image::Format::R8G8B8 => spr.set_data(&image.pixels, 3),
                        gltf::image::Format::R8G8B8A8 => spr.set_data(&image.pixels, 4),
                        gltf::image::Format::B8G8R8 => spr.set_data(&image.pixels, 3),
                        gltf::image::Format::B8G8R8A8 => spr.set_data(&image.pixels, 4),
                        _ => {}
                    }
                    Some((spr, image_index))
                };
            let mut textures: Vec<PBRTexture> = vec![];
            let mut tex_coords: Vec<UV> = vec![];

            if let Some(tex) = material.emissive_texture()
            {
                textures.insert(textures.len(), PBRTexture::Emissive(tex.texture().source().index()))
            }

            if let Some(tex) = material.normal_texture()
            {
                textures.insert(textures.len(), PBRTexture::Normal(tex.texture().source().index()))
            }

            if let Some(tex) = material.pbr_metallic_roughness().metallic_roughness_texture()
            {
                textures.insert(textures.len(), PBRTexture::Roughness(tex.texture().source().index()))
            }

            if let Some(tex) = material.pbr_metallic_roughness().base_color_texture()
            {
                tex_coords = if let Some(tex_coord_iter) = reader.read_tex_coords(tex.tex_coord()) {
                    if let gltf::mesh::util::ReadTexCoords::F32(coords) =
                        tex_coord_iter.into_f32().unwrap()
                    {
                        coords
                            .into_iter()
                            .map(|tc| UV {
                                u: tc[0],
                                v: tc[1],
                                w: 0.0,
                            })
                            .collect()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };
                textures.insert(textures.len(), PBRTexture::Color(tex.texture().source().index()))
            }

            if let Some(vert_iter) = reader.read_positions() {
                let vertices: Vec<[f32; 3]> = vert_iter.into_iter().collect();
                if let Some(ind_iter) = reader.read_indices() {
                    let ind: Vec<u32> = ind_iter.into_u32().into_iter().collect();
                    let mut new_mesh = Mesh {
                        mesh_type: MeshType::Indexed(
                            vertices
                                .iter()
                                .enumerate()
                                .map(|(i, a)| {
                                    if !tex_coords.is_empty() {
                                        (a, tex_coords[i]).into()
                                    } else {
                                        a.into()
                                    }
                                })
                                .collect(),
                            // ((Vector3::new(v[0], v[1], v[2])
                            //   * transform.rot ) + transform.pos)
                            // .into())
                            //.collect(),
                            ind.iter().copied().collect(),
                        ),
                        buffer_indices: vec![],
                        buffer_offset: 0,
                        textures,
                    };
                    new_mesh.calculate_normals(NormalMode::Shaded);
                    out_meshes.insert(out_meshes.len(), new_mesh);
                } else {
                    let mut new_mesh = Mesh {
                        mesh_type: MeshType::NonIndexed(
                            (0..vertices.len() - 3)
                                .step_by(3)
                                .map(|i| {
                                    if !tex_coords.is_empty() {
                                        (
                                            vertices[i].into(),
                                            tex_coords[i],
                                            vertices[i + 1].into(),
                                            tex_coords[i + 1],
                                            vertices[i + 2].into(),
                                            tex_coords[i + 2],
                                        )
                                            .into()
                                    } else {
                                        (
                                            vertices[i].into(),
                                            vertices[i + 1].into(),
                                            vertices[i + 2].into(),
                                        )
                                            .into()
                                    }
                                })
                                .collect(),
                        ),
                        buffer_indices: vec![],
                        buffer_offset: 0,
                        textures,
                    };
                    new_mesh.calculate_normals(NormalMode::Flat);
                    out_meshes.insert(out_meshes.len(), new_mesh);
                }
            }
        }
    }

    (node, transform, out_meshes)
}


pub fn texture_to_sprite (texture: gltf::texture::Texture, images: &[gltf::image::Data]) -> Option<(crate::sprite::Sprite, usize)> {
        let image_index = texture.source().index();
        let image = &images[image_index];
        let mut spr = crate::sprite::Sprite::new(image.width, image.height);
        match image.format {
            gltf::image::Format::R8 => spr.set_data(&image.pixels, 1),
            gltf::image::Format::R8G8 => spr.set_data(&image.pixels, 2),
            gltf::image::Format::R8G8B8 => spr.set_data(&image.pixels, 3),
            gltf::image::Format::R8G8B8A8 => spr.set_data(&image.pixels, 4),
            gltf::image::Format::B8G8R8 => spr.set_data(&image.pixels, 3),
            gltf::image::Format::B8G8R8A8 => spr.set_data(&image.pixels, 4),
            _ => {}
        }
        Some((spr, image_index))
}

pub fn image_to_sprite(image: &gltf::image::Data) -> crate::sprite::Sprite{
    let mut spr = crate::sprite::Sprite::new(image.width, image.height);
    match image.format {
        gltf::image::Format::R8 => spr.set_data(&image.pixels, 1),
        gltf::image::Format::R8G8 => spr.set_data(&image.pixels, 2),
        gltf::image::Format::R8G8B8 => spr.set_data(&image.pixels, 3),
        gltf::image::Format::R8G8B8A8 => spr.set_data(&image.pixels, 4),
        gltf::image::Format::B8G8R8 => spr.set_data(&image.pixels, 3),
        gltf::image::Format::B8G8R8A8 => spr.set_data(&image.pixels, 4),
        _ => {}
    };
    spr
}
