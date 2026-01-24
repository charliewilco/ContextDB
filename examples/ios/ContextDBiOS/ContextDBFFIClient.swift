import Foundation
import ContextDBiOSFeature

enum ContextDBFFIError: LocalizedError {
    case openFailed(String)
    case operationFailed(String)

    var errorDescription: String? {
        switch self {
        case .openFailed(let message):
            return message
        case .operationFailed(let message):
            return message
        }
    }
}

final class ContextDBStore {
    private let handle: UnsafeMutablePointer<ContextDBHandle>

    init() throws {
        let dbURL = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)
            .first
            .map { $0.appendingPathComponent("contextdb.sqlite") }
        let path = dbURL?.path ?? ""
        let handle = path.withCString { contextdb_open($0) }
        guard let handle else {
            throw ContextDBFFIError.openFailed(Self.lastErrorMessage())
        }
        self.handle = handle
    }

    deinit {
        contextdb_close(handle)
    }

    func insert(expression: String, embedding: [Float]) throws {
        let success = embedding.withUnsafeBufferPointer { buffer in
            expression.withCString { cExpression in
                contextdb_insert(handle, cExpression, buffer.baseAddress, buffer.count)
            }
        }
        guard success else {
            throw ContextDBFFIError.operationFailed(Self.lastErrorMessage())
        }
    }

    func queryByMeaning(embedding: [Float], threshold: Float?, limit: Int) throws -> [ContextDBQueryItem] {
        var outLen: Int = 0
        let resultsPointer = embedding.withUnsafeBufferPointer { buffer in
            contextdb_query_meaning(
                handle,
                buffer.baseAddress,
                buffer.count,
                threshold ?? -1,
                limit,
                &outLen
            )
        }
        guard let resultsPointer else {
            throw ContextDBFFIError.operationFailed(Self.lastErrorMessage())
        }
        defer { contextdb_query_results_free(resultsPointer, outLen) }

        let buffer = UnsafeBufferPointer(start: resultsPointer, count: outLen)
        return buffer.map { result in
            let idString = Self.hexString(from: result.id)
            let expression = result.expression.map { String(cString: $0) } ?? ""
            return ContextDBQueryItem(id: idString, score: result.score, expression: expression)
        }
    }

    func queryByExpressionContains(text: String, limit: Int) throws -> [ContextDBQueryItem] {
        var outLen: Int = 0
        let resultsPointer = text.withCString { cText in
            contextdb_query_expression_contains(handle, cText, limit, &outLen)
        }
        guard let resultsPointer else {
            throw ContextDBFFIError.operationFailed(Self.lastErrorMessage())
        }
        defer { contextdb_query_results_free(resultsPointer, outLen) }

        let buffer = UnsafeBufferPointer(start: resultsPointer, count: outLen)
        return buffer.map { result in
            let idString = Self.hexString(from: result.id)
            let expression = result.expression.map { String(cString: $0) } ?? ""
            return ContextDBQueryItem(id: idString, score: result.score, expression: expression)
        }
    }

    func count() throws -> Int {
        var outCount: Int = 0
        let success = contextdb_count(handle, &outCount)
        guard success else {
            throw ContextDBFFIError.operationFailed(Self.lastErrorMessage())
        }
        return outCount
    }

    private static func lastErrorMessage() -> String {
        guard let cString = contextdb_last_error_message() else {
            return "Unknown error"
        }
        defer { contextdb_string_free(cString) }
        return String(cString: cString)
    }

    private static func hexString(from id: (UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8, UInt8)) -> String {
        withUnsafeBytes(of: id) { rawBuffer in
            rawBuffer.map { String(format: "%02x", $0) }.joined()
        }
    }
}

extension ContextDBClient {
    static func live() -> ContextDBClient {
        let store = try? ContextDBStore()
        return ContextDBClient(
            insert: { expression, embedding in
                guard let store else { throw ContextDBClientError.notConfigured }
                try store.insert(expression: expression, embedding: embedding)
            },
            queryByMeaning: { embedding, threshold, limit in
                guard let store else { throw ContextDBClientError.notConfigured }
                return try store.queryByMeaning(embedding: embedding, threshold: threshold, limit: limit)
            },
            queryByExpressionContains: { text, limit in
                guard let store else { throw ContextDBClientError.notConfigured }
                return try store.queryByExpressionContains(text: text, limit: limit)
            },
            count: {
                guard let store else { throw ContextDBClientError.notConfigured }
                return try store.count()
            }
        )
    }
}
