use orbit_core::formats::FileFormat;
use orbit_core::registry::{ConversionError, ConversionOptions, Converter, ProgressReporter};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

fn io_err<E: std::fmt::Display>(e: E) -> ConversionError {
    ConversionError::IoError(e.to_string())
}

fn proc_err<E: std::fmt::Display>(e: E) -> ConversionError {
    ConversionError::ProcessingError(e.to_string())
}

#[derive(Default)]
struct SimpleMesh {
    vertices: Vec<[f32; 3]>,
    triangles: Vec<[usize; 3]>,
}

impl SimpleMesh {
    fn compute_normal(&self, tri: [usize; 3]) -> [f32; 3] {
        if tri[0] >= self.vertices.len()
            || tri[1] >= self.vertices.len()
            || tri[2] >= self.vertices.len()
        {
            return [0.0, 0.0, 1.0];
        }

        let p1 = self.vertices[tri[0]];
        let p2 = self.vertices[tri[1]];
        let p3 = self.vertices[tri[2]];

        let u = [p2[0] - p1[0], p2[1] - p1[1], p2[2] - p1[2]];
        let v = [p3[0] - p1[0], p3[1] - p1[1], p3[2] - p1[2]];

        let nx = u[1] * v[2] - u[2] * v[1];
        let ny = u[2] * v[0] - u[0] * v[2];
        let nz = u[0] * v[1] - u[1] * v[0];

        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        if len > 1e-6 {
            [nx / len, ny / len, nz / len]
        } else {
            [0.0, 0.0, 1.0]
        }
    }
}

pub struct Model3DConverter;

impl Model3DConverter {
    pub fn new() -> Self {
        Self
    }

    fn read_obj(path: &Path) -> Result<SimpleMesh, ConversionError> {
        let file = File::open(path).map_err(io_err)?;
        let reader = BufReader::new(file);
        let mut mesh = SimpleMesh::default();

        for line_res in reader.lines() {
            let line = line_res.map_err(io_err)?;
            let trimmed = line.trim();
            if trimmed.starts_with("v ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 4 {
                    let x: f32 = parts[1].parse().unwrap_or(0.0);
                    let y: f32 = parts[2].parse().unwrap_or(0.0);
                    let z: f32 = parts[3].parse().unwrap_or(0.0);
                    mesh.vertices.push([x, y, z]);
                }
            } else if trimmed.starts_with("f ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 4 {
                    let mut idxs = Vec::new();
                    for part in &parts[1..] {
                        let token = part.split('/').next().unwrap_or(part);
                        if let Ok(idx) = token.parse::<usize>() {
                            if idx > 0 {
                                idxs.push(idx - 1);
                            }
                        }
                    }
                    // Fan triangulation for polygons
                    for i in 1..(idxs.len().saturating_sub(1)) {
                        mesh.triangles.push([idxs[0], idxs[i], idxs[i + 1]]);
                    }
                }
            }
        }

        Ok(mesh)
    }

    fn read_stl(path: &Path) -> Result<SimpleMesh, ConversionError> {
        let mut file = File::open(path).map_err(io_err)?;
        let mut header = [0u8; 84];
        let n = file.read(&mut header).map_err(io_err)?;

        if n < 84 {
            return Err(ConversionError::InvalidInput(
                "STL file is too short".into(),
            ));
        }

        let is_ascii = header.starts_with(b"solid");

        if is_ascii {
            // Read ASCII STL
            let f = File::open(path).map_err(io_err)?;
            let reader = BufReader::new(f);
            let mut mesh = SimpleMesh::default();
            let mut tri_verts = Vec::new();

            for line_res in reader.lines() {
                let line = line_res.map_err(io_err)?;
                let trimmed = line.trim();
                if trimmed.starts_with("vertex ") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 4 {
                        let x: f32 = parts[1].parse().unwrap_or(0.0);
                        let y: f32 = parts[2].parse().unwrap_or(0.0);
                        let z: f32 = parts[3].parse().unwrap_or(0.0);
                        mesh.vertices.push([x, y, z]);
                        tri_verts.push(mesh.vertices.len() - 1);

                        if tri_verts.len() == 3 {
                            mesh.triangles
                                .push([tri_verts[0], tri_verts[1], tri_verts[2]]);
                            tri_verts.clear();
                        }
                    }
                }
            }
            Ok(mesh)
        } else {
            // Binary STL
            let num_triangles = u32::from_le_bytes([header[80], header[81], header[82], header[83]]) as usize;
            let mut mesh = SimpleMesh::default();

            let mut tri_buf = [0u8; 50];
            for _ in 0..num_triangles {
                if file.read_exact(&mut tri_buf).is_err() {
                    break;
                }
                let v1 = [
                    f32::from_le_bytes([tri_buf[12], tri_buf[13], tri_buf[14], tri_buf[15]]),
                    f32::from_le_bytes([tri_buf[16], tri_buf[17], tri_buf[18], tri_buf[19]]),
                    f32::from_le_bytes([tri_buf[20], tri_buf[21], tri_buf[22], tri_buf[23]]),
                ];
                let v2 = [
                    f32::from_le_bytes([tri_buf[24], tri_buf[25], tri_buf[26], tri_buf[27]]),
                    f32::from_le_bytes([tri_buf[28], tri_buf[29], tri_buf[30], tri_buf[31]]),
                    f32::from_le_bytes([tri_buf[32], tri_buf[33], tri_buf[34], tri_buf[35]]),
                ];
                let v3 = [
                    f32::from_le_bytes([tri_buf[36], tri_buf[37], tri_buf[38], tri_buf[39]]),
                    f32::from_le_bytes([tri_buf[40], tri_buf[41], tri_buf[42], tri_buf[43]]),
                    f32::from_le_bytes([tri_buf[44], tri_buf[45], tri_buf[46], tri_buf[47]]),
                ];

                let idx1 = mesh.vertices.len();
                mesh.vertices.push(v1);
                let idx2 = mesh.vertices.len();
                mesh.vertices.push(v2);
                let idx3 = mesh.vertices.len();
                mesh.vertices.push(v3);

                mesh.triangles.push([idx1, idx2, idx3]);
            }
            Ok(mesh)
        }
    }

    fn write_obj(mesh: &SimpleMesh, output: &Path) -> Result<(), ConversionError> {
        let mut file = File::create(output).map_err(io_err)?;
        writeln!(file, "# Exported by OrbitConvert 3D Engine").map_err(io_err)?;
        for v in &mesh.vertices {
            writeln!(file, "v {:.6} {:.6} {:.6}", v[0], v[1], v[2])
                .map_err(io_err)?;
        }
        for t in &mesh.triangles {
            writeln!(file, "f {} {} {}", t[0] + 1, t[1] + 1, t[2] + 1)
                .map_err(io_err)?;
        }
        Ok(())
    }

    fn write_stl(mesh: &SimpleMesh, output: &Path) -> Result<(), ConversionError> {
        let mut file = File::create(output).map_err(io_err)?;
        let mut header = [0u8; 80];
        let title = b"OrbitConvert Binary STL Export";
        header[..title.len()].copy_from_slice(title);
        file.write_all(&header).map_err(io_err)?;

        let count = mesh.triangles.len() as u32;
        file.write_all(&count.to_le_bytes()).map_err(io_err)?;

        for t in &mesh.triangles {
            let normal = mesh.compute_normal(*t);
            let p1 = mesh.vertices[t[0]];
            let p2 = mesh.vertices[t[1]];
            let p3 = mesh.vertices[t[2]];

            file.write_all(&normal[0].to_le_bytes()).map_err(io_err)?;
            file.write_all(&normal[1].to_le_bytes()).map_err(io_err)?;
            file.write_all(&normal[2].to_le_bytes()).map_err(io_err)?;

            file.write_all(&p1[0].to_le_bytes()).map_err(io_err)?;
            file.write_all(&p1[1].to_le_bytes()).map_err(io_err)?;
            file.write_all(&p1[2].to_le_bytes()).map_err(io_err)?;

            file.write_all(&p2[0].to_le_bytes()).map_err(io_err)?;
            file.write_all(&p2[1].to_le_bytes()).map_err(io_err)?;
            file.write_all(&p2[2].to_le_bytes()).map_err(io_err)?;

            file.write_all(&p3[0].to_le_bytes()).map_err(io_err)?;
            file.write_all(&p3[1].to_le_bytes()).map_err(io_err)?;
            file.write_all(&p3[2].to_le_bytes()).map_err(io_err)?;

            file.write_all(&[0u8, 0u8]).map_err(io_err)?;
        }

        Ok(())
    }

    fn write_ply(mesh: &SimpleMesh, output: &Path) -> Result<(), ConversionError> {
        let mut file = File::create(output).map_err(io_err)?;
        writeln!(file, "ply").map_err(io_err)?;
        writeln!(file, "format ascii 1.0").map_err(io_err)?;
        writeln!(file, "comment Exported by OrbitConvert").map_err(io_err)?;
        writeln!(file, "element vertex {}", mesh.vertices.len()).map_err(io_err)?;
        writeln!(file, "property float x").map_err(io_err)?;
        writeln!(file, "property float y").map_err(io_err)?;
        writeln!(file, "property float z").map_err(io_err)?;
        writeln!(file, "element face {}", mesh.triangles.len()).map_err(io_err)?;
        writeln!(file, "property list uchar int vertex_indices").map_err(io_err)?;
        writeln!(file, "end_header").map_err(io_err)?;

        for v in &mesh.vertices {
            writeln!(file, "{:.6} {:.6} {:.6}", v[0], v[1], v[2]).map_err(io_err)?;
        }
        for t in &mesh.triangles {
            writeln!(file, "3 {} {} {}", t[0], t[1], t[2]).map_err(io_err)?;
        }
        Ok(())
    }

    fn write_glb(mesh: &SimpleMesh, output: &Path) -> Result<(), ConversionError> {
        let mut binary_data = Vec::new();

        let indices_byte_offset = binary_data.len();
        for t in &mesh.triangles {
            binary_data.extend_from_slice(&(t[0] as u32).to_le_bytes());
            binary_data.extend_from_slice(&(t[1] as u32).to_le_bytes());
            binary_data.extend_from_slice(&(t[2] as u32).to_le_bytes());
        }
        let indices_byte_length = binary_data.len() - indices_byte_offset;

        while binary_data.len() % 4 != 0 {
            binary_data.push(0);
        }

        let pos_byte_offset = binary_data.len();
        let mut min_bound = [f32::INFINITY; 3];
        let mut max_bound = [f32::NEG_INFINITY; 3];

        for v in &mesh.vertices {
            for k in 0..3 {
                min_bound[k] = min_bound[k].min(v[k]);
                max_bound[k] = max_bound[k].max(v[k]);
            }
            binary_data.extend_from_slice(&v[0].to_le_bytes());
            binary_data.extend_from_slice(&v[1].to_le_bytes());
            binary_data.extend_from_slice(&v[2].to_le_bytes());
        }
        let pos_byte_length = binary_data.len() - pos_byte_offset;

        if mesh.vertices.is_empty() {
            min_bound = [0.0, 0.0, 0.0];
            max_bound = [0.0, 0.0, 0.0];
        }

        while binary_data.len() % 4 != 0 {
            binary_data.push(0);
        }

        let num_indices = mesh.triangles.len() * 3;
        let num_vertices = mesh.vertices.len();

        let gltf_json = serde_json::json!({
            "asset": {
                "generator": "OrbitConvert 3D Engine",
                "version": "2.0"
            },
            "scene": 0,
            "scenes": [
                { "nodes": [0] }
            ],
            "nodes": [
                { "mesh": 0 }
            ],
            "meshes": [
                {
                    "primitives": [
                        {
                            "attributes": {
                                "POSITION": 1
                            },
                            "indices": 0,
                            "mode": 4
                        }
                    ]
                }
            ],
            "accessors": [
                {
                    "bufferView": 0,
                    "byteOffset": 0,
                    "componentType": 5125,
                    "count": num_indices,
                    "type": "SCALAR"
                },
                {
                    "bufferView": 1,
                    "byteOffset": 0,
                    "componentType": 5126,
                    "count": num_vertices,
                    "type": "VEC3",
                    "max": max_bound,
                    "min": min_bound
                }
            ],
            "bufferViews": [
                {
                    "buffer": 0,
                    "byteOffset": indices_byte_offset,
                    "byteLength": indices_byte_length,
                    "target": 34963
                },
                {
                    "buffer": 0,
                    "byteOffset": pos_byte_offset,
                    "byteLength": pos_byte_length,
                    "target": 34962
                }
            ],
            "buffers": [
                {
                    "byteLength": binary_data.len()
                }
            ]
        });

        let mut json_bytes = serde_json::to_vec(&gltf_json).map_err(proc_err)?;

        while json_bytes.len() % 4 != 0 {
            json_bytes.push(b' ');
        }

        let mut file = File::create(output).map_err(io_err)?;

        let magic = 0x46546C67u32; // 'glTF'
        let version = 2u32;
        let total_length = (12 + 8 + json_bytes.len() + 8 + binary_data.len()) as u32;

        file.write_all(&magic.to_le_bytes()).map_err(io_err)?;
        file.write_all(&version.to_le_bytes()).map_err(io_err)?;
        file.write_all(&total_length.to_le_bytes()).map_err(io_err)?;

        let json_chunk_len = json_bytes.len() as u32;
        let json_chunk_type = 0x4E4F534Au32; // 'JSON'
        file.write_all(&json_chunk_len.to_le_bytes()).map_err(io_err)?;
        file.write_all(&json_chunk_type.to_le_bytes()).map_err(io_err)?;
        file.write_all(&json_bytes).map_err(io_err)?;

        let bin_chunk_len = binary_data.len() as u32;
        let bin_chunk_type = 0x004E4942u32; // 'BIN\0'
        file.write_all(&bin_chunk_len.to_le_bytes()).map_err(io_err)?;
        file.write_all(&bin_chunk_type.to_le_bytes()).map_err(io_err)?;
        file.write_all(&binary_data).map_err(io_err)?;

        Ok(())
    }
}

impl Converter for Model3DConverter {
    fn id(&self) -> &'static str {
        "native-3d-geometry-engine"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        let is_3d_src = matches!(from, FileFormat::Obj | FileFormat::Stl | FileFormat::Ply);
        let is_3d_dst = matches!(
            to,
            FileFormat::Obj | FileFormat::Stl | FileFormat::Ply | FileFormat::Glb
        );
        is_3d_src && is_3d_dst && from != to
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        _options: &ConversionOptions,
        progress: ProgressReporter<'_>,
    ) -> Result<(), ConversionError> {
        if let Some(cb) = progress {
            cb(0.1);
        }

        let from_ext = input
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_lowercase();
        let to_ext = output
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_lowercase();

        let mesh = match from_ext.as_str() {
            "obj" => Self::read_obj(input)?,
            "stl" => Self::read_stl(input)?,
            _ => {
                return Err(ConversionError::InvalidInput(format!(
                    "Unsupported 3D input format: {}",
                    from_ext
                )))
            }
        };

        if let Some(cb) = progress {
            cb(0.6);
        }

        match to_ext.as_str() {
            "obj" => Self::write_obj(&mesh, output)?,
            "stl" => Self::write_stl(&mesh, output)?,
            "ply" => Self::write_ply(&mesh, output)?,
            "glb" => Self::write_glb(&mesh, output)?,
            _ => {
                return Err(ConversionError::InvalidInput(format!(
                    "Unsupported 3D target format: {}",
                    to_ext
                )))
            }
        }

        if let Some(cb) = progress {
            cb(1.0);
        }

        Ok(())
    }
}
