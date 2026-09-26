package app.crumb.android.ui.prep

import androidx.compose.ui.graphics.Color
import org.junit.Assert.assertEquals
import org.junit.Test

class IngredientColorTest {
    @Test
    fun matchesTheFirstRuleThatApplies() {
        assertEquals(Color(0xFFFAF5E8), ingredientColor("2 cups flour"))
        assertEquals(Color(0xFFEED27A), ingredientColor("1 tbsp butter"))
        assertEquals(Color(0xFFD2A24C), ingredientColor("glug of olive oil"))
        assertEquals(Color(0xFFB8543A), ingredientColor("cracked pepper"))
        assertEquals(Color(0xFF6A9A72), ingredientColor("fresh basil"))
        assertEquals(Color(0xFF6E4A36), ingredientColor("dark chocolate"))
        assertEquals(Color(0xFFD9854F), ingredientColor("1 carrot"))
        assertEquals(Color(0xFFEADCB6), ingredientColor("1 brown onion"))
        assertEquals(Color(0xFF6C4B6B), ingredientColor("handful of blueberries"))
        assertEquals(Color(0xFFC7DEDC), ingredientColor("ice water"))
    }

    @Test
    fun fallsBackWhenNothingMatches() {
        assertEquals(DefaultIngredientColor, ingredientColor("mystery"))
        assertEquals(DefaultIngredientColor, ingredientColor(""))
    }

    @Test
    fun pepperIsNotPeppercorn() {
        assertEquals(Color(0xFFB8543A), ingredientColor("green pepper"))
        assertEquals(Color(0xFFEED27A), ingredientColor("peppercorn"))
    }
}
