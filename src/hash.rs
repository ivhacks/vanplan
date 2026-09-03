use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::{Read, Write};

pub const PREFIX: &str = "vp1.";

/// gzip(yaml) at level 9, then base64url (no padding), prefixed `vp1.`.
/// The returned string does not include a leading `#`.
pub fn encode(yaml: &str) -> Result<String, String> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::new(9));
    encoder
        .write_all(yaml.as_bytes())
        .map_err(|e| e.to_string())?;
    let gz = encoder.finish().map_err(|e| e.to_string())?;
    Ok(format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(gz)))
}

/// Accepts `vp1.…`, `#vp1.…`, or a full URL containing `#vp1.…`.
pub fn decode(hash: &str) -> Result<String, String> {
    let payload = payload_from(hash)?;
    let gz = URL_SAFE_NO_PAD
        .decode(payload.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut decoder = GzDecoder::new(gz.as_slice());
    let mut yaml = String::new();
    decoder
        .read_to_string(&mut yaml)
        .map_err(|e| e.to_string())?;
    Ok(yaml)
}

fn payload_from(hash: &str) -> Result<String, String> {
    let s = hash.trim();
    let s = if let Some(i) = s.rfind("#vp1.") {
        &s[i + 1..]
    } else if let Some(i) = s.rfind("#") {
        s[i + 1..].trim()
    } else {
        s.trim().trim_start_matches('#')
    };
    let rest = s
        .strip_prefix(PREFIX)
        .ok_or_else(|| "not a vp1 hash".to_string())?;
    if rest.is_empty() {
        return Err("empty vp1 payload".into());
    }
    Ok(rest.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{self, Edge, Node};

    fn two_node_doc() -> (Vec<Node>, Vec<Edge>) {
        let nodes = vec![
            Node {
                id: "a".into(),
                text: "Pick chair".into(),
                color: "#c4b8a8".into(),
                done: true,
            },
            Node {
                id: "b".into(),
                text: "Build desk".into(),
                color: "#a8b0b8".into(),
                done: false,
            },
        ];
        let edges = vec![Edge {
            from: "a".into(),
            to: "b".into(),
        }];
        (nodes, edges)
    }

    #[test]
    fn roundtrip_empty_doc() {
        let yaml = model::to_yaml(&[], &[]).unwrap();
        let hash = encode(&yaml).unwrap();
        assert!(hash.starts_with("vp1."));
        assert!(!hash[4..].contains('='));
        let back = decode(&hash).unwrap();
        let doc = model::from_yaml(&back).unwrap();
        assert!(doc.nodes.is_empty());
        assert!(doc.edges.is_empty());
        assert_eq!(decode(&format!("#{hash}")).unwrap(), back);
    }

    #[test]
    fn roundtrip_two_node_one_edge() {
        let (nodes, edges) = two_node_doc();
        let yaml = model::to_yaml(&nodes, &edges).unwrap();
        let hash = encode(&yaml).unwrap();
        let doc = model::from_yaml(&decode(&hash).unwrap()).unwrap();
        assert_eq!(doc.nodes, nodes);
        assert_eq!(doc.edges, edges);
    }

    #[test]
    fn cycle_cleaned_from_yaml_still_encodes() {
        let yaml = "\
vanplan: 1
nodes:
- id: a
  text: A
  color: '#c4b8a8'
- id: b
  text: B
  color: '#a8b0b8'
edges:
- from: a
  to: b
- from: b
  to: a
";
        let doc = model::from_yaml(yaml).unwrap();
        assert_eq!(doc.edges.len(), 1);
        assert_eq!(doc.edges[0].from, "a");
        assert_eq!(doc.edges[0].to, "b");
        let cleaned = model::to_yaml(&doc.nodes, &doc.edges).unwrap();
        let hash = encode(&cleaned).unwrap();
        let again = model::from_yaml(&decode(&hash).unwrap()).unwrap();
        assert_eq!(again, doc);
    }

    /// Encoder's gzip+base64url of a known YAML string decodes back to that YAML.
    #[test]
    fn known_yaml_gzip_base64url_roundtrip() {
        let yaml = "vanplan: 1\nnodes: []\nedges: []\n";
        let hash = encode(yaml).unwrap();
        assert!(hash.starts_with("vp1."));
        assert!(!hash.contains('='));
        assert_eq!(decode(&hash).unwrap(), yaml);
        assert_eq!(decode(&format!("#{hash}")).unwrap(), yaml);
        assert_eq!(
            decode(&format!("http://127.0.0.1:8080/#{hash}")).unwrap(),
            yaml
        );
    }

    /// Python `gzip.compress(..., compresslevel=9, mtime=0)` + urlsafe b64 of the known YAML.
    /// Same vector as the comment in scripts/vanplan-url.
    const PY_EMPTY_HASH: &str = "vp1.H4sIAAAAAAAC_ytLzCvIScyzUjDkystPSS22UoiO5UpNSYeyABFfqaYfAAAA";
    const PY_EMPTY_YAML: &str = "vanplan: 1\nnodes: []\nedges: []\n";

    #[test]
    fn rust_decode_accepts_python_vector() {
        assert_eq!(decode(PY_EMPTY_HASH).unwrap(), PY_EMPTY_YAML);
        assert_eq!(decode(&format!("#{PY_EMPTY_HASH}")).unwrap(), PY_EMPTY_YAML);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn python_script() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/vanplan-url")
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn python_encode(yaml: &str) -> String {
        let mut child = std::process::Command::new("python3")
            .arg(python_script())
            .arg("encode")
            .arg("-")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn python3");
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(yaml.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "python encode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn python_decode(hash_or_url: &str) -> String {
        let out = std::process::Command::new("python3")
            .arg(python_script())
            .arg("decode")
            .arg(hash_or_url)
            .output()
            .expect("run python3 decode");
        assert!(
            out.status.success(),
            "python decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn python_encode_accepted_by_rust_decode() {
        let yaml = PY_EMPTY_YAML;
        let url = python_encode(yaml);
        let url = url.trim();
        assert!(url.starts_with("http://127.0.0.1:8080/#vp1."), "{url}");
        assert_eq!(decode(url).unwrap(), yaml);

        let (nodes, edges) = two_node_doc();
        let yaml = model::to_yaml(&nodes, &edges).unwrap();
        let url = python_encode(&yaml);
        let doc = model::from_yaml(&decode(url.trim()).unwrap()).unwrap();
        assert_eq!(doc.nodes, nodes);
        assert_eq!(doc.edges, edges);

        let rust_hash = encode(&yaml).unwrap();
        assert_eq!(python_decode(&rust_hash), yaml);
        assert_eq!(
            python_decode(&format!("http://127.0.0.1:8080/#{rust_hash}")),
            yaml
        );
    }
}
