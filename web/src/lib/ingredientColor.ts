/**
 * A plausible colour for what's in the bowl, from the ingredient name. Purely for fun.
 * Muted to sit with the Green Tile palette: creams, butter, sage, clay, earth.
 */
const RULES: [RegExp, string][] = [
  [
    /flour|sugar|salt|milk|cream|yogh?urt|rice|coconut|mayo|egg white|powder|oats?|cornstarch|breadcrumb/i,
    "#faf5e8",
  ],
  [
    /butter|egg|yolk|cheese|parmesan|cheddar|corn|mustard|honey|lemon|pineapple|polenta|turmeric/i,
    "#eed27a",
  ],
  [/oil|vinegar|stock|broth|wine|syrup|maple|soy|beer|juice/i, "#d2a24c"],
  [/tomato|chil+i|paprika|pepper(?!corn)|strawberr|raspberr|beet|red/i, "#b8543a"],
  [
    /basil|parsley|cilantro|coriander|herb|spinach|kale|pea|lime|mint|dill|chive|scallion|spring onion|green|avocado|pesto|zucchini|cucumber|leek|celery|broccoli/i,
    "#6a9a72",
  ],
  [
    /chocolate|cocoa|coffee|espresso|cinnamon|nutmeg|clove|molasses|brown sugar|beef|mince|mushroom|cumin|soy/i,
    "#6e4a36",
  ],
  [/carrot|orange|pumpkin|squash|sweet potato|apricot|peach|salmon/i, "#d9854f"],
  [
    /onion|garlic|shallot|ginger|potato|apple|pear|nut|almond|walnut|pecan|cashew|chicken|pork|tofu|bread|pasta|noodle/i,
    "#eadcb6",
  ],
  [/blueberr|blackberr|plum|grape|cabbage|eggplant|aubergine/i, "#6c4b6b"],
  [/water|ice/i, "#c7dedc"],
]

export function ingredientColor(name: string): string {
  return RULES.find(([re]) => re.test(name))?.[1] ?? "#d8bf9a"
}
