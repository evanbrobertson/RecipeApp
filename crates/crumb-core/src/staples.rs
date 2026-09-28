//! Staples: the ingredients most of the box's recipes call for, for Wee Chef's
//! "keep these on hand" tip on the Suggestions page.
//!
//! Each ingredient line is cut down to what you'd buy: `"2 cups all-purpose flour,
//! sifted"` → flour, `"3 garlic cloves, minced"` → garlic, `"Kosher salt and freshly
//! ground black pepper"` → salt and black pepper. Quantities, units, prep, sizes and
//! other descriptions go; a few common synonyms share one entry ("scallions" and "green
//! onions"), shown as the way the box writes it most.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::ingredients::parse_ingredient;

/// One staple and how many recipes use it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Staple {
    /// Lowercase, as the box most often writes it ("olive oil", "eggs").
    pub name: String,
    /// Recipes calling for it at least once.
    pub recipes: usize,
}

/// `GET /api/staples`: Wee Chef's "keep these on hand" tip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Staples {
    /// Recipes in the box, the "of" in "in 14 of 30 recipes".
    pub recipes: usize,
    /// Most-used first; empty for a box too small to tell.
    pub staples: Vec<Staple>,
}

/// How many staples the tip lists: six rows of three.
pub const LIMIT: usize = 18;

/// Staples nearly every box leans on, left out so the tip shows what's particular to this
/// one ("oh look, I use lots of salt" tells nobody anything). Keys, as [`key_of`] makes them.
const OBVIOUS: [&str; 22] = [
    "salt",
    "black pepper",
    "butter",
    "flour",
    "sugar",
    "brown sugar",
    "powdered sugar",
    "egg",
    "milk",
    "oil",
    "olive oil",
    "vegetable oil",
    "canola oil",
    "neutral oil",
    "cooking spray",
    "baking powder",
    "baking soda",
    "vanilla",
    "vanilla extract",
    "garlic",
    "onion",
    "yellow onion",
];

/// Fewer recipes than this and there's nothing to tell apart from chance.
pub const MIN_RECIPES: usize = 3;

/// Words that describe an ingredient (size, freshness, cut, grade) rather than name it.
static DESCRIPTORS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    "large small medium big jumbo extra fresh freshly ground chopped minced diced sliced grated \
     shredded crushed chilled softened melted room temperature unsalted salted virgin kosher sea \
     fine flaky table finely roughly coarsely coarse thinly thickly organic whole raw dried \
     frozen ripe boneless skinless plain pure optional divided heaping heaped level good quality \
     homemade store bought prepared jarred packed lightly loosely firmly about approximately \
     approx plus more additional taste needed serving garnish of the a an to into for some few \
     peeled cubed halved quartered trimmed rinsed drained washed sifted beaten whisked toasted \
     dark light granulated caster castor superfine all purpose ap white couple mixed"
        .split_whitespace()
        .collect()
});

/// Units, containers and cuts: dropped when something else names the ingredient
/// ("garlic cloves" → garlic), kept when they're all there is ("cloves").
static FORMS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    "clove cloves leaf leaves sprig sprigs stalk stalks rib ribs head heads bulb bulbs bunch \
     bunches handful handfuls pinch pinches dash dashes can cans tin tins jar jars package \
     packages packet packets pkg bag bags box boxes piece pieces slice slices stick sticks"
        .split_whitespace()
        .collect()
});

/// Measures, always dropped: a line that's only these ("250 g") names nothing.
static MEASURES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    "cup cups c t tsp tsps tbsp tbsps tbs tbl teaspoon teaspoons tablespoon tablespoons fl oz \
     ounce ounces lb lbs pound pounds g gr gram grams kg kilogram kilograms ml l liter liters \
     litre litres milliliter milliliters millilitre millilitres quart quarts pint pints"
        .split_whitespace()
        .collect()
});

/// Words kept even though [`DESCRIPTORS`] would drop them, when they're part of the name.
const KEEP_WHOLE: [&str; 4] = [
    "white wine",
    "white vinegar",
    "white rice",
    "white wine vinegar",
];

/// Singular keys that are the same thing to keep in the cupboard → one key.
const SYNONYMS: [(&str, &str); 14] = [
    ("flour", "flour"),
    ("pepper", "black pepper"),
    ("peppercorn", "black pepper"),
    ("scallion", "green onion"),
    ("spring onion", "green onion"),
    ("bicarbonate soda", "baking soda"),
    ("bicarb soda", "baking soda"),
    ("soya sauce", "soy sauce"),
    ("confectioner sugar", "powdered sugar"),
    ("confectioners sugar", "powdered sugar"),
    ("icing sugar", "powdered sugar"),
    ("chicken broth", "chicken stock"),
    ("beef broth", "beef stock"),
    ("vegetable broth", "vegetable stock"),
];

/// "tomatoes" → "tomato", "cherries" → "cherry", "eggs" → "egg". Only ever compared
/// with itself, so an odd stem ("molass") is harmless.
fn singular(word: &str) -> String {
    if word == "leaves" {
        return "leaf".into();
    }
    let len = word.len();
    if len > 4 && word.ends_with("ies") {
        return format!("{}y", &word[..len - 3]);
    }
    if len > 4
        && (word.ends_with("oes")
            || word.ends_with("ches")
            || word.ends_with("shes")
            || word.ends_with("xes"))
    {
        return word[..len - 2].to_string();
    }
    if len > 3 && word.ends_with('s') && !word.ends_with("ss") && !word.ends_with("us") {
        return word[..len - 1].to_string();
    }
    word.to_string()
}

fn key_of(display: &str) -> String {
    let key = display
        .split([' ', '-'])
        .map(singular)
        .collect::<Vec<_>>()
        .join(" ");
    SYNONYMS
        .iter()
        .find(|(from, _)| *from == key)
        .map_or(key, |(_, to)| to.to_string())
}

fn is_descriptor(word: &str) -> bool {
    DESCRIPTORS.contains(word) || (word.contains('-') && word.split('-').all(is_descriptor))
}

/// Tidies one phrase ("freshly ground black pepper") into what you'd buy ("black pepper").
fn tidy(phrase: &str) -> Option<String> {
    let cleaned: String = phrase
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphabetic() || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect();
    let mut words: Vec<&str> = cleaned
        .split_whitespace()
        .map(|w| w.trim_matches('-'))
        .filter(|w| !w.is_empty())
        .collect();
    // "juice of 2 lemons" → lemon juice; "zest and juice of 1 lime" → lime juice
    let of = words.iter().take(4).position(|w| *w == "of");
    let fruit_part = of.is_some_and(|of| {
        of + 1 < words.len()
            && words[..of]
                .iter()
                .all(|w| matches!(*w, "juice" | "zest" | "and"))
    });
    let owned: String;
    if let Some(of) = of.filter(|_| fruit_part) {
        let head = if words[..of].contains(&"juice") {
            "juice"
        } else {
            "zest"
        };
        let fruit = words[of + 1..]
            .iter()
            .map(|w| singular(w))
            .collect::<Vec<_>>();
        owned = format!("{} {head}", fruit.join(" "));
        words = owned.split(' ').collect();
    }
    let joined = words.join(" ");
    let kept = KEEP_WHOLE
        .iter()
        .filter(|k| joined.contains(*k))
        .max_by_key(|k| k.len());
    let mut words: Vec<&str> = match kept {
        Some(k) => k.split(' ').collect(),
        None => words
            .into_iter()
            .filter(|w| !is_descriptor(w) && !MEASURES.contains(w))
            .collect(),
    };
    // "peeled and sliced" leaves a lone "and"
    while words.first() == Some(&"and") {
        words.remove(0);
    }
    while words.last() == Some(&"and") {
        words.pop();
    }
    // "bay leaf" is the ingredient, not a bay
    if words.first() == Some(&"bay") && words.len() == 2 {
        return Some("bay leaves".into());
    }
    let named: Vec<&str> = words
        .iter()
        .copied()
        .filter(|w| !FORMS.contains(w))
        .collect();
    let words = if named.is_empty() { words } else { named };
    // Water isn't something to shop for; four-plus words is a note, not an ingredient
    if words.is_empty()
        || words.iter().filter(|w| **w != "and").count() > 3
        || words.last().is_some_and(|w| matches!(*w, "water" | "ice"))
    {
        return None;
    }
    Some(words.join(" "))
}

/// The staples one ingredient line names: usually one, two for "salt and pepper",
/// none for a heading, water or a line that isn't an ingredient.
pub fn staple_names(line: &str) -> Vec<String> {
    let line = line.trim();
    if line.is_empty() || line.ends_with(':') || !line.chars().any(char::is_alphabetic) {
        return Vec::new();
    }
    let parsed = parse_ingredient(line);
    let mut name = parsed.name.to_lowercase();
    // Whatever's in brackets is a note: "(about 2 lemons)"
    while let (Some(open), Some(close)) = (name.find('('), name.find(')')) {
        if close < open {
            break;
        }
        name.replace_range(open..=close, " ");
    }
    // "butter or margarine" → butter, "oil for frying" → oil; "salt, to taste" was
    // already split off as prep
    let name = format!(" {name} ");
    let name = name
        .split([',', ';'])
        .next()
        .unwrap_or("")
        .split(" or ")
        .next()
        .unwrap_or("")
        .split(" for ")
        .next()
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("");

    // "salt and pepper": both, but "sweet and sour sauce" stays one thing
    let parts: Vec<String> = name
        .split(" and ")
        .flat_map(|p| p.split(" & "))
        .filter_map(tidy)
        .collect();
    let seasoning = |p: &String| matches!(key_of(p).as_str(), "salt" | "black pepper");
    if parts.len() > 1 && parts.iter().any(seasoning) {
        return parts;
    }
    tidy(name).into_iter().collect()
}

/// The ingredients most of the box's recipes use, most-used first: each recipe counts
/// once per staple, only staples in at least two recipes are listed, and the [`OBVIOUS`]
/// ones never are. Empty for a box of fewer than [`MIN_RECIPES`] recipes.
pub fn staples<I, R, S>(recipes: I, limit: usize) -> Vec<Staple>
where
    I: IntoIterator<Item = R>,
    R: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut total = 0;
    // key → (recipes, first seen, how it's written → times)
    let mut seen: HashMap<String, (usize, usize, HashMap<String, usize>)> = HashMap::new();
    for recipe in recipes {
        total += 1;
        let mut in_recipe = HashSet::new();
        for line in recipe {
            for name in staple_names(line.as_ref()) {
                let key = key_of(&name);
                let order = seen.len();
                let entry = seen
                    .entry(key.clone())
                    .or_insert((0, order, HashMap::new()));
                *entry.2.entry(name).or_default() += 1;
                if in_recipe.insert(key) {
                    entry.0 += 1;
                }
            }
        }
    }
    if total < MIN_RECIPES {
        return Vec::new();
    }
    let mut out: Vec<(usize, usize, String)> = seen
        .into_iter()
        .filter(|(key, (n, _, _))| *n >= 2 && !OBVIOUS.contains(&key.as_str()))
        .map(|(key, (n, order, forms))| {
            // Plural when the box ever writes one ("eggs" over "egg"), else its usual spelling
            let name = forms
                .into_iter()
                .max_by(|a, b| {
                    let plural = |(f, _): &(String, usize)| {
                        f.rsplit(' ').next().is_some_and(|w| singular(w) != w)
                    };
                    (plural(a), a.1, &b.0).cmp(&(plural(b), b.1, &a.0))
                })
                .map(|(f, _)| f)
                .unwrap_or(key);
            (n, order, name)
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    out.into_iter()
        .take(limit)
        .map(|(recipes, _, name)| Staple { name, recipes })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(line: &str) -> Vec<String> {
        staple_names(line)
    }

    #[test]
    fn strips_amounts_units_and_prep() {
        assert_eq!(names("2 cups all-purpose flour, sifted"), ["flour"]);
        assert_eq!(names("3 garlic cloves, minced"), ["garlic"]);
        assert_eq!(names("2 cloves garlic"), ["garlic"]);
        assert_eq!(names("1/2 tsp ground cloves"), ["cloves"]);
        assert_eq!(names("2 tbsp extra-virgin olive oil"), ["olive oil"]);
        assert_eq!(names("1 large onion, finely chopped"), ["onion"]);
        assert_eq!(
            names("½ cup (1 stick) unsalted butter, softened"),
            ["butter"]
        );
        assert_eq!(names("1 (14 oz) can diced tomatoes"), ["tomatoes"]);
        assert_eq!(names("2 large eggs, room temperature"), ["eggs"]);
        assert_eq!(names("Juice of 1 lemon"), ["lemon juice"]);
        assert_eq!(names("juice of 2 oranges"), ["orange juice"]);
        assert_eq!(names("Zest and juice of 2 limes"), ["lime juice"]);
        assert_eq!(names("2 bay leaves"), ["bay leaves"]);
        assert_eq!(names("Vegetable oil, for frying"), ["vegetable oil"]);
        assert_eq!(names("oil for frying"), ["oil"]);
        assert_eq!(names("a couple of eggs"), ["eggs"]);
        assert_eq!(names("1 cup light brown sugar, packed"), ["brown sugar"]);
        assert_eq!(names("2 tbsp butter or margarine"), ["butter"]);
        assert_eq!(names("1 cup dry white wine"), ["white wine"]);
        assert_eq!(names("2 tbsp light soy sauce"), ["soy sauce"]);
        assert_eq!(names("1 tbsp white wine vinegar"), ["white wine vinegar"]);
    }

    #[test]
    fn splits_salt_and_pepper() {
        assert_eq!(
            names("Kosher salt and freshly ground black pepper, to taste"),
            ["salt", "black pepper"]
        );
        assert_eq!(names("Salt & pepper"), ["salt", "pepper"]);
        assert_eq!(
            names("1 cup sweet and sour sauce"),
            ["sweet and sour sauce"]
        );
    }

    #[test]
    fn skips_water_headings_and_notes() {
        assert!(names("1 cup warm water").is_empty());
        assert!(names("Ice, for serving").is_empty());
        assert!(names("For the sauce:").is_empty());
        assert!(names("250").is_empty());
        assert!(names("250 g").is_empty());
        assert!(names("For the Dough").is_empty());
        assert!(names("see the note below about the best brand to buy").is_empty());
    }

    #[test]
    fn counts_recipes_merges_synonyms_and_skips_the_obvious() {
        let box_ = [
            vec![
                "2 eggs",
                "1 egg yolk",
                "Salt and pepper",
                "3 scallions, sliced",
            ],
            vec!["1 egg", "1 tsp sea salt", "2 green onions"],
            vec!["1 large egg", "Pinch of kosher salt", "1 egg, beaten"],
            vec!["2 cups water", "1 tsp salt", "1 tbsp soy sauce"],
            vec!["2 tbsp soya sauce", "1 bunch green onions"],
        ];
        let got = staples(box_.clone(), 10);
        assert_eq!(
            got,
            [
                Staple {
                    name: "green onions".into(),
                    recipes: 3
                },
                Staple {
                    name: "soy sauce".into(),
                    recipes: 2
                },
            ]
        );
        assert_eq!(staples(box_, 1).len(), 1);
    }

    #[test]
    fn needs_a_few_recipes() {
        assert!(staples([vec!["salt"], vec!["salt"]], 10).is_empty());
    }
}
