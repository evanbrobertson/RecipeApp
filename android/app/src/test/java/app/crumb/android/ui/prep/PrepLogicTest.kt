package app.crumb.android.ui.prep

import app.crumb.core.MiseItem
import app.crumb.core.Vessel
import org.junit.Assert.assertEquals
import org.junit.Test

class PrepLogicTest {
    private fun item(
        vessel: Vessel,
        quantity: Double? = null,
        quantityMax: Double? = null,
        unit: String? = null,
        name: String = "thing",
    ) = MiseItem("raw", quantity, quantityMax, unit, name, null, vessel, null, null)

    @Test
    fun amountScalesTheQuantityAndPluralises() {
        assertEquals("2 cups", prepAmount(item(Vessel.SMALL, 2.0, unit = "cup"), 1.0))
        assertEquals("4 cups", prepAmount(item(Vessel.SMALL, 2.0, unit = "cup"), 2.0))
        assertEquals("1–2 cups", prepAmount(item(Vessel.SMALL, 1.0, 2.0, "cup"), 1.0))
    }

    @Test
    fun amountWithoutAQuantityReadsAsAPinch() {
        assertEquals("a pinch", prepAmount(item(Vessel.JAR, unit = "pinch"), 1.0))
        assertEquals("", prepAmount(item(Vessel.JAR), 1.0))
    }

    @Test
    fun groupsAreChoppingBowlsThenJarsLargestFirst() {
        val items = listOf(
            item(Vessel.BOARD), // 0
            item(Vessel.MEDIUM), // 1
            item(Vessel.SMALL), // 2
            item(Vessel.JAR), // 3
            item(Vessel.LARGE), // 4
            item(Vessel.PINCH), // 5
            item(Vessel.RAMEKIN), // 6
            item(Vessel.BOARD), // 7
        )
        val groups = prepGroups(items)
        assertEquals(listOf(PrepGroupKind.Chop, PrepGroupKind.Measure, PrepGroupKind.Reach), groups.map { it.kind })
        assertEquals(listOf(0, 7), groups[0].entries.map { it.index })
        assertEquals(listOf(4, 1, 2, 6, 5), groups[1].entries.map { it.index })
        assertEquals(listOf(3), groups[2].entries.map { it.index })
    }

    @Test
    fun emptyGroupsAreDropped() {
        val groups = prepGroups(listOf(item(Vessel.LARGE), item(Vessel.BOARD)))
        assertEquals(listOf(PrepGroupKind.Chop, PrepGroupKind.Measure), groups.map { it.kind })
    }

    @Test
    fun getOutCollapsesBoardsAndSkipsJars() {
        val items = listOf(
            item(Vessel.BOARD),
            item(Vessel.LARGE),
            item(Vessel.LARGE),
            item(Vessel.JAR),
            item(Vessel.BOARD),
        )
        assertEquals(listOf(VesselCount(Vessel.BOARD, 1), VesselCount(Vessel.LARGE, 2)), getOutCounts(items))
    }

    @Test
    fun getOutNounsAreLowercaseAndPlural() {
        assertEquals("large bowls", getOutNoun(VesselCount(Vessel.LARGE, 2)))
        assertEquals("board", getOutNoun(VesselCount(Vessel.BOARD, 1)))
        assertEquals("ramekins", getOutNoun(VesselCount(Vessel.RAMEKIN, 2)))
    }
}
