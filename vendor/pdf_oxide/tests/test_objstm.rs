//! Tests for object stream parsing (PDF 1.5+ feature).

use bytes::Bytes;
use pdf_oxide::object::Object;
use pdf_oxide::objstm::parse_object_stream;
use std::collections::HashMap;

/// Helper to create a test object stream.
///
/// This creates an uncompressed object stream for testing.
/// In real PDFs, these are usually FlateDecode compressed.
fn create_test_object_stream(n: i64, first: i64, data: &[u8]) -> Object {
    let mut dict = HashMap::new();
    dict.insert("Type".to_string(), Object::Name("ObjStm".to_string()));
    dict.insert("N".to_string(), Object::Integer(n));
    dict.insert("First".to_string(), Object::Integer(first));
    dict.insert("Length".to_string(), Object::Integer(data.len() as i64));

    Object::Stream {
        dict,
        data: Bytes::from(data.to_vec()),
    }
}

#[test]
fn test_parse_object_stream_basic() {
    // Create an object stream with 2 objects:
    // Object 10: integer 42
    // Object 11: name /Test

    // Pairs section: "10 0 11 3"
    // Objects section: "42 /Test"
    let pairs = b"10 0 11 3 ";
    let objects = b"42 /Test";
    let first = pairs.len() as i64;

    let mut data = Vec::new();
    data.extend_from_slice(pairs);
    data.extend_from_slice(objects);

    let stream = create_test_object_stream(2, first, &data);
    let result = parse_object_stream(&stream).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result.get(&10).unwrap().as_integer(), Some(42));
    assert_eq!(result.get(&11).unwrap().as_name(), Some("Test"));
}

#[test]
fn test_parse_object_stream_multiple_objects() {
    // Create an object stream with 4 objects:
    // Object 10: integer 1
    // Object 11: boolean true
    // Object 12: boolean false
    // Object 13: null

    let pairs = b"10 0 11 2 12 7 13 13 ";
    let objects = b"1 true false null";
    let first = pairs.len() as i64;

    let mut data = Vec::new();
    data.extend_from_slice(pairs);
    data.extend_from_slice(objects);

    let stream = create_test_object_stream(4, first, &data);
    let result = parse_object_stream(&stream).unwrap();

    assert_eq!(result.len(), 4);
    assert_eq!(result.get(&10).unwrap().as_integer(), Some(1));
    assert_eq!(result.get(&11).unwrap().as_bool(), Some(true));
    assert_eq!(result.get(&12).unwrap().as_bool(), Some(false));
    assert_eq!(result.get(&13).unwrap(), &Object::Null);
}

#[test]
fn test_parse_object_stream_complex_objects() {
    // Create an object stream with complex objects:
    // Object 20: array [1 2 3]
    // Object 21: dictionary << /Type /Page >>

    let pairs = b"20 0 21 10 ";
    let objects = b"[ 1 2 3 ] << /Type /Page >>";
    let first = pairs.len() as i64;

    let mut data = Vec::new();
    data.extend_from_slice(pairs);
    data.extend_from_slice(objects);

    let stream = create_test_object_stream(2, first, &data);
    let result = parse_object_stream(&stream).unwrap();

    assert_eq!(result.len(), 2);

    let array = result.get(&20).unwrap().as_array().unwrap();
    assert_eq!(array.len(), 3);
    assert_eq!(array[0].as_integer(), Some(1));

    let dict = result.get(&21).unwrap().as_dict().unwrap();
    assert_eq!(dict.get("Type").unwrap().as_name(), Some("Page"));
}

#[test]
fn test_parse_object_stream_with_whitespace() {
    // Test that whitespace is handled correctly in pairs section
    let pairs = b"  10   0   11   3  ";
    let objects = b"42 99";
    let first = pairs.len() as i64;

    let mut data = Vec::new();
    data.extend_from_slice(pairs);
    data.extend_from_slice(objects);

    let stream = create_test_object_stream(2, first, &data);
    let result = parse_object_stream(&stream).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result.get(&10).unwrap().as_integer(), Some(42));
    assert_eq!(result.get(&11).unwrap().as_integer(), Some(99));
}

#[test]
fn test_parse_object_stream_not_stream() {
    // Test that we get an error when trying to parse a non-stream object
    let obj = Object::Integer(42);
    let result = parse_object_stream(&obj);
    assert!(result.is_err());
}

#[test]
fn test_parse_object_stream_missing_n() {
    let mut dict = HashMap::new();
    dict.insert("Type".to_string(), Object::Name("ObjStm".to_string()));
    dict.insert("First".to_string(), Object::Integer(5));

    let stream = Object::Stream {
        dict,
        data: Bytes::from(b"1 0 42".to_vec()),
    };

    let result = parse_object_stream(&stream);
    assert!(result.is_err());
}

#[test]
fn test_parse_object_stream_missing_first() {
    let mut dict = HashMap::new();
    dict.insert("Type".to_string(), Object::Name("ObjStm".to_string()));
    dict.insert("N".to_string(), Object::Integer(1));

    let stream = Object::Stream {
        dict,
        data: Bytes::from(b"1 0 42".to_vec()),
    };

    let result = parse_object_stream(&stream);
    assert!(result.is_err());
}

#[test]
fn test_parse_object_stream_invalid_n() {
    let stream = create_test_object_stream(-1, 5, b"1 0 42");
    let result = parse_object_stream(&stream);
    assert!(result.is_err());
}

#[test]
fn test_parse_object_stream_n_too_large() {
    let stream = create_test_object_stream(2_000_000, 5, b"1 0 42");
    let result = parse_object_stream(&stream);
    assert!(result.is_err());
}

#[test]
fn test_parse_object_stream_first_beyond_data() {
    let stream = create_test_object_stream(1, 1000, b"1 0 42");
    let result = parse_object_stream(&stream);
    assert!(result.is_err());
}

#[test]
fn test_parse_object_stream_strings() {
    // Test object stream with string objects
    let pairs = b"30 0 31 13 ";
    let objects = b"(Hello World) <48656C6C6F>";
    let first = pairs.len() as i64;

    let mut data = Vec::new();
    data.extend_from_slice(pairs);
    data.extend_from_slice(objects);

    let stream = create_test_object_stream(2, first, &data);
    let result = parse_object_stream(&stream).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result.get(&30).unwrap().as_string(), Some(&b"Hello World"[..]));
    assert_eq!(result.get(&31).unwrap().as_string(), Some(&b"Hello"[..]));
}

#[test]
fn test_parse_object_stream_nested_structures() {
    // Test nested arrays and dictionaries
    let pairs = b"40 0 ";
    let objects = b"<< /Array [ 1 [ 2 3 ] ] /Dict << /Nested true >> >>";
    let first = pairs.len() as i64;

    let mut data = Vec::new();
    data.extend_from_slice(pairs);
    data.extend_from_slice(objects);

    let stream = create_test_object_stream(1, first, &data);
    let result = parse_object_stream(&stream).unwrap();

    assert_eq!(result.len(), 1);

    let dict = result.get(&40).unwrap().as_dict().unwrap();
    let array = dict.get("Array").unwrap().as_array().unwrap();
    assert_eq!(array.len(), 2);

    let nested_dict = dict.get("Dict").unwrap().as_dict().unwrap();
    assert_eq!(nested_dict.get("Nested").unwrap().as_bool(), Some(true));
}

#[test]
fn test_parse_object_stream_empty() {
    // Test an empty object stream (N=0)
    let stream = create_test_object_stream(0, 0, b"");
    let result = parse_object_stream(&stream).unwrap();
    assert_eq!(result.len(), 0);
}

#[test]
fn test_parse_object_stream_large_object_numbers() {
    // Test with large object numbers
    let pairs = b"999999 0 1000000 5 ";
    let objects = b"true false";
    let first = pairs.len() as i64;

    let mut data = Vec::new();
    data.extend_from_slice(pairs);
    data.extend_from_slice(objects);

    let stream = create_test_object_stream(2, first, &data);
    let result = parse_object_stream(&stream).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result.get(&999999).unwrap().as_bool(), Some(true));
    assert_eq!(result.get(&1000000).unwrap().as_bool(), Some(false));
}

#[test]
fn test_parse_object_stream_references() {
    // Object streams can contain indirect references
    let pairs = b"50 0 ";
    let objects = b"[ 10 0 R 20 0 R ]";
    let first = pairs.len() as i64;

    let mut data = Vec::new();
    data.extend_from_slice(pairs);
    data.extend_from_slice(objects);

    let stream = create_test_object_stream(1, first, &data);
    let result = parse_object_stream(&stream).unwrap();

    assert_eq!(result.len(), 1);

    let array = result.get(&50).unwrap().as_array().unwrap();
    assert_eq!(array.len(), 2);
    assert!(array[0].as_reference().is_some());
    assert!(array[1].as_reference().is_some());
}

#[test]
fn test_parse_object_stream_graceful_failure() {
    // Test that we gracefully handle malformed objects in the stream
    // The first object is valid, the second is malformed
    let pairs = b"60 0 61 5 ";
    let objects = b"true [[[[["; // Unclosed arrays
    let first = pairs.len() as i64;

    let mut data = Vec::new();
    data.extend_from_slice(pairs);
    data.extend_from_slice(objects);

    let stream = create_test_object_stream(2, first, &data);
    let result = parse_object_stream(&stream).unwrap();

    // Object 60 should always be parsed successfully
    assert!(result.contains_key(&60));
    assert_eq!(result.get(&60).unwrap().as_bool(), Some(true));
    // Object 61 may be skipped (parser error) or partially parsed (parser recovery)
    // Either behavior is acceptable for malformed input
}
