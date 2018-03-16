//! The graph is a set of vertices and links between these vertices
use data::Dataset;
use std::collections::HashMap;

/// The parameters of the model
#[derive(Default)]
pub struct Model {
    /// Importance of connected bubbles being close
    pub spring : f64, 
    /// Importance of bubbles not-connecting
    pub repulse : f64, 
    /// Minimum distance (between centres) of two bubbles
    pub repulse_dist : f64, 
    /// Rigidity of bubbles
    pub repulse_rigidity : f64,
    /// Importance of all bubbles forming a sphere
    pub canvas : f64, 
    /// Radius of sphere containing all bubbles
    pub canvas_size : f64, 
    /// Rigidity of containing sphere
    pub canvas_rigidity : f64,
    /// Number of blocks used for near neigbours
    pub n_blocks : usize
}

/// A graph with of size `n` with a set of edges
#[derive(Debug,PartialEq,Clone)]
pub struct Graph {
    pub n: usize,
    values: HashMap<String, usize>,
    pub edges: Vec<Edge>
}

impl Graph {
    /// Create a new empty graph
    pub fn new() -> Graph {
        Graph {
            n: 0,
            values: HashMap::new(),
            edges : Vec::new()
        }
    }

    /// Add a vertex or look up the index of a vertex
    pub fn add_vertex(&mut self, name : &str) -> usize {
        if !self.values.contains_key(name) {
            self.values.insert(name.to_string(), self.n);
            self.n += 1;
            (self.n - 1)
        } else {
            self.values[name]
        }
    }

    /// Estimate the cost of a given set of locations (`loc`) given parameters
    pub fn cost(&self, loc : &Vec<f64>, m : &Model) -> f64 {
        let mut cost = 0.0;

        for edge in self.edges.iter() {
            let x = loc[edge.src * 2] - loc[edge.trg * 2];
            let y = loc[edge.src * 2 + 1] - loc[edge.trg * 2 + 1];
            let d = (x * x + y * y).sqrt();
            cost += m.spring * d;
        }

        if m.n_blocks > 1 {
            let blocking = Blocking::create(loc, m.n_blocks);

            for v1 in 0..self.n {
                for &(v2_id, v2_x, v2_y) in blocking.nearby(loc[v1 * 2], loc[v1 * 2 + 1]).iter() {
                    if v1 != v2_id {
                        let x = loc[v1 * 2] - v2_x;
                        let y = loc[v1 * 2 + 1] - v2_y;
                        cost += repulse_cost(x, y, m);
                    }
                }
            }
        } else {
            for v1 in 0..self.n {
                for v2 in 0..self.n {
                    if v1 != v2 {
                        let x = loc[v1 * 2] - loc[v2 * 2];
                        let y = loc[v1 * 2 + 1] - loc[v2 * 2 + 1];
                        cost += repulse_cost(x, y, m);
                    }
                }
            }
        }
        for v1 in 0..self.n {
            // Centre attraction
            let d = (loc[v1 * 2] * loc[v1 * 2] + 
                     loc[v1 * 2 + 1] * loc[v1 * 2 + 1]).sqrt();
            cost += m.canvas * (d / m.canvas_size).powf(m.canvas_rigidity);
        }
        cost
    }

    /// Calculate the gradient (d cost / d loc) of a set of locations (`loc`)
    pub fn gradient(&self, loc : &Vec<f64>, m : &Model) -> Vec<f64> {
        let mut gradient = Vec::new();
        gradient.resize(self.n * 2, 0.0f64);

        for edge in self.edges.iter() {
            let x = loc[edge.src * 2] - loc[edge.trg * 2];
            let y = loc[edge.src * 2 + 1] - loc[edge.trg * 2 + 1];
            let d = (x * x + y * y).sqrt();

            if d > 0.0 {
                gradient[edge.src * 2] += m.spring * x / d;
                gradient[edge.src * 2 + 1] += m.spring * y / d;
                gradient[edge.trg * 2] -= m.spring * x / d;
                gradient[edge.trg * 2 + 1] -= m.spring * y / d;
            }
        }

        if m.n_blocks > 1 {
            let blocking = Blocking::create(loc, m.n_blocks);
            for v1 in 0..self.n {
                for &(v2_id, v2_x, v2_y) in blocking.nearby(loc[v1 * 2], loc[v1 * 2 + 1]).iter() {
                    // Repulsion 1/||vi - vj||
                    if v1 != v2_id {
                        let x = loc[v1 * 2] - v2_x;
                        let y = loc[v1 * 2 + 1] - v2_y;
                        repulse_grad(&mut gradient, x, y, v1, v2_id, m);
                    }
                }
             }
        } else {
             for v1 in 0..self.n {
                for v2 in 0..self.n {
                    if v1 != v2 {
                        let x = loc[v1 * 2] - loc[v2 * 2];
                        let y = loc[v1 * 2 + 1] - loc[v2 * 2 + 1];
                        repulse_grad(&mut gradient, x, y, v1, v2, m);
                    }
                }
             }
        }

        for v1 in 0..self.n {
            // Centre attraction
            let d = (loc[v1 * 2] * loc[v1 * 2] + 
                     loc[v1 * 2 + 1] * loc[v1 * 2 + 1]).sqrt();
            gradient[v1 * 2] += m.canvas * 
                m.canvas_size.powf(-m.canvas_rigidity) *
                m.canvas_rigidity * loc[v1 * 2] *
                d.powf(m.canvas_rigidity - 2.0);
            gradient[v1 * 2 + 1] += m.canvas * 
                m.canvas_size.powf(-m.canvas_rigidity) *
                m.canvas_rigidity * loc[v1 * 2 + 1] *
                d.powf(m.canvas_rigidity - 2.0);
        }
        gradient
    }
}

fn repulse_cost(x : f64, y : f64, m : &Model) -> f64 {
    let d = (x * x + y * y).sqrt();
    m.repulse * relu(m.repulse_dist - d)
}


fn repulse_grad(gradient : &mut Vec<f64>, x : f64, y : f64,
                v1 : usize, v2 : usize, m : &Model) {
    let d = (x * x + y * y).sqrt();
    let s = sigma(m.repulse_dist - d);
    if d > 0.0 {
        gradient[v1 * 2] -= m.repulse * 2.0 * x * s / d;
        gradient[v1 * 2 + 1] -= m.repulse * 2.0 * y * s / d;
    } else {
        // Superposition, we push in a direction related 
        // to the ID
        gradient[v1 * 2] -= m.repulse * 2.0 * s * (v1 as f64).cos() * 1e-10;
        gradient[v1 * 2 + 1] -= m.repulse * 2.0 * s * (v2 as f64).sin() * 1e-10;
    }
}

/// An edge between two vertices
#[derive(Debug,PartialEq,Clone)]
pub struct Edge {
    pub src : usize,
    pub trg : usize
}

impl Edge {
    /// Create an edge
    pub fn new(from : usize, to  : usize) -> Edge {
        Edge {
            src: from,
            trg: to
        }
    }
}

struct Blocking {
    blocks : Vec<Vec<Vec<(usize,f64,f64)>>>,
    block_size : f64,
    max : f64,
    n_blocks : usize
}

impl Blocking {
    fn create(xs : &Vec<f64>, n_blocks : usize) -> Blocking {
        let mut blocks = Vec::new();
        blocks.resize(n_blocks, Vec::new());
        for i in 0..n_blocks {
            blocks[i].resize(n_blocks, Vec::new());
        }

        let mut max = 0.0;
        for x in xs {
            if x.is_finite() && x.abs() > max {
                max = x.abs();
            }
        }
        max *= 1.01; // To ensure that no value is exactly on the block boundary
        let block_size = max * 2.0 / (n_blocks as f64);

        for i in 0..(xs.len() / 2) {
            if xs[i * 2].is_finite() && xs[i * 2 + 1].is_finite() {
                let x = ((xs[i * 2] + max) / block_size).floor() as usize;
                let y = ((xs[i * 2 + 1] + max) / block_size).floor() as usize;

                blocks[x][y].push((i, xs[i * 2], xs[i * 2 + 1]));
            }
        }
        Blocking {
            blocks: blocks,
            block_size: block_size,
            max : max,
            n_blocks
        }
    }

    fn nearby<'a>(&'a self, x : f64, y : f64) -> Vec<(usize, f64, f64)> {
        if x.is_finite() && y.is_finite() {
            let x_id = ((x + self.max) / self.block_size).floor() as usize;
            let y_id = ((y + self.max) / self.block_size).floor() as usize;

            let mut elems = self.blocks[x_id][y_id].clone();
            if x_id > 0 {
                if y_id > 0 {
                    elems.extend(self.blocks[x_id - 1][y_id - 1].iter());
                }
                if y_id < self.n_blocks - 1 {
                    elems.extend(self.blocks[x_id - 1][y_id + 1].iter());
                }
                elems.extend(self.blocks[x_id - 1][y_id].iter());
            }
            if x_id < self.n_blocks - 1 {
                if y_id > 0 {
                    elems.extend(self.blocks[x_id + 1][y_id - 1].iter());
                }
                if y_id < self.n_blocks - 1 {
                    elems.extend(self.blocks[x_id + 1][y_id + 1].iter());
                }
                elems.extend(self.blocks[x_id + 1][y_id].iter());
            }
            if y_id > 0 {
                elems.extend(self.blocks[x_id][y_id - 1].iter());
            }
            if y_id < self.n_blocks - 1 {
                elems.extend(self.blocks[x_id][y_id + 1].iter());
            }
     
            elems
        } else {
            Vec::new()
        }
    }
}

fn sigma(x : f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

fn relu(x : f64) -> f64 {
    (1.0 + x.exp()).ln()
}

/// Build the graph from the dataset
pub fn build_graph(data : &HashMap<String, Dataset>) -> Graph {
    let mut g = Graph::new();
    for dataset in data.values() {
        if !dataset.links.is_empty() {
            let v1 = g.add_vertex(&dataset.identifier);
            for link in dataset.links.iter() {
                let v2 = g.add_vertex(&link.target);
                g.edges.push(Edge::new(v1,v2));
                g.edges.push(Edge::new(v2,v1));
            }
        }
    }
    g
}
