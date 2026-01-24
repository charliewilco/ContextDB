import Foundation
import SwiftUI

public struct ContentView: View {
    @Environment(\.contextdbClient) private var client
    @State private var expression = "User likes espresso"
    @State private var embeddingText = "0.12, 0.98, 0.33"
    @State private var containsText = "espresso"
    @State private var thresholdText = "0.6"
    @State private var limit = 10
    @State private var results: [ContextDBQueryItem] = []
    @State private var statusMessage = "Ready"
    @State private var entryCount: Int?

    public init() {}

    public var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    statusSection
                    insertSection
                    querySection
                    resultsSection
                }
                .padding(16)
            }
            .navigationTitle("ContextDB Demo")
        }
    }

    private var statusSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Status")
                .font(.headline)
            Text(statusMessage)
                .font(.subheadline)
                .foregroundStyle(.secondary)
            if let entryCount {
                Text("Entries: \(entryCount)")
                    .font(.subheadline)
            }
            Button("Refresh Count", action: refreshCount)
                .buttonStyle(.bordered)
        }
    }

    private var insertSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Insert")
                .font(.headline)
            TextField("Expression", text: $expression)
                .textFieldStyle(.roundedBorder)
            TextField("Embedding (comma-separated)", text: $embeddingText)
                .textFieldStyle(.roundedBorder)
            Button("Insert Entry", action: insertEntry)
                .buttonStyle(.borderedProminent)
        }
    }

    private var querySection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Query")
                .font(.headline)
            TextField("Contains text", text: $containsText)
                .textFieldStyle(.roundedBorder)
            HStack {
                TextField("Similarity threshold", text: $thresholdText)
                    .textFieldStyle(.roundedBorder)
                    .keyboardType(.decimalPad)
                Stepper("Limit \(limit)", value: $limit, in: 1...50)
            }
            HStack {
                Button("Query Meaning", action: queryMeaning)
                    .buttonStyle(.bordered)
                Button("Query Contains", action: queryContains)
                    .buttonStyle(.bordered)
            }
        }
    }

    private var resultsSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Results")
                .font(.headline)
            if results.isEmpty {
                Text("No results yet.")
                    .foregroundStyle(.secondary)
            } else {
                ForEach(results) { result in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(result.expression)
                            .font(.body)
                        Text("Score: \(result.score, specifier: "%.3f")")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Text("ID: \(result.id)")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                    .padding(8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color(.secondarySystemBackground))
                    .clipShape(RoundedRectangle(cornerRadius: 10))
                }
            }
        }
    }

    private func insertEntry() {
        do {
            let embedding = try parseEmbedding()
            try client.insert(expression, embedding)
            statusMessage = "Inserted entry."
            refreshCount()
        } catch {
            statusMessage = "Insert failed: \(error.localizedDescription)"
        }
    }

    private func queryMeaning() {
        do {
            let embedding = try parseEmbedding()
            let threshold = Float(thresholdText.trimmingCharacters(in: .whitespacesAndNewlines))
            results = try client.queryByMeaning(embedding, threshold, limit)
            statusMessage = "Query returned \(results.count) entries."
        } catch {
            statusMessage = "Query failed: \(error.localizedDescription)"
        }
    }

    private func queryContains() {
        do {
            results = try client.queryByExpressionContains(containsText, limit)
            statusMessage = "Query returned \(results.count) entries."
        } catch {
            statusMessage = "Query failed: \(error.localizedDescription)"
        }
    }

    private func refreshCount() {
        do {
            entryCount = try client.count()
        } catch {
            statusMessage = "Count failed: \(error.localizedDescription)"
        }
    }

    private func parseEmbedding() throws -> [Float] {
        let parts = embeddingText.split(separator: ",")
        let values = parts.compactMap { part -> Float? in
            Float(part.trimmingCharacters(in: .whitespacesAndNewlines))
        }
        if values.isEmpty {
            throw NSError(domain: "ContextDB", code: 1, userInfo: [NSLocalizedDescriptionKey: "Embedding is empty or invalid."])
        }
        return values
    }
}
