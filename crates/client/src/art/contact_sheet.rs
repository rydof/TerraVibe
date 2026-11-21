//! Audit helper for held-item 3D visual coverage across all registered items.

use terravibe_sim::item::{ItemId, all_item_ids};

use crate::view::avatar::{HeldVisualKind, held_visual_for_item};

/// Returns `(ItemId, HeldVisualKind)` for every item in `all_item_ids()`.
#[allow(dead_code)]
pub fn build_held_items_audit() -> Vec<(ItemId, HeldVisualKind)> {
    all_item_ids()
        .map(|id| (id, held_visual_for_item(id)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_covers_all_148_items() {
        let audit = build_held_items_audit();
        assert_eq!(audit.len(), 148);
    }
}
