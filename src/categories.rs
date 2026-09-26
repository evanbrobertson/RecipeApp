//! The fixed list of recipe categories, and mapping a site's own wording onto it.
//!
//! Recipe sites publish whatever they like as `recipeCategory` ("Dinner, Entree,
//! Sandwich", "One dish meal", "Main Course"), so Crumb files every recipe under one of
//! [`LIST`] instead. [`normalize`] reads the wording word by word: values are split on
//! commas, slashes, semicolons, "&" and "and", and the first value that maps wins; inside
//! one value the most specific kind wins ("Salad dressing" is a sauce, "Chicken soup" a
//! soup, "Breakfast sandwich" breakfast).
//!
//! Filing rules, calibrated on a hand-sorted library: cakes, cookies, bars, breads, pies,
//! tarts, pastry and frosting are Baking; puddings, ice cream, candy and plated sweets
//! (cobbler, mousse, tiramisu) are Dessert; dips are Snacks, dressings and spreads Sauces;
//! casseroles, one-pot dishes, sandwiches and lunch are Main.

/// Every category, in display order. "Other" is always last.
pub const LIST: [&str; 11] = [
    "Breakfast",
    "Main",
    "Side",
    "Soup",
    "Salad",
    "Baking",
    "Dessert",
    "Snack",
    "Sauce",
    "Drink",
    "Other",
];

/// For picking one kind inside a single value: the most specific kind first. ("Other"
/// is only ever the value itself; see [`listed`].)
const PRIORITY: [&str; 10] = [
    "Drink",
    "Sauce",
    "Breakfast",
    "Soup",
    "Salad",
    "Baking",
    "Dessert",
    "Snack",
    "Side",
    "Main",
];

/// Phrases of two or three words, matched before the single words they contain
/// ("coffee cake" is baking, not a drink).
const PHRASES: &[(&str, &str)] = &[
    ("main course", "Main"),
    ("main dish", "Main"),
    ("main meal", "Main"),
    ("one dish meal", "Main"),
    ("one pot", "Main"),
    ("one pan", "Main"),
    ("sheet pan", "Main"),
    ("stir fry", "Main"),
    ("pot pie", "Main"),
    ("shepherds pie", "Main"),
    ("shepherd s pie", "Main"),
    ("cottage pie", "Main"),
    ("side dish", "Side"),
    ("coffee cake", "Baking"),
    ("pie crust", "Baking"),
    ("baked good", "Baking"),
    ("baked goods", "Baking"),
    ("quick bread", "Baking"),
    ("ice cream", "Dessert"),
    ("bread pudding", "Dessert"),
    ("sweet potato", "Side"),
    ("sweet potatoes", "Side"),
    ("panna cotta", "Dessert"),
    ("hot chocolate", "Drink"),
    ("finger food", "Snack"),
    ("party food", "Snack"),
    ("hors d oeuvre", "Snack"),
    ("hors d oeuvres", "Snack"),
    ("game day", "Snack"),
    ("salad dressing", "Sauce"),
    ("dry rub", "Sauce"),
    ("spice mix", "Sauce"),
    ("spice blend", "Sauce"),
];

/// Single words, in the singular (plurals are found by [`word`]).
const WORDS: &[(&str, &str)] = &[
    // Breakfast
    ("breakfast", "Breakfast"),
    ("brunch", "Breakfast"),
    ("pancake", "Breakfast"),
    ("waffle", "Breakfast"),
    ("crepe", "Breakfast"),
    ("crêpe", "Breakfast"),
    ("oatmeal", "Breakfast"),
    ("porridge", "Breakfast"),
    ("granola", "Breakfast"),
    ("omelet", "Breakfast"),
    ("omelette", "Breakfast"),
    // Main
    ("main", "Main"),
    ("mains", "Main"),
    ("dinner", "Main"),
    ("entree", "Main"),
    ("entrée", "Main"),
    ("supper", "Main"),
    ("lunch", "Main"),
    ("casserole", "Main"),
    ("sandwich", "Main"),
    ("burger", "Main"),
    ("pasta", "Main"),
    ("pizza", "Main"),
    ("curry", "Main"),
    ("stirfry", "Main"),
    ("taco", "Main"),
    ("burrito", "Main"),
    ("enchilada", "Main"),
    ("noodle", "Main"),
    ("lasagna", "Main"),
    ("lasagne", "Main"),
    ("risotto", "Main"),
    ("roast", "Main"),
    ("bbq", "Main"),
    ("barbecue", "Main"),
    ("kebab", "Main"),
    ("meat", "Main"),
    ("poultry", "Main"),
    ("chicken", "Main"),
    ("beef", "Main"),
    ("pork", "Main"),
    ("lamb", "Main"),
    ("seafood", "Main"),
    ("fish", "Main"),
    ("meatloaf", "Main"),
    // Side
    ("side", "Side"),
    ("vegetable", "Side"),
    ("veggie", "Side"),
    ("veg", "Side"),
    ("rice", "Side"),
    ("potato", "Side"),
    ("potatoes", "Side"),
    ("stuffing", "Side"),
    // Soup
    ("soup", "Soup"),
    ("stew", "Soup"),
    ("chili", "Soup"),
    ("chilli", "Soup"),
    ("chowder", "Soup"),
    ("broth", "Soup"),
    ("bisque", "Soup"),
    ("stock", "Soup"),
    ("gumbo", "Soup"),
    // Salad
    ("salad", "Salad"),
    ("slaw", "Salad"),
    ("coleslaw", "Salad"),
    // Baking
    ("baking", "Baking"),
    ("bake", "Baking"),
    ("bread", "Baking"),
    ("loaf", "Baking"),
    ("cookie", "Baking"),
    ("biscuit", "Baking"),
    ("cake", "Baking"),
    ("cupcake", "Baking"),
    ("cheesecake", "Baking"),
    ("muffin", "Baking"),
    ("bar", "Baking"),
    ("brownie", "Baking"),
    ("blondie", "Baking"),
    ("pastry", "Baking"),
    ("pie", "Baking"),
    ("tart", "Baking"),
    ("scone", "Baking"),
    ("shortbread", "Baking"),
    ("meringue", "Baking"),
    ("macaron", "Baking"),
    ("doughnut", "Baking"),
    ("donut", "Baking"),
    ("bagel", "Baking"),
    ("focaccia", "Baking"),
    ("sourdough", "Baking"),
    ("frosting", "Baking"),
    ("icing", "Baking"),
    ("buttercream", "Baking"),
    // Dessert
    ("dessert", "Dessert"),
    ("sweet", "Dessert"),
    ("treat", "Dessert"),
    ("pudding", "Dessert"),
    ("custard", "Dessert"),
    ("mousse", "Dessert"),
    ("gelato", "Dessert"),
    ("sorbet", "Dessert"),
    ("candy", "Dessert"),
    ("fudge", "Dessert"),
    ("truffle", "Dessert"),
    ("cobbler", "Dessert"),
    ("crumble", "Dessert"),
    ("crisp", "Dessert"),
    ("trifle", "Dessert"),
    ("tiramisu", "Dessert"),
    ("cannoli", "Dessert"),
    ("eclair", "Dessert"),
    ("éclair", "Dessert"),
    // Snack
    ("snack", "Snack"),
    ("appetizer", "Snack"),
    ("appetiser", "Snack"),
    ("starter", "Snack"),
    ("dip", "Snack"),
    ("canape", "Snack"),
    ("canapé", "Snack"),
    ("tapas", "Snack"),
    ("nibble", "Snack"),
    // Sauce
    ("sauce", "Sauce"),
    ("dressing", "Sauce"),
    ("gravy", "Sauce"),
    ("condiment", "Sauce"),
    ("marinade", "Sauce"),
    ("spread", "Sauce"),
    ("salsa", "Sauce"),
    ("pesto", "Sauce"),
    ("seasoning", "Sauce"),
    ("chutney", "Sauce"),
    ("relish", "Sauce"),
    ("jam", "Sauce"),
    ("aioli", "Sauce"),
    ("mayo", "Sauce"),
    ("mayonnaise", "Sauce"),
    // Drink
    ("drink", "Drink"),
    ("beverage", "Drink"),
    ("cocktail", "Drink"),
    ("mocktail", "Drink"),
    ("smoothie", "Drink"),
    ("milkshake", "Drink"),
    ("shake", "Drink"),
    ("juice", "Drink"),
    ("lemonade", "Drink"),
    ("tea", "Drink"),
    ("coffee", "Drink"),
    ("punch", "Drink"),
    ("latte", "Drink"),
];

/// The listed name for `s` when it is one already (any case).
pub fn listed(s: &str) -> Option<&'static str> {
    let s = s.trim();
    LIST.iter().copied().find(|c| c.eq_ignore_ascii_case(s))
}

fn lookup(table: &[(&str, &'static str)], key: &str) -> Option<&'static str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, c)| *c)
}

/// A single word, or its singular ("cookies", "pastries", "dishes", "sandwiches").
fn word(w: &str) -> Option<&'static str> {
    lookup(WORDS, w)
        .or_else(|| w.strip_suffix('s').and_then(|s| lookup(WORDS, s)))
        .or_else(|| {
            w.strip_suffix("ies")
                .and_then(|s| lookup(WORDS, &format!("{s}y")))
        })
        .or_else(|| w.strip_suffix("es").and_then(|s| lookup(WORDS, s)))
}

/// The kind one value names, or None: phrases first, then words; the most specific wins.
fn one_value(value: &str) -> Option<&'static str> {
    let words: Vec<String> = value
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(String::from)
        .collect();
    let mut found: Vec<&'static str> = Vec::new();
    let mut i = 0;
    'scan: while i < words.len() {
        for n in [3, 2] {
            if i + n <= words.len()
                && let Some(c) = lookup(PHRASES, &words[i..i + n].join(" "))
            {
                found.push(c);
                i += n;
                continue 'scan;
            }
        }
        if let Some(c) = word(&words[i]) {
            found.push(c);
        }
        i += 1;
    }
    PRIORITY.iter().copied().find(|c| found.contains(c))
}

/// Maps a site's category wording to [`LIST`], or None when nothing in it maps.
pub fn normalize(raw: &str) -> Option<&'static str> {
    if let Some(c) = listed(raw) {
        return Some(c);
    }
    let lower = raw.to_lowercase();
    lower
        .split([',', '/', ';', '|', '&', '+', '\n'])
        .flat_map(|part| part.split(" and "))
        .find_map(one_value)
}

/// A category from outside (a scraped page, pasted text, a file, a photo): the listed
/// value it maps to, or None, so Wee Chef can pick one.
pub fn for_import(raw: Option<&str>) -> Option<String> {
    raw.and_then(normalize).map(String::from)
}

/// A category someone set (the editor, the API, Claude): the listed value it maps to,
/// "Other" when nothing in it maps, None when blank.
pub fn for_save(raw: Option<String>) -> Option<String> {
    let raw = raw?;
    if raw.trim().is_empty() {
        return None;
    }
    Some(normalize(&raw).unwrap_or("Other").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_site_wording_to_the_list() {
        for (raw, want) in [
            ("Dinner, Entree, Sandwich", Some("Main")),
            ("One dish meal", Some("Main")),
            ("One Dish Meal", Some("Main")),
            ("Main Course", Some("Main")),
            ("main dishes", Some("Main")),
            ("Mains", Some("Main")),
            ("Entrée", Some("Main")),
            ("Lunch", Some("Main")),
            ("lunch", Some("Main")),
            ("Supper", Some("Main")),
            ("Casserole", Some("Main")),
            ("Casseroles", Some("Main")),
            ("One-Pot", Some("Main")),
            ("Stir-fry", Some("Main")),
            ("Stir Fry", Some("Main")),
            ("Burgers", Some("Main")),
            ("Pasta", Some("Main")),
            ("Tacos", Some("Main")),
            ("Chicken Pot Pie", Some("Main")),
            ("Shepherd's Pie", Some("Main")),
            ("Chicken Recipes", Some("Main")),
            ("Dessert", Some("Dessert")),
            ("Desserts", Some("Dessert")),
            ("Puddings", Some("Dessert")),
            ("Ice Cream", Some("Dessert")),
            ("Candy", Some("Dessert")),
            ("Cobbler", Some("Dessert")),
            ("cookies", Some("Baking")),
            ("Cookies, Dessert", Some("Baking")),
            ("Dessert, Cookies", Some("Dessert")),
            ("Christmas Cookies", Some("Baking")),
            ("Cakes", Some("Baking")),
            ("Cupcakes", Some("Baking")),
            ("Muffins", Some("Baking")),
            ("Bars", Some("Baking")),
            ("Brownies", Some("Baking")),
            ("Pastries", Some("Baking")),
            ("Pie", Some("Baking")),
            ("Pies and Tarts", Some("Baking")),
            ("Pie Crust", Some("Baking")),
            ("Scones", Some("Baking")),
            ("Biscuits", Some("Baking")),
            ("Bread", Some("Baking")),
            ("Quick Breads", Some("Baking")),
            ("Coffee Cake", Some("Baking")),
            ("Frosting", Some("Baking")),
            ("Baked Goods", Some("Baking")),
            ("Side Dish", Some("Side")),
            ("Side dishes", Some("Side")),
            ("Sides", Some("Side")),
            ("Vegetables", Some("Side")),
            ("Rice", Some("Side")),
            ("Potatoes", Some("Side")),
            ("Soup", Some("Soup")),
            ("Soups and Stews", Some("Soup")),
            ("Chili", Some("Soup")),
            ("Chowder", Some("Soup")),
            ("Chicken Soup", Some("Soup")),
            ("Salad", Some("Salad")),
            ("Salads", Some("Salad")),
            ("Coleslaw", Some("Salad")),
            ("Side Salad", Some("Salad")),
            ("Pasta Salad", Some("Salad")),
            ("Breakfast", Some("Breakfast")),
            ("Brunch", Some("Breakfast")),
            ("Breakfast, Lunch", Some("Breakfast")),
            ("Breakfast Sandwich", Some("Breakfast")),
            ("Pancakes", Some("Breakfast")),
            ("Snack", Some("Snack")),
            ("Snacks", Some("Snack")),
            ("Appetizer", Some("Snack")),
            ("Appetizers & Snacks", Some("Snack")),
            ("Starter", Some("Snack")),
            ("Dip", Some("Snack")),
            ("Dips", Some("Snack")),
            ("Finger Food", Some("Snack")),
            ("Party Food", Some("Snack")),
            ("Sauce", Some("Sauce")),
            ("Sauces", Some("Sauce")),
            ("Dipping Sauce", Some("Sauce")),
            ("Salad Dressing", Some("Sauce")),
            ("Dressings", Some("Sauce")),
            ("Gravy", Some("Sauce")),
            ("Condiment", Some("Sauce")),
            ("Condiments/Sauces", Some("Sauce")),
            ("Marinade", Some("Sauce")),
            ("Spreads", Some("Sauce")),
            ("Drink", Some("Drink")),
            ("Drinks", Some("Drink")),
            ("Beverages", Some("Drink")),
            ("Cocktails", Some("Drink")),
            ("Smoothie", Some("Drink")),
            ("Hot Chocolate", Some("Drink")),
            ("Other", Some("Other")),
            ("OTHER", Some("Other")),
            ("main", Some("Main")),
            ("Holiday", None),
            ("Easy, Quick, Vegan", None),
            ("Holiday, Main Course", Some("Main")),
            ("", None),
            ("   ", None),
            ("Mac and Cheese", None),
            ("Sweet Potatoes", Some("Side")),
            ("Bread Pudding", Some("Dessert")),
            ("Other Desserts", Some("Dessert")),
        ] {
            assert_eq!(normalize(raw), want, "{raw:?}");
        }
    }

    #[test]
    fn every_listed_name_maps_to_itself() {
        for c in LIST {
            assert_eq!(normalize(c), Some(c));
            assert_eq!(normalize(&c.to_lowercase()), Some(c));
        }
    }

    #[test]
    fn imports_drop_what_doesnt_map_and_saves_file_it_under_other() {
        assert_eq!(for_import(Some("Dinner, Entree")), Some("Main".into()));
        assert_eq!(for_import(Some("Holiday")), None);
        assert_eq!(for_import(None), None);
        assert_eq!(for_save(Some("lunch".into())), Some("Main".into()));
        assert_eq!(for_save(Some("whatever".into())), Some("Other".into()));
        assert_eq!(for_save(Some(" ".into())), None);
        assert_eq!(for_save(None), None);
    }
}
