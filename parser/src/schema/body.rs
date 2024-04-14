use crate::body::Body as PublicBody;
use crate::schema::table::{FormDataTable, PrimitiveTable, Transform};
use crate::schema::types::PrimitiveArray;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Body {
    #[default]
    None,
    #[serde(alias = "text")]
    Raw(String),
    #[serde(alias = "file")]
    Binary(String),
    #[serde(
        alias = "form-data",
        alias = "form_data",
        alias = "multipart_form_data",
        alias = "multipart-form-data"
    )]
    FormData(FormDataTable),
    #[serde(
        alias = "x-www-form-urlencoded",
        alias = "form_urlencoded",
        alias = "form-urlencoded"
    )]
    XFormUrlEncoded(PrimitiveTable),
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum FormDataValue {
    Text(PrimitiveArray),
    File(String),
}

impl From<Body> for PublicBody {
    fn from(value: Body) -> Self {
        match value {
            Body::None => PublicBody::None,
            Body::Raw(content) => PublicBody::Raw {
                content,
                mime: mime::TEXT_PLAIN_UTF_8,
            },
            Body::Binary(path) => PublicBody::Binary {
                path,
                mime: mime::APPLICATION_OCTET_STREAM,
            },
            Body::FormData(table) => PublicBody::FormData(table.into_map()),
            Body::XFormUrlEncoded(table) => PublicBody::XFormUrlEncoded(table.into_map()),
        }
    }
}

use crate::body::FormDataValue as PublicFDV;
impl Transform<FormDataValue, PublicFDV> for FormDataTable {
    fn map_value(value: FormDataValue) -> PublicFDV {
        match value {
            FormDataValue::Text(value) => PublicFDV::Text(value.into()),
            FormDataValue::File(path) => PublicFDV::File(path),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::schema::types::{Primitive, PrimitiveArray};

    #[derive(Debug, Deserialize)]
    struct Parent {
        body: Body,
    }

    #[test]
    fn deserialize() {
        assert_eq!(
            toml::from_str::<Parent>(r#"body.raw = "content""#)
                .unwrap()
                .body,
            Body::Raw("content".to_string())
        );

        let toml = r#"body.form-urlencoded = { type = "integer", value = 1 }"#;
        let body = toml::from_str::<Parent>(toml).unwrap().body;
        assert!(matches!(&body, Body::XFormUrlEncoded(map) if map.len() == 2));
        if let Body::XFormUrlEncoded(map) = &body {
            assert_eq!(
                map["type"],
                PrimitiveArray::Single(Primitive::Str("integer".to_string()))
            );
            assert_eq!(map["value"], PrimitiveArray::Single(Primitive::Int(1)));
        }

        let toml = r#"
        [body.form_data]
        raw.text = "raw"
        binary.file = "path"
        "#;
        let body = toml::from_str::<Parent>(toml).unwrap().body;
        assert!(matches!(&body, Body::FormData(map) if map.len() == 2));
        if let Body::FormData(map) = &body {
            assert_eq!(
                map["raw"],
                FormDataValue::Text(PrimitiveArray::Single(Primitive::Str("raw".to_string())))
            );
            assert_eq!(map["binary"], FormDataValue::File("path".to_string()));
        }
    }

    #[test]
    fn only_one_type() {
        let toml = r#"
        [body]
        raw = "raw"
        binary = "file"
        "#;
        let err = toml::from_str::<Parent>(toml).err().unwrap();
        assert!(err.to_string().contains("wanted exactly 1 element"));

        let toml = r#"
        [body.form-data]
        key.text = 2
        key.file = "path"
        "#;
        let err = toml::from_str::<Parent>(toml).err().unwrap();
        assert!(err.to_string().contains("wanted exactly 1 element"));
    }

    #[test]
    fn deserialize_empty() {
        let err = toml::from_str::<Parent>(r#"[body]"#).err().unwrap();
        assert!(err.to_string().contains("wanted exactly 1 element"));
    }
}
