package app.crumb.android.ui.prep

import app.crumb.core.MiseItem
import app.crumb.core.Vessel
import app.crumb.core.formatQuantity
import app.crumb.core.pluralUnit

// Pure groupings and formatting for mise en place (web/src/islands/PrepPage.svelte). The
// vessel order, the "Get out:" tally and the amount line are pinned by PrepLogicTest.

/** Which of the web's three sections an item belongs to. */
enum class PrepGroupKind { Chop, Measure, Reach }

/** An item at its index in the recipe's flat ingredient list (the web's ready-state key). */
data class PrepEntry(val index: Int, val item: MiseItem)

/** One section of the countertop: "Chop & prep", "Measure into bowls" or "Keep within reach". */
data class PrepGroup(val kind: PrepGroupKind, val entries: List<PrepEntry>)

/** Bowls largest first, as the web's SIZE_ORDER (spec §6.1). */
private val SizeOrder = listOf(Vessel.LARGE, Vessel.MEDIUM, Vessel.SMALL, Vessel.RAMEKIN, Vessel.PINCH)

/** The three sections, in order, with the empty ones dropped. */
fun prepGroups(items: List<MiseItem>): List<PrepGroup> {
    val entries = items.mapIndexed { i, item -> PrepEntry(i, item) }
    return listOf(
        PrepGroup(PrepGroupKind.Chop, entries.filter { it.item.vessel == Vessel.BOARD }),
        PrepGroup(
            PrepGroupKind.Measure,
            entries.filter { it.item.vessel in SizeOrder }.sortedBy { SizeOrder.indexOf(it.item.vessel) },
        ),
        PrepGroup(PrepGroupKind.Reach, entries.filter { it.item.vessel == Vessel.JAR }),
    ).filter { it.entries.isNotEmpty() }
}

/** The small uppercase label under each item. */
fun vesselLabel(vessel: Vessel): String = when (vessel) {
    Vessel.LARGE -> "Large bowl"
    Vessel.MEDIUM -> "Medium bowl"
    Vessel.SMALL -> "Small bowl"
    Vessel.RAMEKIN -> "Ramekin"
    Vessel.PINCH -> "Pinch bowl"
    Vessel.BOARD -> "Board"
    Vessel.JAR -> "On hand"
}

/**
 * The amount line (web `amount()`): the scaled quantity, a range when there is one, and the
 * pluralised unit. A line with no quantity but a unit reads "a pinch".
 */
fun prepAmount(item: MiseItem, scale: Double): String {
    val quantity = item.quantity ?: return item.unit?.let { "a $it" } ?: ""
    val max = item.quantityMax?.let { "–${formatQuantity(it * scale)}" } ?: ""
    val n = (item.quantityMax ?: quantity) * scale
    val unit = item.unit?.let { " ${pluralUnit(it, n)}" } ?: ""
    return formatQuantity(quantity * scale) + max + unit
}

/** One line of "Get out:": a vessel and how many of it. */
data class VesselCount(val vessel: Vessel, val count: Int)

/**
 * The "Get out:" tally (spec §6.2): one entry per vessel in first-seen order, seasoning jars
 * left out, and every cutting board collapsed to one ("one board does for all the chopping").
 */
fun getOutCounts(items: List<MiseItem>): List<VesselCount> {
    val counts = LinkedHashMap<Vessel, Int>()
    for (item in items) counts[item.vessel] = (counts[item.vessel] ?: 0) + 1
    if (counts.containsKey(Vessel.BOARD)) counts[Vessel.BOARD] = 1
    return counts.entries.filter { it.key != Vessel.JAR }.map { VesselCount(it.key, it.value) }
}

/** "large bowls" / "board", for the bold count in front. */
fun getOutNoun(count: VesselCount): String =
    vesselLabel(count.vessel).lowercase() + if (count.count > 1) "s" else ""
