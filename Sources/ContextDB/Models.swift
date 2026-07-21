import Foundation

public struct Entry: Codable, Equatable, Identifiable, Sendable {
	public let id: UUID
	public var meaning: [Float]
	public var expression: String
	public var context: JSONValue
	public let createdAt: String
	public var updatedAt: String
	public var relations: [UUID]

	public init(
		id: UUID,
		meaning: [Float],
		expression: String,
		context: JSONValue,
		createdAt: String,
		updatedAt: String,
		relations: [UUID]
	) {
		self.id = id
		self.meaning = meaning
		self.expression = expression
		self.context = context
		self.createdAt = createdAt
		self.updatedAt = updatedAt
		self.relations = relations
	}

	private enum CodingKeys: String, CodingKey {
		case id
		case meaning
		case expression
		case context
		case createdAt = "created_at"
		case updatedAt = "updated_at"
		case relations
	}
}

public struct QueryMatch: Decodable, Sendable {
	public let entry: Entry
	public let similarityScore: Float?
	public let lexicalScore: Float?
	public let combinedScore: Float?
	public let explanation: String?
	public let plan: QueryPlan?

	private enum CodingKeys: String, CodingKey {
		case entry
		case similarityScore = "similarity_score"
		case lexicalScore = "lexical_score"
		case combinedScore = "combined_score"
		case explanation
		case plan
	}
}

public struct QueryPlan: Decodable, Sendable {
	public let backend: String
	public let candidateFilters: [String]
	public let ranking: String
	public let candidatesLoaded: Int
	public let matchesBeforePagination: Int

	private enum CodingKeys: String, CodingKey {
		case backend
		case candidateFilters = "candidate_filters"
		case ranking
		case candidatesLoaded = "candidates_loaded"
		case matchesBeforePagination = "matches_before_pagination"
	}
}
