package app.crumb.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class RecipeChecksTest {
    @Test fun decodesAPhotoOnlyCheckWithNoStatus() {
        val json = """{"status":null,"canUndo":false,"flags":[{"id":7,"field":"image","kind":"dead_photo","state":"open"}]}"""
        val checks = CrumbJson.decodeFromString(RecipeChecks.serializer(), json)
        assertNull(checks.status)
        assertEquals("dead_photo", checks.flags.single().kind)
    }

    @Test fun decodesACheckedRecipe() {
        val checks = CrumbJson.decodeFromString(RecipeChecks.serializer(), """{"status":"done","canUndo":true,"flags":[]}""")
        assertEquals("done", checks.status)
    }
}
