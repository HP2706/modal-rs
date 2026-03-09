#![cfg(feature = "integration")]

/// Integration tests for Modal Serialization module.
/// Translated from libmodal/modal-go/serialization_test.go

use modal::serialization::{encode_parameter_set, ParamValue};
use modal_proto::modal_proto as pb;
use std::collections::HashMap;

#[test]
fn test_parameter_serialization() {
    // Build schema: foo (STRING), i (INT)
    let schema = vec![
        pb::ClassParameterSpec {
            name: "foo".to_string(),
            r#type: pb::ParameterType::ParamTypeString as i32,
            has_default: false,
            ..Default::default()
        },
        pb::ClassParameterSpec {
            name: "i".to_string(),
            r#type: pb::ParameterType::ParamTypeInt as i32,
            has_default: false,
            ..Default::default()
        },
    ];

    let mut values = HashMap::new();
    values.insert("i".to_string(), ParamValue::Int(5));
    values.insert("foo".to_string(), ParamValue::String("bar".to_string()));

    let serialized = encode_parameter_set(&schema, Some(&values)).unwrap();

    // Expected bytes from Go test (Python SDK compatibility)
    let expected: Vec<u8> = b"\n\x0c\n\x03foo\x10\x01\x1a\x03bar\n\x07\n\x01i\x10\x02 \x05".to_vec();
    assert_eq!(serialized, expected);

    // Reverse schema order — output should be deterministic (sorted by name)
    let reversed_schema = vec![schema[1].clone(), schema[0].clone()];
    let serialized2 = encode_parameter_set(&reversed_schema, Some(&values)).unwrap();
    assert_eq!(serialized2, expected);

    // Test with a parameter that has a default value
    let schema_with_default = vec![pb::ClassParameterSpec {
        name: "x".to_string(),
        r#type: pb::ParameterType::ParamTypeBytes as i32,
        has_default: true,
        default_oneof: Some(
            pb::class_parameter_spec::DefaultOneof::BytesDefault(vec![0x00]),
        ),
        ..Default::default()
    }];

    let serialized3 = encode_parameter_set(&schema_with_default, None).unwrap();
    let expected3: Vec<u8> = b"\n\x08\n\x01x\x10\x042\x01\x00".to_vec();
    assert_eq!(serialized3, expected3);
}
