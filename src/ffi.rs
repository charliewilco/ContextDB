use crate::{ContextDB, Entry, ExpressionFilter, Query};
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

struct ContextDBHandle {
	db: ContextDB,
}

#[repr(C)]
pub struct ContextDBQueryResult {
	pub id: [u8; 16],
	pub score: f32,
	pub expression: *mut c_char,
}

thread_local! {
	static LAST_ERROR: RefCell<Option<CString>> = RefCell::new(None);
}

fn set_last_error(message: impl ToString) {
	let message = CString::new(message.to_string()).unwrap_or_else(|_| CString::new("unknown error").unwrap());
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
pub extern "C" fn contextdb_string_free(ptr: *mut c_char) {
	if ptr.is_null() {
		return;
	}
	unsafe {
		drop(CString::from_raw(ptr));
	}
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
pub extern "C" fn contextdb_close(handle: *mut ContextDBHandle) {
	if handle.is_null() {
		return;
	}
	unsafe {
		drop(Box::from_raw(handle));
	}
}

#[no_mangle]
pub extern "C" fn contextdb_insert(
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

	let mut entry = Entry::new(meaning, expression);
	match unsafe { &mut *handle }.db.insert(&mut entry) {
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
pub extern "C" fn contextdb_count(handle: *const ContextDBHandle, out_count: *mut usize) -> bool {
	if handle.is_null() {
		set_last_error("handle was null");
		return false;
	}
	if out_count.is_null() {
		set_last_error("out_count pointer was null");
		return false;
	}

	match unsafe { &*handle }.db.count() {
		Ok(count) => {
			unsafe {
				*out_count = count;
			}
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
pub extern "C" fn contextdb_query_meaning(
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
		unsafe { std::slice::from_raw_parts(meaning_ptr, meaning_len) }.to_vec()
	};

	let threshold = if threshold < 0.0 { None } else { Some(threshold) };
	let mut query = Query::new().with_meaning(meaning, threshold);
	if limit > 0 {
		query = query.with_limit(limit);
	}

	let results = match unsafe { &*handle }.db.query(&query) {
		Ok(results) => results,
		Err(err) => {
			set_last_error(err.to_string());
			return ptr::null_mut();
		}
	};

	let mut out = Vec::with_capacity(results.len());
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

	unsafe {
		*out_len = len;
	}
	clear_last_error();
	ptr
}

#[no_mangle]
pub extern "C" fn contextdb_query_expression_contains(
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

	let results = match unsafe { &*handle }.db.query(&query) {
		Ok(results) => results,
		Err(err) => {
			set_last_error(err.to_string());
			return ptr::null_mut();
		}
	};

	let mut out = Vec::with_capacity(results.len());
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

	unsafe {
		*out_len = len;
	}
	clear_last_error();
	ptr
}

#[no_mangle]
pub extern "C" fn contextdb_query_results_free(results: *mut ContextDBQueryResult, len: usize) {
	if results.is_null() {
		return;
	}
	unsafe {
		let slice = std::slice::from_raw_parts_mut(results, len);
		for item in slice.iter_mut() {
			contextdb_string_free(item.expression);
			item.expression = ptr::null_mut();
		}
		drop(Box::from_raw(slice as *mut [ContextDBQueryResult]));
	}
}
