use modal_proto::modal_proto as pb;
use modal_proto::modal_proto::class_parameter_spec::DefaultOneof;
use modal_proto::modal_proto::class_parameter_value::ValueOneof;
use prost::Message;

use crate::error::ModalError;

/// Supported parameter value types.
#[derive(Debug, Clone)]
pub enum ParamValue {
    String(String),
    Int(i64),
    Bool(bool),
    Bytes(Vec<u8>),
}

/// Encode a parameter set into serialized proto bytes.
/// Parameters are encoded according to their schema specification and sorted by name
/// to produce deterministic output.
pub fn encode_parameter_set(
    schema: &[pb::ClassParameterSpec],
    parameters: Option<&std::collections::HashMap<String, ParamValue>>,
) -> Result<Vec<u8>, ModalError> {
    let mut encoded: Vec<pb::ClassParameterValue> = Vec::new();

    for spec in schema {
        let value = parameters.and_then(|p| p.get(&spec.name));
        let param_value = encode_parameter(spec, value)?;
        encoded.push(param_value);
    }

    // Sort by name for deterministic output
    encoded.sort_by(|a, b| a.name.cmp(&b.name));

    let param_set = pb::ClassParameterSet {
        parameters: encoded,
    };

    let mut buf = Vec::new();
    param_set
        .encode(&mut buf)
        .map_err(|e| ModalError::Serialization(format!("failed to encode parameter set: {}", e)))?;
    Ok(buf)
}

fn encode_parameter(
    spec: &pb::ClassParameterSpec,
    value: Option<&ParamValue>,
) -> Result<pb::ClassParameterValue, ModalError> {
    let name = &spec.name;
    let param_type = spec.r#type;

    let value_oneof = match param_type {
        t if t == pb::ParameterType::ParamTypeString as i32 => {
            let s = match value {
                Some(ParamValue::String(s)) => s.clone(),
                None if spec.has_default => match &spec.default_oneof {
                    Some(DefaultOneof::StringDefault(s)) => s.clone(),
                    _ => String::new(),
                },
                _ => {
                    return Err(ModalError::Serialization(format!(
                        "parameter '{}' must be a string",
                        name
                    )));
                }
            };
            Some(ValueOneof::StringValue(s))
        }
        t if t == pb::ParameterType::ParamTypeInt as i32 => {
            let i = match value {
                Some(ParamValue::Int(i)) => *i,
                None if spec.has_default => match &spec.default_oneof {
                    Some(DefaultOneof::IntDefault(i)) => *i,
                    _ => 0,
                },
                _ => {
                    return Err(ModalError::Serialization(format!(
                        "parameter '{}' must be an integer",
                        name
                    )));
                }
            };
            Some(ValueOneof::IntValue(i))
        }
        t if t == pb::ParameterType::ParamTypeBool as i32 => {
            let b = match value {
                Some(ParamValue::Bool(b)) => *b,
                None if spec.has_default => match &spec.default_oneof {
                    Some(DefaultOneof::BoolDefault(b)) => *b,
                    _ => false,
                },
                _ => {
                    return Err(ModalError::Serialization(format!(
                        "parameter '{}' must be a boolean",
                        name
                    )));
                }
            };
            Some(ValueOneof::BoolValue(b))
        }
        t if t == pb::ParameterType::ParamTypeBytes as i32 => {
            let b = match value {
                Some(ParamValue::Bytes(b)) => b.clone(),
                None if spec.has_default => match &spec.default_oneof {
                    Some(DefaultOneof::BytesDefault(b)) => b.clone(),
                    _ => Vec::new(),
                },
                _ => {
                    return Err(ModalError::Serialization(format!(
                        "parameter '{}' must be a byte slice",
                        name
                    )));
                }
            };
            Some(ValueOneof::BytesValue(b))
        }
        _ => {
            return Err(ModalError::Serialization(format!(
                "unsupported parameter type: {}",
                param_type
            )));
        }
    };

    Ok(pb::ClassParameterValue {
        name: name.clone(),
        r#type: param_type,
        value_oneof,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_parameter_serialization() {
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
        let expected: Vec<u8> =
            b"\n\x0c\n\x03foo\x10\x01\x1a\x03bar\n\x07\n\x01i\x10\x02 \x05".to_vec();
        assert_eq!(serialized, expected);

        // Reverse schema order — result should still be deterministic
        let schema_reversed = vec![schema[1].clone(), schema[0].clone()];
        let serialized2 = encode_parameter_set(&schema_reversed, Some(&values)).unwrap();
        assert_eq!(serialized2, expected);

        // Test with a parameter that has a default value.
        let schema_bytes = vec![pb::ClassParameterSpec {
            name: "x".to_string(),
            r#type: pb::ParameterType::ParamTypeBytes as i32,
            has_default: true,
            default_oneof: Some(DefaultOneof::BytesDefault(vec![0x00])),
            ..Default::default()
        }];

        let serialized3 = encode_parameter_set(&schema_bytes, None).unwrap();
        let expected3: Vec<u8> = b"\n\x08\n\x01x\x10\x042\x01\x00".to_vec();
        assert_eq!(serialized3, expected3);
    }
}
