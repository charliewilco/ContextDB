import CContextDB
import Foundation

public enum ContextDBError: Error, LocalizedError, Sendable {
	case incompatibleABIVersion(UInt32)
	case operationFailed(status: Int32, message: String)
	case missingOutput

	public var errorDescription: String? {
		switch self {
		case .incompatibleABIVersion(let version):
			return "Unsupported ContextDB ABI version: \(version)"
		case .operationFailed(_, let message):
			return message
		case .missingOutput:
			return "ContextDB returned success without output."
		}
	}
}

public final class ContextDatabase {
	private let handle: OpaquePointer
	private let lock = NSLock()
	private let encoder = JSONEncoder()
	private let decoder = JSONDecoder()

	public init(path: URL? = nil) throws {
		let version = contextdb_abi_version()
		guard version == 1 else {
			throw ContextDBError.incompatibleABIVersion(version)
		}

		let opened: OpaquePointer?
		if let path {
			opened = path.path.withCString { contextdb_open($0) }
		} else {
			opened = contextdb_open(nil)
		}
		guard let opened else {
			throw Self.lastError()
		}
		handle = opened
	}

	deinit {
		contextdb_close(handle)
	}

	@discardableResult
	public func insert(
		expression: String,
		meaning: [Float],
		context: JSONValue = .null,
		relations: [UUID] = []
	) throws -> UUID {
		try synchronized {
			let request = InsertRequest(
				expression: expression,
				meaning: meaning,
				context: context,
				relations: relations
			)
			let json = String(decoding: try encoder.encode(request), as: UTF8.self)
			let value = try json.withCString { pointer in
				return try Self.outputString { output in
					contextdb_insert_json(handle, pointer, output)
				}
			}
			guard let id = UUID(uuidString: value) else {
				throw ContextDBError.missingOutput
			}
			return id
		}
	}

	public func get(id: UUID) throws -> Entry {
		try synchronized {
			let data = try id.uuidString.withCString { idPointer in
				try Self.outputData { output in
					contextdb_get_json(handle, idPointer, output)
				}
			}
			return try decoder.decode(Entry.self, from: data)
		}
	}

	public func update(_ entry: Entry) throws {
		try synchronized {
			let json = String(decoding: try encoder.encode(entry), as: UTF8.self)
			try json.withCString { pointer in
				try Self.requireSuccess(contextdb_update_json(handle, pointer))
			}
		}
	}

	public func delete(id: UUID) throws {
		try synchronized {
			try id.uuidString.withCString { idPointer in
				try Self.requireSuccess(contextdb_delete_id(handle, idPointer))
			}
		}
	}

	public func query(_ query: Query) throws -> [QueryMatch] {
		try synchronized {
			let request = String(decoding: try encoder.encode(query), as: UTF8.self)
			let data = try request.withCString { pointer in
				return try Self.outputData { output in
					contextdb_query_json(handle, pointer, output)
				}
			}
			return try decoder.decode([QueryMatch].self, from: data)
		}
	}

	public func count() throws -> Int {
		try synchronized {
			var value = 0
			guard contextdb_count(handle, &value) else {
				throw Self.lastError()
			}
			return value
		}
	}

	private func synchronized<T>(_ operation: () throws -> T) rethrows -> T {
		lock.lock()
		defer { lock.unlock() }
		return try operation()
	}

	private static func outputData(
		_ operation: (UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32
	) throws -> Data {
		Data(try outputString(operation).utf8)
	}

	private static func outputString(
		_ operation: (UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32
	) throws -> String {
		var output: UnsafeMutablePointer<CChar>?
		try requireSuccess(operation(&output))
		guard let output else {
			throw ContextDBError.missingOutput
		}
		defer { contextdb_string_free(output) }
		return String(cString: output)
	}

	private static func requireSuccess(_ status: Int32) throws {
		guard status == CONTEXTDB_STATUS_OK else {
			throw lastError(status: status)
		}
	}

	private static func lastError(status: Int32 = contextdb_last_error_code()) -> ContextDBError {
		guard let message = contextdb_last_error_message() else {
			return .operationFailed(status: status, message: "Unknown ContextDB error")
		}
		defer { contextdb_string_free(message) }
		return .operationFailed(status: status, message: String(cString: message))
	}
}

private struct InsertRequest: Encodable {
	let expression: String
	let meaning: [Float]
	let context: JSONValue
	let relations: [UUID]
}
