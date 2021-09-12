use std::{collections::HashMap, fs::File, io::Read, path::Path};

use crate::{NamespacedId, TagRegistry};
use generated::{Item, ItemStack};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use smartstring::{Compact, SmartString};

/// A registry which contains crafting recipes by type.
#[derive(Clone, Debug, Default)]
pub struct RecipeRegistry {
    blast: Vec<BlastingRecipe>,
    camp: Vec<CampfireRecipe>,
    shaped: Vec<ShapedRecipe>,
    shapeless: Vec<ShapelessRecipe>,
    smelt: Vec<SmeltingRecipe>,
    smith: Vec<SmithingRecipe>,
    smoke: Vec<SmokingRecipe>,
    stone: Vec<StonecuttingRecipe>,
}

impl RecipeRegistry {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
    pub fn from_dir(path: &Path) -> Result<Self, crate::RecipeLoadError> {
        let mut this = Self::new();
        this.add_from_dir(path)?;
        Ok(this)
    }
    pub fn add_from_dir(&mut self, path: &Path) -> Result<(), crate::RecipeLoadError> {
        for file in std::fs::read_dir(path)? {
            let path = file?.path();
            log::trace!("{}", path.to_string_lossy());
            match Recipe::from_file(&path)? {
                Recipe::Blasting(recipe) => self.blast.push(recipe),
                Recipe::Campfire(recipe) => self.camp.push(recipe),
                Recipe::Shaped(recipe) => self.shaped.push(recipe),
                Recipe::Shapeless(recipe) => self.shapeless.push(recipe),
                Recipe::Smelting(recipe) => self.smelt.push(recipe),
                Recipe::Smithing(recipe) => self.smith.push(recipe),
                Recipe::Smoking(recipe) => self.smoke.push(recipe),
                Recipe::Stonecutting(recipe) => self.stone.push(recipe),
                Recipe::Special => {}
            }
        }
        Ok(())
    }
    pub fn match_blasting(&self, item: Item, tag_registry: &TagRegistry) -> Option<(Item, f32)> {
        self.blast
            .iter()
            .find_map(|r| r.match_self(item, tag_registry))
    }
    pub fn match_campfire_cooking(
        &self,
        item: Item,
        tag_registry: &TagRegistry,
    ) -> Option<(Item, f32)> {
        self.camp
            .iter()
            .find_map(|r| r.match_self(item, tag_registry))
    }
    pub fn match_shapeless<'a>(
        &self,
        items: impl Iterator<Item = &'a Item>,
        tag_registry: &TagRegistry,
    ) -> Option<ItemStack> {
        let items: Vec<Item> = items.copied().collect();
        self.shapeless
            .iter()
            .find_map(|r| r.match_self(items.iter(), tag_registry))
    }
    pub fn match_smelting(&self, item: Item, tag_registry: &TagRegistry) -> Option<(Item, f32)> {
        self.smelt
            .iter()
            .find_map(|r| r.match_self(item, tag_registry))
    }
    pub fn match_smithing(
        &self,
        base: Item,
        addition: Item,
        tag_registry: &TagRegistry,
    ) -> Option<Item> {
        self.smith
            .iter()
            .find_map(|r| r.match_self(base, addition, tag_registry))
    }
    pub fn match_smoking(&self, item: Item, tag_registry: &TagRegistry) -> Option<(Item, f32)> {
        self.smoke
            .iter()
            .find_map(|r| r.match_self(item, tag_registry))
    }
    pub fn match_stonecutting(&self, item: Item, tag_registry: &TagRegistry) -> Option<ItemStack> {
        self.stone
            .iter()
            .find_map(|r| r.match_self(item, tag_registry))
    }
}

/// A minecraft crafting recipe.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Recipe {
    #[serde(rename = "minecraft:blasting")]
    Blasting(BlastingRecipe),
    #[serde(rename = "minecraft:campfire_cooking")]
    Campfire(CampfireRecipe),
    #[serde(rename = "minecraft:crafting_shaped")]
    Shaped(ShapedRecipe),
    #[serde(rename = "minecraft:crafting_shapeless")]
    Shapeless(ShapelessRecipe),
    #[serde(rename = "minecraft:smelting")]
    Smelting(SmeltingRecipe),
    #[serde(rename = "minecraft:smithing")]
    Smithing(SmithingRecipe),
    #[serde(rename = "minecraft:smoking")]
    Smoking(SmokingRecipe),
    #[serde(rename = "minecraft:stonecutting")]
    Stonecutting(StonecuttingRecipe),
    #[serde(alias = "minecraft:crafting_special_armordye")]
    #[serde(alias = "minecraft:crafting_special_bannerduplicate")]
    #[serde(alias = "minecraft:crafting_special_bookcloning")]
    #[serde(alias = "minecraft:crafting_special_firework_rocket")]
    #[serde(alias = "minecraft:crafting_special_firework_star")]
    #[serde(alias = "minecraft:crafting_special_firework_star_fade")]
    #[serde(alias = "minecraft:crafting_special_mapcloning")]
    #[serde(alias = "minecraft:crafting_special_mapextending")]
    #[serde(alias = "minecraft:crafting_special_repairitem")]
    #[serde(alias = "minecraft:crafting_special_shielddecoration")]
    #[serde(alias = "minecraft:crafting_special_shulkerboxcoloring")]
    #[serde(alias = "minecraft:crafting_special_tippedarrow")]
    #[serde(alias = "minecraft:crafting_special_suspiciousstew")]
    Special,
}

impl Recipe {
    /// Loads a Recipe from a JSON file.
    pub fn from_file(path: &Path) -> Result<Self, crate::RecipeLoadError> {
        let mut s = String::new();
        File::open(path)?.read_to_string(&mut s)?;
        Self::from_raw(&s)
    }

    /// Tries to parse a string into a recipe from JSON.
    pub fn from_raw<'a>(raw: &'a str) -> Result<Self, crate::RecipeLoadError> {
        Ok(serde_json::from_str(raw)?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RecipeComponent {
    item: Option<Item>,
    tag: Option<NamespacedId>,
}

impl RecipeComponent {
    pub fn matches(&self, item: Item, tag_registry: &TagRegistry) -> bool {
        self.item
            .as_ref()
            .map(|s| item.name() == s.name())
            .unwrap_or(false)
            | self
                .tag
                .as_ref()
                .map(|s| tag_registry.check_item_tag(item, s))
                .unwrap_or(false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
enum Ingredient {
    One(RecipeComponent),
    // 3 is chosen as the size for the smallvec heap because
    // most recipes shouldn't need more, and for ones that need
    // less it isn't too bad
    Many(SmallVec<[RecipeComponent; 3]>),
}

impl Ingredient {
    pub fn matches(&self, item: Item, tag_registry: &TagRegistry) -> bool {
        match self {
            Ingredient::One(o) => o.matches(item, tag_registry),
            Ingredient::Many(vec) => vec.iter().any(|o| o.matches(item, tag_registry)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmeltingRecipe {
    group: Option<SmartString<Compact>>,
    ingredient: Ingredient,
    result: Item,
    experience: f32,
    #[serde(default = "default_smelting_time")]
    cookingtime: u32,
}

impl SmeltingRecipe {
    pub fn matches(&self, item: Item, tag_registry: &TagRegistry) -> bool {
        self.ingredient.matches(item, tag_registry)
    }
    pub fn match_self(&self, item: Item, tag_registry: &TagRegistry) -> Option<(Item, f32)> {
        if self.matches(item, tag_registry) {
            Some((self.result.into(), self.experience))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmokingRecipe {
    group: Option<SmartString<Compact>>,
    ingredient: Ingredient,
    result: Item,
    experience: f32,
    #[serde(default = "default_smoking_time")]
    cookingtime: u32,
}

impl SmokingRecipe {
    pub fn matches(&self, item: Item, tag_registry: &TagRegistry) -> bool {
        self.ingredient.matches(item, tag_registry)
    }
    pub fn match_self(&self, item: Item, tag_registry: &TagRegistry) -> Option<(Item, f32)> {
        if self.matches(item, tag_registry) {
            Some((self.result.into(), self.experience))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlastingRecipe {
    group: Option<SmartString<Compact>>,
    ingredient: Ingredient,
    result: Item,
    experience: f32,
    #[serde(default = "default_blasting_time")]
    cookingtime: u32,
}

impl BlastingRecipe {
    pub fn matches(&self, item: Item, tag_registry: &TagRegistry) -> bool {
        self.ingredient.matches(item, tag_registry)
    }
    pub fn match_self(&self, item: Item, tag_registry: &TagRegistry) -> Option<(Item, f32)> {
        if self.matches(item, tag_registry) {
            Some((self.result.into(), self.experience))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampfireRecipe {
    group: Option<SmartString<Compact>>,
    ingredient: Ingredient,
    result: Item,
    experience: f32,
    #[serde(default = "default_campfire_time")]
    cookingtime: u32,
}

impl CampfireRecipe {
    pub fn matches(&self, item: Item, tag_registry: &TagRegistry) -> bool {
        self.ingredient.matches(item, tag_registry)
    }

    pub fn match_self(&self, item: Item, tag_registry: &TagRegistry) -> Option<(Item, f32)> {
        if self.matches(item, tag_registry) {
            Some((self.result.into(), self.experience))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapelessRecipe {
    group: Option<SmartString<Compact>>,
    ingredients: Ingredient,
    result: ItemStack,
}

impl ShapelessRecipe {
    pub fn matches<'a>(
        &self,
        items: impl Iterator<Item = &'a Item>,
        tag_registry: &TagRegistry,
    ) -> bool {
        let counter = self.ingredients.clone();

        let mut ingredient_items = vec![];

        match counter {
            Ingredient::One(ingredient) => ingredient_items.push(ingredient),
            Ingredient::Many(mut ingredients) => ingredients
                .drain(0..ingredients.len())
                .for_each(|ingredient| ingredient_items.push(ingredient)),
        }

        for i in items {
            match ingredient_items
                .iter()
                .enumerate()
                .find(|(_, ing)| ing.matches(*i, tag_registry))
            {
                Some((index, _)) => {
                    ingredient_items.remove(index);
                }
                None => return false,
            };
        }
        true
    }
    pub fn match_self<'a>(
        &self,
        items: impl Iterator<Item = &'a Item>,
        tag_registry: &TagRegistry,
    ) -> Option<ItemStack> {
        if self.matches(items, tag_registry) {
            Some(self.result.clone())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapedRecipe {
    group: Option<SmartString<Compact>>,
    pattern: [[Option<char>; 3]; 3],
    key: HashMap<char, Ingredient>,
    result: ItemStack,
}

impl ShapedRecipe {
    // TODO: Decide how to pass the crafting grid
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmithingRecipe {
    group: Option<SmartString<Compact>>,
    base: Ingredient,
    addition: Ingredient,
    result: ItemStack,
}

impl SmithingRecipe {
    pub fn matches(&self, base: Item, addition: Item, tag_registry: &TagRegistry) -> bool {
        self.base.matches(base, tag_registry) && self.addition.matches(addition, tag_registry)
    }
    pub fn match_self(
        &self,
        base: Item,
        addition: Item,
        tag_registry: &TagRegistry,
    ) -> Option<Item> {
        if self.matches(base, addition, tag_registry) {
            Some(self.result.item.into())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StonecuttingRecipe {
    group: Option<SmartString<Compact>>,
    ingredient: Ingredient,
    result: Item,
    count: u32,
}

impl StonecuttingRecipe {
    pub fn matches(&self, item: Item, tag_registry: &TagRegistry) -> bool {
        self.ingredient.matches(item, tag_registry)
    }
    pub fn match_self(&self, item: Item, tag_registry: &TagRegistry) -> Option<ItemStack> {
        if self.matches(item, tag_registry) {
            Some(ItemStack {
                item: self.result.into(),
                count: self.count,
                damage: None,
            })
        } else {
            None
        }
    }
}

pub fn default_smelting_time() -> u32 {
    200
}
pub fn default_smoking_time() -> u32 {
    100
}
pub fn default_blasting_time() -> u32 {
    100
}
pub fn default_campfire_time() -> u32 {
    100
}

mod tests {
    use std::str::FromStr;

    use generated::ItemStack;
    use smartstring::SmartString;

    #[test]
    fn test_blasting() {
        use generated::Item;

        use crate::recipe::{Ingredient, RecipeComponent};

        use super::Recipe;

        let recipe = r#"
        {
            "type": "minecraft:blasting",
            "ingredient": {
              "item": "minecraft:diamond_ore"
            },
            "result": "minecraft:diamond",
            "experience": 1,
            "cookingtime": 200
          }
        "#;

        let deserialized = Recipe::from_raw(&recipe);

        if let Ok(Recipe::Blasting(recipe)) = deserialized {
            assert_eq!(recipe.group, None);

            assert_eq!(
                recipe.ingredient,
                Ingredient::One(RecipeComponent {
                    item: Some(Item::DiamondOre),
                    tag: None
                })
            );

            assert_eq!(recipe.result, Item::Diamond);

            assert_eq!(recipe.experience, 1.0);

            assert_eq!(recipe.cookingtime, 200);
        } else {
            panic!("Deserialization Failed.\n{:?}", deserialized)
        }
    }

    #[test]
    fn test_campfire() {
        use generated::Item;

        use crate::recipe::{Ingredient, RecipeComponent};

        use super::Recipe;

        let recipe = r#"
        {
            "type": "minecraft:campfire_cooking",
            "ingredient": {
                "item": "minecraft:emerald_block"
            },
            "result": "minecraft:dead_bush",
            "experience": 727,
            "cookingtime": 452,
            "group": "foobar"
        }
        "#;

        let deserialized = Recipe::from_raw(&recipe);

        if let Ok(Recipe::Campfire(recipe)) = deserialized {
            assert_eq!(recipe.group, Some(SmartString::from_str("foobar").unwrap()));

            assert_eq!(
                recipe.ingredient,
                Ingredient::One(RecipeComponent {
                    item: Some(Item::EmeraldBlock),
                    tag: None
                })
            );

            assert_eq!(recipe.result, Item::DeadBush);

            assert_eq!(recipe.experience, 727.0);

            assert_eq!(recipe.cookingtime, 452);
        } else {
            panic!("Deserialization Failed.\n{:?}", deserialized)
        }
    }

    #[test]
    fn test_shaped() {
        use generated::Item;

        use crate::recipe::{Ingredient, RecipeComponent};

        use super::Recipe;

        let recipe = "
        {
            \"type\": \"minecraft:crafting_shaped\",
            \"pattern\": [
                \"# C\",
                \"WB \"
            ],
            \"key\": {
                \"#\": {
                    \"item\": \"minecraft:dead_bush\"
                },
                \"C\": {
                    \"item\": \"minecraft:chainmail_chestplate\"
                },
                \"W\": {
                    \"item\": \"minecraft:chainmail_boots\"
                },
                \"B\": {
                    \"item\": \"minecraft:bedrock\"
                }
            },
            \"result\": {
                \"item\": \"minecraft:seagrass\",
                \"count\": 64
            }
        }
        ";

        let deserialized = Recipe::from_raw(&recipe);

        if let Ok(Recipe::Shaped(recipe)) = deserialized {
            assert_eq!(recipe.group, None);

            assert_eq!(
                recipe.pattern,
                [
                    [Some('#'), Some(' '), Some('C')],
                    [Some('W'), Some('B'), Some(' ')],
                    [None, None, None]
                ]
            );

            assert_eq!(
                recipe.key.get(&'#'),
                Some(&Ingredient::One(RecipeComponent {
                    item: Some(Item::DeadBush),
                    tag: None
                }))
            );

            assert_eq!(
                recipe.key.get(&'C'),
                Some(&Ingredient::One(RecipeComponent {
                    item: Some(Item::ChainmailChestplate),
                    tag: None
                }))
            );

            assert_eq!(
                recipe.key.get(&'W'),
                Some(&Ingredient::One(RecipeComponent {
                    item: Some(Item::ChainmailBoots),
                    tag: None
                }))
            );

            assert_eq!(
                recipe.key.get(&'B'),
                Some(&Ingredient::One(RecipeComponent {
                    item: Some(Item::Bedrock),
                    tag: None
                }))
            );

            assert_eq!(recipe.result, ItemStack::new(Item::Seagrass, 64))
        } else {
            panic!("Deserialization Failed.\n{:?}", deserialized)
        }
    }

    #[test]
    fn test_shapeless() {
        use generated::Item;

        use crate::recipe::{Ingredient, RecipeComponent};

        use super::Recipe;

        let recipe = r#"
        {
            "type": "minecraft:crafting_shapeless",
            "ingredients": [
                {
                    "item": "minecraft:glass"
                },
                {
                    "item": "minecraft:wet_sponge"
                }
            ],
            "result": {
                "item": "minecraft:tnt",
                "count": 1
            }
        }
        "#;

        let deserialized = Recipe::from_raw(&recipe);

        if let Ok(Recipe::Shapeless(recipe)) = deserialized {
            assert_eq!(recipe.group, None);

            if let Ingredient::Many(ingredients) = recipe.ingredients {
                assert!(ingredients.contains(&RecipeComponent {
                    item: Some(Item::Glass),
                    tag: None
                }));
                assert!(ingredients.contains(&RecipeComponent {
                    item: Some(Item::WetSponge),
                    tag: None
                }))
            }

            assert_eq!(recipe.result, ItemStack::new(Item::Tnt, 1));
        } else {
            panic!("Deserialization Failed.\n{:?}", deserialized)
        }
    }
}
