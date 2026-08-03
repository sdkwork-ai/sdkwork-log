//! Offset pagination aligned with `PAGINATION_SPEC.md` and `sdkwork-utils-rust`.

use sdkwork_utils_rust::{
    offset_list_page_data, validated_offset_list_params, OffsetListPageParams, SdkWorkPageData,
    SdkWorkResultCode,
};

pub fn validated_offset_params(
    page: Option<i64>,
    page_size: Option<i64>,
    legacy_limit: Option<i64>,
) -> Result<OffsetListPageParams, SdkWorkResultCode> {
    let merged_page_size = page_size.or(legacy_limit);
    if merged_page_size == Some(0) {
        return Err(SdkWorkResultCode::InvalidParameter);
    }
    validated_offset_list_params(page, merged_page_size)
}

pub fn offset_page<T>(
    items: Vec<T>,
    total_items: i64,
    params: OffsetListPageParams,
) -> SdkWorkPageData<T> {
    offset_list_page_data(items, total_items, params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_query_validates_spec_bounds() {
        assert!(validated_offset_params(None, None, None).is_ok());
        assert!(validated_offset_params(Some(1), Some(0), None).is_err());
        assert!(validated_offset_params(Some(1), Some(500), None).is_err());
        let params = validated_offset_params(Some(2), Some(10), None).expect("params");
        assert_eq!(2, params.page);
        assert_eq!(10, params.page_size);
    }
}
