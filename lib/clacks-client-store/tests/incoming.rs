//! Ordered native input fixtures using the actual receipt adapter.

use crate::{store::ClientStore, transfer};
use clacks_sync::{
    account::InstalledCheckpoint,
    record::AcceptedRecord,
    transfer::{
        discovery::{DiscoveryEnd, DiscoveryPage, DiscoveryStart},
        receipt::ReceiptState,
    },
};

/// Source metadata and its initial ordinary page.
pub(crate) struct Source {
    pub(crate) start: DiscoveryStart,
    pub(crate) page: DiscoveryPage,
}

/// Message fixture owning the real connection-local receiver state.
pub(crate) struct Incoming {
    receipt: ReceiptState,
}

impl Incoming {
    pub(crate) async fn begin(store: &ClientStore, source: &Source) -> anyhow::Result<Self> {
        let mut receipt = transfer::begin(store, &source.start).await?;
        transfer::install_page(store, &mut receipt, &source.page).await?;
        Ok(Self { receipt })
    }

    pub(crate) async fn page(
        &mut self,
        store: &ClientStore,
        accepted: &[AcceptedRecord],
    ) -> anyhow::Result<()> {
        let sequence = self.receipt.next_page;
        transfer::install_page(
            store,
            &mut self.receipt,
            &DiscoveryPage {
                sequence,
                requirements: vec![],
                accepted: accepted.to_vec(),
                support: vec![],
                baselines: vec![],
                boundaries: vec![],
                units: vec![],
            },
        )
        .await
    }

    pub(crate) async fn finish(&self, store: &ClientStore) -> anyhow::Result<InstalledCheckpoint> {
        transfer::complete(
            store,
            &self.receipt,
            &DiscoveryEnd {
                source: self.receipt.source.clone(),
                pages: self.receipt.next_page,
            },
        )
        .await
    }
}
