use crate::vertex::Vertex;

pub const VERTICES: [Vertex; 3] = [
    Vertex {
        position: [1.0, -1.0, 0.0, 1.0],
        color: [1.0, 0.0, 0.0, 1.0],
    },
    Vertex {
        position: [-1.0, -1.0, 0.0, 1.0],
        color: [0.0, 1.0, 0.0, 1.0],
    },
    Vertex {
        position: [0.0, 1.0, 0.0, 1.0],
        color: [0.0, 0.0, 1.0, 1.0],
    },
];

pub const INDICES: [u32; 3] = [0, 1, 2];

pub const CUBE_VERTICES: [Vertex; 8] = [
    Vertex {
        position: [-0.05, -0.05, -0.05, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
    },
    Vertex {
        position: [0.05, -0.05, -0.05, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
    },
    Vertex {
        position: [0.05, 0.05, -0.05, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
    },
    Vertex {
        position: [-0.05, 0.05, -0.05, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
    },
    Vertex {
        position: [-0.05, -0.05, 0.05, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
    },
    Vertex {
        position: [0.05, -0.05, 0.05, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
    },
    Vertex {
        position: [0.05, 0.05, 0.05, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
    },
    Vertex {
        position: [-0.05, 0.05, 0.05, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
    },
];

pub const CUBE_INDICES: [u32; 36] = [
    0, 1, 2, 2, 3, 0, 1, 5, 6, 6, 2, 1, 5, 4, 7, 7, 6, 5, 4, 0, 3, 3, 7, 4, 3, 2, 6, 6, 7, 3, 4, 5,
    1, 1, 0, 4,
];

pub const GREEN_CUBE_VERTICES: [Vertex; 8] = [
    Vertex {
        position: [-0.05, -0.05, -0.05, 1.0],
        color: [0.0, 1.0, 0.0, 1.0],
    },
    Vertex {
        position: [0.05, -0.05, -0.05, 1.0],
        color: [0.0, 1.0, 0.0, 1.0],
    },
    Vertex {
        position: [0.05, 0.05, -0.05, 1.0],
        color: [0.0, 1.0, 0.0, 1.0],
    },
    Vertex {
        position: [-0.05, 0.05, -0.05, 1.0],
        color: [0.0, 1.0, 0.0, 1.0],
    },
    Vertex {
        position: [-0.05, -0.05, 0.05, 1.0],
        color: [0.0, 1.0, 0.0, 1.0],
    },
    Vertex {
        position: [0.05, -0.05, 0.05, 1.0],
        color: [0.0, 1.0, 0.0, 1.0],
    },
    Vertex {
        position: [0.05, 0.05, 0.05, 1.0],
        color: [0.0, 1.0, 0.0, 1.0],
    },
    Vertex {
        position: [-0.05, 0.05, 0.05, 1.0],
        color: [0.0, 1.0, 0.0, 1.0],
    },
];