use std::collections::HashMap;

pub const NODE_W: f64 = 176.0;
pub const NODE_H: f64 = 52.0;
pub const GAP_X: f64 = 84.0;
pub const GAP_Y: f64 = 28.0;
pub const H_GAP: f64 = NODE_W + GAP_X;
pub const V_GAP: f64 = NODE_H + GAP_Y;
pub const PAD: f64 = 56.0;

const CHAR_W: f64 = 8.4;
const LINE_H: f64 = 18.2;
const CHROME_W: f64 = 68.0;
const CHROME_H: f64 = 20.0;
const ASPECT: f64 = 2.5;

/// Width/height so the card stays a compact rectangle and still fits `text`.
pub fn card_size(text: &str) -> (f64, f64) {
    if text.is_empty() {
        return (NODE_W, NODE_H);
    }
    let min_inner = (NODE_W - CHROME_W).max(80.0);
    let n = text.chars().count() as f64;
    let area = n * CHAR_W * LINE_H;
    let unwrapped = (n * CHAR_W).max(min_inner);
    let mut inner_w = (area * ASPECT).sqrt().max(min_inner).min(unwrapped);
    inner_w = inner_w.max(min_inner);
    let lines = wrap_line_count(text, inner_w) as f64;
    let w = (inner_w + CHROME_W).max(NODE_W);
    let h = (CHROME_H + lines * LINE_H).max(NODE_H);
    (w, h)
}

fn wrap_line_count(text: &str, inner_w: f64) -> usize {
    let max_chars = (inner_w / CHAR_W).floor().max(1.0) as usize;
    let mut lines = 0usize;
    for para in text.split('\n') {
        if para.is_empty() {
            lines += 1;
            continue;
        }
        let mut col = 0usize;
        lines += 1;
        for word in para.split_inclusive(' ') {
            let wlen = word.chars().count();
            if col == 0 {
                if wlen > max_chars {
                    lines += (wlen - 1) / max_chars;
                    col = wlen % max_chars;
                    if col == 0 {
                        col = max_chars;
                    }
                } else {
                    col = wlen;
                }
            } else if col + wlen > max_chars {
                lines += 1;
                if wlen > max_chars {
                    lines += (wlen - 1) / max_chars;
                    col = wlen % max_chars;
                    if col == 0 {
                        col = max_chars;
                    }
                } else {
                    col = wlen;
                }
            } else {
                col += wlen;
            }
        }
    }
    lines.max(1)
}

/// Longest-path layering + a few barycentric sweeps.
/// Isolated nodes sit in a leftover column to the left of the DAG.
pub fn layout(ids: &[String], edges: &[(String, String)]) -> HashMap<String, (f64, f64)> {
    let sizes: HashMap<String, (f64, f64)> = ids
        .iter()
        .map(|id| (id.clone(), (NODE_W, NODE_H)))
        .collect();
    layout_sized(ids, edges, &sizes)
}

pub fn layout_sized(
    ids: &[String],
    edges: &[(String, String)],
    sizes: &HashMap<String, (f64, f64)>,
) -> HashMap<String, (f64, f64)> {
    let mut pos: HashMap<String, (f64, f64)> = HashMap::new();
    if ids.is_empty() {
        return pos;
    }
    let size_of = |id: &str| sizes.get(id).copied().unwrap_or((NODE_W, NODE_H));

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

    let mut layers: Vec<usize> = by_layer.keys().copied().collect();
    layers.sort();
    let max_stack = by_layer
        .values()
        .map(|c| {
            let h: f64 = c.iter().map(|id| size_of(id).1).sum();
            h + GAP_Y * c.len().saturating_sub(1) as f64
        })
        .fold(0.0_f64, f64::max);
    let mut x = PAD;
    for l in layers {
        let col = by_layer.get(&l).cloned().unwrap_or_default();
        let col_w = col
            .iter()
            .map(|id| size_of(id).0)
            .fold(NODE_W, f64::max);
        let total_h: f64 = col.iter().map(|id| size_of(id).1).sum::<f64>()
            + GAP_Y * col.len().saturating_sub(1) as f64;
        let extra = (max_stack - total_h).max(0.0) / 2.0;
        let mut y = PAD + extra;
        for id in col {
            let h = size_of(id).1;
            pos.insert(id.to_string(), (x, y));
            y += h + GAP_Y;
        }
        x += col_w + GAP_X;
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

pub fn port_left(x: f64, y: f64, h: f64) -> (f64, f64) {
    (x, y + h / 2.0)
}

pub fn port_right(x: f64, y: f64, w: f64, h: f64) -> (f64, f64) {
    (x + w, y + h / 2.0)
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

    #[test]
    fn card_grows_with_text() {
        let (w0, h0) = card_size("");
        let (w1, h1) = card_size("CHMSL");
        let (w2, h2) = card_size("CRT wood: check fit, enlarge to open storage");
        let (w3, h3) = card_size(
            "Figure A/C placement then route both tubes out a side-window corner opener",
        );
        assert_eq!((w0, h0), (NODE_W, NODE_H));
        assert!(w1 >= NODE_W && h1 >= NODE_H);
        assert!(w2 * h2 > w1 * h1);
        assert!(w3 * h3 > w2 * h2);
        assert!(w3 / h3 > 1.2 && w3 / h3 < 4.5, "stay roughly rectangular {}", w3 / h3);
    }
}
