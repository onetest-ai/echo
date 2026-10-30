//! Embeddings from the endpoint the user configures (Settings → Brain): Ollama (`POST /api/embed`)
//! or any OpenAI-compatible server (`POST /v1/embeddings`). Echo runs no model itself. Vectors are
//! made unit length here, so a dot product is the cosine.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

/// How many texts go in one request.
pub const BATCH: usize = 32;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// `http://localhost:11434` (Ollama) or `https://…` (OpenAI-compatible); empty: no embeddings.
    #[serde(default)]
    pub endpoint: String,
    /// `ollama` | `openai`.
    #[serde(default)]
    pub api: String,
    #[serde(default)]
    pub model: String,
    /// A bearer token for an OpenAI-compatible server (kept in the 0600 settings file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// The Brain at all (none: off — on only for whoever turns it on in Settings → Brain; an update keeps it off).
    /// Off: nothing is read, indexed, tagged or offered to Claude; the store stays on disk as it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Keep the Brain current in the app (none: yes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto: Option<bool>,
    /// Who reads the pages of thread folders flagged visual: `claude` (the default AI profile's minutes
    /// model; none: this), `endpoint` (the vision endpoint below) or `off` (their text only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pages: Option<String>,
    /// A vision model: Ollama (`/api/chat`) or OpenAI-compatible (`/v1/chat/completions`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub vision_endpoint: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub vision_api: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub vision_model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vision_key: Option<String>,
}

impl Settings {
    pub fn enabled(&self) -> bool {
        self.enabled.unwrap_or(false)
    }

    pub fn auto(&self) -> bool {
        self.auto.unwrap_or(true)
    }

    pub fn pages(&self) -> &str {
        self.pages.as_deref().unwrap_or("claude")
    }

    pub fn ready(&self) -> bool {
        !self.endpoint.trim().is_empty() && !self.model.trim().is_empty()
    }
}

pub trait Embedder {
    /// The model's name, part of every vector's hash (another model re-embeds).
    fn model(&self) -> String;
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

pub fn unit(mut v: Vec<f32>) -> Vec<f32> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        v.iter_mut().for_each(|x| *x /= n);
    }
    v
}

pub struct Http(pub Settings);

pub(crate) fn quote(s: &str) -> String {
    // A newline would end the option and start another (curl reads its config line by line).
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r"))
}

impl Embedder for Http {
    fn model(&self) -> String {
        self.0.model.clone()
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let s = &self.0;
        let base = s.endpoint.trim().trim_end_matches('/');
        let openai = s.api == "openai";
        let url = if openai {
            if base.ends_with("/v1") { format!("{base}/embeddings") } else { format!("{base}/v1/embeddings") }
        } else {
            format!("{base}/api/embed")
        };
        let body = json!({"model": s.model, "input": texts});
        // The token and the texts go in curl's config on stdin, never on a command line.
        let mut cfg = format!(
            "url = {}\nrequest = \"POST\"\nheader = \"Content-Type: application/json\"\ndata-binary = {}\nmax-time = 120\nconnect-timeout = 5\nwrite-out = \"\\n%{{http_code}}\"\n",
            quote(&url),
            quote(&body.to_string())
        );
        if let Some(k) = s.key.as_deref().filter(|k| !k.is_empty()) {
            cfg.push_str(&format!("header = {}\n", quote(&format!("Authorization: Bearer {k}"))));
        }
        if crate::ai::loopback(base) {
            cfg.push_str("noproxy = \"*\"\n");
        }
        let mut c = Command::new("curl");
        #[cfg(windows)]
        {
            use crate::app::NoWindow;
            c = c.no_window();
        }
        let mut child = c.args(["-q", "-sS", "-K", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().context("running curl")?;
        child.stdin.take().unwrap().write_all(cfg.as_bytes())?;
        let out = child.wait_with_output()?;
        if !out.status.success() {
            bail!("the embedding server at {base} didn't answer: {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let (body, status) = text.rsplit_once('\n').unwrap_or((text.as_str(), ""));
        let v: Value = serde_json::from_str(body).map_err(|_| anyhow!("the embedding server answered HTTP {}: {}", status.trim(), body.chars().take(200).collect::<String>()))?;
        if let Some(e) = v.get("error") {
            bail!("the embedding server: {}", e["message"].as_str().or(e.as_str()).unwrap_or("error"));
        }
        let vecs: Vec<Vec<f32>> = if openai {
            let mut d: Vec<&Value> = v["data"].as_array().into_iter().flatten().collect();
            d.sort_by_key(|x| x["index"].as_u64().unwrap_or(0));
            d.iter().map(|x| x["embedding"].as_array().into_iter().flatten().filter_map(|f| f.as_f64().map(|f| f as f32)).collect()).collect()
        } else {
            v["embeddings"].as_array().into_iter().flatten().map(|e| e.as_array().into_iter().flatten().filter_map(|f| f.as_f64().map(|f| f as f32)).collect()).collect()
        };
        if vecs.len() != texts.len() || vecs.iter().any(|v| v.is_empty()) {
            bail!("the embedding server gave {} vectors for {} texts (is {:?} an embedding model?)", vecs.len(), texts.len(), s.model);
        }
        Ok(vecs.into_iter().map(unit).collect())
    }
}

/// Tests: a vector from the text's words (hashed into 64 dimensions), so texts that share words are
/// close — enough to test fusion and related without a server.
#[cfg(test)]
pub struct Words;

#[cfg(test)]
impl Embedder for Words {
    fn model(&self) -> String {
        "test-words".into()
    }
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        use sha2::Digest;
        Ok(texts
            .iter()
            .map(|t| {
                let mut v = vec![0f32; 64];
                for w in t.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 2) {
                    let d = sha2::Sha256::digest(w.as_bytes());
                    v[d[0] as usize % 64] += 1.0;
                }
                unit(v)
            })
            .collect())
    }
}
