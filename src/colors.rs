use std::collections::HashMap;

pub type Rgba = [u8; 4];

pub struct ColorMap {
    exact: HashMap<String, Rgba>,
}

fn strip_ns(name: &str) -> &str {
    name.strip_prefix("minecraft:").unwrap_or(name)
}

const DYE_COLORS: &[(&str, Rgba)] = &[
    ("white", [233, 236, 236, 255]),
    ("orange", [240, 118, 19, 255]),
    ("magenta", [189, 68, 179, 255]),
    ("light_blue", [58, 175, 217, 255]),
    ("yellow", [248, 198, 39, 255]),
    ("lime", [112, 185, 25, 255]),
    ("pink", [237, 141, 172, 255]),
    ("gray", [62, 68, 71, 255]),
    ("light_gray", [142, 142, 134, 255]),
    ("silver", [142, 142, 134, 255]),
    ("cyan", [21, 137, 145, 255]),
    ("purple", [121, 42, 172, 255]),
    ("blue", [53, 57, 157, 255]),
    ("brown", [114, 71, 40, 255]),
    ("green", [84, 109, 27, 255]),
    ("red", [161, 39, 34, 255]),
    ("black", [20, 21, 25, 255]),
];

fn builtin_table() -> HashMap<String, Rgba> {
    let entries: &[(&str, Rgba)] = &[
        ("stone", [125, 125, 125, 255]),
        ("granite", [149, 103, 85, 255]),
        ("polished_granite", [154, 106, 89, 255]),
        ("diorite", [188, 188, 188, 255]),
        ("polished_diorite", [193, 193, 193, 255]),
        ("andesite", [136, 136, 136, 255]),
        ("polished_andesite", [132, 134, 133, 255]),
        ("deepslate", [80, 80, 82, 255]),
        ("cobbled_deepslate", [77, 77, 80, 255]),
        ("tuff", [108, 109, 102, 255]),
        ("calcite", [223, 224, 220, 255]),
        ("grass_block", [127, 178, 56, 255]),
        ("grass", [127, 178, 56, 255]),
        ("dirt", [134, 96, 67, 255]),
        ("dirt_with_roots", [124, 88, 60, 255]),
        ("coarse_dirt", [119, 85, 59, 255]),
        ("podzol", [90, 63, 24, 255]),
        ("mycelium", [111, 99, 105, 255]),
        ("mud", [60, 57, 60, 255]),
        ("muddy_mangrove_roots", [70, 58, 45, 255]),
        ("cobblestone", [110, 110, 110, 255]),
        ("mossy_cobblestone", [100, 118, 90, 255]),
        ("bedrock", [70, 70, 70, 255]),
        ("water", [63, 118, 228, 255]),
        ("flowing_water", [63, 118, 228, 255]),
        ("lava", [207, 92, 20, 255]),
        ("flowing_lava", [207, 92, 20, 255]),
        ("sand", [219, 211, 160, 255]),
        ("red_sand", [190, 102, 33, 255]),
        ("gravel", [126, 124, 122, 255]),
        ("sandstone", [217, 210, 158, 255]),
        ("red_sandstone", [186, 99, 29, 255]),
        ("gold_ore", [143, 139, 125, 255]),
        ("iron_ore", [135, 130, 126, 255]),
        ("coal_ore", [115, 115, 115, 255]),
        ("copper_ore", [124, 125, 120, 255]),
        ("diamond_ore", [129, 140, 143, 255]),
        ("emerald_ore", [117, 135, 120, 255]),
        ("lapis_ore", [102, 112, 134, 255]),
        ("redstone_ore", [132, 107, 107, 255]),
        ("oak_log", [109, 85, 50, 255]),
        ("spruce_log", [58, 37, 16, 255]),
        ("birch_log", [216, 215, 210, 255]),
        ("jungle_log", [87, 67, 26, 255]),
        ("acacia_log", [103, 96, 86, 255]),
        ("dark_oak_log", [61, 47, 26, 255]),
        ("mangrove_log", [83, 66, 41, 255]),
        ("cherry_log", [54, 33, 44, 255]),
        ("oak_leaves", [60, 120, 40, 255]),
        ("spruce_leaves", [45, 80, 45, 255]),
        ("birch_leaves", [95, 130, 70, 255]),
        ("jungle_leaves", [55, 125, 30, 255]),
        ("acacia_leaves", [90, 130, 45, 255]),
        ("dark_oak_leaves", [50, 100, 35, 255]),
        ("mangrove_leaves", [60, 115, 45, 255]),
        ("cherry_leaves", [233, 173, 206, 255]),
        ("azalea_leaves", [92, 120, 55, 255]),
        ("leaves", [60, 120, 40, 255]),
        ("leaves2", [60, 110, 40, 255]),
        ("planks", [157, 128, 79, 255]),
        ("oak_planks", [162, 130, 78, 255]),
        ("spruce_planks", [114, 84, 48, 255]),
        ("birch_planks", [192, 175, 121, 255]),
        ("jungle_planks", [160, 115, 80, 255]),
        ("acacia_planks", [168, 90, 50, 255]),
        ("dark_oak_planks", [66, 43, 20, 255]),
        ("glass", [175, 213, 219, 160]),
        ("ice", [145, 183, 253, 255]),
        ("packed_ice", [141, 180, 250, 255]),
        ("blue_ice", [116, 167, 253, 255]),
        ("snow", [248, 248, 248, 255]),
        ("snow_layer", [248, 248, 248, 255]),
        ("powder_snow", [248, 253, 253, 255]),
        ("clay", [160, 166, 179, 255]),
        ("bricks", [151, 97, 83, 255]),
        ("brick_block", [151, 97, 83, 255]),
        ("bookshelf", [157, 128, 79, 255]),
        ("obsidian", [21, 18, 30, 255]),
        ("crying_obsidian", [42, 2, 74, 255]),
        ("netherrack", [111, 54, 52, 255]),
        ("soul_sand", [84, 64, 51, 255]),
        ("soul_soil", [75, 57, 46, 255]),
        ("glowstone", [254, 217, 108, 255]),
        ("nether_bricks", [44, 22, 26, 255]),
        ("nether_brick", [44, 22, 26, 255]),
        ("nether_wart_block", [123, 2, 2, 255]),
        ("warped_wart_block", [22, 119, 121, 255]),
        ("crimson_nylium", [130, 31, 31, 255]),
        ("warped_nylium", [43, 114, 101, 255]),
        ("crimson_stem", [92, 25, 29, 255]),
        ("warped_stem", [58, 58, 77, 255]),
        ("basalt", [73, 72, 77, 255]),
        ("smooth_basalt", [72, 72, 78, 255]),
        ("blackstone", [42, 36, 41, 255]),
        ("magma", [142, 63, 31, 255]),
        ("shroomlight", [240, 146, 70, 255]),
        ("ancient_debris", [94, 66, 58, 255]),
        ("end_stone", [219, 222, 158, 255]),
        ("end_stone_bricks", [218, 224, 162, 255]),
        ("purpur_block", [169, 125, 169, 255]),
        ("chorus_plant", [93, 57, 93, 255]),
        ("chorus_flower", [151, 120, 151, 255]),
        ("quartz_block", [235, 229, 222, 255]),
        ("stone_bricks", [122, 122, 122, 255]),
        ("stonebrick", [122, 122, 122, 255]),
        ("mossy_stone_bricks", [115, 121, 105, 255]),
        ("smooth_stone", [158, 158, 158, 255]),
        ("hardened_clay", [152, 94, 67, 255]),
        ("terracotta", [152, 94, 67, 255]),
        ("farmland", [98, 68, 38, 255]),
        ("dirt_path", [148, 121, 65, 255]),
        ("grass_path", [148, 121, 65, 255]),
        ("wheat", [220, 200, 80, 255]),
        ("carrots", [90, 160, 40, 255]),
        ("potatoes", [90, 160, 40, 255]),
        ("beetroot", [110, 140, 60, 255]),
        ("pumpkin", [198, 118, 24, 255]),
        ("carved_pumpkin", [198, 118, 24, 255]),
        ("melon_block", [111, 153, 31, 255]),
        ("cactus", [85, 127, 43, 255]),
        ("sugar_cane", [148, 192, 101, 255]),
        ("reeds", [148, 192, 101, 255]),
        ("bamboo", [110, 160, 40, 255]),
        ("vine", [60, 110, 40, 200]),
        ("short_grass", [110, 160, 60, 180]),
        ("tall_grass", [110, 160, 60, 180]),
        ("tallgrass", [110, 160, 60, 180]),
        ("double_plant", [110, 160, 60, 180]),
        ("fern", [100, 150, 60, 180]),
        ("large_fern", [100, 150, 60, 180]),
        ("seagrass", [70, 130, 60, 200]),
        ("kelp", [60, 120, 50, 220]),
        ("poppy", [200, 60, 50, 255]),
        ("dandelion", [230, 210, 60, 255]),
        ("torch", [255, 200, 100, 255]),
        ("mob_spawner", [40, 60, 80, 255]),
        ("chest", [160, 115, 50, 255]),
        ("crafting_table", [140, 105, 60, 255]),
        ("furnace", [120, 120, 120, 255]),
        ("mushroom_stem", [200, 190, 180, 255]),
        ("brown_mushroom_block", [140, 105, 80, 255]),
        ("red_mushroom_block", [180, 45, 40, 255]),
        ("brown_mushroom", [140, 105, 80, 255]),
        ("red_mushroom", [180, 45, 40, 255]),
        ("moss_block", [90, 120, 60, 255]),
        ("moss_carpet", [90, 120, 60, 255]),
        ("azalea", [95, 125, 60, 255]),
        ("big_dripleaf", [95, 135, 65, 255]),
        ("dripstone_block", [134, 107, 92, 255]),
        ("pointed_dripstone", [134, 107, 92, 255]),
        ("amethyst_block", [133, 97, 191, 255]),
        ("budding_amethyst", [132, 96, 186, 255]),
        ("copper_block", [192, 107, 79, 255]),
        ("cut_copper", [191, 106, 80, 255]),
        ("exposed_copper", [161, 125, 103, 255]),
        ("weathered_copper", [108, 153, 110, 255]),
        ("oxidized_copper", [82, 162, 132, 255]),
        ("raw_iron_block", [166, 135, 107, 255]),
        ("raw_gold_block", [221, 169, 46, 255]),
        ("raw_copper_block", [154, 105, 79, 255]),
        ("iron_block", [220, 220, 220, 255]),
        ("gold_block", [246, 208, 61, 255]),
        ("diamond_block", [98, 219, 214, 255]),
        ("emerald_block", [42, 203, 87, 255]),
        ("lapis_block", [30, 66, 140, 255]),
        ("redstone_block", [171, 27, 9, 255]),
        ("coal_block", [16, 16, 16, 255]),
        ("netherite_block", [66, 61, 63, 255]),
        ("hay_block", [166, 136, 38, 255]),
        ("bone_block", [210, 206, 179, 255]),
        ("sponge", [195, 192, 74, 255]),
        ("wet_sponge", [170, 175, 60, 255]),
        ("sea_lantern", [172, 199, 190, 255]),
        ("prismarine", [99, 156, 151, 255]),
        ("prismarine_bricks", [99, 171, 158, 255]),
        ("dark_prismarine", [51, 91, 75, 255]),
        ("mangrove_roots", [74, 59, 38, 255]),
        ("sculk", [12, 29, 36, 255]),
        ("sculk_catalyst", [15, 31, 38, 255]),
        ("pale_oak_log", [140, 135, 129, 255]),
        ("pale_oak_leaves", [130, 138, 120, 255]),
        ("pale_moss_block", [160, 168, 150, 255]),
        ("web", [230, 230, 230, 200]),
        ("scaffolding", [180, 145, 80, 200]),
        ("honey_block", [235, 160, 50, 255]),
        ("honeycomb_block", [229, 148, 29, 255]),
        ("slime", [110, 190, 90, 200]),
        ("tnt", [200, 60, 40, 255]),
        ("mud_bricks", [137, 103, 79, 255]),
        ("packed_mud", [142, 106, 79, 255]),
        ("ochre_froglight", [250, 245, 206, 255]),
        ("verdant_froglight", [229, 244, 228, 255]),
        ("pearlescent_froglight", [245, 235, 245, 255]),
    ];
    let mut map = HashMap::new();
    for (k, v) in entries {
        map.insert((*k).to_string(), *v);
    }
    map
}

impl ColorMap {
    pub fn builtin() -> Self {
        Self {
            exact: builtin_table(),
        }
    }

    pub fn get(&self, name: &str, color_state: Option<&str>) -> Rgba {
        let short = strip_ns(name);

        if let Some(c) = self.exact.get(short) {
            if let Some(state) = color_state {
                if let Some(dye) = dye(state) {
                    if is_dyed_family(short) {
                        return dye;
                    }
                }
            }
            return *c;
        }

        for &(dye_name, dye_color) in DYE_COLORS {
            if let Some(rest) = short.strip_prefix(dye_name) {
                if let Some(family) = rest.strip_prefix('_') {
                    return dyed_family_color(family, dye_color);
                }
            }
        }
        if let Some(state) = color_state {
            if let Some(c) = dye(state) {
                return c;
            }
        }

        heuristic(short)
    }
}

fn dye(name: &str) -> Option<Rgba> {
    DYE_COLORS.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

fn is_dyed_family(name: &str) -> bool {
    matches!(
        name,
        "wool" | "carpet" | "concrete" | "concrete_powder" | "stained_glass"
            | "stained_glass_pane" | "stained_hardened_clay" | "shulker_box" | "bed"
    )
}

fn dyed_family_color(family: &str, dye: Rgba) -> Rgba {
    match family {
        "terracotta" | "stained_hardened_clay" => [
            ((dye[0] as u16 + 120) / 2) as u8,
            ((dye[1] as u16 + 90) / 2) as u8,
            ((dye[2] as u16 + 70) / 2) as u8,
            255,
        ],
        "stained_glass" | "stained_glass_pane" => [dye[0], dye[1], dye[2], 160],
        _ => dye,
    }
}

fn heuristic(short: &str) -> Rgba {
    let n = short;
    let has = |s: &str| n.contains(s);

    if has("water") {
        return [63, 118, 228, 255];
    }
    if has("lava") || has("magma") {
        return [207, 92, 20, 255];
    }
    if has("leaves") || has("foliage") {
        return [60, 115, 40, 255];
    }
    if has("_log") || has("_wood") || has("stem") {
        return [100, 80, 50, 255];
    }
    if has("planks") || has("fence") || has("_stairs") && has("oak") {
        return [157, 128, 79, 255];
    }
    if has("sand") {
        return [219, 211, 160, 255];
    }
    if has("grass") || has("flower") || has("plant") || has("bush") || has("sapling") {
        return [110, 160, 60, 255];
    }
    if has("snow") || has("ice") {
        return [230, 240, 250, 255];
    }
    if has("nether") {
        return [111, 54, 52, 255];
    }
    if has("end_") || has("purpur") {
        return [219, 222, 158, 255];
    }
    if has("deepslate") {
        return [80, 80, 82, 255];
    }
    if has("copper") {
        return [192, 107, 79, 255];
    }
    if has("coral") {
        return [190, 100, 150, 255];
    }
    if has("mushroom") || has("fungus") {
        return [160, 110, 90, 255];
    }
    if has("brick") {
        return [151, 97, 83, 255];
    }
    if has("stone") || has("cobble") || has("rock") || has("andesite") || has("ore") {
        return [125, 125, 125, 255];
    }
    if has("dirt") || has("mud") || has("soil") {
        return [134, 96, 67, 255];
    }
    if has("glass") {
        return [175, 213, 219, 160];
    }

    [128, 128, 128, 255]
}
