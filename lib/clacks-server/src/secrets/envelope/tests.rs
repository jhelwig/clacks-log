//! Compact encrypted-value framing owned by Clacks.

use super::*;

#[test]
fn complete_values_preserve_their_metadata_and_reject_ambiguous_framing() -> anyhow::Result<()> {
    let sealed = || Ciphertext {
        nonce: [7; 12],
        bytes: vec![8; 16],
    };
    let receipt = ReceiptEnvelope::new(4, sealed())?.encode();
    assert!(ReceiptEnvelope::parse(&receipt)?.encode() == receipt);
    let wrapped = WrappedKeyEnvelope::new("release$旧".into(), sealed())?.encode();
    let restored = WrappedKeyEnvelope::parse(&wrapped)?;
    assert_eq!(restored.wrapping_key(), "release$旧");
    assert!(restored.encode() == wrapped);
    for value in [
        receipt.replacen("$1$", "$2$", 1),
        receipt.replacen("$4$", "$04$", 1),
        format!("{receipt}$extra"),
        receipt.replacen("BwcHBwcHBwcHBwcH", "Bw", 1),
        receipt.replacen("CAgICAgICAgICAgICAgICA", "CA", 1),
    ] {
        assert!(ReceiptEnvelope::parse(&value).is_err());
    }
    assert!(WrappedKeyEnvelope::parse(&receipt).is_err());
    assert!(ReceiptEnvelope::parse(&wrapped).is_err());
    Ok(())
}
