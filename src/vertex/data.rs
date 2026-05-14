use crate::vertex::Vertex;

pub const EDGE_VERTS: [Vertex; 4] = [
    Vertex {
        position: [-0.5, -0.5, 0.0, 1.0],
        color: [1.0, 0.0, 0.0, 1.0],
    },
    Vertex {
        position: [0.5, -0.5, 0.0, 1.0],
        color: [0.0, 1.0, 0.0, 1.0],
    },
    Vertex {
        position: [-0.5, 0.5, 0.0, 1.0],
        color: [0.0, 0.0, 1.0, 1.0],
    },
    Vertex {
        position: [0.5, 0.5, 0.0, 1.0],
        color: [1.0, 1.0, 0.0, 1.0],
    },
];

pub const EDGE_INDICES: [u32; 4] = [0, 1, 2, 3];

// pub const CUBE_VERTICES: [Vertex; 8] = [
//     Vertex {
//         position: [-0.05, -0.05, -0.05, 1.0],
//         color: [1.0, 1.0, 1.0, 1.0],
//     },
//     Vertex {
//         position: [0.05, -0.05, -0.05, 1.0],
//         color: [1.0, 1.0, 1.0, 1.0],
//     },
//     Vertex {
//         position: [0.05, 0.05, -0.05, 1.0],
//         color: [1.0, 1.0, 1.0, 1.0],
//     },
//     Vertex {
//         position: [-0.05, 0.05, -0.05, 1.0],
//         color: [1.0, 1.0, 1.0, 1.0],
//     },
//     Vertex {
//         position: [-0.05, -0.05, 0.05, 1.0],
//         color: [1.0, 1.0, 1.0, 1.0],
//     },
//     Vertex {
//         position: [0.05, -0.05, 0.05, 1.0],
//         color: [1.0, 1.0, 1.0, 1.0],
//     },
//     Vertex {
//         position: [0.05, 0.05, 0.05, 1.0],
//         color: [1.0, 1.0, 1.0, 1.0],
//     },
//     Vertex {
//         position: [-0.05, 0.05, 0.05, 1.0],
//         color: [1.0, 1.0, 1.0, 1.0],
//     },
// ];