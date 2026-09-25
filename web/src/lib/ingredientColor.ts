/** A plausible colour for what's in the bowl, from the ingredient name. Purely for fun. */
const RULES: [RegExp, string][] = [
  [
    /flour|sugar|salt|milk|cream|yogh?urt|rice|coconut|mayo|egg white|powder|oats?|cornstarch|breadcrumb/i,
    "#f5efe3",
  ],
  [
    /butter|egg|yolk|cheese|parmesan|cheddar|corn|mustard|honey|lemon|pineapple|polenta|turmeric/i,
    "#f2cf5b",
  ],
  [/oil|vinegar|stock|broth|wine|syrup|maple|soy|beer|juice/i, "#d9a441"],
  [/tomato|chil+i|paprika|pepper(?!corn)|strawberr|raspberr|beet|red/i, "#d9472b"],
  [
    /basil|parsley|cilantro|coriander|herb|spinach|kale|pea|lime|mint|dill|chive|scallion|spring onion|green|avocado|pesto|zucchini|cucumber|leek|celery|broccoli/i,
    "#6f9a57",
  ],
  [
    /chocolate|cocoa|coffee|espresso|cinnamon|nutmeg|clove|molasses|brown sugar|beef|mince|mushroom|cumin|soy/i,
    "#6b4226",
  ],
  [/carrot|orange|pumpkin|squash|sweet potato|apricot|peach|salmon/i, "#ee8a3c"],
  [
    /onion|garlic|shallot|ginger|potato|apple|pear|nut|almond|walnut|pecan|cashew|chicken|pork|tofu|bread|pasta|noodle/i,
    "#e8d3a8",
  ],
  [/blueberr|blackberr|plum|grape|cabbage|eggplant|aubergine/i, "#5b3b73"],
  [/water|ice/i, "#bfe3f0"],
]

export function ingredientColor(name: string): string {
  return RULES.find(([re]) => re.test(name))?.[1] ?? "#d9b99b"
}
