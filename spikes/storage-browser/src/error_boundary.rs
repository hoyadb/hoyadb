//! API-0010 / TC-0035 S10/S13: foreign errors retain a static kind only.
use hoyadb_core::storage::StorageError;

pub(super) fn fields(error: StorageError) -> Vec<(&'static str, String)> {
    let kind = match error {
        StorageError::BackendFailure {
            ..
        } => "BackendFailure",
        StorageError::MigrationFailure {
            ..
        } => "MigrationFailure",
        StorageError::SchemaConflict {
            ..
        } => "SchemaConflict",
        StorageError::InvalidSchema {
            ..
        } => "InvalidSchema",
        StorageError::InvalidQuery {
            ..
        } => "InvalidQuery",
        StorageError::InvalidRow {
            ..
        } => "InvalidRow",
        StorageError::TableNotFound {
            ..
        } => "TableNotFound",
        StorageError::RowAlreadyExists {
            ..
        } => "RowAlreadyExists",
        StorageError::RowNotFound {
            ..
        } => "RowNotFound",
        StorageError::StoreInUse => "StoreInUse",
    };
    vec![("kind", kind.into())]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn s10_s13_storage_error_fields_preserve_kind_and_strip_all_payloads() {
        const SECRET: &str = "synthetic-secret-key-path-identity-table-record";
        let cases = [
            (
                "BackendFailure",
                StorageError::BackendFailure {
                    message: SECRET.into(),
                },
            ),
            (
                "MigrationFailure",
                StorageError::MigrationFailure {
                    message: SECRET.into(),
                },
            ),
            (
                "SchemaConflict",
                StorageError::SchemaConflict {
                    message: SECRET.into(),
                },
            ),
            (
                "InvalidSchema",
                StorageError::InvalidSchema {
                    message: SECRET.into(),
                },
            ),
            (
                "InvalidQuery",
                StorageError::InvalidQuery {
                    message: SECRET.into(),
                },
            ),
            (
                "InvalidRow",
                StorageError::InvalidRow {
                    table: SECRET.into(),
                    id: SECRET.into(),
                    message: SECRET.into(),
                },
            ),
            (
                "TableNotFound",
                StorageError::TableNotFound {
                    table: SECRET.into(),
                },
            ),
            (
                "RowAlreadyExists",
                StorageError::RowAlreadyExists {
                    table: SECRET.into(),
                    id: SECRET.into(),
                },
            ),
            (
                "RowNotFound",
                StorageError::RowNotFound {
                    table: SECRET.into(),
                    id: SECRET.into(),
                },
            ),
        ];
        for (kind, error) in cases {
            let public = fields(error);
            assert_eq!(public, vec![("kind", kind.to_owned())]);
            assert!(!format!("{public:?}").contains(SECRET));
        }
    }
}
