import Foundation
import SwiftUI

public struct ContextDBQueryItem: Identifiable, Hashable {
    public let id: String
    public let score: Float
    public let expression: String

    public init(id: String, score: Float, expression: String) {
        self.id = id
        self.score = score
        self.expression = expression
    }
}

public enum ContextDBClientError: LocalizedError, Hashable {
    case notConfigured

    public var errorDescription: String? {
        switch self {
        case .notConfigured:
            return "ContextDB client is not configured."
        }
    }
}

public struct ContextDBClient {
    public var insert: (_ expression: String, _ embedding: [Float]) throws -> Void
    public var queryByMeaning: (_ embedding: [Float], _ threshold: Float?, _ limit: Int) throws -> [ContextDBQueryItem]
    public var queryByExpressionContains: (_ text: String, _ limit: Int) throws -> [ContextDBQueryItem]
    public var count: () throws -> Int

    public init(
        insert: @escaping (_ expression: String, _ embedding: [Float]) throws -> Void,
        queryByMeaning: @escaping (_ embedding: [Float], _ threshold: Float?, _ limit: Int) throws -> [ContextDBQueryItem],
        queryByExpressionContains: @escaping (_ text: String, _ limit: Int) throws -> [ContextDBQueryItem],
        count: @escaping () throws -> Int
    ) {
        self.insert = insert
        self.queryByMeaning = queryByMeaning
        self.queryByExpressionContains = queryByExpressionContains
        self.count = count
    }
}

public struct ContextDBClientKey: EnvironmentKey {
    public static let defaultValue = ContextDBClient(
        insert: { _, _ in throw ContextDBClientError.notConfigured },
        queryByMeaning: { _, _, _ in throw ContextDBClientError.notConfigured },
        queryByExpressionContains: { _, _ in throw ContextDBClientError.notConfigured },
        count: { throw ContextDBClientError.notConfigured }
    )
}

public extension EnvironmentValues {
    var contextdbClient: ContextDBClient {
        get { self[ContextDBClientKey.self] }
        set { self[ContextDBClientKey.self] = newValue }
    }
}
