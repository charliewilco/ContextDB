import Foundation

public struct Query: Encodable, Sendable {
	public var meaning: MeaningFilter?
	public var expression: ExpressionFilter?
	public var context: ContextFilter?
	public var relations: RelationFilter?
	public var temporal: TemporalFilter?
	public var limit: Int?
	public var offset: Int
	public var cursor: QueryCursor?
	public var order: QueryOrder
	public var hybridWeights: HybridWeights?
	public var explain: Bool

	public init(
		meaning: MeaningFilter? = nil,
		expression: ExpressionFilter? = nil,
		context: ContextFilter? = nil,
		relations: RelationFilter? = nil,
		temporal: TemporalFilter? = nil,
		limit: Int? = nil,
		offset: Int = 0,
		cursor: QueryCursor? = nil,
		order: QueryOrder = .createdAtAscending,
		hybridWeights: HybridWeights? = nil,
		explain: Bool = false
	) {
		self.meaning = meaning
		self.expression = expression
		self.context = context
		self.relations = relations
		self.temporal = temporal
		self.limit = limit
		self.offset = offset
		self.cursor = cursor
		self.order = order
		self.hybridWeights = hybridWeights
		self.explain = explain
	}

	private enum CodingKeys: String, CodingKey {
		case meaning
		case expression
		case context
		case relations
		case temporal
		case limit
		case offset
		case cursor
		case order
		case hybridWeights = "hybrid_weights"
		case explain
	}
}

public struct MeaningFilter: Encodable, Sendable {
	public var vector: [Float]
	public var threshold: Float?
	public var topK: Int?

	public init(vector: [Float], threshold: Float? = nil, topK: Int? = nil) {
		self.vector = vector
		self.threshold = threshold
		self.topK = topK
	}

	private enum CodingKeys: String, CodingKey {
		case vector
		case threshold
		case topK = "top_k"
	}
}

public enum ExpressionFilter: Encodable, Sendable {
	case equals(String)
	case contains(String)
	case startsWith(String)
	case matches(String)
	case fullText(String)

	public func encode(to encoder: Encoder) throws {
		let value: JSONValue
		switch self {
		case .equals(let text):
			value = .object(["Equals": .string(text)])
		case .contains(let text):
			value = .object(["Contains": .string(text)])
		case .startsWith(let text):
			value = .object(["StartsWith": .string(text)])
		case .matches(let pattern):
			value = .object(["Matches": .string(pattern)])
		case .fullText(let query):
			value = .object(["FullText": .string(query)])
		}
		try value.encode(to: encoder)
	}
}

public indirect enum ContextFilter: Encodable, Sendable {
	case pathExists(String)
	case pathEquals(String, JSONValue)
	case pathContains(String, JSONValue)
	case and([ContextFilter])
	case or([ContextFilter])

	public func encode(to encoder: Encoder) throws {
		try jsonValue().encode(to: encoder)
	}

	private func jsonValue() throws -> JSONValue {
		switch self {
		case .pathExists(let path):
			return .object(["PathExists": .string(path)])
		case .pathEquals(let path, let expected):
			return .object(["PathEquals": .array([.string(path), expected])])
		case .pathContains(let path, let expected):
			return .object(["PathContains": .array([.string(path), expected])])
		case .and(let filters):
			return .object(["And": .array(try filters.map { try $0.jsonValue() })])
		case .or(let filters):
			return .object(["Or": .array(try filters.map { try $0.jsonValue() })])
		}
	}
}

public enum RelationFilter: Encodable, Sendable {
	case directlyRelatedTo(UUID)
	case withinDistance(from: UUID, maxHops: Int)
	case hasRelations
	case noRelations

	public func encode(to encoder: Encoder) throws {
		let value: JSONValue
		switch self {
		case .directlyRelatedTo(let id):
			value = .object(["DirectlyRelatedTo": .string(id.uuidString.lowercased())])
		case .withinDistance(let from, let maxHops):
			value = .object([
				"WithinDistance": .object([
					"from": .string(from.uuidString.lowercased()),
					"max_hops": .number(Double(maxHops)),
				]),
			])
		case .hasRelations:
			value = .string("HasRelations")
		case .noRelations:
			value = .string("NoRelations")
		}
		try value.encode(to: encoder)
	}
}

public enum TemporalFilter: Encodable, Sendable {
	case createdAfter(String)
	case createdBefore(String)
	case createdBetween(String, String)
	case updatedAfter(String)
	case updatedBefore(String)

	public func encode(to encoder: Encoder) throws {
		let value: JSONValue
		switch self {
		case .createdAfter(let timestamp):
			value = .object(["CreatedAfter": .string(timestamp)])
		case .createdBefore(let timestamp):
			value = .object(["CreatedBefore": .string(timestamp)])
		case .createdBetween(let start, let end):
			value = .object(["CreatedBetween": .array([.string(start), .string(end)])])
		case .updatedAfter(let timestamp):
			value = .object(["UpdatedAfter": .string(timestamp)])
		case .updatedBefore(let timestamp):
			value = .object(["UpdatedBefore": .string(timestamp)])
		}
		try value.encode(to: encoder)
	}
}

public struct QueryCursor: Encodable, Sendable {
	public let after: UUID

	public init(after: UUID) {
		self.after = after
	}
}

public struct HybridWeights: Encodable, Sendable {
	public let semantic: Float
	public let lexical: Float

	public init(semantic: Float, lexical: Float) {
		self.semantic = semantic
		self.lexical = lexical
	}
}

public enum QueryOrder: String, Encodable, Sendable {
	case createdAtAscending = "CreatedAtAsc"
	case createdAtDescending = "CreatedAtDesc"
	case updatedAtAscending = "UpdatedAtAsc"
	case updatedAtDescending = "UpdatedAtDesc"
	case expressionAscending = "ExpressionAsc"
	case expressionDescending = "ExpressionDesc"
}
