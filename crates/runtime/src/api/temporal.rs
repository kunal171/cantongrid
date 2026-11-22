//! Temporal transport filters retain exact source-local offsets and the common authority guards.
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct TemporalFilter {
    as_of_offset: String,
    limit: Option<usize>,
    cursor: Option<String>,
}
/// Require the canonical nonnegative i64 representation: no floats, signs or ambiguous leading zeroes.
pub(super) fn offset(value: &str) -> Result<i64, &'static str> {
    value
        .parse::<i64>()
        .ok()
        .filter(|n| *n >= 0 && n.to_string() == value)
        .ok_or("invalid temporal offset")
}
/// REST and GraphQL use the same source-boundary query, never client-supplied grants.
pub(super) async fn state(
    State(service): State<Service>,
    headers: HeaderMap,
    Query(filter): Query<TemporalFilter>,
) -> Response {
    let Ok(offset) = offset(&filter.as_of_offset) else {
        return error(StatusCode::BAD_REQUEST);
    };
    rest(
        service,
        headers,
        Filter {
            limit: filter.limit,
            cursor: filter.cursor,
        },
        Selection::Temporal { offset },
    )
    .await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offset_inputs_preserve_exact_i64_and_reject_aliases() {
        assert_eq!(offset("9223372036854775807"), Ok(i64::MAX));
        assert_eq!(offset("0"), Ok(0));
        for value in ["-1", "+1", "01", "1.0", "1e3", " 1", "9223372036854775808"] {
            assert!(offset(value).is_err());
        }
    }
}
