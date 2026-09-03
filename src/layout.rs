use std::collections::HashMap;

pub const NODE_W: f64 = 176.0;
pub const NODE_H: f64 = 52.0;
pub const H_GAP: f64 = 260.0;
pub const V_GAP: f64 = 80.0;
pub const PAD: f64 = 56.0;

/// Longest-path layering + a few barycentric sweeps.
/// Isolated nodes sit in a leftover column to the left of the DAG.
pub fn layout(ids: &[String], edges: &[(String, String)]) -> HashMap<String, (f64, f64)> {
    let mut pos: HashMap<String, (f64, f64)> = HashMap::new();
    if ids.is_empty() {
        return pos;
    }

    let mut preds: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut succs: HashMap<&str, Vec<&str>> = HashMap::new();
    for id in ids {
        preds.entry(id.as_str()).or_default();
        succs.entry(id.as_str()).or_default();
    }
    for (f, t) in edges {
        if preds.contains_key(f.as_str()) && preds.contains_key(t.as_str()) {
            succs.get_mut(f.as_str()).unwrap().push(t.as_str());
            preds.get_mut(t.as_str()).unwrap().push(f.as_str());
        }
    }

    let isolated: Vec<&str> = ids
        .iter()
        .map(|s| s.as_str())
        .filter(|id| preds[id].is_empty() && succs[id].is_empty())
        .collect();
    let connected: Vec<&str> = ids
        .iter()
        .map(|s| s.as_str())
        .filter(|id| !preds[id].is_empty() || !succs[id].is_empty())
        .collect();

    // Longest-path layers for connected nodes (sources = 0).
    let mut layer: HashMap<&str, usize> = HashMap::new();
    for id in &connected {
        layer.insert(*id, 0);
    }
    for _ in 0..connected.len().max(1) {
        let mut changed = false;
        for (f, t) in edges {
            if let (Some(&lf), Some(&lt)) = (layer.get(f.as_str()), layer.get(t.as_str())) {
                if lt < lf + 1 {
                    layer.insert(t.as_str(), lf + 1);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    let leftover = !isolated.is_empty() && !connected.is_empty();
    let shift = if leftover { 1 } else { 0 };

    let mut by_layer: HashMap<usize, Vec<&str>> = HashMap::new();
    if leftover || connected.is_empty() {
        let mut col = isolated.clone();
        col.sort();
        by_layer.insert(0, col);
    }
    for id in &connected {
        let l = layer.get(id).copied().unwrap_or(0) + shift;
        by_layer.entry(l).or_default().push(*id);
    }
    for v in by_layer.values_mut() {
        v.sort();
    }

    barycentric(&mut by_layer, &preds, &succs);

    let max_len = by_layer.values().map(|v| v.len()).max().unwrap_or(1);
    let mut layers: Vec<usize> = by_layer.keys().copied().collect();
    layers.sort();
    for l in layers {
        let col = by_layer.get(&l).cloned().unwrap_or_default();
        let extra = (max_len.saturating_sub(col.len())) as f64 * V_GAP / 2.0;
        for (i, id) in col.iter().enumerate() {
            let x = PAD + l as f64 * H_GAP;
            let y = PAD + extra + i as f64 * V_GAP;
            pos.insert((*id).to_string(), (x, y));
        }
    }
    pos
}

fn barycentric(
    by_layer: &mut HashMap<usize, Vec<&str>>,
    preds: &HashMap<&str, Vec<&str>>,
    succs: &HashMap<&str, Vec<&str>>,
) {
    let mut max_l = 0;
    for l in by_layer.keys() {
        max_l = max_l.max(*l);
    }
    for _ in 0..6 {
        for l in 0..=max_l {
            sort_layer(by_layer, l, |id, order| bary(id, preds, order));
        }
        for l in (0..=max_l).rev() {
            sort_layer(by_layer, l, |id, order| bary(id, succs, order));
        }
    }
}

fn bary(id: &str, nbrs: &HashMap<&str, Vec<&str>>, order: &HashMap<&str, usize>) -> f64 {
    let Some(list) = nbrs.get(id) else {
        return 0.0;
    };
    let mut sum = 0.0;
    let mut n = 0.0;
    for p in list {
        if let Some(o) = order.get(p) {
            sum += *o as f64;
            n += 1.0;
        }
    }
    if n == 0.0 {
        order.get(id).copied().unwrap_or(0) as f64
    } else {
        sum / n
    }
}

fn sort_layer(
    by_layer: &mut HashMap<usize, Vec<&str>>,
    l: usize,
    score: impl Fn(&str, &HashMap<&str, usize>) -> f64,
) {
    let order = current_order(by_layer);
    let Some(col) = by_layer.get_mut(&l) else {
        return;
    };
    let mut keyed: Vec<(u64, &str)> = col
        .iter()
        .map(|id| (score(id, &order).to_bits(), *id))
        .collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    *col = keyed.into_iter().map(|(_, id)| id).collect();
}

fn current_order<'a>(by_layer: &HashMap<usize, Vec<&'a str>>) -> HashMap<&'a str, usize> {
    let mut order = HashMap::new();
    for col in by_layer.values() {
        for (i, id) in col.iter().enumerate() {
            order.insert(*id, i);
        }
    }
    order
}

pub fn port_left(x: f64, y: f64) -> (f64, f64) {
    (x, y + NODE_H / 2.0)
}

pub fn port_right(x: f64, y: f64) -> (f64, f64) {
    (x + NODE_W, y + NODE_H / 2.0)
}

/// Cubic from dependent's left port to prereq's right port.
pub fn wire_d(x1: f64, y1: f64, x2: f64, y2: f64) -> String {
    let dx = (x1 - x2).abs().max(48.0) * 0.5;
    format!(
        "M {x1:.1} {y1:.1} C {c1x:.1} {y1:.1}, {c2x:.1} {y2:.1}, {x2:.1} {y2:.1}",
        c1x = x1 - dx,
        c2x = x2 + dx
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    fn edges(xs: &[(&str, &str)]) -> Vec<(String, String)> {
        xs.iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn chain_layers_ltr() {
        let pos = layout(&ids(&["a", "b", "c"]), &edges(&[("a", "b"), ("b", "c")]));
        let xa = pos["a"].0;
        let xb = pos["b"].0;
        let xc = pos["c"].0;
        assert!(xa < xb && xb < xc, "expected LTR {xa} < {xb} < {xc}");
    }

    #[test]
    fn isolated_leftover_column() {
        let pos = layout(&ids(&["iso", "a", "b"]), &edges(&[("a", "b")]));
        assert!(
            pos["iso"].0 < pos["a"].0,
            "isolated should sit left of the DAG"
        );
        assert!(pos["a"].0 < pos["b"].0);
    }

    #[test]
    fn siblings_same_layer_even() {
        let pos = layout(&ids(&["a", "b", "c"]), &edges(&[("a", "b"), ("a", "c")]));
        assert_eq!(pos["b"].0, pos["c"].0);
        let dy = (pos["b"].1 - pos["c"].1).abs();
        assert!((dy - V_GAP).abs() < 0.01, "siblings evenly spaced, dy={dy}");
    }

    #[test]
    fn all_isolated_one_column() {
        let pos = layout(&ids(&["z", "a", "m"]), &[]);
        let xs: Vec<f64> = pos.values().map(|p| p.0).collect();
        assert!(xs.iter().all(|x| (*x - xs[0]).abs() < 0.01));
    }
}
