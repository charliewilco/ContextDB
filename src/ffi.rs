use crate::{ContextDB, Entry, ExpressionFilter, Query};
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
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
}

fn set_last_error(message: impl ToString) {
	let message = CString::new(message.to_string())
		.unwrap_or_else(|_| CString::new("unknown error").unwrap());
	LAST_ERROR.with(|cell| {
		*cell.borrow_mut() = Some(message);
	});
}

fn clear_last_error() {
	LAST_ERROR.with(|cell| {
		*cell.borrow_mut() = None;
	});
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
	let db = if path.is_null() {
		ContextDB::in_memory()
	} else {
		match cstr_to_string(path, "path") {
			Ok(path) if path.is_empty() => ContextDB::in_memory(),
			Ok(path) => ContextDB::new(path),
			Err(message) => {
				set_last_error(message);
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
			set_last_error(err.to_string());
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
	if handle.is_null() {
		set_last_error("handle was null");
		return false;
	}
	if meaning_ptr.is_null() && meaning_len > 0 {
		set_last_error("meaning pointer was null");
		return false;
	}

	let expression = match cstr_to_string(expression, "expression") {
		Ok(value) => value,
		Err(message) => {
			set_last_error(message);
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
			set_last_error(err.to_string());
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
	if handle.is_null() {
		set_last_error("handle was null");
		return false;
	}
	if out_count.is_null() {
		set_last_error("out_count pointer was null");
		return false;
	}

	match (&*handle).db.count() {
		Ok(count) => {
			*out_count = count;
			clear_last_error();
			true
		}
		Err(err) => {
			set_last_error(err.to_string());
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
	if handle.is_null() {
		set_last_error("handle was null");
		return ptr::null_mut();
	}
	if meaning_ptr.is_null() && meaning_len > 0 {
		set_last_error("meaning pointer was null");
		return ptr::null_mut();
	}
	if out_len.is_null() {
		set_last_error("out_len pointer was null");
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
			set_last_error(err.to_string());
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
	if handle.is_null() {
		set_last_error("handle was null");
		return ptr::null_mut();
	}
	if out_len.is_null() {
		set_last_error("out_len pointer was null");
		return ptr::null_mut();
	}

	let expression = match cstr_to_string(expression, "expression") {
		Ok(value) => value,
		Err(message) => {
			set_last_error(message);
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
			set_last_error(err.to_string());
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
			contextdb_insert(
				handle,
				expression.as_ptr(),
				meaning.as_ptr(),
				meaning.len(),
			)
		};
		assert!(inserted, "contextdb_insert failed");

		let mut out_len = 0usize;
		let results = unsafe {
			contextdb_query_expression_contains(handle, expression.as_ptr(), 10, &mut out_len)
		};
		assert!(!results.is_null(), "contextdb_query_expression_contains returned null");
		assert!(out_len >= 1, "expected at least one result");

		unsafe {
			contextdb_query_results_free(results, out_len);
		}

		let mut meaning_len = 0usize;
		let meaning_results = unsafe {
			contextdb_query_meaning(handle, meaning.as_ptr(), meaning.len(), 0.0, 10, &mut meaning_len)
		};
		assert!(!meaning_results.is_null(), "contextdb_query_meaning returned null");
		assert!(meaning_len >= 1, "expected at least one meaning result");

		unsafe {
			contextdb_query_results_free(meaning_results, meaning_len);
			contextdb_close(handle);
		}
	}
}
