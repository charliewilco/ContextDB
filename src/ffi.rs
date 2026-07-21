use crate::{ContextDB, Entry, ExpressionFilter, Query};
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

#[repr(C)]
pub struct ContextDBHandle {
	db: ContextDB,
}

#[repr(C)]
pub struct ContextDBQueryResult {
	pub id: [u8; 16],
	pub score: f32,
	pub expression: *mut c_char,
}

thread_local! {
	static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
	static LAST_ERROR_CODE: RefCell<i32> = const { RefCell::new(CONTEXTDB_STATUS_OK) };
}

pub const CONTEXTDB_STATUS_OK: i32 = 0;
pub const CONTEXTDB_STATUS_INVALID_ARGUMENT: i32 = 1;
pub const CONTEXTDB_STATUS_NOT_FOUND: i32 = 2;
pub const CONTEXTDB_STATUS_DATABASE: i32 = 3;
pub const CONTEXTDB_STATUS_PANIC: i32 = 4;

#[derive(serde::Deserialize)]
struct InsertRequest {
	expression: String,
	meaning: Vec<f32>,
	#[serde(default)]
	context: serde_json::Value,
	#[serde(default)]
	relations: Vec<uuid::Uuid>,
}

fn set_last_error(message: impl ToString) {
	set_last_error_with_code(CONTEXTDB_STATUS_DATABASE, message);
}

fn set_invalid_argument(message: impl ToString) {
	set_last_error_with_code(CONTEXTDB_STATUS_INVALID_ARGUMENT, message);
}

fn set_storage_error(error: crate::StorageError) {
	let (code, message) = storage_status(error);
	set_last_error_with_code(code, message);
}

fn set_last_error_with_code(code: i32, message: impl ToString) {
	let message = CString::new(message.to_string())
		.unwrap_or_else(|_| CString::new("unknown error").unwrap());
	LAST_ERROR.with(|cell| {
		*cell.borrow_mut() = Some(message);
	});
	LAST_ERROR_CODE.with(|cell| *cell.borrow_mut() = code);
}

fn clear_last_error() {
	LAST_ERROR.with(|cell| {
		*cell.borrow_mut() = None;
	});
	LAST_ERROR_CODE.with(|cell| *cell.borrow_mut() = CONTEXTDB_STATUS_OK);
}

fn status_guard(operation: impl FnOnce() -> Result<(), (i32, String)>) -> i32 {
	match catch_unwind(AssertUnwindSafe(operation)) {
		Ok(Ok(())) => {
			clear_last_error();
			CONTEXTDB_STATUS_OK
		}
		Ok(Err((code, message))) => {
			set_last_error_with_code(code, message);
			code
		}
		Err(_) => {
			set_last_error_with_code(CONTEXTDB_STATUS_PANIC, "Rust panic crossed FFI operation");
			CONTEXTDB_STATUS_PANIC
		}
	}
}

fn value_guard<T>(fallback: T, operation: impl FnOnce() -> T) -> T {
	match catch_unwind(AssertUnwindSafe(operation)) {
		Ok(value) => value,
		Err(_) => {
			set_last_error_with_code(CONTEXTDB_STATUS_PANIC, "Rust panic crossed FFI operation");
			fallback
		}
	}
}

fn storage_status(error: crate::StorageError) -> (i32, String) {
	let code = match &error {
		crate::StorageError::NotFound(_) => CONTEXTDB_STATUS_NOT_FOUND,
		crate::StorageError::InvalidDimensions | crate::StorageError::InvalidArgument(_) => {
			CONTEXTDB_STATUS_INVALID_ARGUMENT
		}
		crate::StorageError::Database(_)
		| crate::StorageError::Serialization(_)
		| crate::StorageError::Backend(_) => CONTEXTDB_STATUS_DATABASE,
	};
	(code, error.to_string())
}

fn cstr_to_string(ptr: *const c_char, field: &str) -> Result<String, String> {
	if ptr.is_null() {
		return Err(format!("{field} pointer was null"));
	}
	unsafe { CStr::from_ptr(ptr) }
		.to_str()
		.map(|s| s.to_string())
		.map_err(|_| format!("{field} was not valid UTF-8"))
}

fn cstring_from_string(value: String, field: &str) -> Result<CString, String> {
	CString::new(value).map_err(|_| format!("{field} contained interior NUL"))
}

#[no_mangle]
pub extern "C" fn contextdb_last_error_message() -> *mut c_char {
	LAST_ERROR.with(|cell| match &*cell.borrow() {
		Some(message) => message.clone().into_raw(),
		None => ptr::null_mut(),
	})
}

#[no_mangle]
pub extern "C" fn contextdb_abi_version() -> u32 {
	1
}

#[no_mangle]
pub extern "C" fn contextdb_last_error_code() -> i32 {
	LAST_ERROR_CODE.with(|cell| *cell.borrow())
}

/// Write an allocated C string to an output pointer.
unsafe fn write_output_string(
	output: *mut *mut c_char,
	value: String,
	field: &str,
) -> Result<(), (i32, String)> {
	if output.is_null() {
		return Err((
			CONTEXTDB_STATUS_INVALID_ARGUMENT,
			format!("{field} pointer was null"),
		));
	}
	let value = cstring_from_string(value, field)
		.map_err(|message| (CONTEXTDB_STATUS_INVALID_ARGUMENT, message))?;
	*output = value.into_raw();
	Ok(())
}

#[no_mangle]
/// Insert an entry described by JSON and return its generated UUID string.
///
/// # Safety
/// `handle` must be valid, `json` must be a valid C string, and `out_id` must
/// point to writable storage for a C string pointer.
pub unsafe extern "C" fn contextdb_insert_json(
	handle: *mut ContextDBHandle,
	json: *const c_char,
	out_id: *mut *mut c_char,
) -> i32 {
	status_guard(|| {
		if handle.is_null() {
			return Err((
				CONTEXTDB_STATUS_INVALID_ARGUMENT,
				"handle was null".to_string(),
			));
		}
		if out_id.is_null() {
			return Err((
				CONTEXTDB_STATUS_INVALID_ARGUMENT,
				"out_id pointer was null".to_string(),
			));
		}
		*out_id = ptr::null_mut();
		let json = cstr_to_string(json, "json")
			.map_err(|message| (CONTEXTDB_STATUS_INVALID_ARGUMENT, message))?;
		let request: InsertRequest = serde_json::from_str(&json)
			.map_err(|error| (CONTEXTDB_STATUS_INVALID_ARGUMENT, error.to_string()))?;
		let mut entry =
			Entry::new(request.meaning, request.expression).with_context(request.context);
		for relation in request.relations {
			entry = entry.add_relation(relation);
		}
		(&mut *handle).db.insert(&entry).map_err(storage_status)?;
		write_output_string(out_id, entry.id.to_string(), "out_id")
	})
}

#[no_mangle]
/// Get an entry as JSON by UUID.
///
/// # Safety
/// All pointers must be valid for their documented C representations.
pub unsafe extern "C" fn contextdb_get_json(
	handle: *const ContextDBHandle,
	id: *const c_char,
	out_json: *mut *mut c_char,
) -> i32 {
	status_guard(|| {
		if handle.is_null() {
			return Err((
				CONTEXTDB_STATUS_INVALID_ARGUMENT,
				"handle was null".to_string(),
			));
		}
		let id = cstr_to_string(id, "id")
			.map_err(|message| (CONTEXTDB_STATUS_INVALID_ARGUMENT, message))?;
		let id = uuid::Uuid::parse_str(&id)
			.map_err(|error| (CONTEXTDB_STATUS_INVALID_ARGUMENT, error.to_string()))?;
		let entry = (&*handle).db.get(id).map_err(storage_status)?;
		let json = serde_json::to_string(&entry)
			.map_err(|error| (CONTEXTDB_STATUS_DATABASE, error.to_string()))?;
		write_output_string(out_json, json, "out_json")
	})
}

#[no_mangle]
/// Update an entry from its complete JSON representation.
///
/// # Safety
/// `handle` and `json` must be valid pointers.
pub unsafe extern "C" fn contextdb_update_json(
	handle: *mut ContextDBHandle,
	json: *const c_char,
) -> i32 {
	status_guard(|| {
		if handle.is_null() {
			return Err((
				CONTEXTDB_STATUS_INVALID_ARGUMENT,
				"handle was null".to_string(),
			));
		}
		let json = cstr_to_string(json, "json")
			.map_err(|message| (CONTEXTDB_STATUS_INVALID_ARGUMENT, message))?;
		let entry: Entry = serde_json::from_str(&json)
			.map_err(|error| (CONTEXTDB_STATUS_INVALID_ARGUMENT, error.to_string()))?;
		(&mut *handle).db.update(&entry).map_err(storage_status)
	})
}

#[no_mangle]
/// Delete an entry by UUID.
///
/// # Safety
/// `handle` and `id` must be valid pointers.
pub unsafe extern "C" fn contextdb_delete_id(
	handle: *mut ContextDBHandle,
	id: *const c_char,
) -> i32 {
	status_guard(|| {
		if handle.is_null() {
			return Err((
				CONTEXTDB_STATUS_INVALID_ARGUMENT,
				"handle was null".to_string(),
			));
		}
		let id = cstr_to_string(id, "id")
			.map_err(|message| (CONTEXTDB_STATUS_INVALID_ARGUMENT, message))?;
		let id = uuid::Uuid::parse_str(&id)
			.map_err(|error| (CONTEXTDB_STATUS_INVALID_ARGUMENT, error.to_string()))?;
		(&mut *handle).db.delete(id).map_err(storage_status)
	})
}

#[no_mangle]
/// Execute a serialized `Query` and return serialized `QueryResult` values.
///
/// # Safety
/// All pointers must be valid for their documented C representations.
pub unsafe extern "C" fn contextdb_query_json(
	handle: *const ContextDBHandle,
	json: *const c_char,
	out_json: *mut *mut c_char,
) -> i32 {
	status_guard(|| {
		if handle.is_null() {
			return Err((
				CONTEXTDB_STATUS_INVALID_ARGUMENT,
				"handle was null".to_string(),
			));
		}
		let json = cstr_to_string(json, "json")
			.map_err(|message| (CONTEXTDB_STATUS_INVALID_ARGUMENT, message))?;
		let query: Query = serde_json::from_str(&json)
			.map_err(|error| (CONTEXTDB_STATUS_INVALID_ARGUMENT, error.to_string()))?;
		let results = (&*handle).db.query(&query).map_err(storage_status)?;
		let json = serde_json::to_string(&results)
			.map_err(|error| (CONTEXTDB_STATUS_DATABASE, error.to_string()))?;
		write_output_string(out_json, json, "out_json")
	})
}

#[no_mangle]
/// # Safety
/// `ptr` must be a valid pointer returned by `contextdb_last_error_message` or
/// other ContextDB FFI functions that allocate C strings, and must not be freed
/// more than once.
pub unsafe extern "C" fn contextdb_string_free(ptr: *mut c_char) {
	if ptr.is_null() {
		return;
	}
	drop(CString::from_raw(ptr));
}

#[no_mangle]
pub extern "C" fn contextdb_open(path: *const c_char) -> *mut ContextDBHandle {
	value_guard(ptr::null_mut(), || contextdb_open_impl(path))
}

fn contextdb_open_impl(path: *const c_char) -> *mut ContextDBHandle {
	let db = if path.is_null() {
		ContextDB::in_memory()
	} else {
		match cstr_to_string(path, "path") {
			Ok(path) if path.is_empty() => ContextDB::in_memory(),
			Ok(path) => ContextDB::new(path),
			Err(message) => {
				set_invalid_argument(message);
				return ptr::null_mut();
			}
		}
	};

	match db {
		Ok(db) => {
			clear_last_error();
			Box::into_raw(Box::new(ContextDBHandle { db }))
		}
		Err(err) => {
			set_storage_error(err);
			ptr::null_mut()
		}
	}
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by `contextdb_open` and must not be
/// used after it is closed.
pub unsafe extern "C" fn contextdb_close(handle: *mut ContextDBHandle) {
	if handle.is_null() {
		return;
	}
	drop(Box::from_raw(handle));
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by `contextdb_open`.
/// If `meaning_len` is greater than zero, `meaning_ptr` must be a valid pointer
/// to an array of `meaning_len` `f32` values.
pub unsafe extern "C" fn contextdb_insert(
	handle: *mut ContextDBHandle,
	expression: *const c_char,
	meaning_ptr: *const f32,
	meaning_len: usize,
) -> bool {
	value_guard(false, || {
		contextdb_insert_impl(handle, expression, meaning_ptr, meaning_len)
	})
}

unsafe fn contextdb_insert_impl(
	handle: *mut ContextDBHandle,
	expression: *const c_char,
	meaning_ptr: *const f32,
	meaning_len: usize,
) -> bool {
	if handle.is_null() {
		set_invalid_argument("handle was null");
		return false;
	}
	if meaning_ptr.is_null() && meaning_len > 0 {
		set_invalid_argument("meaning pointer was null");
		return false;
	}

	let expression = match cstr_to_string(expression, "expression") {
		Ok(value) => value,
		Err(message) => {
			set_invalid_argument(message);
			return false;
		}
	};

	let meaning = if meaning_len == 0 {
		Vec::new()
	} else {
		unsafe { std::slice::from_raw_parts(meaning_ptr, meaning_len) }.to_vec()
	};

	let entry = Entry::new(meaning, expression);
	match (&mut *handle).db.insert(&entry) {
		Ok(()) => {
			clear_last_error();
			true
		}
		Err(err) => {
			set_storage_error(err);
			false
		}
	}
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by `contextdb_open`.
/// `out_count` must be a valid, writable pointer to a `usize`.
pub unsafe extern "C" fn contextdb_count(
	handle: *const ContextDBHandle,
	out_count: *mut usize,
) -> bool {
	value_guard(false, || contextdb_count_impl(handle, out_count))
}

unsafe fn contextdb_count_impl(handle: *const ContextDBHandle, out_count: *mut usize) -> bool {
	if handle.is_null() {
		set_invalid_argument("handle was null");
		return false;
	}
	if out_count.is_null() {
		set_invalid_argument("out_count pointer was null");
		return false;
	}

	match (&*handle).db.count() {
		Ok(count) => {
			*out_count = count;
			clear_last_error();
			true
		}
		Err(err) => {
			set_storage_error(err);
			false
		}
	}
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by `contextdb_open`.
/// If `meaning_len` is greater than zero, `meaning_ptr` must be a valid pointer
/// to an array of `meaning_len` `f32` values.
/// `out_len` must be a valid, writable pointer to a `usize`.
pub unsafe extern "C" fn contextdb_query_meaning(
	handle: *const ContextDBHandle,
	meaning_ptr: *const f32,
	meaning_len: usize,
	threshold: f32,
	limit: usize,
	out_len: *mut usize,
) -> *mut ContextDBQueryResult {
	value_guard(ptr::null_mut(), || {
		contextdb_query_meaning_impl(handle, meaning_ptr, meaning_len, threshold, limit, out_len)
	})
}

unsafe fn contextdb_query_meaning_impl(
	handle: *const ContextDBHandle,
	meaning_ptr: *const f32,
	meaning_len: usize,
	threshold: f32,
	limit: usize,
	out_len: *mut usize,
) -> *mut ContextDBQueryResult {
	if handle.is_null() {
		set_invalid_argument("handle was null");
		return ptr::null_mut();
	}
	if meaning_ptr.is_null() && meaning_len > 0 {
		set_invalid_argument("meaning pointer was null");
		return ptr::null_mut();
	}
	if out_len.is_null() {
		set_invalid_argument("out_len pointer was null");
		return ptr::null_mut();
	}

	let meaning = if meaning_len == 0 {
		Vec::new()
	} else {
		std::slice::from_raw_parts(meaning_ptr, meaning_len).to_vec()
	};

	let threshold = if threshold < 0.0 {
		None
	} else {
		Some(threshold)
	};
	let mut query = Query::new().with_meaning(meaning, threshold);
	if limit > 0 {
		query = query.with_limit(limit);
	}

	let results = match (&*handle).db.query(&query) {
		Ok(results) => results,
		Err(err) => {
			set_storage_error(err);
			return ptr::null_mut();
		}
	};

	let mut out: Vec<ContextDBQueryResult> = Vec::with_capacity(results.len());
	for result in results {
		let expression = match cstring_from_string(result.entry.expression, "expression") {
			Ok(value) => value.into_raw(),
			Err(message) => {
				for item in out.drain(..) {
					contextdb_string_free(item.expression);
				}
				set_last_error(message);
				return ptr::null_mut();
			}
		};

		let mut id = [0u8; 16];
		id.copy_from_slice(result.entry.id.as_bytes());
		out.push(ContextDBQueryResult {
			id,
			score: result.similarity_score.unwrap_or(0.0),
			expression,
		});
	}

	let mut boxed = out.into_boxed_slice();
	let len = boxed.len();
	let ptr = boxed.as_mut_ptr();
	std::mem::forget(boxed);

	*out_len = len;
	clear_last_error();
	ptr
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by `contextdb_open`.
/// `expression` must be a valid, null-terminated C string.
/// `out_len` must be a valid, writable pointer to a `usize`.
pub unsafe extern "C" fn contextdb_query_expression_contains(
	handle: *const ContextDBHandle,
	expression: *const c_char,
	limit: usize,
	out_len: *mut usize,
) -> *mut ContextDBQueryResult {
	value_guard(ptr::null_mut(), || {
		contextdb_query_expression_contains_impl(handle, expression, limit, out_len)
	})
}

unsafe fn contextdb_query_expression_contains_impl(
	handle: *const ContextDBHandle,
	expression: *const c_char,
	limit: usize,
	out_len: *mut usize,
) -> *mut ContextDBQueryResult {
	if handle.is_null() {
		set_invalid_argument("handle was null");
		return ptr::null_mut();
	}
	if out_len.is_null() {
		set_invalid_argument("out_len pointer was null");
		return ptr::null_mut();
	}

	let expression = match cstr_to_string(expression, "expression") {
		Ok(value) => value,
		Err(message) => {
			set_invalid_argument(message);
			return ptr::null_mut();
		}
	};

	let mut query = Query::new().with_expression(ExpressionFilter::Contains(expression));
	if limit > 0 {
		query = query.with_limit(limit);
	}

	let results = match (&*handle).db.query(&query) {
		Ok(results) => results,
		Err(err) => {
			set_storage_error(err);
			return ptr::null_mut();
		}
	};

	let mut out: Vec<ContextDBQueryResult> = Vec::with_capacity(results.len());
	for result in results {
		let expression = match cstring_from_string(result.entry.expression, "expression") {
			Ok(value) => value.into_raw(),
			Err(message) => {
				for item in out.drain(..) {
					contextdb_string_free(item.expression);
				}
				set_last_error(message);
				return ptr::null_mut();
			}
		};

		let mut id = [0u8; 16];
		id.copy_from_slice(result.entry.id.as_bytes());
		out.push(ContextDBQueryResult {
			id,
			score: result.similarity_score.unwrap_or(0.0),
			expression,
		});
	}

	let mut boxed = out.into_boxed_slice();
	let len = boxed.len();
	let ptr = boxed.as_mut_ptr();
	std::mem::forget(boxed);

	*out_len = len;
	clear_last_error();
	ptr
}

#[no_mangle]
/// # Safety
/// `results` must be a valid pointer returned by a query function and `len`
/// must match the length provided by that function. The pointer must not be
/// freed more than once.
pub unsafe extern "C" fn contextdb_query_results_free(
	results: *mut ContextDBQueryResult,
	len: usize,
) {
	if results.is_null() {
		return;
	}
	let slice = std::slice::from_raw_parts_mut(results, len);
	for item in slice.iter_mut() {
		contextdb_string_free(item.expression);
		item.expression = ptr::null_mut();
	}
	drop(Box::from_raw(slice as *mut [ContextDBQueryResult]));
}

#[cfg(all(test, feature = "ffi"))]
mod tests {
	use super::*;
	use std::ffi::CString;

	#[test]
	fn test_ffi_round_trip() {
		let handle = contextdb_open(ptr::null());
		assert!(!handle.is_null(), "contextdb_open returned null");

		let expression = CString::new("ffi round trip").expect("valid CString");
		let meaning = [0.25f32, 0.5f32, 0.75f32];

		let inserted = unsafe {
			contextdb_insert(handle, expression.as_ptr(), meaning.as_ptr(), meaning.len())
		};
		assert!(inserted, "contextdb_insert failed");

		let mut out_len = 0usize;
		let results = unsafe {
			contextdb_query_expression_contains(handle, expression.as_ptr(), 10, &mut out_len)
		};
		assert!(
			!results.is_null(),
			"contextdb_query_expression_contains returned null"
		);
		assert!(out_len >= 1, "expected at least one result");

		unsafe {
			contextdb_query_results_free(results, out_len);
		}

		let mut meaning_len = 0usize;
		let meaning_results = unsafe {
			contextdb_query_meaning(
				handle,
				meaning.as_ptr(),
				meaning.len(),
				0.0,
				10,
				&mut meaning_len,
			)
		};
		assert!(
			!meaning_results.is_null(),
			"contextdb_query_meaning returned null"
		);
		assert!(meaning_len >= 1, "expected at least one meaning result");

		unsafe {
			contextdb_query_results_free(meaning_results, meaning_len);
			contextdb_close(handle);
		}
	}

	#[test]
	fn test_json_ffi_crud_and_query_round_trip() {
		assert_eq!(contextdb_abi_version(), 1);
		let handle = contextdb_open(ptr::null());
		assert!(!handle.is_null());
		let request = CString::new(
			r#"{"expression":"json ffi","meaning":[0.1,0.2],"context":{"kind":"test"}}"#,
		)
		.unwrap();
		let mut id_ptr = ptr::null_mut();
		assert_eq!(
			unsafe { contextdb_insert_json(handle, request.as_ptr(), &mut id_ptr) },
			CONTEXTDB_STATUS_OK
		);
		let id = unsafe { CStr::from_ptr(id_ptr) }
			.to_string_lossy()
			.into_owned();
		unsafe { contextdb_string_free(id_ptr) };

		let id_c = CString::new(id.clone()).unwrap();
		let mut entry_ptr = ptr::null_mut();
		assert_eq!(
			unsafe { contextdb_get_json(handle, id_c.as_ptr(), &mut entry_ptr) },
			CONTEXTDB_STATUS_OK
		);
		let entry_json = unsafe { CStr::from_ptr(entry_ptr) }
			.to_string_lossy()
			.into_owned();
		unsafe { contextdb_string_free(entry_ptr) };
		let mut entry: Entry = serde_json::from_str(&entry_json).unwrap();
		assert_eq!(entry.context["kind"], "test");

		entry.expression = "updated json ffi".to_string();
		let update = CString::new(serde_json::to_string(&entry).unwrap()).unwrap();
		assert_eq!(
			unsafe { contextdb_update_json(handle, update.as_ptr()) },
			CONTEXTDB_STATUS_OK
		);

		let query = Query::new().with_context(crate::ContextFilter::PathEquals(
			"/kind".to_string(),
			serde_json::json!("test"),
		));
		let query = CString::new(serde_json::to_string(&query).unwrap()).unwrap();
		let mut results_ptr = ptr::null_mut();
		assert_eq!(
			unsafe { contextdb_query_json(handle, query.as_ptr(), &mut results_ptr) },
			CONTEXTDB_STATUS_OK
		);
		let results_json = unsafe { CStr::from_ptr(results_ptr) }
			.to_string_lossy()
			.into_owned();
		unsafe { contextdb_string_free(results_ptr) };
		let results: Vec<crate::QueryResult> = serde_json::from_str(&results_json).unwrap();
		assert_eq!(results.len(), 1);
		assert_eq!(results[0].entry.expression, "updated json ffi");

		assert_eq!(
			unsafe { contextdb_delete_id(handle, id_c.as_ptr()) },
			CONTEXTDB_STATUS_OK
		);
		assert_eq!(
			unsafe { contextdb_get_json(handle, id_c.as_ptr(), &mut entry_ptr) },
			CONTEXTDB_STATUS_NOT_FOUND
		);
		assert_eq!(contextdb_last_error_code(), CONTEXTDB_STATUS_NOT_FOUND);
		unsafe { contextdb_close(handle) };
	}

	#[test]
	fn test_json_ffi_rejects_invalid_arguments() {
		let handle = contextdb_open(ptr::null());
		let invalid = CString::new("not json").unwrap();
		let mut out = ptr::null_mut();

		assert_eq!(
			unsafe { contextdb_insert_json(handle, invalid.as_ptr(), &mut out) },
			CONTEXTDB_STATUS_INVALID_ARGUMENT
		);
		assert_eq!(
			contextdb_last_error_code(),
			CONTEXTDB_STATUS_INVALID_ARGUMENT
		);

		let valid = CString::new(r#"{"expression":"no output","meaning":[0.1]}"#).unwrap();
		assert_eq!(
			unsafe { contextdb_insert_json(handle, valid.as_ptr(), ptr::null_mut()) },
			CONTEXTDB_STATUS_INVALID_ARGUMENT
		);
		let mut count = usize::MAX;
		assert!(unsafe { contextdb_count(handle, &mut count) });
		assert_eq!(count, 0, "a rejected insert must not mutate the database");
		unsafe { contextdb_close(handle) };
	}

	#[test]
	fn test_json_ffi_preserves_domain_error_categories() {
		let handle = contextdb_open(ptr::null());
		assert!(!handle.is_null());

		let empty_vector = CString::new(r#"{"expression":"empty","meaning":[]}"#).unwrap();
		let mut output = ptr::null_mut();
		assert_eq!(
			unsafe { contextdb_insert_json(handle, empty_vector.as_ptr(), &mut output) },
			CONTEXTDB_STATUS_INVALID_ARGUMENT
		);
		assert_eq!(
			contextdb_last_error_code(),
			CONTEXTDB_STATUS_INVALID_ARGUMENT
		);

		let request = CString::new(r#"{"expression":"valid","meaning":[0.1,0.2]}"#).unwrap();
		assert_eq!(
			unsafe { contextdb_insert_json(handle, request.as_ptr(), &mut output) },
			CONTEXTDB_STATUS_OK
		);
		let id = unsafe { CStr::from_ptr(output) }
			.to_string_lossy()
			.into_owned();
		unsafe { contextdb_string_free(output) };

		let invalid_queries = [
			Query::new().with_meaning(vec![0.1, 0.2], Some(-0.1)),
			Query::new()
				.with_meaning(vec![0.1, 0.2], None)
				.with_top_k(0),
			Query::new().with_expression(ExpressionFilter::Matches("[".to_string())),
			Query::new().with_temporal(crate::TemporalFilter::CreatedBetween(
				chrono::DateTime::parse_from_rfc3339("2026-01-02T00:00:00Z")
					.unwrap()
					.with_timezone(&chrono::Utc),
				chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
					.unwrap()
					.with_timezone(&chrono::Utc),
			)),
		];
		for query in invalid_queries {
			let query = CString::new(serde_json::to_string(&query).unwrap()).unwrap();
			let mut results = ptr::null_mut();
			assert_eq!(
				unsafe { contextdb_query_json(handle, query.as_ptr(), &mut results) },
				CONTEXTDB_STATUS_INVALID_ARGUMENT
			);
			assert_eq!(
				contextdb_last_error_code(),
				CONTEXTDB_STATUS_INVALID_ARGUMENT
			);
		}

		let id = uuid::Uuid::parse_str(&id).unwrap();
		let mut entry = unsafe { (&*handle).db.get(id).unwrap() };
		entry.relations = vec![id];
		let update = CString::new(serde_json::to_string(&entry).unwrap()).unwrap();
		assert_eq!(
			unsafe { contextdb_update_json(handle, update.as_ptr()) },
			CONTEXTDB_STATUS_INVALID_ARGUMENT
		);

		entry.relations = vec![uuid::Uuid::new_v4()];
		let update = CString::new(serde_json::to_string(&entry).unwrap()).unwrap();
		assert_eq!(
			unsafe { contextdb_update_json(handle, update.as_ptr()) },
			CONTEXTDB_STATUS_NOT_FOUND
		);
		assert_eq!(contextdb_last_error_code(), CONTEXTDB_STATUS_NOT_FOUND);

		unsafe { contextdb_close(handle) };
	}
}
