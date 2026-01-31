# Usage

### 1. LLM Memory Systems

**Problem**: LLMs need to maintain user context across sessions

**Solution**: Store preferences, facts, and conversation history in ContextDB

```rust
// Store user preference
let pref = Entry::new(
    embedding_from_llm("dietary preference about gluten"),
    "User is gluten-free".to_string()
).with_context(json!({
    "type": "dietary",
    "learned_from": "conversation_id_123",
    "confidence": 0.95
}));

db.insert(&pref)?;

// LLM retrieves relevant context before responding
let context = db.query(
    &Query::new()
        .with_meaning(conversation_embedding, Some(0.7))
        .with_limit(5)
)?;

// Feed context to LLM prompt
```

### 2. RAG Systems with Explainability

**Problem**: Can't debug why specific documents were retrieved

**Solution**: Store documents with embeddings, query with explanations

```rust
// Index document
let doc = Entry::new(
    document_embedding,
    document_text.clone()
).with_context(json!({
    "source": "internal_docs",
    "author": "engineering",
    "last_updated": "2026-01-15"
}));

db.insert(&doc)?;

// Query with explanation
let results = db.query(
    &Query::new()
        .with_meaning(query_embedding, Some(0.7))
        .with_context(ContextFilter::PathEquals("/source".to_string(), json!("internal_docs")))
        .with_explanation()
)?;

// Show user why documents matched
for result in results {
    println!("Retrieved: {}", result.entry.expression);
    println!("Because: {}", result.explanation.unwrap());
}
```

### 3. Personal AI Assistants

**Problem**: Users want transparency into what AI "knows" about them

**Solution**: Provide human-readable memory inspection

```rust
// AI stores learned facts
let fact = Entry::new(
    embedding,
    "User prefers morning meetings".to_string()
).with_context(json!({
    "category": "scheduling",
    "learned_at": "2026-01-15T09:30:00Z"
}));

db.insert(&fact)?;

// User inspects their memories
let my_memories = db.query(
    &Query::new()
        .with_expression(ExpressionFilter::Contains("prefer".to_string()))
        .with_temporal(TemporalFilter::CreatedAfter(last_month))
)?;

// Display in UI: "Here's what I know about you..."
```

### 4. Multi-Agent Systems

**Problem**: Different agents need different views of shared memory

**Solution**: Each agent queries differently

```rust
// Agent 1 (Planner): Semantic retrieval
let planning_context = db.query(
    &Query::new()
        .with_meaning(task_embedding, Some(0.8))
        .with_context(ContextFilter::PathEquals("/priority".to_string(), json!("high")))
)?;

// Agent 2 (Executor): Structured query
let tasks = db.query(
    &Query::new()
        .with_context(ContextFilter::And(vec![
            ContextFilter::PathEquals("/status".to_string(), json!("pending")),
            ContextFilter::PathEquals("/assigned_to".to_string(), json!("executor"))
        ]))
)?;

// Human supervisor: Natural language inspection
let overview = db.query(
    &Query::new()
        .with_expression(ExpressionFilter::Contains("urgent".to_string()))
        .with_temporal(TemporalFilter::CreatedAfter(today))
)?;
```

### 5. Debugging AI Applications

**Problem**: "Why did the AI do that?"

**Solution**: Inspect what memories/context it retrieved

```rust
// AI made unexpected decision
// Developer investigates:

// What did it retrieve?
let retrieved = db.query(
    &Query::new()
        .with_expression(ExpressionFilter::Contains("decision keyword".to_string()))
        .with_explanation()
)?;

// What memories exist about this topic?
let related = db.query(
    &Query::new()
        .with_meaning(topic_embedding, Some(0.6))
        .with_context(ContextFilter::PathExists("/decision_factor".to_string()))
)?;

// When were these memories created?
let timeline = db.query(
    &Query::new()
        .with_context(ContextFilter::PathEquals("/topic".to_string(), json!("the topic")))
        .with_temporal(TemporalFilter::CreatedBetween(start, end))
)?;
```

---
