use miette::NamedSource;
use miette::SourceSpan;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use toml::Value;

use crate::parser::AssertSql;
use crate::validator::Assertion;
use crate::validator::ValidationError;

/// Helper function to find the span of a key in the source contents.
fn find_key_span(src: Option<&(String, String)>, key: &str) -> Option<SourceSpan> {
    let (_, content) = src?;
    // This simple find assumes the key is unique and finds its first occurrence.
    let start = content.find(key)?;
    Some(SourceSpan::new(start.into(), key.len()))
}

/// Helper function to find the span of a value in the source contents.
fn find_value_span(src: Option<&(String, String)>, value: &str) -> Option<SourceSpan> {
    let (_, content) = src?;
    let start = content.find(value)?;
    Some(SourceSpan::new(start.into(), value.len()))
}

/// Macro to simplify the creation of a ValidationError with source context.
macro_rules! validation_err {
    ($src:expr, $field:expr, $message:expr, $span_fn:expr) => {
        ValidationError {
            field: $field.to_string(),
            message: $message,
            src: $src
                .as_ref()
                .map(|(name, content)| NamedSource::new(name.clone(), content.clone())),
            span: $span_fn,
        }
    };
}

/// Parses a single header key-value pair and adds it to the HeaderMap.
fn parse_single_header(
    header_map: &mut HeaderMap,
    key: &str,
    value: &Value,
    src: Option<&(String, String)>,
) -> Result<(), ValidationError> {
    let v_str = value.as_str().ok_or_else(|| {
        validation_err!(
            src,
            key,
            format!("Header value must be a string, got {value:?}"),
            find_key_span(src, key)
        )
    })?;

    let name = HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
        validation_err!(
            src,
            key,
            format!("Invalid header name `{key}`: {e}"),
            find_key_span(src, key)
        )
    })?;

    let h_value = HeaderValue::from_str(v_str).map_err(|e| {
        validation_err!(
            src,
            key,
            format!("Invalid header value for `{key}`: {e}"),
            find_value_span(src, v_str)
        )
    })?;

    header_map.insert(name, h_value);
    Ok(())
}

/// Parses the optional header assertions from a TOML Value table.
pub fn parse_header_map(
    value: &Value,
    src: Option<&(String, String)>,
) -> Result<HeaderMap, ValidationError> {
    let map = value.as_table().ok_or_else(|| {
        validation_err!(
            src,
            "headers",
            format!("Expected a table for headers, got {value:?}"),
            None
        )
    })?;

    let mut header_map = HeaderMap::new();

    for (k, v) in map {
        parse_single_header(&mut header_map, k, v, src)?;
    }

    Ok(header_map)
}

/// Parses all available assertion configurations (status, headers, etc.) into a
/// Vec<Assertion>.
pub fn parse_assertions(
    assert_status: &Option<i32>,
    assert_headers: &Option<Value>,
    assert_sql: &Option<AssertSql>,
    assert_json: &Option<serde_json::Value>,
    src: Option<(&str, &str)>,
) -> Result<Vec<Assertion>, ValidationError> {
    let mut assert_vec = vec![];
    let src_ref = src.as_ref().map(|(n, c)| (n.to_string(), c.to_string()));

    if let Some(status) = assert_status {
        assert_vec.push(Assertion::Status(*status));
    }

    if let Some(value) = assert_headers {
        let header_map = parse_header_map(value, src_ref.as_ref())?;
        assert_vec.push(Assertion::Headers(header_map));
    }

    if let Some(sql) = assert_sql {
        assert_vec.push(Assertion::Sql {
            query: sql.query.clone(),
            expect: sql.expect.clone(),
            got: None,
        });
    }

    if let Some(json) = assert_json {
        assert_vec.push(Assertion::Json(json.clone()));
    }

    Ok(assert_vec)
}
