//! Relational definitions rows and field groups shared within the client store.

/// Stored columns for a unary export expression and its child link.
pub(crate) struct ExpressionRow {
    /// Native expression discriminator.
    pub(crate) kind: String,
    /// Literal, field path, binding name or join separator.
    pub(crate) text: Option<String>,
    /// Station side for an identifier selection.
    pub(crate) side: Option<String>,
    /// Nested expression row for capture and join nodes.
    pub(crate) child: Option<i64>,
    /// Capture regular expression.
    pub(crate) pattern: Option<String>,
    /// Named capture selected from the pattern.
    pub(crate) capture_name: Option<String>,
}

/// Borrowed columns describing when an output field applies.
pub(crate) struct BindingFields<'a> {
    /// Binding-condition discriminator.
    pub(crate) condition: &'a str,
    /// Required expansion binding.
    pub(crate) binding: Option<&'a str>,
    /// Required station-side association.
    pub(crate) side: Option<&'a str>,
}

/// Borrowed columns for an output partition rule.
pub(crate) struct PartitionFields<'a> {
    /// Expression defining a value partition.
    pub(crate) expression_id: Option<i64>,
    /// Station side defining a reference partition.
    pub(crate) side: Option<&'a str>,
    /// Expansion binding for a reference partition.
    pub(crate) binding: Option<&'a str>,
}

/// Optional catalog metadata and its lookup expression.
pub(crate) struct CatalogFields {
    /// Catalog dataset identity.
    pub(crate) catalog_id: Option<String>,
    /// Catalog display label.
    pub(crate) catalog_display: Option<String>,
    /// Expression deriving the lookup key.
    pub(crate) catalog_lookup: Option<i64>,
}
