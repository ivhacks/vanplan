use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub text: String,
    pub color: String,
    #[serde(default)]
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub vanplan: u32,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
}

pub const COLORS: &[&str] = &[
    "#c4b8a8", "#a8b0b8", "#a8b4a8", "#b8a8b0", "#b0a090", "#90a4a8", "#a8a090", "#9ca0b0",
];

pub fn next_color(current: &str) -> &'static str {
    let i = COLORS.iter().position(|c| *c == current).unwrap_or(0);
    COLORS[(i + 1) % COLORS.len()]
}

pub fn text_color(hex: &str) -> &'static str {
    let lum = luminance(hex);
    if lum > 0.45 {
        "#1a1a1c"
    } else {
        "#ece8e0"
    }
}

fn luminance(hex: &str) -> f64 {
    let h = hex.trim().trim_start_matches('#');
    if h.len() < 6 {
        return 0.5;
    }
    let ok = u8::from_str_radix(&h[0..2], 16)
        .ok()
        .zip(u8::from_str_radix(&h[2..4], 16).ok())
        .zip(u8::from_str_radix(&h[4..6], 16).ok());
    match ok {
        Some(((r, g), b)) => {
            let r = r as f64 / 255.0;
            let g = g as f64 / 255.0;
            let b = b as f64 / 255.0;
            0.2126 * r + 0.7152 * g + 0.0722 * b
        }
        None => 0.5,
    }
}

pub fn fresh_id(next_n: &mut u32, nodes: &[Node]) -> String {
    let existing: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    loop {
        let id = format!("n{next_n}");
        *next_n = next_n.saturating_add(1);
        if !existing.contains(id.as_str()) {
            return id;
        }
    }
}

/// Adding `from` (prereq) → `to` (dependent) cycles if `to` can already reach `from`.
pub fn would_cycle(edges: &[Edge], from: &str, to: &str) -> bool {
    if from == to {
        return true;
    }
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in edges {
        adj.entry(e.from.as_str()).or_default().push(e.to.as_str());
    }
    let mut seen: HashSet<&str> = HashSet::new();
    let mut q: VecDeque<&str> = VecDeque::new();
    q.push_back(to);
    seen.insert(to);
    while let Some(cur) = q.pop_front() {
        if cur == from {
            return true;
        }
        if let Some(next) = adj.get(cur) {
            for n in next {
                if seen.insert(n) {
                    q.push_back(n);
                }
            }
        }
    }
    false
}

pub fn edge_exists(edges: &[Edge], from: &str, to: &str) -> bool {
    edges.iter().any(|e| e.from == from && e.to == to)
}

pub fn add_edge(edges: &mut Vec<Edge>, from: String, to: String) -> bool {
    if edge_exists(edges, &from, &to) {
        return false;
    }
    if would_cycle(edges, &from, &to) {
        return false;
    }
    edges.push(Edge { from, to });
    true
}

pub fn to_yaml(nodes: &[Node], edges: &[Edge]) -> Result<String, String> {
    let doc = Document {
        vanplan: 1,
        nodes: nodes.to_vec(),
        edges: edges.to_vec(),
    };
    serde_yaml::to_string(&doc).map_err(|e| e.to_string())
}

pub fn from_yaml(src: &str) -> Result<Document, String> {
    let mut doc: Document = serde_yaml::from_str(src).map_err(|e| e.to_string())?;
    if doc.vanplan != 1 {
        return Err(format!("unsupported vanplan version {}", doc.vanplan));
    }
    let ids: HashSet<String> = doc.nodes.iter().map(|n| n.id.clone()).collect();
    let mut clean: Vec<Edge> = Vec::new();
    for e in doc.edges.drain(..) {
        if !ids.contains(&e.from) || !ids.contains(&e.to) {
            continue;
        }
        add_edge(&mut clean, e.from, e.to);
    }
    doc.edges = clean;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_reject_self() {
        assert!(would_cycle(&[], "a", "a"));
    }

    #[test]
    fn cycle_reject_back_edge() {
        let edges = vec![Edge {
            from: "a".into(),
            to: "b".into(),
        }];
        assert!(would_cycle(&edges, "b", "a"));
        assert!(!would_cycle(&edges, "a", "c"));
        assert!(!would_cycle(&edges, "c", "a"));
    }

    #[test]
    fn cycle_reject_longer() {
        let mut edges = Vec::new();
        assert!(add_edge(&mut edges, "a".into(), "b".into()));
        assert!(add_edge(&mut edges, "b".into(), "c".into()));
        assert!(!add_edge(&mut edges, "c".into(), "a".into()));
        assert!(add_edge(&mut edges, "a".into(), "c".into()));
    }

    #[test]
    fn yaml_roundtrip() {
        let nodes = vec![
            Node {
                id: "chair".into(),
                text: "Pick chair location".into(),
                color: "#e8d5b7".into(),
                done: true,
            },
            Node {
                id: "desk".into(),
                text: "Build desk".into(),
                color: "#c4a574".into(),
                done: false,
            },
        ];
        let edges = vec![Edge {
            from: "chair".into(),
            to: "desk".into(),
        }];
        let yaml = to_yaml(&nodes, &edges).unwrap();
        assert!(yaml.contains("vanplan: 1"));
        assert!(yaml.contains("done: true"));
        assert!(yaml.contains("done: false"));
        let doc = from_yaml(&yaml).unwrap();
        assert_eq!(doc.nodes, nodes);
        assert_eq!(doc.edges, edges);
        assert!(doc.nodes[0].done);
        assert!(!doc.nodes[1].done);
    }

    #[test]
    fn yaml_done_defaults_false_on_old_docs() {
        let yaml = "\
vanplan: 1
nodes:
- id: a
  text: A
  color: '#c4b8a8'
edges: []
";
        let doc = from_yaml(yaml).unwrap();
        assert_eq!(doc.nodes.len(), 1);
        assert!(!doc.nodes[0].done);
    }
}
