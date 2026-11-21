//! Pure crafting helpers for the inventory screen: row state, station probe, ordering and
//! feedback text. The server owns every craft; the screen only sends `ClientMsg::Craft`.

use terravibe_sim::crafting::{
    CraftError, Recipe, RecipeId, craft, recipe, recipe_station_name, recipes,
};
use terravibe_sim::inventory::Inventory;
use terravibe_sim::item::{ItemId, Prefix, display_name};
use terravibe_sim::{ANVIL, BlockId, FURNACE, WORKBENCH};

pub const STATIONS: [BlockId; 3] = [WORKBENCH, FURNACE, ANVIL];

fn item_label(item: terravibe_sim::item::ItemId, count: u32) -> String {
    let name = display_name(item, Prefix(0));
    if count > 1 {
        format!("{name} x{count}")
    } else {
        name
    }
}

pub fn title_text(r: &Recipe) -> String {
    item_label(r.output.item, r.output.count)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowState {
    Craftable,
    MissingStation,
    MissingIngredients,
    InventoryFull,
}

pub fn row_state(inv: &Inventory, id: RecipeId, station_ok: bool) -> RowState {
    let Some(r) = recipe(id) else {
        return RowState::MissingIngredients;
    };
    if r.station.is_some() && !station_ok {
        return RowState::MissingStation;
    }
    match craft(&mut inv.clone(), id, 1) {
        Ok(_) => RowState::Craftable,
        Err(CraftError::InventoryFull) => RowState::InventoryFull,
        Err(_) => RowState::MissingIngredients,
    }
}

pub fn craft_feedback(id: RecipeId, crafted: u32, error: Option<CraftError>) -> String {
    match error {
        None => match recipe(id) {
            Some(r) => format!("Crafted {}", item_label_plural(r, crafted)),
            None => "Craft failed".into(),
        },
        Some(CraftError::MissingStation) => {
            format!(
                "Need a nearby {}",
                recipe_station_name(id).unwrap_or("station")
            )
        }
        Some(CraftError::MissingIngredients) => "Not enough materials".into(),
        Some(CraftError::InventoryFull) => "Inventory full".into(),
        Some(_) => "Craft failed".into(),
    }
}

/// "3 Torches" / "Work Bench": count is output units.
fn item_label_plural(r: &Recipe, crafted: u32) -> String {
    let name = display_name(r.output.item, Prefix(0));
    if crafted > 1 {
        format!(
            "{crafted} {name}{}",
            if name.ends_with('h') { "es" } else { "s" }
        )
    } else {
        name
    }
}

/// Whether `r`'s station (if any) is among the stations `near` the player.
pub fn station_ok(r: &Recipe, near: &[BlockId]) -> bool {
    r.station.is_none_or(|s| near.contains(&s))
}

/// Returns missing ingredients and shortfall count for `recipe` given player's `inv`.
pub fn missing_ingredients(recipe: &Recipe, inv: &Inventory) -> Vec<(ItemId, u32)> {
    recipe
        .ingredients
        .iter()
        .filter_map(|ing| {
            let have = inv.count_of(ing.item);
            if have < ing.count {
                Some((ing.item, ing.count - have))
            } else {
                None
            }
        })
        .collect()
}

/// Filters candidate recipe indices matching case-insensitive substring `query` on the output item's name.
pub fn filter_recipes(recipes_slice: &[usize], query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    let all = recipes();
    recipes_slice
        .iter()
        .copied()
        .filter(|&idx| {
            if q.is_empty() {
                return true;
            }
            let r = all
                .get(idx)
                .or_else(|| u16::try_from(idx).ok().map(RecipeId).and_then(recipe));
            let Some(r) = r else { return false };
            let name = display_name(r.output.item, Prefix(0));
            name.to_lowercase().contains(&q)
        })
        .collect()
}

/// Every recipe id: craftable first, then by `RecipeId`.
pub fn sorted_order(inv: &Inventory, near: &[BlockId]) -> Vec<RecipeId> {
    let mut v: Vec<(bool, RecipeId)> = recipes()
        .iter()
        .map(|r| {
            let ok = row_state(inv, r.id, station_ok(r, near)) == RowState::Craftable;
            (!ok, r.id)
        })
        .collect();
    v.sort_by_key(|&(not_ok, id)| (not_ok, id.0));
    v.into_iter().map(|(_, id)| id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::AIR;
    use terravibe_sim::chunk::BlockQuery;
    use terravibe_sim::crafting::{
        COPPER_BAR_RECIPE, CRAFT_RANGE_X, CRAFT_RANGE_Y, FURNACE_RECIPE, TORCH_RECIPE,
        WOODEN_SWORD_RECIPE, WORKBENCH_RECIPE, station_in_reach,
    };
    use terravibe_sim::inventory::ItemStack;
    use terravibe_sim::item::{COPPER_PICKAXE, GEL, WOOD};

    fn stack(id: terravibe_sim::item::ItemId, count: u32) -> ItemStack {
        ItemStack {
            id,
            count,
            prefix: Prefix(0),
        }
    }

    fn inv(items: &[(terravibe_sim::item::ItemId, u32)]) -> Inventory {
        let mut i = Inventory::empty();
        for &(id, n) in items {
            assert!(i.add(stack(id, n)).is_none());
        }
        i
    }

    #[test]
    fn craft_rows_report_missing_station() {
        let i = inv(&[(WOOD, 7)]);
        assert_eq!(
            row_state(&i, WOODEN_SWORD_RECIPE, false),
            RowState::MissingStation
        );
    }

    #[test]
    fn craft_rows_report_missing_ingredients() {
        let i = inv(&[(WOOD, 7)]);
        assert_eq!(
            row_state(&i, TORCH_RECIPE, true),
            RowState::MissingIngredients
        );
    }

    #[test]
    fn craft_rows_are_craftable() {
        let i = inv(&[(WOOD, 10), (GEL, 1)]);
        assert_eq!(row_state(&i, TORCH_RECIPE, false), RowState::Craftable);
        assert_eq!(row_state(&i, WORKBENCH_RECIPE, false), RowState::Craftable);
    }

    #[test]
    fn craft_rows_report_full_inventory() {
        let mut i = Inventory::empty();
        i.slots = [Some(stack(COPPER_PICKAXE, 1)); terravibe_sim::inventory::SLOTS];
        i.slots[0] = Some(stack(WOOD, 10));
        i.slots[1] = Some(stack(GEL, 2));
        assert_eq!(row_state(&i, TORCH_RECIPE, false), RowState::InventoryFull);
    }

    #[test]
    fn panel_lists_every_recipe() {
        let o = sorted_order(&Inventory::empty(), &[]);
        assert_eq!(o.len(), recipes().len());
        let mut ids: Vec<u16> = o.iter().map(|i| i.0).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), recipes().len());
    }

    #[test]
    fn missing_station_feedback_names_the_station() {
        assert_eq!(
            craft_feedback(COPPER_BAR_RECIPE, 0, Some(CraftError::MissingStation)),
            "Need a nearby Furnace"
        );
    }

    #[test]
    fn craftable_rows_sort_first() {
        let i = inv(&[(WOOD, 10), (GEL, 1)]);
        let o = sorted_order(&i, &[]);
        assert_eq!(o[0], WORKBENCH_RECIPE);
        assert_eq!(o[1], TORCH_RECIPE);
        // Wood Platform needs no station, so it is craftable too.
        assert_eq!(o[2], terravibe_sim::crafting::WOOD_PLATFORM_RECIPE);
        let rest: Vec<u16> = o[3..].iter().map(|r| r.0).collect();
        assert!(rest.windows(2).all(|w| w[0] < w[1]));
    }

    struct One([i32; 3], BlockId);
    impl BlockQuery for One {
        fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
            if [x, y, z] == self.0 { self.1 } else { AIR }
        }
    }

    #[test]
    fn client_station_probe_matches_server_reach() {
        let feet = [0.5, 10.0, 0.5];
        for (at, ok) in [
            ([5, 10, 0], true),
            ([6, 10, 0], false),
            ([0, 13, 0], true),
            ([0, 14, 0], false),
        ] {
            for st in STATIONS {
                assert_eq!(station_in_reach(&One(at, st), feet, st), ok, "{at:?}");
            }
        }
    }

    #[test]
    fn reach_constants_are_shared() {
        // The panel probe has no range of its own; it uses the sim function and consts.
        assert_eq!((CRAFT_RANGE_X, CRAFT_RANGE_Y), (5, 3));
    }

    #[test]
    fn missing_ingredients_lists_exact_shortfall() {
        let i = inv(&[(WOOD, 3)]);
        let r = recipe(WORKBENCH_RECIPE).expect("workbench recipe");
        let missing = missing_ingredients(r, &i);
        assert_eq!(missing, vec![(WOOD, 7)]);
    }

    #[test]
    fn filter_recipes_matches_case_insensitive_substring() {
        let candidates = vec![TORCH_RECIPE.0 as usize, FURNACE_RECIPE.0 as usize];
        assert_eq!(
            filter_recipes(&candidates, "torch"),
            vec![TORCH_RECIPE.0 as usize]
        );
        assert_eq!(
            filter_recipes(&candidates, "TORCH"),
            vec![TORCH_RECIPE.0 as usize]
        );
        let all_indices: Vec<usize> = (0..recipes().len()).collect();
        let matches = filter_recipes(&all_indices, "torch");
        assert!(matches.contains(&(TORCH_RECIPE.0 as usize)));
        assert!(!matches.contains(&(FURNACE_RECIPE.0 as usize)));
    }

    #[test]
    fn craft_feedback_matches_result() {
        assert_eq!(craft_feedback(TORCH_RECIPE, 3, None), "Crafted 3 Torches");
        assert_eq!(
            craft_feedback(WOODEN_SWORD_RECIPE, 0, Some(CraftError::MissingStation)),
            "Need a nearby Workbench"
        );
        assert_eq!(
            craft_feedback(TORCH_RECIPE, 0, Some(CraftError::MissingIngredients)),
            "Not enough materials"
        );
        assert_eq!(
            craft_feedback(TORCH_RECIPE, 0, Some(CraftError::InventoryFull)),
            "Inventory full"
        );
    }
}
