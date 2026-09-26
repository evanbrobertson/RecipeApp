package app.crumb.android.ui.prep

import androidx.compose.ui.graphics.Color

/**
 * A plausible colour for what's in the bowl, from the ingredient name (web
 * web/src/lib/ingredientColor.ts). Purely for fun, and muted to sit with the Green Tile
 * palette: creams, butter, sage, clay, earth. The rules and their order are pinned by
 * IngredientColorTest.
 */
private val IngredientColorRules: List<Pair<Regex, Color>> = listOf(
    Regex(
        "flour|sugar|salt|milk|cream|yogh?urt|rice|coconut|mayo|egg white|powder|oats?|cornstarch|breadcrumb",
        RegexOption.IGNORE_CASE,
    ) to Color(0xFFFAF5E8),
    Regex(
        "butter|egg|yolk|cheese|parmesan|cheddar|corn|mustard|honey|lemon|pineapple|polenta|turmeric",
        RegexOption.IGNORE_CASE,
    ) to Color(0xFFEED27A),
    Regex("oil|vinegar|stock|broth|wine|syrup|maple|soy|beer|juice", RegexOption.IGNORE_CASE) to Color(0xFFD2A24C),
    Regex(
        "tomato|chil+i|paprika|pepper(?!corn)|strawberr|raspberr|beet|red",
        RegexOption.IGNORE_CASE,
    ) to Color(0xFFB8543A),
    Regex(
        "basil|parsley|cilantro|coriander|herb|spinach|kale|pea|lime|mint|dill|chive|scallion|spring onion|green|avocado|pesto|zucchini|cucumber|leek|celery|broccoli",
        RegexOption.IGNORE_CASE,
    ) to Color(0xFF6A9A72),
    Regex(
        "chocolate|cocoa|coffee|espresso|cinnamon|nutmeg|clove|molasses|brown sugar|beef|mince|mushroom|cumin|soy",
        RegexOption.IGNORE_CASE,
    ) to Color(0xFF6E4A36),
    Regex("carrot|orange|pumpkin|squash|sweet potato|apricot|peach|salmon", RegexOption.IGNORE_CASE) to Color(0xFFD9854F),
    Regex(
        "onion|garlic|shallot|ginger|potato|apple|pear|nut|almond|walnut|pecan|cashew|chicken|pork|tofu|bread|pasta|noodle",
        RegexOption.IGNORE_CASE,
    ) to Color(0xFFEADCB6),
    Regex(
        "blueberr|blackberr|plum|grape|cabbage|eggplant|aubergine",
        RegexOption.IGNORE_CASE,
    ) to Color(0xFF6C4B6B),
    Regex("water|ice", RegexOption.IGNORE_CASE) to Color(0xFFC7DEDC),
)

/** The fallback for anything the rules don't recognise. */
val DefaultIngredientColor = Color(0xFFD8BF9A)

fun ingredientColor(name: String): Color =
    IngredientColorRules.firstOrNull { it.first.containsMatchIn(name) }?.second ?: DefaultIngredientColor
