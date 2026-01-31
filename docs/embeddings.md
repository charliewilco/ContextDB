# Getting Started with Embeddings

## Overview
How to generate and manage embeddings for ContextDB.

## When to use
- You need to choose models or dimensions.

## Examples

## What are Embeddings?

Embeddings are vector representations of text that capture semantic meaning. Similar concepts have similar vectors.

```
"cat" → [0.2, 0.8, 0.1, ...]
"dog" → [0.3, 0.7, 0.15, ...] // Similar to "cat"
"car" → [0.9, 0.1, 0.05, ...] // Different from "cat"
```

## Quick Options

### 1. OpenAI API (Easiest)

```bash
# Install OpenAI Rust client
cargo add async-openai
cargo add tokio --features full
```

```rust
use async_openai::{types::CreateEmbeddingRequestArgs, Client};
use contextdb::{ContextDB, Entry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let mut db = ContextDB::in_memory()?;
    
    let request = CreateEmbeddingRequestArgs::default()
        .model("text-embedding-3-small")
        .input("User doesn't like red onions")
        .build()
        .unwrap();
    
    let response = client.embeddings().create(request).await.unwrap();
    let embedding = &response.data[0].embedding;
    
    // Use with ContextDB
    let entry = Entry::new(embedding.clone(), "User doesn't like red onions".to_string());
    db.insert(&entry)?;

    Ok(())
}
```

**Pros**: High quality, easy to use  
**Cons**: Requires API key, costs money, sends data to OpenAI

### 2. Sentence Transformers (Python → Rust)

Generate embeddings in Python, use in Rust:

```python
# Python script: generate_embeddings.py
from sentence_transformers import SentenceTransformer
import json

model = SentenceTransformer('all-MiniLM-L6-v2')

texts = [
    "User doesn't like red onions",
    "User is gluten free",
    "User prefers TypeScript"
]

embeddings = model.encode(texts)

# Save to JSON
data = []
for text, embedding in zip(texts, embeddings):
    data.append({
        "text": text,
        "embedding": embedding.tolist()
    })

with open('embeddings.json', 'w') as f:
    json.dump(data, f)
```

```rust
// Rust: load and use embeddings
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct EmbeddingData {
    text: String,
    embedding: Vec<f32>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::open("embeddings.json")?;
    let data: Vec<EmbeddingData> = serde_json::from_reader(file)?;
    
    let mut db = ContextDB::in_memory()?;
    
    for item in data {
        let entry = Entry::new(item.embedding, item.text);
        db.insert(&entry)?;
    }
    
    Ok(())
}
```

**Pros**: Free, runs locally, good quality  
**Cons**: Requires Python, two-step process

### 3. ONNX Runtime (Pure Rust)

Run models directly in Rust using ONNX:

```bash
cargo add ort --features download-binaries
cargo add tokenizers
```

```rust
use ort::{Environment, SessionBuilder, Value};
use tokenizers::Tokenizer;

fn generate_embedding(text: &str) -> Vec<f32> {
    // Load model (one-time setup)
    let environment = Environment::builder().build().unwrap();
    let session = SessionBuilder::new(&environment)
        .unwrap()
        .with_model_from_file("model.onnx")
        .unwrap();
    
    // Tokenize
    let tokenizer = Tokenizer::from_file("tokenizer.json").unwrap();
    let encoding = tokenizer.encode(text, false).unwrap();
    let input_ids = encoding.get_ids();
    
    // Run inference
    let inputs = vec![Value::from_array(session.allocator(), &[input_ids]).unwrap()];
    let outputs = session.run(inputs).unwrap();
    
    // Extract embedding
    let embedding = outputs[0].extract_tensor().unwrap();
    embedding.view().iter().copied().collect()
}

fn main() {
    let embedding = generate_embedding("User doesn't like red onions");
    let entry = Entry::new(embedding, "User doesn't like red onions".to_string());
    db.insert(&entry)?;
}
```

**Pros**: Pure Rust, runs locally, no Python  
**Cons**: Complex setup, need to convert models to ONNX

### 4. REST API Service

Create a microservice that generates embeddings:

```python
# embedding_service.py
from flask import Flask, request, jsonify
from sentence_transformers import SentenceTransformer

app = Flask(__name__)
model = SentenceTransformer('all-MiniLM-L6-v2')

@app.route('/embed', methods=['POST'])
def embed():
    text = request.json['text']
    embedding = model.encode(text).tolist()
    return jsonify({'embedding': embedding})

if __name__ == '__main__':
    app.run(port=8080)
```

```rust
// Rust client
use contextdb::{ContextDB, Entry};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct EmbedRequest {
    text: String,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
}

fn get_embedding(text: &str) -> Vec<f32> {
    let client = Client::new();
    let response = client
        .post("http://localhost:8080/embed")
        .json(&EmbedRequest { text: text.to_string() })
        .send()
        .unwrap();
    
    let result: EmbedResponse = response.json().unwrap();
    result.embedding
}

fn main() {
    let mut db = ContextDB::in_memory().unwrap();
    let embedding = get_embedding("User doesn't like red onions");
    let entry = Entry::new(embedding, "User doesn't like red onions".to_string());
    db.insert(&entry).unwrap();
}
```

**Pros**: Separation of concerns, reusable service  
**Cons**: Extra service to manage, network overhead

## Recommended Approach

For **prototyping**: Use OpenAI API (easiest)  
For **production (low volume)**: Use OpenAI API  
For **production (high volume)**: Run local models (Sentence Transformers or ONNX)  
For **microservices**: REST API service

## Common Models

| Model | Dimensions | Quality | Speed | Use Case |
|-------|-----------|---------|-------|----------|
| OpenAI text-embedding-3-small | 1536 | Excellent | Fast | General purpose |
| OpenAI text-embedding-3-large | 3072 | Best | Slower | High quality needed |
| all-MiniLM-L6-v2 | 384 | Good | Very Fast | Local, resource-constrained |
| all-mpnet-base-v2 | 768 | Better | Fast | Local, balanced |
| BGE-large-en | 1024 | Excellent | Medium | Local, high quality |

## Chunking Long Documents

Embeddings work best on focused text. For long documents, chunk and add metadata:

```rust
let entry = Entry::new(embedding, chunk_text.to_string()).with_context(json!({
    "source": "handbook.pdf",
    "chunk_index": 3,
    "chunk_count": 12,
    "section": "Incident response",
}));
```

Typical chunk sizes:
- 200–500 tokens for dense retrieval
- 500–1200 tokens for general RAG

The best size depends on model and use case; validate with retrieval quality tests.

## Model Consistency and Drift

- **Consistency**: All entries in one database should share the same model and dimension.
- **Drift**: If you upgrade models, store `model` and `version` in `context` so you can
  filter or migrate entries safely.

## Tips

### Consistency is Key

**All entries must use the same embedding model and dimensions!**

```rust
// ❌ BAD: Mixing models
let entry1 = Entry::new(openai_embedding, "text1"); // 1536 dims
let entry2 = Entry::new(miniml_embedding, "text2"); // 384 dims
// Similarity calculations will be meaningless!
// (ContextDB returns 0.0 if dimensions differ.)

// ✅ GOOD: Same model for all
let embedding1 = model.encode("text1");
let embedding2 = model.encode("text2");
let entry1 = Entry::new(embedding1, "text1");
let entry2 = Entry::new(embedding2, "text2");
```

### Batch Processing

Generate embeddings in batches for efficiency:

```python
# Generate many embeddings at once
texts = ["text1", "text2", "text3", ...]
embeddings = model.encode(texts, batch_size=32)
```

### Cache Embeddings

Don't regenerate the same embeddings:

```rust
use std::collections::HashMap;

struct EmbeddingCache {
    cache: HashMap<String, Vec<f32>>,
    model: Box<dyn Fn(&str) -> Vec<f32>>,
}

impl EmbeddingCache {
    fn get_or_generate(&mut self, text: &str) -> Vec<f32> {
        if let Some(embedding) = self.cache.get(text) {
            embedding.clone()
        } else {
            let embedding = (self.model)(text);
            self.cache.insert(text.to_string(), embedding.clone());
            embedding
        }
    }
}
```

### Normalize Vectors

Some models require normalization:

```rust
fn normalize(vector: &mut Vec<f32>) {
    let magnitude: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    for x in vector.iter_mut() {
        *x /= magnitude;
    }
}

let mut embedding = generate_embedding("text");
normalize(&mut embedding);
let entry = Entry::new(embedding, "text".to_string());
```

### Store Provenance

Record embedding metadata alongside content so you can audit later:

```rust
let entry = Entry::new(embedding, text.to_string()).with_context(json!({
    "embedding_model": "text-embedding-3-small",
    "embedding_dim": 1536,
    "source": "conversation",
}));
```

### Privacy Notes

Embeddings can encode sensitive information. Treat them as sensitive data and apply
the same retention and access policies you would use for raw text.

## Full Example

Here's a complete example using OpenAI:

```rust
use async_openai::{types::CreateEmbeddingRequestArgs, Client};
use contextdb::{ContextDB, Entry, Query};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup
    let openai = Client::new();
    let mut db = ContextDB::new("memories.db")?;
    
    // Helper to generate embeddings
    async fn embed(client: &Client, text: &str) -> Vec<f32> {
        let request = CreateEmbeddingRequestArgs::default()
            .model("text-embedding-3-small")
            .input(text)
            .build()
            .unwrap();
        
        let response = client.embeddings().create(request).await.unwrap();
        response.data[0].embedding.clone()
    }
    
    // Store some memories
    let memories = vec![
        ("User doesn't like red onions", json!({"category": "dietary"})),
        ("User prefers TypeScript", json!({"category": "work"})),
        ("User is gluten free", json!({"category": "dietary"})),
    ];
    
    for (text, context) in memories {
        let embedding = embed(&openai, text).await;
        let entry = Entry::new(embedding, text.to_string()).with_context(context);
        db.insert(&entry)?;
    }
    
    // Query by semantic similarity
    let query_text = "What are user's food restrictions?";
    let query_embedding = embed(&openai, query_text).await;
    
    let results = db.query(
        &Query::new()
            .with_meaning(query_embedding, Some(0.7))
            .with_limit(5)
    )?;
    
    println!("Found {} relevant memories:", results.len());
    for result in results {
        println!("- {} ({:.1}%)", 
            result.entry.expression,
            result.similarity_score.unwrap() * 100.0
        );
    }
    
    Ok(())
}
```

Add to `Cargo.toml`:
```toml
[dependencies]
contextdb = { path = "../contextdb" }
async-openai = "0.20"
tokio = { version = "1", features = ["full"] }
serde_json = "1.0"
```

## Next Steps

1. Choose an embedding approach
2. Generate embeddings for your data
3. Store in ContextDB
4. Query using semantic similarity!

For more examples, see the `examples/` directory.

## Pitfalls
- Mixing embedding sizes in a DB will hurt similarity.

## Next steps
- See `query-language.md` for semantic search.
---

| Prev | Next |
| --- | --- |
| [Entry Lifecycle](lifecycle.md) | [Use Cases](usage.md) |
