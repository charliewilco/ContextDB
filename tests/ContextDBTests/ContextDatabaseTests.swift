import Foundation
import Testing
@testable import ContextDB

@Test
func crudAndCompoundQueryRoundTrip() throws {
	let database = try ContextDatabase()
	let id = try database.insert(
		expression: "Swift package entry",
		meaning: [1, 0],
		context: .object(["source": .string("swift")])
	)

	var entry = try database.get(id: id)
	#expect(entry.expression == "Swift package entry")
	#expect(entry.context == .object(["source": .string("swift")]))

	entry.expression = "Updated Swift package entry"
	entry.updatedAt = ISO8601DateFormatter().string(from: Date().addingTimeInterval(1))
	try database.update(entry)

	let matches = try database.query(
		Query(
			meaning: MeaningFilter(vector: [1, 0]),
			expression: .fullText("updated"),
			context: .pathEquals("/source", .string("swift")),
			hybridWeights: HybridWeights(semantic: 0.7, lexical: 0.3),
			explain: true
		)
	)
	#expect(matches.count == 1)
	#expect(matches[0].entry.id == id)
	#expect(matches[0].plan?.backend == "SQLite")

	try database.delete(id: id)
	#expect(try database.count() == 0)
}

@Test
func relationAndStructuredContextRoundTrip() throws {
	let database = try ContextDatabase()
	let targetID = try database.insert(
		expression: "Relation target",
		meaning: [0, 1],
		context: .object([
			"metadata": .object([
				"enabled": .bool(true),
				"count": .number(2),
			]),
		])
	)
	let sourceID = try database.insert(
		expression: "Relation source",
		meaning: [1, 0],
		relations: [targetID]
	)

	let related = try database.query(
		Query(relations: .directlyRelatedTo(sourceID))
	)
	#expect(related.map(\.entry.id) == [targetID])

	let nested = try database.query(
		Query(context: .pathEquals("/metadata/enabled", .bool(true)))
	)
	#expect(nested.map(\.entry.id) == [targetID])
	#expect(
		nested[0].entry.context == .object([
			"metadata": .object([
				"enabled": .bool(true),
				"count": .number(2),
			]),
		])
	)
}

@Test
func missingEntryPreservesStructuredError() throws {
	let database = try ContextDatabase()

	#expect(throws: ContextDBError.self) {
		_ = try database.get(id: UUID())
	}
}
